//! Deterministic synthetic journey corpus (#234): 200 works and 1,000 comments reusing
//! #229's document corpus generator and real `vink_app::domain` validators/serializers.
use serde::Serialize;
use sha2::{Digest, Sha256};
use uuid::Uuid;
use vink_app::domain::discussion::body::CommentBody;
use vink_app::domain::document::{Document, DocumentData, Title};
use vink_app::domain::revision::ContentHash;

use super::corpus;

pub const JOURNEY_SEED: u64 = 234;
pub const WORK_COUNT: usize = 200;
pub const COMMENT_COUNT: usize = 1_000;

#[derive(Debug, Clone, Serialize)]
pub struct PerfJourneyManifest {
    pub journey_seed: u64,
    pub document_corpus_seed: u64,
    pub corpus_digest: String,
    pub work_count: usize,
    pub comment_count: usize,
    pub roles: PerfRoles,
    pub works: Vec<PerfWork>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PerfRoles {
    pub standard_work_id: String,
    pub standard_work_title: String,
    pub long_poem_work_id: String,
    pub long_vi_work_id: String,
    pub medium_emoji_work_id: String,
    pub withdrawal_work_id: String,
    pub empty_discussion_work_id: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct PerfWork {
    pub index: usize,
    pub role: &'static str,
    pub work_id: String,
    pub revision_id: String,
    pub snapshot_id: String,
    pub thread_id: String,
    pub title: String,
    pub shape: &'static str,
    pub scalars: usize,
    pub canonical_json_bytes: usize,
    pub content_hash: String,
    pub form_label: &'static str,
    pub theme_label: &'static str,
    pub published_offset_secs: i64,
    pub has_working_draft: bool,
    pub document: DocumentData,
    pub comments: Vec<PerfComment>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PerfComment {
    pub comment_id: String,
    pub kind: &'static str,
    pub root_id: Option<String>,
    pub reply_to_id: Option<String>,
    pub replied_to_reply: bool,
    pub body: String,
    pub created_offset_ms: i64,
}

fn deterministic_uuid(tag: &str) -> String {
    let mut bytes = [0u8; 16];
    let digest = Sha256::digest(tag.as_bytes());
    bytes.copy_from_slice(&digest[..16]);
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Uuid::from_bytes(bytes).to_string()
}

fn spec_for_index(idx: usize) -> (&'static str, &'static str, usize, usize, i64) {
    const STANDARD_SHAPES: [&str; 6] = [
        "vi_nfc",
        "vi_nfd",
        "poem_tabs",
        "ascii_repeat",
        "escaping",
        "emoji",
    ];
    const STANDARD_SIZES: [usize; 6] = [1_200, 1_500, 2_400, 900, 1_400, 1_800];
    match idx {
        0 => ("standard", "poem_tabs", 2_400, 120, 10),
        1 => ("long_poem_200k", "poem_tabs", 200_000, 50, 20),
        2 => ("withdrawal_target", "vi_nfc", 1_600, 20, 30),
        3 => ("long_vi_200k", "vi_nfc", 200_000, 50, 40),
        4 => ("medium_emoji_30k", "emoji", 30_000, 40, 50),
        5..=148 => (
            "standard_pool",
            STANDARD_SHAPES[(idx - 5) % STANDARD_SHAPES.len()],
            STANDARD_SIZES[(idx - 5) % STANDARD_SIZES.len()],
            4,
            ((idx + 1) * 60) as i64,
        ),
        149..=196 => (
            "standard_pool",
            STANDARD_SHAPES[(idx - 5) % STANDARD_SHAPES.len()],
            STANDARD_SIZES[(idx - 5) % STANDARD_SIZES.len()],
            3,
            ((idx + 1) * 60) as i64,
        ),
        _ => (
            "empty_discussion",
            STANDARD_SHAPES[(idx - 5) % STANDARD_SHAPES.len()],
            STANDARD_SIZES[(idx - 5) % STANDARD_SIZES.len()],
            0,
            ((idx + 1) * 60) as i64,
        ),
    }
}

fn build_comments(work_idx: usize, total: usize) -> Vec<PerfComment> {
    if total == 0 {
        return Vec::new();
    }
    let mut out = Vec::with_capacity(total);
    let mut seq = 0usize;
    let mut next_offset_ms = 1_000i64;

    if work_idx == 0 {
        // 60 roots + 60 replies = 120 comments:
        // first 5 roots get 6 replies each (exercises "Xem thêm phản hồi" beyond preview 3),
        // next 15 roots get 2 replies each, remaining 40 roots get 0 replies (3 root pages of 20).
        let mut root_ids = Vec::with_capacity(60);
        for r in 0..60usize {
            let cid = deterministic_uuid(&format!("vink-234-comment-{work_idx}-r{r}"));
            let raw = format!(
                "[Mẫu #234] Bình luận gốc #{r:02} — Mưa qua hiên cũ, chữ tiếng Việt vẫn tròn dấu."
            );
            let body = CommentBody::parse(&raw).unwrap().as_str().to_string();
            out.push(PerfComment {
                comment_id: cid.clone(),
                kind: "root",
                root_id: None,
                reply_to_id: None,
                replied_to_reply: false,
                body,
                created_offset_ms: next_offset_ms,
            });
            root_ids.push(cid);
            next_offset_ms += 1_000;
            seq += 1;
        }
        for (r, root_id) in root_ids.iter().enumerate() {
            let replies = if r < 5 {
                6
            } else if r < 20 {
                2
            } else {
                0
            };
            let mut prev_reply: Option<String> = None;
            for k in 0..replies {
                let cid = deterministic_uuid(&format!("vink-234-comment-{work_idx}-r{r}-p{k}"));
                let raw = format!(
                    "[Mẫu #234] Phản hồi #{k:02} dưới gốc #{r:02} — Nhịp đọc giữ nguyên khi mở thêm."
                );
                let body = CommentBody::parse(&raw).unwrap().as_str().to_string();
                let reply_to = if k >= 2 { prev_reply.clone() } else { None };
                out.push(PerfComment {
                    comment_id: cid.clone(),
                    kind: "reply",
                    root_id: Some(root_id.clone()),
                    reply_to_id: reply_to.clone(),
                    replied_to_reply: reply_to.is_some(),
                    body,
                    created_offset_ms: next_offset_ms,
                });
                prev_reply = Some(cid);
                next_offset_ms += 1_000;
                seq += 1;
            }
        }
        assert_eq!(seq, total);
        return out;
    }

    // For all other works: ~half roots, ~half replies, respecting two-tier placement.
    let root_count = if total >= 2 { total.div_ceil(2) } else { total };
    let reply_count = total - root_count;
    let mut root_ids = Vec::with_capacity(root_count);
    for r in 0..root_count {
        let cid = deterministic_uuid(&format!("vink-234-comment-{work_idx}-r{r}"));
        let raw = format!("[Mẫu #234] Nhận xét #{r:02} cho tác phẩm #{work_idx:03}.");
        let body = CommentBody::parse(&raw).unwrap().as_str().to_string();
        out.push(PerfComment {
            comment_id: cid.clone(),
            kind: "root",
            root_id: None,
            reply_to_id: None,
            replied_to_reply: false,
            body,
            created_offset_ms: next_offset_ms,
        });
        root_ids.push(cid);
        next_offset_ms += 1_000;
    }
    for k in 0..reply_count {
        let root_id = &root_ids[k % root_ids.len()];
        let cid = deterministic_uuid(&format!("vink-234-comment-{work_idx}-p{k}"));
        let raw = format!("[Mẫu #234] Trả lời #{k:02} trong tác phẩm #{work_idx:03}.");
        let body = CommentBody::parse(&raw).unwrap().as_str().to_string();
        out.push(PerfComment {
            comment_id: cid,
            kind: "reply",
            root_id: Some(root_id.clone()),
            reply_to_id: None,
            replied_to_reply: false,
            body,
            created_offset_ms: next_offset_ms,
        });
        next_offset_ms += 1_000;
    }
    out
}

pub fn generate() -> PerfJourneyManifest {
    const FORMS: [&str; 2] = ["Thơ", "Tản văn"];
    const THEMES: [&str; 4] = ["Ký ức", "Gia đình", "Mùa", "Đô thị"];

    let mut works = Vec::with_capacity(WORK_COUNT);
    let mut total_comments = 0usize;
    let mut hasher = Sha256::new();
    hasher.update(b"vink-perf-journey-234-v1");
    hasher.update(JOURNEY_SEED.to_be_bytes());
    hasher.update(corpus::SEED.to_be_bytes());

    for idx in 0..WORK_COUNT {
        let (role, shape, scalars, comment_target, published_offset_secs) = spec_for_index(idx);
        let raw_doc = corpus::corpus(shape, scalars);
        let doc = Document::parse(raw_doc).expect("corpus document is valid");
        let canonical = doc.to_data();
        let raw_title = match role {
            "standard" => format!("[Mẫu #234] Khúc mưa hiên cũ — chuẩn hành trình ({idx:03})"),
            "long_poem_200k" => format!("[Mẫu #234] Trường ca 200k #229 ({idx:03})"),
            "long_vi_200k" => format!("[Mẫu #234] Văn xuôi Việt 200k #229 ({idx:03})"),
            "medium_emoji_30k" => format!("[Mẫu #234] Ký hiệu 30k #229 ({idx:03})"),
            "withdrawal_target" => format!("[Mẫu #234] Tác phẩm kiểm thu hồi thật ({idx:03})"),
            _ => format!("[Mẫu #234] Ghi chép {shape} ({idx:03})"),
        };
        let title = Title::parse(&raw_title).expect("corpus title is valid");
        let content_hash = ContentHash::of(&title, &doc);
        let canonical_json = serde_json::to_vec(&canonical).expect("canonical serialize");
        let work_id = deterministic_uuid(&format!("vink-234-work-{idx}"));
        let revision_id = deterministic_uuid(&format!("vink-234-revision-{idx}"));
        let snapshot_id = deterministic_uuid(&format!("vink-234-snapshot-{idx}"));
        let thread_id = deterministic_uuid(&format!("vink-234-thread-{idx}"));
        let form_label = match idx {
            0 | 1 => "Thơ",
            2..=4 => "Tản văn",
            _ => FORMS[idx % FORMS.len()],
        };
        let theme_label = match idx {
            0 | 3 => "Ký ức",
            1 => "Mùa",
            2 => "Đô thị",
            4 => "Gia đình",
            _ => THEMES[idx % THEMES.len()],
        };
        let comments = build_comments(idx, comment_target);
        total_comments += comments.len();

        hasher.update(work_id.as_bytes());
        hasher.update(revision_id.as_bytes());
        hasher.update(content_hash.as_str().as_bytes());
        hasher.update((canonical_json.len() as u64).to_be_bytes());
        hasher.update(&canonical_json);
        for comment in &comments {
            hasher.update(comment.comment_id.as_bytes());
            hasher.update(comment.body.as_bytes());
        }

        works.push(PerfWork {
            index: idx,
            role,
            work_id,
            revision_id,
            snapshot_id,
            thread_id,
            title: title.as_str().to_string(),
            shape,
            scalars,
            canonical_json_bytes: canonical_json.len(),
            content_hash: content_hash.as_str().to_string(),
            form_label,
            theme_label,
            published_offset_secs,
            has_working_draft: role == "withdrawal_target",
            document: canonical,
            comments,
        });
    }

    assert_eq!(works.len(), WORK_COUNT);
    assert_eq!(total_comments, COMMENT_COUNT);

    let digest_bytes = hasher.finalize();
    let mut corpus_digest = String::with_capacity(64);
    for b in digest_bytes {
        corpus_digest.push_str(&format!("{b:02x}"));
    }

    let roles = PerfRoles {
        standard_work_id: works[0].work_id.clone(),
        standard_work_title: works[0].title.clone(),
        long_poem_work_id: works[1].work_id.clone(),
        long_vi_work_id: works[3].work_id.clone(),
        medium_emoji_work_id: works[4].work_id.clone(),
        withdrawal_work_id: works[2].work_id.clone(),
        empty_discussion_work_id: works[199].work_id.clone(),
    };

    PerfJourneyManifest {
        journey_seed: JOURNEY_SEED,
        document_corpus_seed: corpus::SEED,
        corpus_digest,
        work_count: works.len(),
        comment_count: total_comments,
        roles,
        works,
    }
}
