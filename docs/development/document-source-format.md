# Registry triển khai — Nguồn văn bản theo định dạng (#255)

Registry này theo dõi hiện trạng code, biên triển khai, quyết định còn chờ signoff và bằng chứng
kiểm chứng của [issue #255](https://github.com/loveoverflowcom/vink/issues/255). Tài liệu sở hữu
kiến trúc nằm tại [document-source-format.md](../architecture/document-source-format.md).

## 1. Biên triển khai hiện tại (`AC-05`)

| Thành phần | Vị trí | Trạng thái | Ghi chú |
| --- | --- | --- | --- |
| Hợp đồng nguồn v2, capability schema, v1 legacy reader & loss summary | `services/vink-app/src/domain/source.rs` | `core` | `SOURCE_SCHEMA_VERSION = 2`, `SourceFormat` (`default`, `markdown`), `DeferredFormat::AsciiDoc`, `LineEnding`, `DocumentSource`, `StoredDocument`, `EffectiveFormat`, `LegacyLossSummary`. Tách riêng `to_nfc_search_projection()` và `to_derived_v1_lossy()` khỏi `source`. |
| Canonical hash v2 & snapshot content v2 | `services/vink-app/src/domain/revision.rs` | `core` | Bổ sung `ContentHash::of_source_v2` (`vink-revision-content-v2`) và `RevisionContentV2`; giữ nguyên `ContentHash::of` (`vink-revision-content-v1`). |
| Reader compatibility adapter phía Web | `apps/vink-web/src/document.rs` | `web` (cô lập, chưa nối UI) | Bổ sung `DocumentSourceDto`, `StoredDocumentDto`, `read_document_envelope`, `format_capabilities`; giữ nguyên `DocumentDto`, `SCHEMA_VERSION = 1`, `text_to_document` và `public_persistence` (`ENABLED = false`). |
| Đề xuất quyết định kiến trúc | `decisions/proposals/VDR-DOCUMENT-SOURCE-FORMAT.r1.toml` | `PENDING` | Chưa có file dưới `decisions/signoffs/`. Không mở writer v2 và không thay đổi `VDR-CONTENT-STORAGE@r1` (`PENDING`). |
| Test tương thích PostgreSQL 17 scratch | `services/vink-app/tests/postgres/document_source_compat.rs` | `postgres` (scratch + read v1) | Kiểm v1 sản xuất bất biến, bẫy `IS TRUE` trước `NULL` trong JSONB `CHECK`, round-trip NFD/ZWJ/CRLF qua PG JSONB. |
| Writer v2, migration bảng thật, route API v2 & UI tích hợp | `persistence/`, `http/`, `apps/vink-web/src/pages/` | `NOT_IMPLEMENTED` (bị chặn bởi gate `AC-05`) | Chỉ triển khai theo kế hoạch 2 pha sau signoff và tích hợp `#262`, `#263`, `#256`, `#264`. |

## 2. Sổ bằng chứng kiểm chứng (`develop@d352dba` + nhánh `codex/editor-format`)

### Lệnh đã chạy và kết quả thực tế

| Biên kiểm chứng | Lệnh thực chạy | Kết quả quan sát |
| --- | --- | --- |
| Domain thuần (`vink-app`) | `PATH="/opt/homebrew/bin:$PATH" ./tools/local-budget run -- cargo test --locked -p vink-app --lib domain::` | 218 passed, 0 failed (trong đó có 8 test `domain::source::tests::*`, test golden vector v2 độc lập bằng Python `0ca4bf098f2b48053839130b399e35d46068e7b55c0199f12496bfa889849eee` và vector v1 không đổi). |
| Web adapter (`vink-web` SSR) | `PATH="/opt/homebrew/bin:$PATH" ./tools/local-budget run -- cargo test --locked -p vink-web --features ssr --lib document` | 5 passed, 0 failed (bao gồm `dual_version_reader_treats_v1_as_legacy_default_and_preserves_v2_source`). |
| PostgreSQL 17 tương thích & JSONB NFD | `DATABASE_URL="postgres://manh.pd1@127.0.0.1:5432/postgres" VINK_TEST_DATABASE_URL="postgres://manh.pd1@127.0.0.1:5432/postgres" PATH="/opt/homebrew/bin:$PATH" ./tools/local-budget run -- cargo test --locked -p vink-app --test postgres document_source_compat -- --test-threads=1` | 2 passed, 0 failed trên PostgreSQL 17.11 (Homebrew). Phát hiện và khắc phục bằng `IS TRUE` trường hợp missing JSONB key trả `NULL` qua `CHECK`. |
| Decision registry | `PATH="/opt/homebrew/bin:$PATH" ./tools/local-budget run -- cargo xtask decision validate` | Passed: `127 revisions; snapshot 8354df2cde868013df8324e1934351d094f93739728e485c4f59a1e1d46462b8; 88 declared references`. `VDR-DOCUMENT-SOURCE-FORMAT@r1` ở trạng thái `PENDING`. |
| Cổng `verify quick` | `PATH="/opt/homebrew/bin:$PATH" ./tools/local-budget run -- cargo xtask verify quick` | Passed mọi bước (`ui tokens`, `empty artwork`, `expressions`, `visual evidence oracles`, `web perf analyzer`, `fmt`, `clippy server/browser/ui-preview`, doctests `compile_fail` + twin, Reader SSR boundary, unit + contract tests, repository checks). |

### Phần `NOT_RUN` và ngoài phạm vi PR

- `verify web` (trình duyệt Playwright + hydration/editor browser smoke), `verify preview`, `ui-preview cmp` và kiểm thử Android/iOS (`NOT_RUN` — PR chưa nối vào UI hay route live).
- `verify kani` và `verify discussion` (`NOT_RUN` riêng lẻ ngoài kiểm biên `services/vink-app/kani` trong `verify quick`).
- Test IME hệ điều hành (macOS/Windows/iOS/Android), tích hợp live `#256`, `#262`, `#263`, `#264` và migration DB trên production (`NOT_RUN` — chờ signoff `VDR-DOCUMENT-SOURCE-FORMAT` và các PR kế tiếp).

## 3. Điều kiện tiên quyết để mở Pha 1 & Pha 2

1. Chủ sản phẩm signoff chính xác revision `VDR-DOCUMENT-SOURCE-FORMAT@r1` qua `VDR-DECISION-SIGNOFF@r1`.
2. Pha 1: thêm migration `IS TRUE`-guarded `CHECK` cho `working_drafts` và `revisions`; nâng toàn bộ consumer DB/HTTP/worker (`drafts.rs`, `publication.rs`, `evaluation.rs`, `moderation.rs`, `appeals.rs`, `http::contract`, `apps/vink-web`) đọc qua `StoredDocument` v1+v2 trong khi giữ writer ở v1.
3. Pha 2: hoàn thành điểm nối `#262` (chọn định dạng), `#263` (title H1), `#264` (Markdown Reader) và kiểm moderation/evaluation trước khi bật writer v2 có cờ.
