use std::error::Error as _;
use std::io::Read as _;
use std::os::unix::fs::{OpenOptionsExt as _, PermissionsExt as _};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use reqwest::header::{AUTHORIZATION, CONTENT_TYPE, HeaderValue, RETRY_AFTER};
use reqwest::{Client, Response, Url};
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use time::OffsetDateTime;

use super::{Failure, MailSender, Message, Purpose, SendFuture, render};

const ENDPOINT: &str = "https://api.resend.com/emails";
const MAX_PROVIDER_BODY: usize = 8192;

#[derive(Clone, PartialEq, Eq)]
pub struct ResendSettings {
    pub from: String,
    pub key_file: PathBuf,
    pub timeout: Duration,
    pub proxy: Option<String>,
}

impl std::fmt::Debug for ResendSettings {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ResendSettings")
            .field("timeout", &self.timeout)
            .field("proxy_configured", &self.proxy.is_some())
            .finish_non_exhaustive()
    }
}

pub struct Resend {
    settings: ResendSettings,
    client: Client,
    endpoint: Url,
}

#[derive(Serialize)]
struct Payload<'a> {
    from: &'a str,
    to: [&'a str; 1],
    subject: String,
    text: String,
}

impl Resend {
    pub fn new(settings: ResendSettings) -> Result<Self, &'static str> {
        Self::build(
            settings,
            Url::parse(ENDPOINT).expect("fixed Resend URL"),
            true,
        )
    }

    fn build(
        settings: ResendSettings,
        endpoint: Url,
        https_only: bool,
    ) -> Result<Self, &'static str> {
        let from = settings.from.as_str();
        // Resend accepts either a bare address or a single display name and address.
        let address = from
            .split_once(" <")
            .and_then(|(name, rest)| {
                (!name.trim().is_empty() && !name.contains(['<', '>', ',']))
                    .then(|| rest.strip_suffix('>'))
                    .flatten()
            })
            .unwrap_or(from);
        if from.chars().any(char::is_control) || !bare_address(address) {
            return Err("sender must be a single valid mailbox");
        }
        if !(Duration::from_secs(1)..=Duration::from_secs(30)).contains(&settings.timeout)
            && https_only
        {
            return Err("timeout must be between 1 and 30 seconds");
        }
        authorization(&settings.key_file)?;
        let mut builder = Client::builder()
            .no_proxy()
            .https_only(https_only)
            // Keep this true even if another dependency enables reqwest's HTTP/2 feature.
            .http1_only()
            .redirect(reqwest::redirect::Policy::none())
            .connection_verbose(false)
            .connect_timeout(settings.timeout)
            .timeout(settings.timeout);
        if let Some(raw) = &settings.proxy {
            let proxy = Url::parse(raw).map_err(|_| "proxy must be http://host:port")?;
            if proxy.scheme() != "http"
                || proxy.host_str().is_none()
                || proxy.port_or_known_default().is_none_or(|port| port == 0)
                || !proxy.username().is_empty()
                || proxy.password().is_some()
                || proxy.path() != "/"
                || proxy.query().is_some()
                || proxy.fragment().is_some()
            {
                return Err(
                    "proxy must be http://host:port without credentials, path, query or fragment",
                );
            }
            builder =
                builder.proxy(reqwest::Proxy::https(proxy).map_err(|_| "invalid HTTPS proxy")?);
        }
        let client = builder
            .build()
            .map_err(|_| "cannot initialize HTTPS client")?;
        Ok(Self {
            settings,
            client,
            endpoint,
        })
    }

    fn payload(&self, message: &Message) -> Vec<u8> {
        let (subject, text) = render(message);
        serde_json::to_vec(&Payload {
            from: &self.settings.from,
            to: [message.to.as_str()],
            subject,
            text,
        })
        .expect("all payload fields are JSON strings")
    }

    async fn submit(&self, message: &Message) -> Result<(), Failure> {
        // Parse strictly at the external provider boundary: a JSON array containing one string
        // must not be interpreted as a list or as a display-address by a downstream mail parser.
        if !bare_address(message.to.as_str()) {
            return Err(Failure::Permanent { status: None });
        }
        let key = authorization(&self.settings.key_file).map_err(|_| Failure::Configuration)?;
        if message.purpose != Purpose::TransportCheck
            && message.expires_at <= OffsetDateTime::now_utc()
        {
            return Err(Failure::Permanent { status: None });
        }
        let response = self
            .client
            .post(self.endpoint.clone())
            .header(AUTHORIZATION, key)
            .header(CONTENT_TYPE, "application/json")
            .header(
                "Idempotency-Key",
                format!("vink-identity/{}", message.delivery.simple()),
            )
            .body(self.payload(message))
            .send()
            .await
            .map_err(network_failure)?;

        let status = response.status();
        let status_code = status.as_u16();
        let after = retry_after(response.headers().get(RETRY_AFTER), SystemTime::now());
        if status.is_success() {
            #[derive(Deserialize)]
            struct Receipt {
                id: uuid::Uuid,
            }
            // An unexpected success body does not prove acceptance; retry the exact same request.
            return bounded_json::<Receipt>(response)
                .await?
                .filter(|receipt| !receipt.id.is_nil())
                .map(|_| ())
                .ok_or(Failure::Transient {
                    status: Some(status_code),
                });
        }
        let concurrent = if status_code == 409 {
            #[derive(Deserialize)]
            struct Conflict {
                name: String,
            }
            bounded_json::<Conflict>(response)
                .await?
                .is_some_and(|error| error.name == "concurrent_idempotent_requests")
        } else {
            false
        };
        Err(match status_code {
            // Request timed out at an intermediary, another identical request is in flight,
            // provider throttled us or had an error. None proves non-acceptance.
            408 | 429 | 500..=599 => delayed_failure(status_code, after),
            409 if concurrent => delayed_failure(status_code, after),
            // Do not retry unknown conflicts or invalid_idempotent_request with different data.
            // Invalid request, unauthorized, forbidden and redirects are also definitive.
            _ => Failure::Permanent {
                status: Some(status_code),
            },
        })
    }
}

