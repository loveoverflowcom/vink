# #263 — corpus tiêu đề và hồi quy v1 độc lập

Base `develop@9e91534f302b08738492f42364010adc299c8678`, ngày 17/11/2026;
checkout riêng `codex/editor-title` tại
`/Users/manhblue/Documents/Codex/2026-11-17/task-7/vink`, Darwin 24.6.0 arm64,
Rust 1.99.1, Node 22.15.0.
[Owner/spec](../../../architecture/editor-title-authority.md),
[corpus](../../../../services/vink-app/tests/fixtures/title-authority/README.md).

Đã đọc toàn issue #263 (0 comment), #255 (0 comment), tình trạng #256/#264,
PR mở và PR liên quan; không có PR mở hoặc worker build/test #263 trùng tại lúc kiểm.
Các checkout/tiến trình PostgreSQL và Chrome khác được giữ nguyên. Clone riêng từ
task-6 rồi fetch GitHub `develop`; không đổi checkout người dùng. Ổ đĩa chỉ còn
khoảng 3 GiB nên dùng APFS copy-on-write `cp -cR` cho target của task-6 vào target
checkout này; không hardlink hoặc ghi vào target gốc.

## Ranh giới

- Product: Default tên riêng, Markdown H1 đầu tiên, no-H1 placeholder và AsciiDoc hoãn
  là `PRODUCT_CONFIRMED` theo issue/lời chủ sản phẩm. PRD đã dẫn đúng owner.
- Architecture: `VDR-CONTENT-STORAGE@r1` vẫn `PENDING`; CHOICE-01/02 của #263 và chi tiết
  flatten corpus là đề xuất, không chọn hoặc signoff schema/hash thay owner #255.
- Implementation: thêm 10 ca regression v1 qua application/recording-store/revision;
  30 ca Markdown chuẩn bị, **không parser, format-aware persistence hay title UI**.
- Evidence: regression không là PG/CAS, browser, extraction hoặc differential PASS.
  Không thay runtime nên không thêm model Quint/state machine.

## Kiểm local

Mọi Cargo run dùng
`CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0`,
qua `./tools/local-budget run --` (tối đa 2 compiler jobs/2 test threads).
Log gốc của lượt này nằm ở `/tmp/vink-263-{focused,negative,quick}.log` trên máy;
kết quả dưới đây phân biệt positive run và negative control có chủ ý.

| Check | Kết quả / boundary |
| --- | --- |
| `cargo test --locked -p vink-app --lib legacy_v1_title_authority_corpus` | PASS — 1 test, 10 ca, 334 filtered out; 91.77s gồm build. Chỉ application recording store + pure revision/hash. |
| Negative control suy tên từ body H1 | PASS về độ nhạy — cùng test trả FAIL/exit 101 như dự kiến: `explicit-over-atx`, actual `# Tên trong thân`, expected `Tên riêng`. Mutation tạm thay `Title::parse(&command.title)` bằng parse dòng đầu body; đã phục hồi byte-for-byte `application/mod.rs` trước lượt cuối. |
| `cargo xtask verify quick` | PASS — 217.10s; fmt, Clippy SSR/hydrate/CSR, 55 literary doctests, isolated Reader SSR, unit/contract (App 335 passed gồm corpus), repository checks. 1 ignored có sẵn ở xtask; không PG/browser runtime. |
| `target/debug/xtask decision query D-02-CONTENT` và `… VDR-CONTENT-STORAGE` | PASS — CLI hiện tại đọc `D-02-CONTENT@r1` DESIGN_PROPOSED; storage `r1` PENDING/current null. Snapshot `9dbad85666397d5d9b76d811440fc80ecf12f034328dea0ec7c22114e3596cb3`; storage payload `4dfde6cc9fb3bae44badbafc05d19ad7ec5edfd97ccb0336badc0ded6f533f41`. Không ghi registry/signoff. |
| Corpus JSON và diff/link review | PASS — 10/30 ca JSON, IDs duy nhất, outcome v1 loại trừ nhau, output đề xuất NFC/single-line và phân loại 200/201; không là extraction. Link review đầu FAIL vì thừa một `..` ở evidence→corpus, đã sửa và kiểm lại PASS; `git diff --check` PASS. |
| Markdown extraction / TS-Rust differential | NOT_RUN — chưa có contract/parser chung #255/#256/#264. |
| PG source/format/title CAS + tags/lost ACK/snapshot | NOT_RUN — chưa tích hợp format-aware save; không claim lại PASS lịch sử. |
| Web/Reader/IME/responsive/benchmark | NOT_RUN — không có consumer Markdown trong lát này. |

[Tóm tắt output và SHA-256 inputs thực kiểm](checks.txt). Sau quick chỉ hoàn thiện
báo cáo này; không thay behavior/test/fixture. Self-review theo `vink-code-review`
đã đọc diff, owner save/validation/hash/search/publication và issue prerequisites:
không còn finding actionable trong scope độc lập. Không có production path, dependency,
migration, wire payload, writable authority hoặc model mới. Corpus Markdown có nhãn
proposal rõ và mọi AC tích hợp còn unchecked. Đây là self-review, không phải review
độc lập của một agent/người khác.

## Acceptance còn thiếu

| AC | Phần độc lập | Kết luận whole-AC |
| --- | --- | --- |
| AC-01 | Bảo vệ tên riêng/no-sniff v1; mô tả dispatch/authority. | NOT_RUN phần Markdown/runtime, giữ unchecked. |
| AC-02 | 30 input/output đề xuất và 10 v1 executable cases. | NOT_RUN extraction, source roundtrip và differential, giữ unchecked. |
| AC-03 | Mapping save/standing/CAS/tags và resume tests. | NOT_RUN PG/runtime format-aware, giữ unchecked. |
| AC-04 | Mapping search/list/Reader/snapshot; fixed v1 hash guard. | NOT_RUN consumers theo format và PG snapshot, giữ unchecked. |
| AC-05 | Guard tên rỗng với body H1 phải `MissingTitle`, lỗi tên không ghi store. | NOT_RUN Markdown buffer/UI, giữ unchecked. |
| AC-06 | Reconcile PRD/owner/feature map, tách product/architecture/implementation/evidence. | Partial; pure/PG/Web boundary toàn issue chưa hoàn tất, giữ unchecked. |

Tiếp tục theo [điều kiện resume](../../../architecture/editor-title-authority.md#điều-kiện-tiếp-tục).
Issue #263 giữ mở. Không kiểm/chờ CI và không deploy VPS.
