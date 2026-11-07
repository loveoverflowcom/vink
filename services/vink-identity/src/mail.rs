//! Delivery of one-time codes.
//!
//! Only a **local outbox** exists: each message is a file (mode `0600`) in a directory that only
//! the Identity user can open. It makes the flow executable and inspectable on a developer machine
//! and in `verify postgres`; nobody outside the machine receives a code, so it is not a production
//! mail transport. SMTP or a mail API (deliverability, SPF/DKIM/DMARC, bounces) is
//! NOT_IMPLEMENTED and blocks production use of the authority.

use std::io::Write;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
use uuid::Uuid;

use crate::email_code::{Code, EmailAddress};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Purpose {
    SignIn,
    ChangeEmail,
}

impl Purpose {
    pub fn label(self) -> &'static str {
        match self {
            Purpose::SignIn => "sign_in",
            Purpose::ChangeEmail => "change_email",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeliveryFailed;

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

    pub fn deliver(
        &self,
        challenge: Uuid,
        to: &EmailAddress,
        code: &Code,
        purpose: Purpose,
        expires_at: OffsetDateTime,
    ) -> Result<(), DeliveryFailed> {
        let expires = expires_at.format(&Rfc3339).map_err(|_| DeliveryFailed)?;
        let action = match purpose {
            Purpose::SignIn => "đăng nhập Vink",
            Purpose::ChangeEmail => "xác nhận địa chỉ email mới cho tài khoản Vink",
        };
        let message = format!(
            "To: {to}\nSubject: Mã {action}\nX-Vink-Purpose: {purpose}\nX-Vink-Expires-At: {expires}\nX-Vink-Code: {code}\n\n\
             Mã {action} của bạn là {code}.\n\
             Mã chỉ dùng được một lần, trên trình duyệt hoặc ứng dụng vừa yêu cầu, và hết hạn sau 10 phút.\n\
             Nếu bạn không yêu cầu mã này, hãy bỏ qua thư; không ai đăng nhập được nếu không có mã.\n",
            to = to.as_str(),
            purpose = purpose.label(),
            code = code.expose(),
        );
        let path = self.dir.join(format!("{}.txt", challenge.simple()));
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&path)
            .map_err(|_| DeliveryFailed)?;
        // No fsync: on a busy disk it can take seconds and break the call deadline; a message
        // lost in a crash only means the person asks for a new code.
        file.write_all(message.as_bytes())
            .map_err(|_| DeliveryFailed)
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

    #[test]
    fn the_outbox_is_private_and_never_overwrites() {
        assert!(LocalOutbox::open(Path::new("/nonexistent/vink-outbox")).is_err());
        let open = scratch("open", 0o750);
        assert!(LocalOutbox::open(&open).is_err());
        let dir = scratch("private", 0o700);
        let outbox = LocalOutbox::open(&dir).unwrap();
        let id = Uuid::now_v7();
        let to = EmailAddress::parse("nguoi.viet@vi-du.vn").unwrap();
        let code = Code::parse("012345").unwrap();
        let expires = OffsetDateTime::UNIX_EPOCH;
        outbox
            .deliver(id, &to, &code, Purpose::SignIn, expires)
            .unwrap();
        let path = dir.join(format!("{}.txt", id.simple()));
        assert_eq!(std::fs::metadata(&path).unwrap().mode() & 0o777, 0o600);
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("X-Vink-Code: 012345"), "{text}");
        assert!(
            text.contains("Mã đăng nhập Vink của bạn là 012345"),
            "{text}"
        );
        // A second delivery for the same challenge is refused, not appended or replaced.
        assert_eq!(
            outbox.deliver(id, &to, &code, Purpose::SignIn, expires),
            Err(DeliveryFailed)
        );
        std::fs::remove_dir_all(&dir).unwrap();
        std::fs::remove_dir_all(&open).unwrap();
    }
}
