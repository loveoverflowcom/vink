//! Mail transports for one-time codes. Codes are never sent from a request: the challenge and a
//! delivery intent are committed together ([`crate::delivery`]) and the serve worker hands each
//! due message to one [`MailSender`] outside any transaction.
//!
//! **Provider seam.** The outbox, the retry policy and the message text ([`render`]) know nothing
//! about the provider: the worker holds a [`Transport`] (`Arc<dyn MailSender>`). Moving to another
//! SMTP provider is configuration only (`VINK_IDENTITY_SMTP_PROVIDER` presets, or explicit
//! `VINK_IDENTITY_SMTP_*`); a provider reached by another protocol (an HTTP API) is one more
//! `MailSender` and one configuration branch, with no change to the outbox or the worker.
//!
//! - [`LocalOutbox`]: each message is a file (mode `0600`) in a directory only the Identity user
//!   can open. Development and `verify postgres` read codes from it like a mailbox. Nobody outside
//!   the machine receives anything, so it is never production mail.
//! - [`Smtp`]: SMTP submission over TLS (implicit TLS on 465 or required STARTTLS on 587) with
//!   optional authentication. Provider-neutral: any relay that accepts authenticated submission
//!   works. Plaintext is accepted only towards a loopback relay (configuration refuses anything
//!   else). The password is read from a file at every delivery, so replacing the file rotates it
//!   without a restart.
//!
//! Nothing here logs or returns a recipient, a code or provider response text: failures are
//! reduced to a [`Failure`] category and, for SMTP replies, the numeric status.

use std::future::Future;
use std::io::Write;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use lettre::message::header::{
    ContentTransferEncoding, ContentType, HeaderName, HeaderValue, MessageId,
};
use lettre::message::{Body, Mailbox, Message as Email};
use lettre::transport::smtp::authentication::Credentials;
use lettre::transport::smtp::client::{Tls, TlsParameters};
use lettre::transport::smtp::extension::ClientId;
use lettre::{AsyncSmtpTransport, AsyncTransport, Tokio1Executor};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
use uuid::Uuid;

use crate::email_code::{Code, EmailAddress};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Purpose {
    SignIn,
    ChangeEmail,
    /// `vink-identity mail test`: proves the transport end to end. Never stored (the outbox
    /// schema accepts only the two code purposes) and its text contains no usable code.
    TransportCheck,
}

impl Purpose {
    pub fn label(self) -> &'static str {
        match self {
            Purpose::SignIn => "sign_in",
            Purpose::ChangeEmail => "change_email",
            Purpose::TransportCheck => "transport_check",
        }
    }

    pub fn from_label(label: &str) -> Option<Self> {
        match label {
            "sign_in" => Some(Purpose::SignIn),
            "change_email" => Some(Purpose::ChangeEmail),
            _ => None,
        }
    }
}

/// One code to deliver. `delivery` is the challenge id, which is not a secret on its own.
#[derive(Debug, Clone)]
pub struct Message {
    pub delivery: Uuid,
    pub to: EmailAddress,
    pub code: Code,
    pub purpose: Purpose,
    pub expires_at: OffsetDateTime,
}

/// Why a transport did not accept a message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Failure {
    Timeout,
    Connect,
    Tls,
    /// A 4xx reply, or a local condition worth retrying.
    Transient {
        status: Option<u16>,
    },
    /// A 5xx reply: the relay will not accept this message as it is.
    Permanent {
        status: Option<u16>,
    },
    /// The transport cannot be built from its configuration (unreadable password file, invalid
    /// sender). Retried, because an operator can fix it while the code is still valid.
    Configuration,
}

impl Failure {
    pub fn label(self) -> &'static str {
        match self {
            Failure::Timeout => "timeout",
            Failure::Connect => "connect",
            Failure::Tls => "tls",
            Failure::Transient { .. } => "transient",
            Failure::Permanent { .. } => "permanent",
            Failure::Configuration => "configuration",
        }
    }

    pub fn status(self) -> Option<u16> {
        match self {
            Failure::Transient { status } | Failure::Permanent { status } => status,
            _ => None,
        }
    }

    /// Only a definitive refusal ends a delivery before its attempts or its code run out.
    pub fn is_permanent(self) -> bool {
        matches!(self, Failure::Permanent { .. })
    }
}

