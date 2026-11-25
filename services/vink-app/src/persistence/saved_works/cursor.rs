//! The anchor carries its original position even after clear/resave. Only the
//! server knows the pair's random key, retained with the fence until owner deletion.
use hmac::{Hmac, KeyInit, Mac};
use sha2::Sha256;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::domain::ids::PrincipalId;

pub(super) struct Anchor {
    pub work: Uuid,
    pub at: OffsetDateTime,
}

impl Anchor {
    fn mac(&self, owner: PrincipalId, key: Uuid) -> Hmac<Sha256> {
        let mut mac =
            Hmac::<Sha256>::new_from_slice(key.as_bytes()).expect("HMAC accepts UUID keys");
        mac.update(b"vink:saved-works:v1:saved-desc,work-asc:20");
        mac.update(owner.as_uuid().as_bytes());
        mac.update(self.work.as_bytes());
        mac.update(&self.at.unix_timestamp_nanos().to_be_bytes());
        mac
    }

    pub fn encode(&self, owner: PrincipalId, key: Uuid) -> String {
        let digest = self.mac(owner, key).finalize().into_bytes();
        let tag = Uuid::from_slice(&digest[..16]).expect("16-byte prefix");
        format!(
            "s1.{}.{}.{}",
            self.work.simple(),
            self.at.unix_timestamp_nanos(),
            tag.simple()
        )
    }

    pub fn parse(raw: &str) -> Option<(Self, Uuid)> {
        let mut fields = raw.strip_prefix("s1.")?.split('.');
        let work = Uuid::try_parse(fields.next()?).ok()?;
        let at = OffsetDateTime::from_unix_timestamp_nanos(fields.next()?.parse().ok()?).ok()?;
        let tag = Uuid::try_parse(fields.next()?).ok()?;
        fields.next().is_none().then_some((Self { work, at }, tag))
    }

    pub fn verifies(&self, owner: PrincipalId, key: Uuid, tag: Uuid) -> bool {
        self.mac(owner, key)
            .verify_truncated_left(tag.as_bytes())
            .is_ok()
    }
}
