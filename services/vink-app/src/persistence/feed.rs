//! The private Following Feed. Publication's exact first-public view supplies today's
//! pointer and the immutable sort key. Every request rechecks live membership.
use std::collections::HashMap;

use hmac::{Hmac, KeyInit, Mac};
use sha2::Sha256;
use sqlx::PgConnection;
use time::OffsetDateTime;
use uuid::Uuid;

use super::{
    PgStore,
    discovery::{CardRow, project_cards},
    discussion::{integrity, read_snapshot},
    follow::require_active_actor_read,
    handle::ready,
    postgres::classify,
};
use crate::{
    application::PublicWorkSummary,
    application::{discovery::summary, follow::FollowError, ports::StoreError},
    domain::ids::PrincipalId,
};

pub const FEED_PAGE_SIZE: usize = 12;
pub const FEED_CHECK_LIMIT: usize = 48;

pub enum FeedRead<'a> {
    First { around: Option<Uuid> },
    Next { cursor: &'a str, seen: &'a [Uuid] },
    Check(&'a [Uuid]),
}

pub enum FeedReadout {
    Page {
        has_following: bool,
        works: Vec<PublicWorkSummary>,
        retained: Vec<PublicWorkSummary>,
        next_cursor: Option<String>,
    },
    Checked(Vec<PublicWorkSummary>),
    Restart,
}

// All reads, including position lookup and revalidation, use this set. The work and
// graph joins precede any limit or materialization of a revision's content.
macro_rules! eligible_works {
    ($projection:literal, $suffix:literal) => {
        concat!(
            $projection,
            " FROM publications_with_exact_first_time f
              JOIN works w ON w.id = f.work_id
              JOIN author_follows g ON g.followee_id = w.owner_id
              JOIN author_profiles a ON a.principal_id = w.owner_id
             WHERE g.follower_id = $1 AND g.active AND a.current_handle IS NOT NULL ",
            $suffix
        )
    };
}

/// Also used by the PostgreSQL plan test. The cursor uses DESC for both sort keys.
pub const FOLLOWING_FEED_PAGE_SQL: &str = eligible_works!(
    "SELECT f.work_id",
    "AND ($2::timestamptz IS NULL OR (
            (f.first_published_at, f.work_id) <= ($2, $3::uuid)
            AND ($4::bool OR f.work_id <> $3)))
      ORDER BY f.first_published_at DESC, f.work_id DESC
      LIMIT $5"
);

const POSITION_SQL: &str = eligible_works!("SELECT f.first_published_at", "AND f.work_id = $2");

const CARDS_SQL: &str = concat!(
    "WITH eligible AS MATERIALIZED (",
    eligible_works!(
        "SELECT f.work_id, f.revision_id, f.metadata_snapshot_id, f.first_published_at, w.owner_id",
        "AND f.work_id = ANY($2::uuid[])"
    ),
    ")
    SELECT p.work_id, r.title, r.document, a.display_name, a.handle, a.retired,
           p.first_published_at AS published_at, p.metadata_snapshot_id, s.digest,
           p.first_published_at
      FROM eligible p
      JOIN revisions r ON r.work_id = p.work_id AND r.id = p.revision_id
      JOIN public_author_attribution a ON a.id = p.owner_id
      LEFT JOIN work_metadata_snapshots s
             ON s.work_id = p.work_id AND s.id = p.metadata_snapshot_id"
);

async fn position(
    conn: &mut PgConnection,
    actor: Uuid,
    id: Uuid,
) -> Result<Option<OffsetDateTime>, StoreError> {
    sqlx::query_scalar(POSITION_SQL)
        .bind(actor)
        .bind(id)
        .fetch_optional(conn)
        .await
        .map_err(classify)
}

