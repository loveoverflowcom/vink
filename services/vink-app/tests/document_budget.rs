//! Run without a database: `cargo test -p vink-app --test document_budget`.
#[path = "support/document_budget.rs"]
mod corpus;
#[path = "support/perf_journey.rs"]
mod perf_journey;

use vink_app::domain::document::{Document, DocumentError, MAX_DOCUMENT_CHARS, Title};
use vink_app::domain::revision::ContentHash;

#[test]
fn long_document_corpus_uses_real_validator_serializer_and_v1_hash() {
    let title = Title::parse("[Mẫu #229] Ngân sách văn bản").unwrap();
    for size in [30_000, 100_000, 200_000] {
        for shape in corpus::SHAPES {
            let data = corpus::corpus(shape, size);
            let raw = serde_json::to_vec(&data).unwrap();
            let doc = Document::parse(serde_json::from_slice(&raw).unwrap()).unwrap();
            let scalars: usize = doc.blocks().flatten().map(|s| s.chars().count()).sum();
            assert_eq!(scalars, size, "{shape}/{size}");
            let encoded = serde_json::to_vec(&doc.to_data()).unwrap();
            let decoded = Document::parse(serde_json::from_slice(&encoded).unwrap()).unwrap();
            assert_eq!(doc, decoded);
            let hash = ContentHash::of(&title, &doc);
            assert_eq!(hash, ContentHash::of(&title, &decoded));
            if shape == "vi_nfd" {
                let nfc = Document::parse(corpus::corpus("vi_nfc", size)).unwrap();
                assert_eq!(doc, nfc);
                assert!(raw.len() > encoded.len());
            }
            for at in [0, size / 2, size - 1] {
                let mut changed = doc.to_data();
                corpus::edit(&mut changed, at);
                let changed = Document::parse(changed).unwrap();
                assert_ne!(hash, ContentHash::of(&title, &changed));
            }
        }
    }
}

#[test]
fn long_document_scalar_boundaries_do_not_become_json_or_cache_limits() {
    for size in [
        MAX_DOCUMENT_CHARS - 1,
        MAX_DOCUMENT_CHARS,
        MAX_DOCUMENT_CHARS + 1,
    ] {
        for shape in ["vi_nfd", "emoji", "tiny_blocks"] {
            let raw = corpus::corpus(shape, size);
            let result = Document::parse(raw);
            if size > MAX_DOCUMENT_CHARS {
                assert_eq!(result, Err(DocumentError::TooLong));
            } else {
                assert!(result.is_ok(), "{shape}/{size}");
            }
        }
    }
    let tiny = Document::parse(corpus::corpus("tiny_blocks", MAX_DOCUMENT_CHARS)).unwrap();
    assert!(serde_json::to_vec(&tiny.to_data()).unwrap().len() > 2 * 1024 * 1024);
    let emoji = Document::parse(corpus::corpus("emoji", MAX_DOCUMENT_CHARS)).unwrap();
    assert!(serde_json::to_vec(&emoji.to_data()).unwrap().len() > 512 * 1024);
}

#[test]
fn long_document_oversized_public_record_is_a_cache_miss_without_losing_the_snapshot() {
    use vink_web::public_persistence::{Envelope, MAX_RECORD_BYTES};
    use vink_web::public_reads::{PublicKey, Snapshot};
    let mut publication: vink_web::contract::Publication = serde_json::from_str(include_str!(
        "../contracts/http/v1/fixtures/publication.get.json"
    ))
    .unwrap();
    publication.document = serde_json::from_value(
        serde_json::to_value(corpus::corpus("emoji", MAX_DOCUMENT_CHARS)).unwrap(),
    )
    .unwrap();
    let key = PublicKey::Reader(publication.work_id.clone());
    let snapshot = Snapshot::Publication(publication.clone());
    let envelope =
        Envelope::from_origin(key.clone(), &snapshot, "https://vink.test".into(), 10_000).unwrap();
    let raw = serde_json::to_string(&envelope).unwrap();
    assert!(raw.len() > MAX_RECORD_BYTES);
    assert!(envelope.encode().is_none());
    assert!(Envelope::decode(&raw, "https://vink.test", &key).is_none());
    match envelope.snapshot() {
        Snapshot::Publication(kept) => assert_eq!(kept, publication),
        _ => panic!("cache quota must not change the accepted online snapshot"),
    }
}

#[test]
fn perf_journey_corpus_reuses_229_corpus_with_200_works_and_1000_comments() {
    use std::collections::{BTreeMap, BTreeSet};
    use vink_app::domain::discussion::page::{DEFAULT_PAGE, PREVIEW_REPLIES};

    let first = perf_journey::generate();
    let second = perf_journey::generate();
    assert_eq!(first.corpus_digest, second.corpus_digest);
    assert_eq!(first.work_count, 200);
    assert_eq!(first.works.len(), 200);
    assert_eq!(first.comment_count, 1_000);
    assert_eq!(
        first.works.iter().map(|w| w.comments.len()).sum::<usize>(),
        1_000
    );

    let mut work_ids = BTreeSet::new();
    for work in &first.works {
        assert!(work_ids.insert(work.work_id.clone()));
        let parsed = Document::parse(work.document.clone()).unwrap();
        let title = Title::parse(&work.title).unwrap();
        assert_eq!(ContentHash::of(&title, &parsed).as_str(), work.content_hash);

        let mut roots = BTreeSet::new();
        let mut replies_by_id = BTreeMap::new();
        for comment in &work.comments {
            match comment.kind {
                "root" => {
                    assert!(comment.root_id.is_none());
                    assert!(comment.reply_to_id.is_none());
                    assert!(!comment.replied_to_reply);
                    assert!(roots.insert(comment.comment_id.clone()));
                }
                "reply" => {
                    let root = comment.root_id.as_ref().expect("reply needs root_id");
                    assert!(roots.contains(root));
                    if let Some(parent_reply) = &comment.reply_to_id {
                        assert!(comment.replied_to_reply);
                        assert_eq!(replies_by_id.get(parent_reply), Some(root));
                    }
                    replies_by_id.insert(comment.comment_id.clone(), root.clone());
                }
                other => panic!("unexpected comment kind {other}"),
            }
        }
    }

    let standard = &first.works[0];
    let standard_roots = standard
        .comments
        .iter()
        .filter(|c| c.kind == "root")
        .count();
    assert!(standard_roots > usize::from(DEFAULT_PAGE));
    let max_replies_on_root = standard
        .comments
        .iter()
        .filter(|c| c.root_id.as_deref() == Some(&standard.comments[0].comment_id))
        .count();
    assert!(max_replies_on_root > usize::from(PREVIEW_REPLIES));
    assert_eq!(first.works[1].scalars, MAX_DOCUMENT_CHARS);
    assert_eq!(first.works[3].scalars, MAX_DOCUMENT_CHARS);
}
