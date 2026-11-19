//! Standing, profile CAS and display-name update commit under the same principal fence.
use sqlx::types::Json;
use sqlx::{PgConnection, Postgres, Transaction};
use time::OffsetDateTime;
use uuid::Uuid;

use super::{
    PgStore, discussion::read_snapshot, handle::ready, postgres::classify,
    principal::require_active,
};
use crate::application::handle::{AuthorRef, Resolution};
use crate::application::profile::{
    OwnProfile, ProfileError, ProfileWork, ProfileWorks, SaveProfile,
};
use crate::domain::discovery::{DiscoveryCursor, PagePosition, PageSize, SearchScope, cut_page};
use crate::domain::handle;
use crate::domain::ids::{PrincipalId, WorkId};
use crate::domain::principal::PenName;
use crate::domain::profile::{Bio, ProfilePolicies};

#[derive(sqlx::FromRow)]
struct OwnRow {
    pen_name: String,
    handle: String,
    handle_setup_complete: bool,
    remaining_handle_changes: i16,
    profile_version: i64,
    bio: Option<String>,
    policies: Json<ProfilePolicies>,
}

async fn own(conn: &mut PgConnection, principal: Uuid) -> Result<OwnProfile, ProfileError> {
    let row: OwnRow = sqlx::query_as(
        "SELECT p.display_name AS pen_name, s.current_handle AS handle,
                s.setup_complete AS handle_setup_complete, s.remaining_changes AS remaining_handle_changes,
                s.profile_version, s.bio, s.policies
           FROM author_profiles s JOIN principals p ON p.id = s.principal_id
          WHERE s.principal_id = $1 AND s.current_handle IS NOT NULL",
    )
    .bind(principal)
    .fetch_optional(conn)
    .await
    .map_err(classify)?
    .ok_or(ProfileError::Missing)?;
    Ok(OwnProfile {
        principal_id: principal,
        pen_name: row.pen_name,
        handle: row.handle,
        handle_setup_complete: row.handle_setup_complete,
        remaining_handle_changes: row.remaining_handle_changes,
        profile_version: row.profile_version,
        bio: row.bio,
        policies: row.policies.0,
    })
}

impl PgStore {
    pub async fn own_profile(&self, actor: PrincipalId) -> Result<OwnProfile, ProfileError> {
        ready(self.pool()).await?;
        let mut tx = self.pool().begin().await.map_err(classify)?;
        require_active(&mut tx, actor).await?;
        let profile = own(&mut tx, actor.as_uuid()).await?;
        tx.commit().await.map_err(classify)?;
        Ok(profile)
    }

    pub async fn save_profile(
        &self,
        actor: PrincipalId,
        command: SaveProfile,
    ) -> Result<OwnProfile, ProfileError> {
        let id = actor.as_uuid();
        if id != command.principal_id {
            return Err(ProfileError::WrongOwner);
        }
        let pen = PenName::parse(&command.pen_name).ok_or(ProfileError::Invalid("pen_name"))?;
        let bio = Bio::parse(command.bio.as_deref().unwrap_or_default())
            .ok_or(ProfileError::Invalid("bio"))?;
        if command.expected_version < 0 {
            return Err(ProfileError::Invalid("expected_version"));
        }
        ready(self.pool()).await?;
        let mut tx = self.pool().begin().await.map_err(classify)?;
        require_active(&mut tx, actor).await?;
        let current = own(&mut tx, id).await?;
        if current.profile_version != command.expected_version {
            return Err(ProfileError::Stale(current.profile_version));
        }
        if !current.policies.permits_change(command.policies) {
            return Err(ProfileError::DisclosurePending);
        }
        sqlx::query(
            "UPDATE author_profiles SET bio = $2, policies = $3, profile_version = profile_version + 1
              WHERE principal_id = $1",
        )
        .bind(id)
        .bind(bio.value())
        .bind(Json(command.policies))
        .execute(&mut *tx)
        .await
        .map_err(classify)?;
        if current.pen_name != pen.as_str() {
            sqlx::query("UPDATE principals SET display_name = $2 WHERE id = $1")
                .bind(id)
                .bind(pen.as_str())
                .execute(&mut *tx)
                .await
                .map_err(classify)?;
        }
        let saved = own(&mut tx, id).await?;
        tx.commit().await.map_err(classify)?;
        Ok(saved)
    }

