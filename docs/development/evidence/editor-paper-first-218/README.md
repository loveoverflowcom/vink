# Editor Paper First #218 — kiểm chứng local

Base `develop@a78c9ff5138a0df26e9febe4fdf6146f5915d28a`; source kiểm chứng
`c5e54db` (commit sau chỉ cập nhật evidence). macOS arm64, Rust 1.99.1,
Node 22.15.0, Chrome headless 157/CDP, PostgreSQL 17.11 cluster scratch riêng port 55518.
`CARGO_BUILD_JOBS=2 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0`;
`VINK_CHROME=/Applications/Google Chrome.app/Contents/MacOS/Google Chrome`,
`npm_config_cache=/tmp/vink-218-npm`, PostgreSQL tools trong PATH.

[LayoutSpec và owner map](../../../design/ui-editor-paper-first-218.md).
Sản phẩm WR/UX/INV giữ nguyên; `DESIGN_PROPOSED`, mức `web`;
`NO_APPROVED_REFERENCE`. Không signoff registry, autosave/local recovery hoặc publication rule mới.
Review ảnh là **self-review**, không independent hoặc phê duyệt thẩm mỹ của chủ sản phẩm.

## Thay đổi và lựa chọn

- Bỏ container/sticky của tools; canvas low, giấy surface; không font/measure/gutter mới.
- CHOICE-01 B: native Thêm (`details/summary`), Enter/Space/Tab, Escape không đóng native disclosure.
  Không ARIA menu/focus trap; preview trực tiếp đóng tools rồi mở Dialog, dismiss về trigger sống.
- CHOICE-02 A: flex wrap, status giữ basis 7rem cũ; không giảm font/target để ép một hàng.
- CHOICE-03 B: scoped surface swap, shared pattern opt-in, default consumer không đổi.
- CHOICE-04 B: summary Thêm chính là `#editor-save-details > summary` luôn hiển thị;
  `sync_save_slot_focus` và toàn bộ `project_save_slot`/`DraftSession`/CAS/LeaveGuard giữ nguyên.
- Export thường ở Thêm; recovery/export lỗi/xung đột giữ ngoài disclosure; preview và export đọc live buffer.

## Kiểm tra

| Lệnh | Kết quả / boundary |
| --- | --- |
| `cargo xtask verify web --scenario main` | **PASS** source `c5e54db`; PostgreSQL scratch thật + Chrome, SSR/privacy/hydration/editor/save/reload/conflict/leave/preview/tags và public storage trong main |
| `cargo test --locked -p vink-patterns --test editor_tools` | **PASS 2/2**; SSR anatomy, sole live owner, utility type, default compatibility; được chạy lại trong Quick |
| `node --check` cho ba file E2E đã sửa/thêm | **PASS**, syntax |
| `cargo xtask verify quick` | **PASS**, lượt cuối source `c5e54db`; fmt/clippy SSR+hydrate+preview, doctests, unit/contracts và repository checks |
| `cargo xtask ui-preview leptos verify` | **PASS**, lượt cuối source `c5e54db`, 862 captures; token/build/media-hydration/Chrome assertions |

Raw App runs: baseline `target/verify-web/1794822500161519000-32866`,
final `target/verify-web/1794824153546656000-60393` (repo root).
Log và hash cuối: [verification.json](verification.json); [trích đoạn main](main-excerpt.txt); source hashes:
[source-inputs.json](source-inputs.json). Preview là sample DTO-free, không là App/CodeMirror parity.

### Coverage có ý nghĩa

- App preference matrix: 32 cấu hình clean/dirty × 320/390/834/1440 × light/dark ×
  **chọn cài đặt app text100/200 thật**, 64 closed/open-tools captures. Chờ fonts.ready,
  ghi `data-text`, font computed, viewport/clientWidth/DPR/rail/scroll; không thay bằng browser zoom.