/// Subject and plain-text body in Vietnamese. The code appears only in the body.
pub fn render(message: &Message) -> (String, String) {
    let action = match message.purpose {
        Purpose::SignIn => "đăng nhập Vink",
        Purpose::ChangeEmail => "xác nhận địa chỉ email mới cho tài khoản Vink",
        Purpose::TransportCheck => {
            return (
                "Vink: thư kiểm tra đường gửi".into(),
                "Đây là thư kiểm tra đường gửi thư của Vink do người vận hành yêu cầu.\n\
                 Thư không chứa mã và không cần làm gì.\n\n\
                 Vink — thư tự động, vui lòng không trả lời.\n"
                    .into(),
            );
        }
    };
    let subject = format!("Mã {action}");
    let body = format!(
        "Mã {action} của bạn là {code}.\n\n\
         Mã chỉ dùng được một lần, trên trình duyệt hoặc ứng dụng vừa yêu cầu, và hết hạn sau 10 phút.\n\
         Nếu bạn không yêu cầu mã này, hãy bỏ qua thư; không ai đăng nhập được nếu không có mã.\n\n\
         Vink — thư tự động, vui lòng không trả lời.\n",
        code = message.code.expose(),
    );
    (subject, body)
}

pub type SendFuture<'a> = Pin<Box<dyn Future<Output = Result<(), Failure>> + Send + 'a>>;

/// One way of handing a rendered message to the outside world. Implementations reduce every
/// failure to a [`Failure`] (never provider text) and must tolerate the same message twice
/// (delivery is at least once).
pub trait MailSender: Send + Sync + 'static {
    /// Reported by readiness and logs (`local_outbox`, `smtp`, …).
    fn label(&self) -> &'static str;
    fn send<'a>(&'a self, message: &'a Message) -> SendFuture<'a>;
}

/// The sender the worker uses; any [`MailSender`].
pub type Transport = Arc<dyn MailSender>;

impl MailSender for LocalOutbox {
    fn label(&self) -> &'static str {
        "local_outbox"
    }

    fn send<'a>(&'a self, message: &'a Message) -> SendFuture<'a> {
        Box::pin(std::future::ready(self.deliver(message)))
    }
}

impl MailSender for Smtp {
    fn label(&self) -> &'static str {
        "smtp"
    }

    fn send<'a>(&'a self, message: &'a Message) -> SendFuture<'a> {
        Box::pin(Smtp::send(self, message))
    }
}

/// Writes each message to `<dir>/<challenge id>.txt`.
#[derive(Debug, Clone)]
pub struct LocalOutbox {
    dir: PathBuf,
}

impl LocalOutbox {
    /// The directory must exist and grant nothing to group or others (`chmod 700`): messages
    /// contain live codes.
    pub fn open(dir: &Path) -> Result<Self, String> {
        let meta = std::fs::metadata(dir)
            .map_err(|_| format!("directory {} does not exist", dir.display()))?;
        if !meta.is_dir() || meta.permissions().mode() & 0o077 != 0 {
            return Err(format!(
                "{} must be a directory without group or other access (chmod 700)",
                dir.display()
            ));
        }
        Ok(Self {
            dir: dir.to_path_buf(),
        })
    }

    /// Idempotent per challenge: a repeated delivery (at-least-once) finds the file already
    /// written and succeeds without touching it.
    pub fn deliver(&self, message: &Message) -> Result<(), Failure> {
        let expires = message
            .expires_at
            .format(&Rfc3339)
            .map_err(|_| Failure::Configuration)?;
        let (subject, body) = render(message);
        let text = format!(
            "To: {to}\nSubject: {subject}\nX-Vink-Purpose: {purpose}\nX-Vink-Expires-At: {expires}\nX-Vink-Code: {code}\n\n{body}",
            to = message.to.as_str(),
            purpose = message.purpose.label(),
            code = message.code.expose(),
        );
        let path = self.dir.join(format!("{}.txt", message.delivery.simple()));
        let mut file = match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&path)
        {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => return Ok(()),
            Err(_) => return Err(Failure::Transient { status: None }),
        };
        // No fsync: a message lost in a crash only means the person asks for a new code.
        file.write_all(text.as_bytes())
            .map_err(|_| Failure::Transient { status: None })
    }
}

/// How the SMTP connection is protected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SmtpSecurity {
    /// TLS from the first byte (submissions port 465).
    ImplicitTls,
    /// Plain connection upgraded with STARTTLS, which is required (port 587).
    StartTls,
    /// No TLS. Configuration accepts it only for a loopback relay (tests, a local MTA).
    Plaintext,
}

impl SmtpSecurity {
    pub fn label(self) -> &'static str {
        match self {
            SmtpSecurity::ImplicitTls => "tls",
            SmtpSecurity::StartTls => "starttls",
            SmtpSecurity::Plaintext => "none",
        }
    }
}