async fn cards(
    conn: &mut PgConnection,
    actor: Uuid,
    page: &[Uuid],
    seen: &[Uuid],
) -> Result<(Vec<PublicWorkSummary>, Vec<PublicWorkSummary>), StoreError> {
    if page.is_empty() && seen.is_empty() {
        return Ok((Vec::new(), Vec::new()));
    }
    let ids: Vec<_> = page.iter().chain(seen).copied().collect();
    let rows: Vec<CardRow> = sqlx::query_as(CARDS_SQL)
        .bind(actor)
        .bind(ids)
        .fetch_all(&mut *conn)
        .await
        .map_err(classify)?;
    let (works, catalog) = project_cards(conn, rows).await?;
    let by_id = works
        .into_iter()
        .map(|work| Ok((work.work_id.as_uuid(), summary(work, &catalog)?)))
        .collect::<Result<HashMap<_, _>, StoreError>>()?;
    let works = page
        .iter()
        .map(|id| {
            by_id
                .get(id)
                .cloned()
                .ok_or_else(|| integrity("feed page card missing", id))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let retained = seen
        .iter()
        .filter_map(|id| by_id.get(id).cloned())
        .collect();
    Ok((works, retained))
}

// The only position payload is a public WorkId. A per-anchor MAC authenticates its
// use in this viewer/session, representation, fixed query, order and page size.
struct CursorKey {
    viewer: PrincipalId,
    session: [u8; 32],
}

impl CursorKey {
    fn mac(&self, work: Uuid) -> Hmac<Sha256> {
        let mut mac = Hmac::<Sha256>::new_from_slice(&self.session)
            .expect("HMAC supports a 32-byte session key");
        mac.update(b"vink:following-feed:v1:unfiltered:first-public-desc,work-desc:12");
        mac.update(self.viewer.as_uuid().as_bytes());
        mac.update(work.as_bytes());
        mac
    }

    fn encode(&self, work: Uuid) -> String {
        let signature = self.mac(work).finalize().into_bytes();
        let hex: String = signature.iter().map(|byte| format!("{byte:02x}")).collect();
        format!("f1.{}.{hex}", work.simple())
    }

    fn decode(&self, token: &str) -> Option<Uuid> {
        if token.len() != 100 {
            return None;
        }
        let (id, signature) = token.strip_prefix("f1.")?.split_once('.')?;
        if id.len() != 32 || signature.len() != 64 {
            return None;
        }
        let work = Uuid::try_parse(id).ok()?;
        let mut signature_bytes = [0; 32];
        let (hex_bytes, _) = signature.as_bytes().as_chunks::<2>();
        for (out, hex) in signature_bytes.iter_mut().zip(hex_bytes) {
            let hex = std::str::from_utf8(hex).ok()?;
            *out = u8::from_str_radix(hex, 16).ok()?;
        }
        self.mac(work).verify_slice(&signature_bytes).ok()?;
        Some(work)
    }
}

impl PgStore {
    pub async fn following_feed(
        &self,
        viewer: PrincipalId,
        session: [u8; 32],
        request: FeedRead<'_>,
    ) -> Result<FeedReadout, FollowError> {
        ready(self.pool()).await?;
        let mut tx = read_snapshot(self.pool()).await?;
        require_active_actor_read(&mut tx, viewer).await?;
        let actor = viewer.as_uuid();
        let key = CursorKey { viewer, session };
        let (start, inclusive, seen) = match request {
            FeedRead::Check(seen) => {
                if seen.len() > FEED_CHECK_LIMIT {
                    return Err(FollowError::Invalid("seen"));
                }
                let (_, retained) = cards(&mut tx, actor, &[], seen).await?;
                tx.commit().await.map_err(classify)?;
                return Ok(FeedReadout::Checked(retained));
            }
            FeedRead::First { around: None } => (None, false, &[][..]),
            FeedRead::First { around: Some(id) } => (
                position(&mut tx, actor, id).await?.map(|at| (at, id)),
                true,
                &[][..],
            ),
            FeedRead::Next { cursor, seen } => {
                if seen.len() > FEED_CHECK_LIMIT {
                    return Err(FollowError::Invalid("seen"));
                }
                let Some(id) = key.decode(cursor) else {
                    return Ok(FeedReadout::Restart);
                };
                let Some(at) = position(&mut tx, actor, id).await? else {
                    return Ok(FeedReadout::Restart);
                };
                (Some((at, id)), false, seen)
            }
        };
        // Like public discovery, plan with this viewer/position rather than a generic
        // plan shared between a sparse follower and someone following many prolific authors.
        sqlx::query("SET LOCAL plan_cache_mode = force_custom_plan")
            .execute(&mut *tx)
            .await
            .map_err(classify)?;
        let mut ids: Vec<Uuid> = sqlx::query_scalar(FOLLOWING_FEED_PAGE_SQL)
            .bind(actor)
            .bind(start.map(|(at, _)| at))
            .bind(start.map(|(_, id)| id))
            .bind(inclusive)
            .bind((FEED_PAGE_SIZE + 1) as i64)
            .fetch_all(&mut *tx)
            .await
            .map_err(classify)?;
        let next_cursor = (ids.len() > FEED_PAGE_SIZE).then(|| {
            ids.truncate(FEED_PAGE_SIZE);
            key.encode(ids[FEED_PAGE_SIZE - 1])
        });
        let has_following = !ids.is_empty() || sqlx::query_scalar(
            "SELECT EXISTS (
                SELECT FROM author_follows g JOIN author_profiles a ON a.principal_id = g.followee_id
                WHERE g.follower_id = $1 AND g.active AND a.current_handle IS NOT NULL)",
        )
        .bind(actor)
        .fetch_one(&mut *tx)
        .await
        .map_err(classify)?;
        let (works, retained) = cards(&mut tx, actor, &ids, seen).await?;
        tx.commit().await.map_err(classify)?;
        Ok(FeedReadout::Page {
            has_following,
            works,
            retained,
            next_cursor,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cursor_binds_anchor_viewer_session_and_representation() {
        let viewer = PrincipalId::from_uuid(Uuid::from_u128(1));
        let key = CursorKey {
            viewer,
            session: [7; 32],
        };
        let id = Uuid::from_u128(3);
        let token = key.encode(id);
        assert_eq!(key.decode(&token), Some(id));
        assert_eq!(
            CursorKey {
                viewer,
                session: [8; 32]
            }
            .decode(&token),
            None
        );
        assert_eq!(
            CursorKey {
                viewer: PrincipalId::from_uuid(Uuid::from_u128(2)),
                session: [7; 32]
            }
            .decode(&token),
            None
        );
        let changed_id = token.replacen(
            &id.simple().to_string(),
            &Uuid::from_u128(4).simple().to_string(),
            1,
        );
        assert_eq!(key.decode(&changed_id), None);
        assert_eq!(key.decode(&token.replacen("f1", "f2", 1)), None);
        assert_eq!(key.decode(""), None);
    }
}