impl MailSender for Resend {
    fn label(&self) -> &'static str {
        "resend_api"
    }

    fn payload_fingerprint(&self, message: &Message) -> Option<[u8; 32]> {
        Some(Sha256::digest(self.payload(message)).into())
    }

    fn send<'a>(&'a self, message: &'a Message) -> SendFuture<'a> {
        Box::pin(self.submit(message))
    }
}

fn bare_address(value: &str) -> bool {
    !value.contains(['<', '>', ',', ';', '"', '\\'])
        && !value.chars().any(|c| c.is_control() || c.is_whitespace())
        && email_address::EmailAddress::parse_with_options(
            value,
            email_address::Options::default().without_display_text(),
        )
        .is_ok()
}

fn authorization(path: &Path) -> Result<HeaderValue, &'static str> {
    // Inspect the same file descriptor we read, so replace-by-rename cannot race validation.
    // NONBLOCK prevents startup or a retry from hanging on an accidental named pipe.
    let file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NONBLOCK)
        .open(path)
        .map_err(|_| "API key file is not readable")?;
    let meta = file
        .metadata()
        .map_err(|_| "API key file metadata unavailable")?;
    if !meta.is_file() || meta.permissions().mode() & 0o077 != 0 {
        return Err("API key must be a file without group or other access (chmod 600)");
    }
    let mut token = String::new();
    file.take(1025)
        .read_to_string(&mut token)
        .map_err(|_| "cannot read API key file")?;
    let trimmed = token.trim_end_matches(['\r', '\n']);
    if token.len() > 1024
        || trimmed.strip_prefix("re_").is_none_or(|value| {
            value.is_empty()
                || !value
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
        })
    {
        return Err("API key file must contain one Resend key");
    }
    let mut header = HeaderValue::from_str(&format!("Bearer {trimmed}"))
        .map_err(|_| "invalid API key header")?;
    header.set_sensitive(true);
    Ok(header)
}

async fn bounded_json<T: serde::de::DeserializeOwned>(
    mut response: Response,
) -> Result<Option<T>, Failure> {
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(network_failure)? {
        if chunk.len() > MAX_PROVIDER_BODY - body.len() {
            return Ok(None);
        }
        body.extend_from_slice(&chunk);
    }
    Ok(serde_json::from_slice(&body).ok())
}

fn retry_after(header: Option<&HeaderValue>, now: SystemTime) -> Option<Duration> {
    let text = header?.to_str().ok()?.trim();
    if !text.is_empty() && text.bytes().all(|b| b.is_ascii_digit()) {
        return Some(Duration::from_secs(text.parse().unwrap_or(u64::MAX)));
    }
    Some(
        httpdate::parse_http_date(text)
            .ok()?
            .duration_since(now)
            .unwrap_or_default(),
    )
}