- Matrix editor cũ dùng direct root font-size riêng, thêm 1024 và pending + newer edits,
  failed/uncertain/rejected/conflict. 40 cặp clean/dirty cùng fixture/theme/scale/scroll trong
  [paired-geometry.json](paired-geometry.json): BodyEditor width không đổi (sai số <1 px),
  title top giảm ≥25.59 px. Ví dụ 1440/100 sáng giảm 25.59 px; 390/200 tối giảm 36.81 px;
  dirty 320/200 sáng giảm 28.81 px. Không dùng chiều cao doc ước lượng của CM virtualization làm invariant.
- Save focus: closed/open tools, keyboard ACK, pointer ACK ở 320×420/text200,
  body shortcut, lỗi/recovery, pending có sửa mới và response tới sau khi editor đã disposed.
  Negative control giấu trigger trong outer details đóng bị oracle từ chối.
  Ảnh pointer ACK chứng minh đích hiển thị; focus ring keyboard được kiểm riêng.
- Clean/pending không PUT thừa; partial chữ/thẻ, lost ACK/reconciliation, refused/tag conflict,
  hai bước discard, cancel, Back/Forward và late result giữ các guard #217/#208.
- Live raw NFC/NFD, newlines/indent, title native/full-title, EditorView identity, selection,
  undo/redo và CDP composition giữ đúng. Export sau sửa khi tools còn mở lấy buffer mới;
  preview đóng tools và dismiss về Preview đang sống. Utilities không save/publish.
- Loading, draft GET503 và no-JS đều giữ back/H1; native disclosure/keyboard/touch-equivalent,
  accessible names/roles qua Chrome AX và forced colors được kiểm. Không screen-reader claim.

[Contrast thực](contrast.json): clean readout trên canvas **6.28:1 sáng / 10.00:1 tối**;
Thêm **6.33 / 9.85**; title trên paper **15.81 / 15.33**. Bộ đo các state ≥4.5:1.
Export Text dùng cùng primary/canvas pair của Thêm theo CSS; không có phép đo riêng khi mở.

### Ảnh đã mở — không phải golden

[Metadata, hash PNG và inspection](captures.json) ghi đúng build/scale/viewport của từng ảnh.
Tất cả 12 PNG đính kèm đã mở; ảnh mới đều là build của `c5e54db`. Before là build mới từ
`a78c9ff`, không dùng capture lịch sử BUILD_SHA_UNVERIFIED của issue.

| Cảnh | Trước | Sau |
| --- | --- | --- |
| Clean desktop 1440/100 sáng | [PNG](before-editor-clean-1440-100-light.png) | [PNG](after-editor-clean-1440-100-light.png) |
| Clean 390/200 tối | [PNG](before-editor-clean-390-200-dark.png) | [PNG](after-editor-clean-390-200-dark.png) |
| Dirty 320/200 sáng | [PNG](before-editor-dirty-320-200-light.png) | [PNG](after-editor-dirty-320-200-light.png) |
| Recovery 320/200 sáng | [PNG](before-editor-failed-320-200-light-recovery.png) | [PNG](after-editor-failed-320-200-light-recovery.png) |

Thêm: [native tools 320/200](after-paper-dirty-320-200-light-tools.png),
[ACK 320×420/200](after-paper-ack-focus-320x420-200-dark.png),
[lỗi tải](after-paper-load-error.png), [no-JS](after-paper-no-js.png).
100% là mục tiêu bố cục; 200% vẫn wrap nhiều hàng và cuộn dọc theo nội dung.

Hai lượt self-review ảnh app, một vòng sửa: bỏ flex-basis 10rem mới (clean834/text200
xuống hàng thừa), trở lại 7rem cũ; export dùng `Button.leading` chuẩn. Render cuối không còn
finding này; không thêm vòng sửa hoặc coi hết ngân sách là Visual PASS.

### Kết quả năm chiều (coverage Chrome local)

