//! Worker-only code delivery over Resend HTTPS or a private local outbox.
//! The challenge and delivery intent are committed before either transport runs.
//! Errors are reduced to a category and numeric status; no provider text is retained.

use std::future::Future;
use std::io::Write;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
use uuid::Uuid;

use crate::email_code::{Code, EmailAddress};

mod resend;
pub use resend::{Resend, ResendSettings};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Purpose {
    SignIn,
    ChangeEmail,
    /// Fresh proof of mailbox control before a signed-in account adds a username/password
    /// credential (ACC-03). Bound to the account; never a sign-in or a recovery.
    EnrollPassword,
    /// First verified email on an existing password account.
    AttachEmail,
    DeleteAccount,
    /// `vink-identity mail test`: proves the transport end to end. Never stored (the outbox
    /// schema accepts only the code purposes) and its text contains no usable code.
    TransportCheck,
}

impl Purpose {
    pub fn label(self) -> &'static str {
        match self {
            Purpose::SignIn => "sign_in",
            Purpose::ChangeEmail => "change_email",
            Purpose::EnrollPassword => "enroll_password",
            Purpose::AttachEmail => "attach_email",
            Purpose::DeleteAccount => "delete_account",
            Purpose::TransportCheck => "transport_check",
        }
    }

    pub fn from_label(label: &str) -> Option<Self> {
        match label {
            "sign_in" => Some(Purpose::SignIn),
            "change_email" => Some(Purpose::ChangeEmail),
            "enroll_password" => Some(Purpose::EnrollPassword),
            "attach_email" => Some(Purpose::AttachEmail),
            "delete_account" => Some(Purpose::DeleteAccount),
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
    /// A retryable response, or a local condition worth retrying.
    Transient {
        status: Option<u16>,
    },
    /// Provider requested a minimum delay before another attempt.
    ProviderRetry {
        status: u16,
        after: Duration,
    },
    /// Definitive provider refusal, such as an invalid payload or credential.
    Permanent {
        status: Option<u16>,
    },
    /// Local configuration became unusable (e.g. key file rotation).
    Configuration,
}

impl Failure {
    pub fn label(self) -> &'static str {
        match self {
            Failure::Timeout => "timeout",
            Failure::Connect => "connect",
            Failure::Tls => "tls",
            Failure::Transient { .. } | Failure::ProviderRetry { .. } => "transient",
            Failure::Permanent { .. } => "permanent",
            Failure::Configuration => "configuration",
        }
    }

    pub fn status(self) -> Option<u16> {
        match self {
            Failure::Transient { status } | Failure::Permanent { status } => status,
            Failure::ProviderRetry { status, .. } => Some(status),
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
        Purpose::EnrollPassword => "thêm mật khẩu đăng nhập cho tài khoản Vink",
        Purpose::AttachEmail => "xác minh email cho tài khoản Vink",
        Purpose::DeleteAccount => "xác nhận xóa vĩnh viễn tài khoản Vink",
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
    let ignore = match message.purpose {
        Purpose::DeleteAccount => {
            "Nếu bạn không yêu cầu xóa tài khoản, hãy bỏ qua thư và giữ riêng mã này."
        }
        Purpose::AttachEmail => {
            "Nếu bạn không yêu cầu mã này, hãy bỏ qua thư; email sẽ không được gắn vào tài khoản."
        }
        Purpose::EnrollPassword => {
            "Nếu bạn không yêu cầu mã này, hãy bỏ qua thư; mật khẩu sẽ không được thêm vào tài khoản."
        }
        _ => {
            "Nếu bạn không yêu cầu mã này, hãy bỏ qua thư; không ai đăng nhập được nếu không có mã."
        }
    };
    let body = format!(
        "Mã {action} của bạn là {code}.\n\n\
         Mã chỉ dùng được một lần, trên trình duyệt hoặc ứng dụng vừa yêu cầu, và hết hạn sau 10 phút.\n\
         {ignore}\n\n\
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
    /// Reported by readiness and logs (`local_outbox`, `resend_api`).
    fn label(&self) -> &'static str;
    /// Digest of the exact provider payload, if this transport needs stable cross-process retries.
    fn payload_fingerprint(&self, _message: &Message) -> Option<[u8; 32]> {
        None
    }
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

#[cfg(test)]
mod tests {
    use std::os::unix::fs::MetadataExt;

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
}
