# Bằng chứng — document canonical (#255) và title theo định dạng (#263)

Kết quả tích hợp #286 và signoff r2 nằm ở [review ngày 18/11](integration-286/README.md).

**Phạm vi lịch sử:** các kết quả dưới đây do tác giả báo trước rebase, trên base
`develop@607ef83` (17/11/2026), nhánh `codex/editor-canonical-replacement`, worktree riêng.
Không dùng bảng này làm bằng chứng trên head `39641d5` (sau rebase lên `29185dd`) hay trên
head tích hợp `develop@9640bde`. Xem [review độc lập trên Mac](mac-review/README.md) cho SHA,
raw logs và kết quả sau reconcile develop.

macOS arm64, Rust 1.99.1, PostgreSQL 17.11 (Homebrew, cluster scratch `127.0.0.1:55432`, reset
được), Chrome cục bộ. Cargo nặng chạy qua `./tools/local-budget run --` với `CARGO_INCREMENTAL=0`;
lane trình duyệt chạy trực tiếp (launcher không hỗ trợ). Kiến trúc:
[document-source-format.md](../../../architecture/document-source-format.md).

## Bốn chiều

- Sản phẩm: `PRODUCT_CONFIRMED` — thay hoàn toàn (17/11/2026), Mặc định tên riêng / Markdown H1
  (16/11/2026); AsciiDoc `DEFERRED`.
- Kiến trúc: `VDR-DOCUMENT-SOURCE-FORMAT@r2` `PENDING` (sha256
  `478a09f823816984d5f92228046c9cf30ab23b78ffae70febf272dbc49643738`), không có signoff.
- Triển khai: `persistence-api` + `web` cho Mặc định; Markdown `persistence-api` riêng tư.
- Kiểm chứng: bảng dưới; cutover trên DB có revision lịch sử **bị chặn có chủ ý**.

## Kết quả

| Lệnh | Boundary | Kết quả |
| --- | --- | --- |
| `cargo xtask decision validate` | registry | PASS — 128 revisions, snapshot `e6e8f231…c4c2ca`, 88 references |
| `cargo xtask verify quick` | fmt, clippy SSR/hydrate/CSR, doctests compile_fail + twin, Reader SSR, unit/contract, repository checks | PASS — 13/13 bước; App lib 342, Web lib 233, xtask 83 (+1 ignored sẵn có) |
| `cargo test -p vink-app --test postgres` | PostgreSQL scratch | PASS — 211 passed, 0 failed, 6 ignored (probe opt-in) |
| `cargo xtask verify postgres` | migration lifecycle, provisioned roles, binaries, Identity | PASS — mọi bước, gồm “migrate: runtime credential refused…; migrator applies and reruns” |
| `cargo xtask verify web --scenario main` (`VINK_CHROME`, chạy trực tiếp) | browser + hydration + editor + API thật | PASS — 5 867 assertion `ok`, 0 FAIL; gồm `desk refuses drafts it cannot save back unchanged` |
| `cargo xtask verify kani` | Kani (manifest có `pulldown-cmark`) | PASS — 3 + 33 harness, 0 failure |

Test PostgreSQL mới (`tests/postgres/canonical_document.rs`, 6 test):

- `markdown_title_is_materialized_in_the_same_save_and_cas` — source CRLF/NFD/tab/dòng trống
  nguyên văn; title H1 NFC trong cùng dòng/version; lost-ACK retry → `409`; xóa H1 đầu → H1 sau;
  không H1 → rỗng; title client và H1 quá 200 bị từ chối, không ghi.
- `snapshot_title_comes_from_the_captured_source_and_markdown_stays_private` — sửa H1 sau capture
  không đổi snapshot; hash snapshot = `ContentHash::of`; `request_public` →
  `FormatNotPublishable`, không receipt, không public read.
- `database_accepts_only_the_canonical_shape` — 10 shape lỗi (gồm shape cũ, `format: null`,
  `asciidoc`, khóa lạ) và source > 2 MiB → `23514`; title Markdown lệch → `Internal`.
- `cutover_converts_retired_drafts_exactly_once` — DB trước cutover có nháp cũ: chuyển tất định,
  giữ title/version/updated_at, nháp rỗng → `""`, chạy lại no-op, backout chính xác rồi forward
  lại cho cùng kết quả.
- `cutover_refuses_while_immutable_revisions_exist` — revision cũ (hash v1 tính bằng SQL):
  migration lỗi “cutover blocked”, không ghi lịch sử migration, nháp không đổi; hash canonical của
  cùng chữ ≠ hash lưu; `UPDATE revisions` bị trigger bất biến chặn.
- `backout_refuses_when_it_would_lose_text_or_history` — backout từ chối khi có dòng trống thừa,
  dòng chỉ khoảng trắng, CR, NFD, dòng trống đầu, Markdown, hoặc khi có revision.

Các test upgrade/backout của migration cũ (abuse, outbox, post-review, publication, taxonomy, discussion, governance) chạy trên schema ngay trước cutover với
`tests/support/pre_cutover_seed.sql` (dữ liệu lịch sử đúng shape/hash cũ) và
`upgrade_before_cutover`, đồng thời kiểm cutover từ chối DB đó. Phần đọc runtime trên hàng trước
cutover được chuyển sang SQL vì runtime không còn reader legacy.

Lỗi thật phát hiện khi kiểm: (1) migration bản đầu dùng `CREATE TEMP TABLE`, role migrator đã
provision không có quyền TEMP → `verify postgres` FAIL; đã bỏ bảng tạm ở cả migration và backout.
(2) CodeMirror chuẩn hóa CR → LF khi nạp; Bàn viết nay từ chối mở nháp có CR (và Markdown) thay vì
lưu đè âm thầm (`desk_editable`, browser check `desk refuses drafts it cannot save back unchanged`).

## NOT_RUN

- Migration trên DB thật/production và mọi DB có revision trước cutover: **bị chặn** chờ kế hoạch
  lịch sử được ủy quyền (P1/P2/P3 trong tài liệu kiến trúc).
- Differential parser Rust/TS: `NOT_APPLICABLE` — chưa có parser TS (#256).
- `verify discussion`, `verify preview`, CMP/native, IME OS thật, screen reader,
  benchmark #229 opt-in (`long_document_budget`, `content_storage_probe`) — không chạy trong lượt này.
