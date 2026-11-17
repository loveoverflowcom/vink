# Đối soát tích hợp PR #267–#271 — 17/11/2026

Base `origin/develop@e947a42eaea9cbddd9a89d8f10c43cc37a39e3e7` (merge PR #271), branch
`codex/reconcile-pr267-271`. Phạm vi: delta năm PR
[#267](https://github.com/loveoverflowcom/vink/pull/267) (#259),
[#268](https://github.com/loveoverflowcom/vink/pull/268) (#260),
[#269](https://github.com/loveoverflowcom/vink/pull/269) (#261),
[#270](https://github.com/loveoverflowcom/vink/pull/270) (#262),
[#271](https://github.com/loveoverflowcom/vink/pull/271) (#263), owners/callers trực tiếp và
dependency #258/PR #266, #255, #256, #264. Đây là evidence của một lượt đối soát; checklist
live vẫn ở từng issue, trạng thái triển khai vẫn ở owner registry tương ứng.

## Dependency và quyết định tại base

- #255/#256/#264 còn mở, không có PR cùng outcome. Source vẫn `SCHEMA_VERSION = 1`,
  `DocumentData { schema_version, blocks }`, editor CodeMirror plain text (`editor.ts` ghi rõ
  không diễn giải Markdown), Reader v1. Không bỏ blocker nào của #262/#263.
- `cargo xtask decision query`, snapshot `9dbad85666397d5d9b76d811440fc80ecf12f034328dea0ec7c22114e3596cb3`:
  `D-04@r1` current `DESIGN_PROPOSED` (sha256 `d50ba0e9…6105`, legacy import);
  `VDR-CONTENT-STORAGE` current `null`, head r1 `PENDING` (`4dfde6cc…3f41`);
  `VDR-EDITOR-SAVE-PROJECTION` current `null`, head r1 `PENDING` (`81f31055…66f7`).
  Lượt này không đổi decision, PRD hay proposal.

## Findings

| # | Phân loại | Trigger → tác động | Owner / nguồn | Có từ | Sửa |
| --- | --- | --- | --- | --- | --- |
| F1 | VERIFICATION_GAP | Corpus “legacy v1” dựng document bằng `SCHEMA_VERSION` production; `ContentHash::of` không hash version. Khi #255 đổi version, test vẫn PASS nhưng kiểm version mới. Đã chứng minh: test gốc PASS với `SCHEMA_VERSION = 2` tạm thời. | `application/tests/title_authority.rs` | #271 | Hằng `LEGACY_V1_SCHEMA_VERSION = 1` riêng của test. |
| F2 | VERIFICATION_GAP | Ca 200/201 chỉ dùng `ơ` precomposed nên không phân biệt đếm trước/sau NFC; không test nào phủ tên NFD 200 ký tự (400 scalar). | `legacy-v1.json`, `Title::parse` | #271 (gap domain có trước) | Thêm `title-200-nfc-from-nfd-input`; guard ca `nfd` phải giữ input chưa NFC. |
| F3 | DOC_DRIFT | README corpus nói `expected_projection` “trước validation” nhưng `nfd-emoji` kỳ vọng NFC từ source NFD. | fixtures README, owner title | #271 | Ghi thứ tự extraction → trim/NFC → validation. |
| F4 | DOC_DRIFT | Comment `FirstDraft` còn “Reserved for the Desk consumer” dù Desk đã dùng. | `apps/vink-web/src/ui/empty_state.rs` | #266, cũ sau #267 | Sửa comment. |
| F5 | VERIFIED_REDUNDANCY | `.vk-desk-empty { max-width: none; }` override `.vk-empty` đã bị xóa từ `a2a1fdd` (03/11/2026); không rule nào khác đặt max-width cho wrapper, `none` là giá trị khởi tạo. | `apps/vink-web/style/vink.css` | có trước, owner #267 chạm | Xóa dòng, giữ rule ẩn icon CTA. |
| F6 | DOC_DRIFT | Comment `#community-stream … .vk-ui-panel` nói empty/no-match/unavailable chung panel; từ #258 chỉ `public-unavailable` còn StatePanel. | `vink.css` | #266 | Sửa comment; rule vẫn sống cho Unavailable. |
| F7 | DOC_DRIFT | `states.md` Activity ghi Android/iOS “chưa có host, NOT_IMPLEMENTED”; KDoc `VinkMotionActivity` ghi “Lifecycle STARTED / scene foreground — no native host”. Diagnostic hosts thực map `onResume/onPause` và `applicationDidBecomeActive/WillResignActive` từ `25188eb` (UIS-06). | `shared/states.md`, `VinkFeedback.kt` | có trước, owner #269 chạm | Mô tả đúng mapping, giữ “chưa có native product app”. |
| F8 | DOC_DRIFT | Ghi chú adoption #23 “hiện không có auth” (02/11/2026) đọc như contract hiện hành; từ Auth Web Desk thêm link “Đăng nhập ở mục Tôi” khi `sign_in_open()`, không dùng `Reauthenticate`. | `shared/states.md`, `pages/desk.rs` | có trước | Gắn mốc lịch sử và mô tả hiện trạng. |
| F9 | SOURCE_CONFIRMED (P3) | Byte budget lệch biên: `verify.mjs` nhận `≤ 30 KiB`, Gradle `generateEmptyArtwork` đòi `< 30 KiB`, README ghi `<`. Asset đúng 30.720 bytes qua build Web nhưng làm hỏng build CMP. Chưa có trigger (lớn nhất 20.070). | `cmp/diagnostics/build.gradle.kts`, asset README | #269 | Gradle `<=` theo validator canonical; README `≤30 KiB`. |
| F10 | DOC_DRIFT | Owner asset canonical chỉ liệt kê consumer Web, không biết CMP Gradle đọc manifest/bytes với ID cố định. | `apps/vink-web/assets/empty/README.md` | #269 | Thêm consumer CMP và lệnh phải chạy khi đổi bytes/ID. |
| F11 | Test coupling (P3) | AX oracle #260 chốt `names.length===6` (đúng seed 6 thẻ của `verify web`), geometry cùng phase nhận `>=6`; thêm thẻ seed sẽ làm AX fail dù tên đúng. | `e2e/discovery-chip-geometry.mjs` | #268 | `labels>=6` và multiset AX names = labels của form; vẫn bắt thiếu/trùng/“✓”. |
| F12 | DOC_DRIFT | Link “LayoutSpec, lựa chọn và evidence” chỉ trỏ evidence; LayoutSpec/CHOICE nằm ở `docs/design/discovery-chip-260.md`. | `docs/development/discovery-ui.md` | #268 | Tách hai link. |
| F13 | DOC_DRIFT (tích hợp) | LayoutSpec #262 (merge trước #271) vẫn ghi #263 chưa có gì, không dẫn tới owner title mới. | `docs/design/editor-format-picker-262.md` | #270↔#271 | Dẫn tới `editor-title-authority.md`, giữ “chưa có parser/save”. |
| F14 | INTENTIONAL_DIFFERENCE chưa ghi | Nhãn action mẫu CMP khác Web ở Explore/empty và Desk/no-results; câu trạng thái trùng. | `docs/design/empty-artwork-cmp-261.md` | #269 | Ghi rõ là copy mẫu; không đổi code/captures. |
| F15 | VERIFICATION_GAP | #270 thêm `appearance`/`reserve` vào `checkSelectGeometry` dùng chung, nhưng runner App (`selectChevron`, Desk `#draft-sort`) không chạy lại trong #270. | `select-geometry.mjs`, `browser-smoke.mjs` | #270 | Không đổi code; chạy lại trong `verify web --scenario main`. |

Không phải finding: `EmptyArtwork` Rust/Kotlin là hai adapter của cùng manifest (CHOICE-01 B,
khác platform/trust boundary); `SelectVariant::Inset16` chưa có consumer App là phần chuẩn bị có
gallery + regression; `markdown-proposed.json` là corpus `DESIGN_PROPOSED` có resume path, không
phải code thừa; first-use Desk đòi head không continuation là semantics đã chốt ở #259.

## Kiểm chứng

Tự chạy trên macOS 26.5.2 arm64 (16 GiB), rustc 1.99.1, Node 26.12.0, JDK 17.0.19,
Chrome 157.0.8103.79, PostgreSQL 17.11 scratch riêng trên 127.0.0.1. Source: implementation
`532e028` (tree `a4e4da3a`), diff so với base sha256 `d00695ab…cb807`; `quick`, CMP và Web
chạy trên đúng nội dung đó. Trích log, lệnh và giới hạn: [checks.txt](checks.txt).

| Boundary | Lệnh | Kết quả |
| --- | --- | --- |
| unit (application, recording store) | `cargo test --locked -p vink-app --lib title_authority` qua `tools/local-budget` | Base `e947a42` PASS; sau sửa PASS, 11 ca. |
| Độ nhạy | Tạm đổi `Title::parse` đếm trước NFC; tạm đặt `SCHEMA_VERSION = 2` | Cả hai FAIL đúng ca (`title-200-nfc-from-nfd-input: TooLong`; `UnsupportedSchemaVersion(1)`). Đối chứng: test/fixture gốc #271 **PASS** khi `SCHEMA_VERSION = 2` — gap F1 có thật. Source production đã khôi phục. |
| quick | `./tools/local-budget run -- cargo xtask verify quick` | PASS toàn lane: tokens, empty artwork, fmt, clippy SSR/hydrate/CSR, doctests, Reader SSR, 335 App tests, repository checks (decisions, links…). |
| cmp (Desktop/common) | `cargo xtask ui-preview cmp verify` (Gradle ngoài launcher, `UNSUPPORTED` với ngân sách) | PASS; `generateEmptyArtwork` chạy lại từ đầu; desktopTest XML 105 tests, 0 fail/skip (6 EmptyArtwork). |
| web (Chrome + PostgreSQL) | `cargo xtask verify web --scenario main --scenario discovery` | PASS; `main` 5.818 assertions gồm `deskEmptyState` và `selectChevron` với oracle #270 (`appearance: none`, reserve 38px); `discovery-corpus` 181 assertions gồm geometry/AX #260 (6 tên = 6 nhãn, cả forced-colors). SIGTERM graceful. |
| Asset/test syntax | `node …/verify.mjs`, `node --test …/verify.test.mjs`, `node --check`, `git diff --check` | PASS. |

`NOT_RUN`: `verify postgres` (không đổi SQL/CAS/persistence), `verify discussion`/Quint
(không đổi transition), Android/iOS native, OS IME, screen reader có tên, browser zoom, chụp
before/after mới (thay đổi UI duy nhất là xóa một khai báo CSS có giá trị bằng giá trị khởi
tạo; desk-empty geometry/contrast/fallback của `main` vẫn PASS). Kết quả trên không thay native
qualification của #261 (AC-04/06/07) hay AC tích hợp của #262/#263.

## Còn phụ thuộc

- #262/#263 AC-01..06 cần #255 (source/format/hash, có thể cần signoff schema/hash),
  #256 (provider) và #264 (Reader). Corpus Markdown vẫn `DESIGN_PROPOSED`.
- #261 AC-04/06/07: Android API24/28+ và iOS runtime chưa nghiệm thu. Máy của lượt này có
  Xcode 26.6 và iOS Simulator; lượt này không chạy native vì không đổi decoder/host.
