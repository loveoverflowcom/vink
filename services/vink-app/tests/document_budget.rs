//! Run without a database: `cargo test -p vink-app --test document_budget`.
#[path = "support/document_budget.rs"]
mod corpus;

use unicode_normalization::UnicodeNormalization;
use vink_app::domain::document::{Document, DocumentError, MAX_DOCUMENT_CHARS, Title};
use vink_app::domain::revision::ContentHash;

#[test]
fn draft_http_budget_covers_compact_client_serialization_and_unicode_expansion() {
    use vink_app::domain::document::{BlockData, DocumentData, MAX_TITLE_CHARS};
    use vink_app::http::contract::{MAX_SAVE_DRAFT_BODY_BYTES, SaveDraftRequest};
    // Pin the canonical decomposition bound to the dependency's actual Unicode tables.
    let maximum = (0..=0x10ffff)
        .filter_map(char::from_u32)
        .map(|c| c.to_string().nfd().count())
        .max()
        .unwrap();
    assert_eq!(maximum, 4);
    // Each raw scalar expands to at least one NFD scalar; a canonical NFC scalar to <=4.
    // serde_json emits <=6 UTF-8 bytes/scalar (including control escapes); no ASCII-only
    // re-escaping, insignificant whitespace or arbitrary trimmed title padding is promised.
    let envelope_allowance = 128;
    let upper = (15 + 4 * 6) * MAX_DOCUMENT_CHARS + 4 * 6 * MAX_TITLE_CHARS + envelope_allowance;
    assert_eq!(upper, 7_804_928);
    assert!(upper <= MAX_SAVE_DRAFT_BODY_BYTES);
    for character in [
        "x".into(),
        "\u{1f82}".nfd().collect::<String>(),
        "🧑".into(),
        "\"".into(),
        "\\".into(),
    ] {
        let raw = DocumentData {
            schema_version: 1,
            blocks: vec![
                BlockData {
                    lines: vec![character.clone()]
                };
                MAX_DOCUMENT_CHARS
            ],
        };
        let title = character.repeat(MAX_TITLE_CHARS);
        let request = vink_web::contract::SaveDraftRequest {
            expected_version: i64::MAX,
            title: title.clone(),
            document: serde_json::from_value(serde_json::to_value(&raw).unwrap()).unwrap(),
        };
        let encoded = serde_json::to_vec(&request).unwrap();
        let provider: SaveDraftRequest = serde_json::from_slice(&encoded).unwrap();
        assert_eq!(provider.expected_version, i64::MAX);
        assert_eq!(provider.title, title);
        assert_eq!(provider.document, raw);
        assert!(encoded.len() <= upper);
        assert!(Document::parse(raw).is_ok());
        assert!(Title::parse(&title).is_ok());
    }
}

#[test]
fn long_document_corpus_uses_real_validator_serializer_and_v1_hash() {
    // #279 shares the hex encoder with hash v2. Pin v1 to the measured #249 corpus,
    // rather than merely comparing two hashes produced by today's implementation.
    let historical: serde_json::Value = serde_json::from_str(include_str!(
        "../../../docs/development/evidence/long-document-229/followup/postgres.json"
    ))
    .unwrap();
    assert_eq!(
        historical["source_sha"],
        "4f2883630ff27f36c7549e036e6b554166a86692"
    );
    let cases = historical["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 21);
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
            let name = format!("{shape}-{size}");
            let baseline = cases.iter().find(|case| case["case"] == name).unwrap();
            let mut expected = baseline["pure"].as_object().unwrap().clone();
            expected.remove("samples_ms");
            let measured_title = Title::parse(&format!("[Mẫu #229] {name}")).unwrap();
            let wire = serde_json::to_vec(&serde_json::json!({
                "expected_version": 1, "title": measured_title.as_str(), "document": &data
            }))
            .unwrap();
            let actual = serde_json::json!({
                "blocks": doc.blocks().len(),
                "lines": doc.blocks().map(|lines| lines.len()).sum::<usize>(),
                "raw_line_scalars": data.blocks.iter().flat_map(|b| &b.lines).map(|s| s.chars().count()).sum::<usize>(),
                "nfc_line_scalars": scalars,
                "raw_line_utf8_bytes": data.blocks.iter().flat_map(|b| &b.lines).map(String::len).sum::<usize>(),
                "nfc_line_utf8_bytes": doc.blocks().flatten().map(|line| line.len()).sum::<usize>(),
                "raw_document_json_bytes": raw.len(),
                "canonical_document_json_bytes": encoded.len(),
                "save_request_bytes": wire.len(),
                "content_hash_v1": ContentHash::of(&measured_title, &doc).as_str(),
            });
            assert_eq!(actual, serde_json::Value::Object(expected), "{name}");
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