fn delayed_failure(status: u16, after: Option<Duration>) -> Failure {
    match after {
        Some(after) => Failure::ProviderRetry { status, after },
        None => Failure::Transient {
            status: Some(status),
        },
    }
}

fn network_failure(error: reqwest::Error) -> Failure {
    if error.is_timeout() {
        return Failure::Timeout;
    }
    let mut source = error.source();
    while let Some(cause) = source {
        if cause.is::<rustls::Error>()
            || cause
                .downcast_ref::<std::io::Error>()
                .and_then(std::io::Error::get_ref)
                .is_some_and(|e| e.is::<rustls::Error>())
        {
            return Failure::Tls;
        }
        source = cause.source();
    }
    if error.is_connect() {
        Failure::Connect
    } else {
        Failure::Transient { status: None }
    }
}

#[cfg(test)]
mod tests {

    use axum::body::Bytes;
    use axum::extract::Request;
    use axum::http::{HeaderMap, StatusCode};
    use axum::response::IntoResponse;
    use tokio::net::TcpListener;
    use tokio::task::JoinHandle;

    use crate::email_code::{Code, EmailAddress};

    use super::*;

    const RECEIPT: &str = r#"{"id":"49a3999c-0ce1-4ea6-ab68-afcd6dc2e794"}"#;
    const FAKE_KEY: &str = "re_fixture_abcdef";

    struct Scratch {
        dir: PathBuf,
        key: PathBuf,
    }

    impl Scratch {
        fn new() -> Self {
            let dir = std::env::temp_dir().join(format!("vink-resend-{}", uuid::Uuid::now_v7()));
            std::fs::DirBuilder::new().create(&dir).unwrap();
            std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).unwrap();
            let key = dir.join("key");
            std::fs::write(&key, format!("{FAKE_KEY}\n")).unwrap();
            std::fs::set_permissions(&key, std::fs::Permissions::from_mode(0o600)).unwrap();
            Self { dir, key }
        }

        fn settings(&self) -> ResendSettings {
            ResendSettings {
                from: "Việt Ink <no-reply@example.test>".into(),
                key_file: self.key.clone(),
                timeout: Duration::from_secs(2),
                proxy: None,
            }
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    fn message(purpose: Purpose) -> Message {
        Message {
            delivery: uuid::Uuid::now_v7(),
            to: EmailAddress::parse("reader@example.test").unwrap(),
            code: Code::parse("654321").unwrap(),
            purpose,
            expires_at: OffsetDateTime::now_utc() + time::Duration::minutes(10),
        }
    }

    type Captured = (String, String, HeaderMap, Bytes);

    async fn mock(
        code: StatusCode,
        body: String,
        retry: Option<&'static str>,
        delay: Duration,
    ) -> (
        Url,
        JoinHandle<()>,
        tokio::sync::mpsc::UnboundedReceiver<Captured>,
    ) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let app = axum::Router::new().fallback(move |request: Request| {
            let tx = tx.clone();
            let body = body.clone();
            async move {
                let (parts, request_body) = request.into_parts();
                let bytes = axum::body::to_bytes(request_body, 4096).await.unwrap();
                let _ = tx.send((
                    parts.method.to_string(),
                    parts.uri.to_string(),
                    parts.headers,
                    bytes,
                ));
                tokio::time::sleep(delay).await;
                let mut response = (code, body).into_response();
                if let Some(value) = retry {
                    response
                        .headers_mut()
                        .insert(RETRY_AFTER, HeaderValue::from_static(value));
                }
                if code.is_redirection() {
                    response.headers_mut().insert(
                        reqwest::header::LOCATION,
                        HeaderValue::from_static("/redirected"),
                    );
                }
                response
            }
        });
        let job = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        (
            Url::parse(&format!("http://{address}/emails")).unwrap(),
            job,
            rx,
        )
    }