/// SMTP submission settings. The password is a path, never a value.
#[derive(Clone, PartialEq, Eq)]
pub struct SmtpSettings {
    pub host: String,
    pub port: u16,
    pub security: SmtpSecurity,
    pub username: Option<String>,
    pub password_file: Option<PathBuf>,
    /// RFC 5322 mailbox of the sender, e.g. `Vink <no-reply@vink.vn>`.
    pub from: String,
    /// Bound on one whole SMTP conversation.
    pub timeout: Duration,
}

impl std::fmt::Debug for SmtpSettings {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SmtpSettings")
            .field("host", &self.host)
            .field("port", &self.port)
            .field("security", &self.security)
            .field("username", &self.username.as_ref().map(|_| "<set>"))
            .field("password_file", &self.password_file)
            .field("from", &self.from)
            .field("timeout", &self.timeout)
            .finish()
    }
}

#[derive(Debug, Clone)]
pub struct Smtp {
    settings: SmtpSettings,
    from: Mailbox,
    /// Domain of the sender: EHLO name and right-hand side of Message-ID.
    domain: String,
}

impl Smtp {
    /// Validates what can be checked without the network: the sender parses, and the password
    /// file, when given, is readable by this user only.
    pub fn new(settings: SmtpSettings) -> Result<Self, String> {
        let from: Mailbox = settings
            .from
            .parse()
            .map_err(|_| "sender is not an RFC 5322 mailbox such as `Vink <no-reply@vink.vn>`")?;
        let domain = from.email.domain().to_string();
        if let Some(path) = &settings.password_file {
            let meta = std::fs::metadata(path)
                .map_err(|_| format!("password file {} is not readable", path.display()))?;
            if !meta.is_file() || meta.permissions().mode() & 0o077 != 0 {
                return Err(format!(
                    "password file {} must be a file without group or other access (chmod 600)",
                    path.display()
                ));
            }
            read_secret(path)
                .ok_or_else(|| format!("password file {} is empty", path.display()))?;
        }
        Ok(Self {
            settings,
            from,
            domain,
        })
    }

    pub fn settings(&self) -> &SmtpSettings {
        &self.settings
    }

    fn email(&self, message: &Message) -> Result<Email, Failure> {
        let to: Mailbox = message
            .to
            .as_str()
            .parse()
            .map_err(|_| Failure::Permanent { status: None })?;
        let (subject, body) = render(message);
        // One Message-ID per challenge: a duplicate from an at-least-once retry is recognisable
        // as the same message by relays and mail clients that deduplicate.
        let id = format!("<{}.code@{}>", message.delivery.simple(), self.domain);
        Email::builder()
            .from(self.from.clone())
            .to(to)
            .subject(subject)
            .header(MessageId::from(id))
            .header(ContentType::TEXT_PLAIN)
            .raw_header(HeaderValue::new(
                HeaderName::new_from_ascii_str("Auto-Submitted"),
                "auto-generated".into(),
            ))
            // Quoted-printable: 7-bit safe through any relay, Vietnamese text intact.
            .body(
                Body::new_with_encoding(body, ContentTransferEncoding::QuotedPrintable)
                    .map_err(|_| Failure::Permanent { status: None })?,
            )
            .map_err(|_| Failure::Permanent { status: None })
    }

    fn transport(&self) -> Result<AsyncSmtpTransport<Tokio1Executor>, Failure> {
        let s = &self.settings;
        let tls = || TlsParameters::new(s.host.clone()).map_err(|_| Failure::Configuration);
        let builder = AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(s.host.as_str())
            .port(s.port)
            .timeout(Some(s.timeout))
            .hello_name(ClientId::Domain(self.domain.clone()))
            .tls(match s.security {
                SmtpSecurity::ImplicitTls => Tls::Wrapper(tls()?),
                SmtpSecurity::StartTls => Tls::Required(tls()?),
                SmtpSecurity::Plaintext => Tls::None,
            });
        let builder = match (&s.username, &s.password_file) {
            (Some(user), Some(path)) => {
                let password = read_secret(path).ok_or(Failure::Configuration)?;
                builder.credentials(Credentials::new(user.clone(), password))
            }
            _ => builder,
        };
        Ok(builder.build())
    }

    pub async fn send(&self, message: &Message) -> Result<(), Failure> {
        let email = self.email(message)?;
        let transport = self.transport()?;
        // lettre bounds each command; this bounds the whole conversation as well.
        let limit = self.settings.timeout + Duration::from_secs(2);
        match tokio::time::timeout(limit, transport.send(email)).await {
            Err(_) => Err(Failure::Timeout),
            Ok(Ok(_)) => Ok(()),
            Ok(Err(error)) => Err(classify(&error)),
        }
    }
}

