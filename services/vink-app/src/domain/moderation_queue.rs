//! Queue position only: never a grant, frozen snapshot, or authority token.
use super::ids::PrincipalId;
use sha2::{Digest, Sha256};
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum QueueKind {
    Works = 0,
    Appeals = 1,
}
#[derive(Clone, Debug)]
pub struct QueueQuery {
    size: usize,
    scope: [u8; 32],
    after: Option<(OffsetDateTime, Uuid)>,
}
impl QueueQuery {
    pub fn parse(
        actor: PrincipalId,
        kind: QueueKind,
        limit: Option<&str>,
        cursor: Option<&str>,
    ) -> Result<Self, &'static str> {
        let size = match limit {
            None => 50,
            Some(s) if !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()) => s
                .parse::<usize>()
                .ok()
                .filter(|n| (1..=100).contains(n))
                .ok_or("limit")?,
            _ => return Err("limit"),
        };
        let mut h = Sha256::new();
        h.update(b"vink-moderation-queue-v1");
        h.update(actor.as_uuid().as_bytes());
        h.update([kind as u8, size as u8]);
        let mut query = Self {
            size,
            scope: h.finalize().into(),
            after: None,
        };
        if let Some(raw) = cursor {
            if raw.len() != 114 {
                return Err("cursor");
            }
            let bytes = raw
                .as_bytes()
                .as_chunks::<2>()
                .0
                .iter()
                .map(|p| {
                    let digit = |b| match b {
                        b'0'..=b'9' => Ok(b - b'0'),
                        b'a'..=b'f' => Ok(b - b'a' + 10),
                        _ => Err("cursor"),
                    };
                    Ok((digit(p[0])? << 4) | digit(p[1])?)
                })
                .collect::<Result<Vec<u8>, &'static str>>()?;
            if bytes[0] != 1 || bytes[1..33] != query.scope {
                return Err("cursor");
            }
            let at = i64::from_be_bytes(bytes[33..41].try_into().map_err(|_| "cursor")?);
            let at = OffsetDateTime::from_unix_timestamp_nanos(i128::from(at) * 1000)
                .map_err(|_| "cursor")?;
            let id = Uuid::from_slice(&bytes[41..]).map_err(|_| "cursor")?;
            query.after = Some((at, id));
        }
        Ok(query)
    }
    /// Opaque effective-authority fingerprint, not a transferable capability.
    pub fn authority_key(&self, global: bool, works: &[uuid::Uuid]) -> String {
        let mut h = Sha256::new();
        h.update(self.scope);
        h.update([global as u8]);
        if !global {
            let mut ids = works.to_vec();
            ids.sort();
            ids.dedup();
            for id in ids {
                h.update(id.as_bytes());
            }
        }
        h.finalize().iter().map(|b| format!("{b:02x}")).collect()
    }
    pub fn size(&self) -> usize {
        self.size
    }
    pub fn fetch(&self) -> i64 {
        (self.size + 1) as i64
    }
    pub fn bounds(&self) -> (Option<OffsetDateTime>, Option<Uuid>) {
        self.after
            .map_or((None, None), |(at, id)| (Some(at), Some(id)))
    }
    pub fn cursor_after(&self, at: OffsetDateTime, id: Uuid) -> String {
        let mut bytes = vec![1];
        bytes.extend(self.scope);
        bytes.extend(((at.unix_timestamp_nanos() / 1000) as i64).to_be_bytes());
        bytes.extend(id.as_bytes());
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn queue_cursor_is_canonical_versioned_and_actor_endpoint_limit_bound() {
        let actor = PrincipalId::from_uuid(Uuid::from_u128(1));
        let q = QueueQuery::parse(actor, QueueKind::Works, None, None).unwrap();
        assert_eq!((q.size(), q.fetch()), (50, 51));
        let at = OffsetDateTime::UNIX_EPOCH;
        let c = q.cursor_after(at, Uuid::from_u128(2));
        assert_eq!(
            QueueQuery::parse(actor, QueueKind::Works, None, Some(&c))
                .unwrap()
                .bounds(),
            (Some(at), Some(Uuid::from_u128(2)))
        );
        assert!(QueueQuery::parse(actor, QueueKind::Appeals, None, Some(&c)).is_err());
        assert!(QueueQuery::parse(actor, QueueKind::Works, Some("1"), Some(&c)).is_err());
        assert!(
            QueueQuery::parse(
                PrincipalId::from_uuid(Uuid::from_u128(3)),
                QueueKind::Works,
                None,
                Some(&c)
            )
            .is_err()
        );
        for raw in ["", "00", &c.to_uppercase(), &format!("{c}00")] {
            assert!(QueueQuery::parse(actor, QueueKind::Works, None, Some(raw)).is_err());
        }
        for raw in ["0", "101", "-1", "1.0", " 2", "9999999999999999999999999"] {
            assert!(QueueQuery::parse(actor, QueueKind::Works, Some(raw), None).is_err());
        }
    }
}