    #[tokio::test]
    async fn wire_contract_and_key_rotation_keep_request_body_and_idempotency_stable() {
        let scratch = Scratch::new();
        let (url, job, mut captured) =
            mock(StatusCode::OK, RECEIPT.into(), None, Duration::ZERO).await;
        let resend = Resend::build(scratch.settings(), url, false).unwrap();
        let code = message(Purpose::SignIn);
        resend.submit(&code).await.unwrap();
        let first = captured.try_recv().unwrap();
        let rotated = scratch.dir.join("next-key");
        std::fs::write(&rotated, "re_fixture_rotated").unwrap();
        std::fs::set_permissions(&rotated, std::fs::Permissions::from_mode(0o600)).unwrap();
        std::fs::rename(rotated, &scratch.key).unwrap();
        resend.submit(&code).await.unwrap();
        let second = captured.try_recv().unwrap();
        let payload: serde_json::Value = serde_json::from_slice(&first.3).unwrap();
        assert_eq!((first.0.as_str(), first.1.as_str()), ("POST", "/emails"));
        assert_eq!(first.2[AUTHORIZATION], format!("Bearer {FAKE_KEY}"));
        assert_eq!(second.2[AUTHORIZATION], "Bearer re_fixture_rotated");
        assert_eq!(first.2[CONTENT_TYPE], "application/json");
        assert_eq!(
            first.2["idempotency-key"],
            format!("vink-identity/{}", code.delivery.simple())
        );
        assert_eq!(first.2["idempotency-key"], second.2["idempotency-key"]);
        assert_eq!(first.3, second.3);
        assert_eq!(
            resend.payload_fingerprint(&code),
            Some(Sha256::digest(&first.3).into())
        );
        assert_eq!(payload["from"], "Việt Ink <no-reply@example.test>");
        assert_eq!(payload["to"], serde_json::json!(["reader@example.test"]));
        assert_eq!(payload["subject"], "Mã đăng nhập Vink");
        assert!(payload["text"].as_str().unwrap().contains("là 654321"));
        assert_eq!(payload.as_object().unwrap().len(), 4);
        for purpose in [
            Purpose::ChangeEmail,
            Purpose::AttachEmail,
            Purpose::EnrollPassword,
            Purpose::TransportCheck,
        ] {
            let mut code = message(purpose);
            if purpose == Purpose::TransportCheck {
                code.expires_at = OffsetDateTime::UNIX_EPOCH;
            }
            resend.submit(&code).await.unwrap();
            let payload: serde_json::Value =
                serde_json::from_slice(&captured.try_recv().unwrap().3).unwrap();
            assert_eq!(payload["subject"], render(&code).0);
            assert_eq!(
                payload["text"].as_str().unwrap().contains("654321"),
                purpose != Purpose::TransportCheck
            );
        }
        let mut expired = message(Purpose::SignIn);
        expired.expires_at = OffsetDateTime::UNIX_EPOCH;
        assert!(resend.submit(&expired).await.unwrap_err().is_permanent());
        assert!(captured.try_recv().is_err());
        job.abort();
    }

    #[tokio::test]
    async fn provider_replies_are_sanitized_and_classified_without_following_redirects() {
        let scratch = Scratch::new();
        let cases = [
            (307, "", None, Failure::Permanent { status: Some(307) }),
            (
                400,
                r#"{"message":"DO_NOT_SURFACE fake key/code/address"}"#,
                None,
                Failure::Permanent { status: Some(400) },
            ),
            (401, "", None, Failure::Permanent { status: Some(401) }),
            (403, "", None, Failure::Permanent { status: Some(403) }),
            (
                409,
                r#"{"name":"invalid_idempotent_request"}"#,
                Some("12"),
                Failure::Permanent { status: Some(409) },
            ),
            (
                409,
                r#"{"name":"concurrent_idempotent_requests"}"#,
                Some("12"),
                Failure::ProviderRetry {
                    status: 409,
                    after: Duration::from_secs(12),
                },
            ),
            (
                409,
                r#"{"name":"something_else"}"#,
                None,
                Failure::Permanent { status: Some(409) },
            ),
            (422, "", None, Failure::Permanent { status: Some(422) }),
            (
                429,
                "",
                Some("17"),
                Failure::ProviderRetry {
                    status: 429,
                    after: Duration::from_secs(17),
                },
            ),
            (500, "", None, Failure::Transient { status: Some(500) }),
            (
                503,
                "",
                Some("18"),
                Failure::ProviderRetry {
                    status: 503,
                    after: Duration::from_secs(18),
                },
            ),
            (
                200,
                "unexpected acceptance body",
                None,
                Failure::Transient { status: Some(200) },
            ),
        ];
        let code = message(Purpose::SignIn);
        for (status, body, retry, expected) in cases {
            let (url, job, mut captured) = mock(
                StatusCode::from_u16(status).unwrap(),
                body.into(),
                retry,
                Duration::ZERO,
            )
            .await;
            let resend = Resend::build(scratch.settings(), url, false).unwrap();
            let failure = resend.submit(&code).await.unwrap_err();
            assert_eq!(failure, expected);
            assert!(!format!("{failure:?}").contains("DO_NOT_SURFACE"));
            assert!(captured.try_recv().is_ok());
            assert!(
                captured.try_recv().is_err(),
                "must not redirect or retry internally"
            );
            job.abort();
        }
    }