fn read_secret(path: &Path) -> Option<String> {
    let raw = std::fs::read_to_string(path).ok()?;
    let secret = raw.trim_end_matches(['\n', '\r']).to_string();
    (!secret.is_empty()).then_some(secret)
}

/// Reduces a lettre error to a category. The error's text is never kept: an SMTP reply can quote
/// the recipient.
fn classify(error: &lettre::transport::smtp::Error) -> Failure {
    let status = error
        .status()
        .and_then(|code| code.to_string().parse::<u16>().ok());
    classify_parts(
        error.is_timeout(),
        error.is_tls() || certificate_problem(error),
        error.is_permanent(),
        error.is_transient(),
        error.is_client() || error.to_string().contains("onnection"),
        status,
    )
}

/// lettre reports a refused certificate (an unknown issuer, such as a TLS-intercepting firewall)
/// as a connection error whose source is an `InvalidData` I/O error.
fn certificate_problem(error: &(dyn std::error::Error + 'static)) -> bool {
    let mut source = error.source();
    while let Some(cause) = source {
        if cause
            .downcast_ref::<std::io::Error>()
            .is_some_and(|io| io.kind() == std::io::ErrorKind::InvalidData)
        {
            return true;
        }
        source = cause.source();
    }
    false
}

/// Pure decision behind [`classify`]. Authentication refusals (530, 534, 535) are configuration
/// problems an operator can fix while codes are still valid, so they are retried, not final.
fn classify_parts(
    timeout: bool,
    tls: bool,
    permanent: bool,
    transient: bool,
    connection: bool,
    status: Option<u16>,
) -> Failure {
    if timeout {
        Failure::Timeout
    } else if tls {
        Failure::Tls
    } else if matches!(status, Some(530 | 534 | 535)) {
        Failure::Configuration
    } else if permanent {
        Failure::Permanent { status }
    } else if transient {
        Failure::Transient { status }
    } else if connection && status.is_none() {
        Failure::Connect
    } else {
        Failure::Transient { status }
    }
}

#[cfg(test)]
#[path = "../tests/support/smtp_sink.rs"]
pub(crate) mod smtp_sink;

#[cfg(test)]
mod tests {
    use std::os::unix::fs::MetadataExt;

    use super::smtp_sink::{Reply, Sink};
    use super::*;

    fn scratch(name: &str, mode: u32) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("vink-outbox-{name}-{}", Uuid::now_v7()));
        std::fs::create_dir(&dir).unwrap();
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(mode)).unwrap();
        dir
    }

    fn message(code: &str) -> Message {
        Message {
            delivery: Uuid::now_v7(),
            to: EmailAddress::parse("nguoi.viet@vi-du.vn").unwrap(),
            code: Code::parse(code).unwrap(),
            purpose: Purpose::SignIn,
            expires_at: OffsetDateTime::UNIX_EPOCH,
        }
    }

    #[test]
    fn the_outbox_is_private_and_idempotent_per_challenge() {
        assert!(LocalOutbox::open(Path::new("/nonexistent/vink-outbox")).is_err());
        let open = scratch("open", 0o750);
        assert!(LocalOutbox::open(&open).is_err());
        let dir = scratch("private", 0o700);
        let outbox = LocalOutbox::open(&dir).unwrap();
        let first = message("012345");
        outbox.deliver(&first).unwrap();
        let path = dir.join(format!("{}.txt", first.delivery.simple()));
        assert_eq!(std::fs::metadata(&path).unwrap().mode() & 0o777, 0o600);
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("X-Vink-Code: 012345"), "{text}");
        assert!(
            text.contains("Mã đăng nhập Vink của bạn là 012345"),
            "{text}"
        );
        // A duplicate delivery of the same challenge succeeds and leaves the message intact.
        let mut duplicate = first.clone();
        duplicate.code = Code::parse("999999").unwrap();
        outbox.deliver(&duplicate).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), text);
        std::fs::remove_dir_all(&dir).unwrap();
        std::fs::remove_dir_all(&open).unwrap();
    }

    fn settings(port: u16) -> SmtpSettings {
        SmtpSettings {
            host: "127.0.0.1".into(),
            port,
            security: SmtpSecurity::Plaintext,
            username: None,
            password_file: None,
            from: "Vink <no-reply@vink.example>".into(),
            timeout: Duration::from_secs(2),
        }
    }

    #[test]
    fn failures_are_categorised_and_authentication_is_configuration() {
        let c = classify_parts;
        assert_eq!(
            c(true, true, true, false, true, Some(550)),
            Failure::Timeout
        );
        assert_eq!(c(false, true, false, false, true, None), Failure::Tls);
        for status in [530, 534, 535] {
            assert_eq!(
                c(false, false, true, false, false, Some(status)),
                Failure::Configuration
            );
        }
        assert_eq!(
            c(false, false, true, false, false, Some(550)),
            Failure::Permanent { status: Some(550) }
        );
        assert_eq!(
            c(false, false, false, true, false, Some(421)),
            Failure::Transient { status: Some(421) }
        );
        assert_eq!(c(false, false, false, false, true, None), Failure::Connect);
        assert_eq!(
            c(false, false, false, false, false, None),
            Failure::Transient { status: None }
        );
        assert!(!Failure::Configuration.is_permanent() && !Failure::Tls.is_permanent());
    }

    #[test]
    fn smtp_settings_validate_the_sender_and_a_private_password_file() {
        assert!(
            Smtp::new(SmtpSettings {
                from: "not a mailbox".into(),
                ..settings(25)
            })
            .is_err()
        );
        let dir = scratch("secret", 0o700);
        let file = dir.join("password");
        std::fs::write(&file, "s3cret\n").unwrap();
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o644)).unwrap();
        let with = |path: &Path| SmtpSettings {
            username: Some("vink-user".into()),
            password_file: Some(path.to_path_buf()),
            ..settings(25)
        };
        assert!(Smtp::new(with(&file)).unwrap_err().contains("chmod 600"));
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o600)).unwrap();
        let smtp = Smtp::new(with(&file)).unwrap();
        let printed = format!("{smtp:?}");
        assert!(!printed.contains("s3cret"), "{printed}");
        assert!(!printed.contains("vink-user"), "{printed}");
        std::fs::write(&file, "\n").unwrap();
        assert!(Smtp::new(with(&file)).unwrap_err().contains("empty"));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// The adapter against a real SMTP conversation over loopback TCP: accepted, refused (4xx/5xx)
    /// and silent relays. The failure categories never carry the recipient or the code.
    #[tokio::test]
    async fn smtp_delivers_and_classifies_relay_answers() {
        let sink = Sink::start(Reply::Accept).await;
        let smtp = Smtp::new(settings(sink.port())).unwrap();
        let sent = message("424242");
        smtp.send(&sent).await.unwrap();
        let received = sink.messages();
        assert_eq!(received.len(), 1);
        let mail = &received[0];
        assert_eq!(mail.rcpt, vec!["<nguoi.viet@vi-du.vn>".to_string()]);
        assert!(mail.data.contains("quoted-printable"), "{}", mail.data);
        assert!(
            mail.text().contains("Mã đăng nhập Vink của bạn là 424242."),
            "{}",
            mail.text()
        );
        assert!(
            mail.data
                .contains(&format!("{}.code@vink.example", sent.delivery.simple())),
            "{}",
            mail.data
        );
        assert!(mail.data.contains("Auto-Submitted: auto-generated"));
        assert!(!mail.data.contains("X-Vink-Code"));

        for (reply, expected) in [
            (
                Reply::RejectRcpt(450),
                Failure::Transient { status: Some(450) },
            ),
            (
                Reply::RejectRcpt(550),
                Failure::Permanent { status: Some(550) },
            ),
            (
                Reply::RejectData(554),
                Failure::Permanent { status: Some(554) },
            ),
        ] {
            let sink = Sink::start(reply).await;
            let smtp = Smtp::new(settings(sink.port())).unwrap();
            let failure = smtp.send(&message("111111")).await.unwrap_err();
            assert_eq!(failure, expected, "{reply:?}");
            assert!(!format!("{failure:?}").contains("nguoi.viet"));
        }

        let sink = Sink::start(Reply::Hang).await;
        let smtp = Smtp::new(SmtpSettings {
            timeout: Duration::from_millis(300),
            ..settings(sink.port())
        })
        .unwrap();
        let started = std::time::Instant::now();
        assert_eq!(
            smtp.send(&message("222222")).await.unwrap_err(),
            Failure::Timeout
        );
        assert!(started.elapsed() < Duration::from_secs(5));

        // Nothing listens: a connection failure, retried by the worker.
        let closed = Sink::unused_port();
        let failure = Smtp::new(settings(closed))
            .unwrap()
            .send(&message("333333"))
            .await
            .unwrap_err();
        assert!(!failure.is_permanent(), "{failure:?}");
    }
}