| Chiều | Kết quả | Evidence / giới hạn |
| --- | --- | --- |
| Functional | PASS | Main thật và SSR unit; không đổi protocol/domain |
| Geometry | PASS | 40 paired records, matrix targets/reflow; preserve title/body widths |
| Visual | PASS trong 12 ảnh đã mở | Quiet tools, paper/canvas light/dark, recovery còn rõ; self-review, NO_APPROVED_REFERENCE |
| Accessibility/Responsive | PASS trong phạm vi Chrome | App text200, narrow/short, AX names/roles, contrast, keyboard/forced-colors; screen reader/device thật NOT_RUN |
| Interaction | PASS trong phạm vi CDP | Native tools, preview/export, ACK/dispose, title/CM selection/history và leave; OS IME NOT_RUN |

### Đối chiếu AC — không tick/đóng issue thay chủ sản phẩm

| AC | Source / evidence |
| --- | --- |
| AC-01, AC-08 | Opt-in pattern + scoped CSS; 40 paired records, light/dark và 12 PNG |
| AC-02, AC-05 | `editorPaperFirst` + `editor_tools`; named native tools, no mutation, keyboard/touch/layer-return |
| AC-03 | Main failure/conflict/refused/tag-conflict + export trực tiếp; authority giữ nguyên |
| AC-04 | Actual app text matrix, direct matrix riêng, contrast, targets ≥48 px |
| AC-06 | `editorWriting` + editorChrome + main, live raw/identity/history/caret/export/leave; OS IME thật NOT_RUN |
| AC-07 | Logs/source hashes, paired PNG đã mở, bảng năm chiều, docs #217/#208; không claim full Web |
| AC-09 | Visible closed/open/narrow focus, hidden-target negative, late ACK/dispose; no focus-steal |
| AC-10 | Native title/WorkInfo/Publication giữ consumer, loading/error/no-JS, actual settings/font/navigation |

## Những lỗi lượt kiểm đầu đã xử lý

- Setup đầu gọi nhầm CLI `seed` thay `dev-seed`, App chưa start nên browser timeout;
  dùng đúng CLI đã xác minh rồi chạy thành công. Không sửa runtime để vượt kiểm.
- Focused test đầu giả định 100% luôn có `data-text=100`; source thật bỏ attribute ở 100%.
  Sửa assertion về contract thật (`data-text || 100`); actual app matrix và Main cuối PASS.
  Logs ban đầu giữ ở `/tmp/vink-218-focused.log`, `/tmp/vink-218-focused-r1.log`.
- Quick/preview đầu PASS được giữ làm lịch sử; kết quả cuối chỉ dùng source cuối.

## Bảo toàn môi trường

Checkout riêng trong task-2; không reset/stash checkout khác. Trước build đọc process list:
không build khác đang chạy; giữ browser #204 và PostgreSQL cũ 55435/55499. Chỉ dọn
`target/debug` và `target/wasm32-unknown-unknown` không dùng của checkout cũ ngày 10/11 và 09/11;
giữ `target/site`, mọi source/evidence/DB. Cargo cache từ checkout review được clone APFS vào
checkout mới, sau đó mọi artifact App/bundle được build lại theo source hiện hành.

Worktree `/Users/manhblue/Projects/loveoverflow/vink-204`: status và SHA-256 của **30** file
sửa/untracked trùng trước/sau. Không sửa #204. Cluster #218 port55518 và App3128 do lượt này
start đã dừng; data/log giữ lại. Không merge/deploy hoặc sửa checklist issue.


## Ranh giới chưa chạy

OS IME Telex/VNI thật, VoiceOver/screen reader thật, mobile device thật,
Safari/Firefox và browser zoom: NOT_RUN. Chrome CDP composition/viewport và cài đặt app
text200 không thay các boundary đó. Full Web ngoài scenario main, PostgreSQL integration
lane, worker, native/CMP và deployment: NOT_RUN; diff không đổi các owner đó.
GitHub CI: NOT_REQUIRED, không đọc/poll/chờ/rerun Actions/status/log.