    pub async fn resolve_handle(
        &self,
        name: &str,
        cursor: Option<&str>,
    ) -> Result<Resolution, ProfileError> {
        if !handle::valid(name) {
            return Ok(Resolution::Missing);
        }
        let mut tx = read_snapshot(self.pool()).await?;
        if !sqlx::query_scalar::<_, bool>("SELECT ready FROM author_handle_rollout WHERE singleton")
            .fetch_one(&mut *tx)
            .await
            .map_err(classify)?
        {
            return Err(ProfileError::Store(
                crate::application::ports::StoreError::Unavailable(
                    "author profile backfill pending".into(),
                ),
            ));
        }

        #[derive(sqlx::FromRow)]
        struct PublicRow {
            principal_id: Uuid,
            pen_name: String,
            current_handle: Option<String>,
            bio: Option<String>,
            policies: Json<ProfilePolicies>,
            has_public_works: bool,
        }

        // No private value or version is selected. The owner and all page rows share one snapshot.
        let row: Option<PublicRow> = sqlx::query_as(
            "SELECT s.principal_id, p.display_name AS pen_name, s.current_handle,
                    CASE WHEN s.policies->>'bio' = 'everyone' THEN s.bio END AS bio,
                    s.policies,
                    EXISTS (SELECT FROM works w JOIN publications pub ON pub.work_id = w.id
                             WHERE w.owner_id = s.principal_id) AS has_public_works
               FROM author_handle_names n
               JOIN author_profiles s USING (principal_id)
               JOIN principals p ON p.id = s.principal_id
              WHERE n.handle = $1",
        )
        .bind(name)
        .fetch_optional(&mut *tx)
        .await
        .map_err(classify)?;
        let result = match row {
            None => Resolution::Missing,
            Some(PublicRow {
                current_handle: None,
                ..
            }) => Resolution::Gone,
            Some(PublicRow {
                current_handle: Some(current),
                ..
            }) if current != name => Resolution::Alias {
                href: AuthorRef::href_for(&current),
            },
            Some(row) => {
                let relationships = super::follow::public_relationships_summary(
                    &mut tx,
                    row.principal_id,
                    name,
                    row.policies.0,
                )
                .await?;
                Resolution::Current {
                    author: AuthorRef::new(row.pen_name, name.to_owned()),
                    bio: row.bio,
                    has_public_works: row.has_public_works,
                    publications: public_works(&mut tx, row.principal_id, name, cursor).await?,
                    relationships: Box::new(relationships),
                }
            }
        };
        tx.commit().await.map_err(classify)?;
        Ok(result)
    }
}

async fn public_works(
    tx: &mut Transaction<'_, Postgres>,
    owner: Uuid,
    handle: &str,
    cursor: Option<&str>,
) -> Result<ProfileWorks, ProfileError> {
    let scope = SearchScope::of(Some(&format!("author-profile-v1:@{handle}")), &[]);
    let after = match cursor {
        None | Some("") => None,
        Some(raw) => match DiscoveryCursor::decode(raw) {
            Ok(cursor) if cursor.scope() == scope => Some(cursor.after()),
            _ => return Ok(ProfileWorks::Restart),
        },
    };
    let size = PageSize::DEFAULT;
    let rows: Vec<(Uuid, String, OffsetDateTime)> = sqlx::query_as(
        "SELECT p.work_id, r.title, p.published_at
           FROM works w JOIN publications p ON p.work_id = w.id
           JOIN revisions r ON r.work_id = p.work_id AND r.id = p.revision_id
          WHERE w.owner_id = $1
            AND ($2::timestamptz IS NULL OR p.published_at < $2
                 OR (p.published_at = $2 AND p.work_id > $3::uuid))
          ORDER BY p.published_at DESC, p.work_id
          LIMIT $4",
    )
    .bind(owner)
    .bind(after.map(|pos| pos.published_at))
    .bind(after.map(|pos| pos.work_id.as_uuid()))
    .bind(size.fetch() as i64)
    .fetch_all(&mut **tx)
    .await
    .map_err(classify)?;
    let (rows, next) = cut_page(rows, size, scope, |row| PagePosition {
        work_id: WorkId::from_uuid(row.0),
        published_at: row.2,
    });
    Ok(ProfileWorks::Page {
        works: rows
            .into_iter()
            .map(|(work_id, title, _)| ProfileWork { work_id, title })
            .collect(),
        next_cursor: next.map(|cursor| cursor.encode()),
    })
}
