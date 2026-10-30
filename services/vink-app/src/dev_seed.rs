//! Synthetic development fixture, applied only by `vink-app dev-seed` in the local profile.
//!
//! It is not a migration and never runs at startup. Every value is labelled as sample data so it
//! cannot pass for real authors, statistics or community content.

use sqlx::PgPool;
use sqlx::types::Json;
use uuid::{Uuid, uuid};

use crate::domain::document::{BlockData, Document, DocumentData, SCHEMA_VERSION, Title};
use crate::domain::revision::RevisionContent;

/// Owner of the sample work. `.env.example` points `VINK_DEV_PRINCIPAL` here.
pub const SAMPLE_AUTHOR: Uuid = uuid!("11111111-1111-4111-8111-111111111111");
/// A second principal for checking that drafts stay private to their owner.
pub const OTHER_PRINCIPAL: Uuid = uuid!("22222222-2222-4222-8222-222222222222");
pub const SAMPLE_WORK: Uuid = uuid!("33333333-3333-4333-8333-333333333333");
pub const SAMPLE_REVISION: Uuid = uuid!("44444444-4444-4444-8444-444444444444");

pub const SAMPLE_TITLE: &str = "[Mẫu] Mưa trên phố cũ";

/// Published stanzas: original synthetic verse written for this fixture.
pub const PUBLISHED_STANZAS: [&[&str]; 2] = [
    &[
        "Phố cũ mưa giăng sợi mỏng,",
        "mái ngói rêu xanh lặng thầm.",
        "Ai để quên chiếc ô vàng",
        "bên hiên quán nước ven sông.",
    ],
    &[
        "Mực loang trên trang giấy trắng,",
        "chữ nghiêng như gió qua cầu.",
        "Câu thơ chưa kịp đặt tên",
        "    đã theo mưa về rất lâu.",
    ],
];

/// Exists only in the working draft; public surfaces must never show it.
pub const DRAFT_ONLY_LINE: &str = "Khổ đang viết dở — chưa công khai.";

fn document(stanzas: &[&[&str]]) -> Document {
    Document::parse(DocumentData {
        schema_version: SCHEMA_VERSION,
        blocks: stanzas
            .iter()
            .map(|lines| BlockData {
                lines: lines.iter().map(|l| l.to_string()).collect(),
            })
            .collect(),
    })
    .expect("fixture document is valid")
}

/// Idempotent: existing fixture rows are left untouched.
pub async fn apply(pool: &PgPool) -> Result<(), sqlx::Error> {
    let published = RevisionContent::new(
        Title::parse(SAMPLE_TITLE).expect("fixture title is valid"),
        document(&PUBLISHED_STANZAS),
    )
    .expect("fixture revision is valid");
    let mut draft_stanzas: Vec<&[&str]> = PUBLISHED_STANZAS.to_vec();
    draft_stanzas.push(&[DRAFT_ONLY_LINE]);
    let draft = document(&draft_stanzas);

    let mut tx = pool.begin().await?;
    for (id, name) in [
        (SAMPLE_AUTHOR, "Tác giả mẫu (dữ liệu phát triển)"),
        (OTHER_PRINCIPAL, "Người viết khác (dữ liệu phát triển)"),
    ] {
        sqlx::query(
            "INSERT INTO principals (id, display_name) VALUES ($1, $2) ON CONFLICT (id) DO NOTHING",
        )
        .bind(id)
        .bind(name)
        .execute(&mut *tx)
        .await?;
    }
    sqlx::query("INSERT INTO works (id, owner_id) VALUES ($1, $2) ON CONFLICT (id) DO NOTHING")
        .bind(SAMPLE_WORK)
        .bind(SAMPLE_AUTHOR)
        .execute(&mut *tx)
        .await?;
    sqlx::query(
        "INSERT INTO revisions (id, work_id, title, document, content_hash, created_by)
         VALUES ($1, $2, $3, $4, $5, $6) ON CONFLICT (id) DO NOTHING",
    )
    .bind(SAMPLE_REVISION)
    .bind(SAMPLE_WORK)
    .bind(published.title().as_str())
    .bind(Json(published.document().to_data()))
    .bind(published.hash().as_str())
    .bind(SAMPLE_AUTHOR)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "INSERT INTO publications (work_id, revision_id, generation, published_at)
         VALUES ($1, $2, 1, now()) ON CONFLICT (work_id) DO NOTHING",
    )
    .bind(SAMPLE_WORK)
    .bind(SAMPLE_REVISION)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "INSERT INTO working_drafts (work_id, version, title, document)
         VALUES ($1, 1, $2, $3) ON CONFLICT (work_id) DO NOTHING",
    )
    .bind(SAMPLE_WORK)
    .bind(SAMPLE_TITLE)
    .bind(Json(draft.to_data()))
    .execute(&mut *tx)
    .await?;
    tx.commit().await
}