    #[tokio::test]
    async fn ambiguous_timeout_and_connect_failures_do_not_claim_acceptance() {
        let scratch = Scratch::new();
        let (url, job, mut captured) =
            mock(StatusCode::OK, RECEIPT.into(), None, Duration::from_secs(1)).await;
        let mut settings = scratch.settings();
        settings.timeout = Duration::from_millis(200);
        let resend = Resend::build(settings, url, false).unwrap();
        let code = message(Purpose::SignIn);
        assert_eq!(resend.submit(&code).await, Err(Failure::Timeout));
        assert!(
            captured.try_recv().is_ok(),
            "server received the timed-out request"
        );
        job.abort();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = Url::parse(&format!("http://{}/emails", listener.local_addr().unwrap())).unwrap();
        drop(listener);
        let resend = Resend::build(scratch.settings(), url, false).unwrap();
        assert_eq!(resend.submit(&code).await, Err(Failure::Connect));
    }

    #[tokio::test]
    async fn malformed_tls_peer_is_not_classified_as_plain_network_failure() {
        use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};

        let scratch = Scratch::new();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = Url::parse(&format!(
            "https://{}/emails",
            listener.local_addr().unwrap()
        ))
        .unwrap();
        let job = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut hello = [0; 4096];
            assert!(stream.read(&mut hello).await.unwrap() > 0);
            stream.write_all(b"HTTP/1.1 200 OK\r\n\r\n").await.unwrap();
        });
        let resend = Resend::build(scratch.settings(), url, true).unwrap();
        assert_eq!(
            resend.submit(&message(Purpose::SignIn)).await,
            Err(Failure::Tls)
        );
        job.await.unwrap();
    }

    #[tokio::test]
    async fn production_client_forces_tls_and_fails_closed_for_invalid_settings() {
        let scratch = Scratch::new();
        let resend = Resend::new(scratch.settings()).unwrap();
        let header = authorization(&scratch.key).unwrap();
        assert!(header.is_sensitive());
        assert!(!format!("{header:?}").contains(FAKE_KEY));
        assert_eq!(resend.endpoint.as_str(), ENDPOINT);
        assert!(
            resend
                .client
                .get("http://127.0.0.1:9/emails")
                .send()
                .await
                .unwrap_err()
                .is_builder()
        );
        let mut invalid = scratch.settings();
        invalid.from = "sender@example.test, second@example.test".into();
        assert!(Resend::new(invalid).is_err());
        for proxy in [
            "https://proxy:443",
            "http://user:pass@proxy",
            "http://proxy/path",
        ] {
            let mut invalid = scratch.settings();
            invalid.proxy = Some(proxy.into());
            assert!(Resend::new(invalid).is_err());
        }
        let mut valid = scratch.settings();
        valid.proxy = Some("http://egress:3128".into());
        assert!(Resend::new(valid).is_ok());
        std::fs::set_permissions(&scratch.key, std::fs::Permissions::from_mode(0o640)).unwrap();
        assert!(Resend::new(scratch.settings()).is_err());
        assert_eq!(
            resend.submit(&message(Purpose::SignIn)).await,
            Err(Failure::Configuration)
        );
        std::fs::set_permissions(&scratch.key, std::fs::Permissions::from_mode(0o600)).unwrap();
        std::fs::write(&scratch.key, "re_has\nembedded_newline").unwrap();
        assert!(Resend::new(scratch.settings()).is_err());
        std::fs::remove_file(&scratch.key).unwrap();
        assert!(Resend::new(scratch.settings()).is_err());
    }

    #[test]
    fn parses_retry_after_seconds_absolute_http_date_and_overflow() {
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_100_000);
        let date = httpdate::fmt_http_date(now + Duration::from_secs(42));
        assert_eq!(
            retry_after(Some(&HeaderValue::from_str(&date).unwrap()), now),
            Some(Duration::from_secs(42))
        );
        assert_eq!(
            retry_after(Some(&HeaderValue::from_static("22")), now),
            Some(Duration::from_secs(22))
        );
        assert_eq!(
            retry_after(
                Some(&HeaderValue::from_static("999999999999999999999999")),
                now
            ),
            Some(Duration::from_secs(u64::MAX))
        );
        assert_eq!(
            retry_after(Some(&HeaderValue::from_static("not-a-date")), now),
            None
        );
    }
}
