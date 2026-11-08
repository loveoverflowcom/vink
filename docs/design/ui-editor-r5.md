# Editor — Quiet UI R4 và Surface R5 (#101)

## Phạm vi và nguồn

- Issue [#101](https://github.com/loveoverflowcom/vink/issues/101), develop sau PR #103,
  base `82e86aef70` (SHA đầy đủ trong manifest); WR-01/WR-05, UX-01, INV-08.
- [Concept R5](https://drive.google.com/file/d/1TrdLQwnXmQzsjNevxWLnFlo-ibSFq2Oc/view)
  đã mở; đây là ảnh đề xuất, không phải screenshot hoặc nguồn lấy màu.
- Owner: `pages/desk.rs`, `vink.css`, pattern `EditorToolbar`, shared
  `PlainTooltip`/`SupportingDetails` và registry Material Icons hiện hành.

## LayoutSpec đã chọn

```text
Editor
  Header — Bàn viết / Bản nháp
  Toolbar tonal — trạng thái ngắn + Chi tiết, Xem trước, Tải .txt, Lưu
  Recovery có điều kiện — trạng thái + nhắc tải bản sao + actions luôn thấy + Chi tiết native
  Writing sheet — label/title, label/body CodeMirror, hướng dẫn ngắt dòng tùy chọn
```

CHOICE-01 A: trạng thái và actions cùng toolbar, wrap tự nhiên theo bề rộng/chữ.
Không giảm target hoặc font để ép một hàng. CHOICE-02 A: dùng `SupportingDetails`
native từ API #90; cảnh báo mất chữ và actions nằm ngoài disclosure.

Tiêu đề giữ `FloatingTonal`, native input, label và giới hạn 200 ký tự; title/body
nhận cùng nền của sheet. Toolbar/notice dùng surface/semantic roles hiện có, không
stroke, shadow giả viền, gradient hay divider ở trạng thái nghỉ. Focus vẫn có một
perimeter ở control đang nhận focus; forced-colors giữ dấu hiệu hệ thống.
Toolbar chỉ sticky ở desktop đủ rộng/cao; mở Chi tiết thì trả về document flow.
CodeMirror tiếp tục đo chiều cao sticky để giữ caret rõ.

## Hành vi và sửa lỗi có bằng chứng

Status ngắn: Đã lưu / Chưa lưu / Đang lưu… / Chưa xác nhận lưu. / Chưa lưu được. /
Có bản mới trên máy chủ. Version ACK thật nằm trong Chi tiết. Memo tách trạng thái
khỏi mỗi phím gõ; mutation observer xác nhận gõ/undo trong Unsaved không cập nhật
lặp live region. Toolbar là owner công bố polite duy nhất; notice không đọc trùng.

Unknown, rejection và conflict vẫn khác nhau. Luôn thấy “Tải bản sao trước khi rời
trang.”; unknown/rejection có Thử lại + Tải .txt; conflict giữ lưu đè có chủ ý.
Chi tiết nói đúng nội dung còn trên trang và chưa có khôi phục tự động. Không đổi
save protocol, GET reconciliation, ticket fences, leave guard hoặc dữ liệu.
Hint shortcut chuyển sang mô tả truy cập ẩn và tooltip theo nền tảng; ID mô tả
CodeMirror vẫn tồn tại. Không remount EditorView.

Adoption đã tìm ra lỗi tooltip: sau pointerdown nhận focus, tooltip hiện ở vị trí
tạm trước RAF và nhận pointerup, làm click Lưu không phát submit. Telemetry thấy
`mousedown → button`, `mouseup → tooltip`, không có submit. Bản sửa giữ tooltip
khỏi hit-testing tới khi đã đặt vị trí; ca cũ nay phát click/submit/PUT. Tooltip
có focus được đặt lại vị trí khi trang tự cuộn; Escape không làm nó tự mở lại.
`Button.described_by` nối mô tả vào native button và giữ note của state.

Glyph Download lấy nguyên upstream Material Icons đã pin, có SHA-256/license trong
registry; Rust/Kotlin được generator sinh lại. Không có native editor mới.

## Bằng chứng local

[Manifest, source hashes và geometry](../development/evidence/editor-r5/manifest.json).
Các ảnh dưới đã mở, cùng fixture/title/poem/state/theme; ảnh toàn trang so ở scrollY=0.
Ảnh writing/recovery dùng cùng chính sách cuộn tới đầu vùng, không so absolute scrollY.

| Ca | Before | After |
| --- | --- | --- |
| Clean, 390px, 100%, sáng | [Ảnh](../development/evidence/editor-r5/before-editor-clean-390-100-light.png) | [Ảnh](../development/evidence/editor-r5/after-editor-clean-390-100-light.png) |
| Unknown, 390px, 100%, tối | [Ảnh](../development/evidence/editor-r5/before-editor-failed-390-100-dark.png) | [Ảnh](../development/evidence/editor-r5/after-editor-failed-390-100-dark.png) |
| Writing, 320px, 200%, sáng | [Ảnh](../development/evidence/editor-r5/before-editor-clean-320-200-light-writing.png) | [Ảnh](../development/evidence/editor-r5/after-editor-clean-320-200-light-writing.png) |
| Recovery, 320px, 200%, sáng | [Ảnh](../development/evidence/editor-r5/before-editor-failed-320-200-light-recovery.png) | [Ảnh](../development/evidence/editor-r5/after-editor-failed-320-200-light-recovery.png) |

Ở after 390/100: toolbar cao 120px; title y=324.69 clean/sáng và y=547.88
unknown/tối. Đây là hai state khác nhau, không là số đo mức tiết kiệm. Ở 320/200,
mọi control wrap trong flow và phục hồi vẫn tới được. Không khẳng định số pixel
cải thiện: baseline `contexts.json` bị ENOSPC cắt mất, PNG được giữ nguyên.
Fresh baseline build đã fail ENOSPC sau bước sinh browser bundle; baseline dùng
cached server và bundle trước patch, không được báo là fresh full-build PASS.

Môi trường: macOS, Headless Chrome 156, DPR 1, root font 100/200%, loopback
App/PostgreSQL scratch với LocalFixture. Cwd là checkout Vink. Các lệnh dùng
`CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0`,
`CARGO_BUILD_JOBS=4`; npm cache dưới `/tmp`. Web đặt `VINK_TEST_DATABASE_URL` tới
cluster scratch và `VINK_CHROME` tới Chrome local.

| Kiểm tra | Kết quả |
| --- | --- |
| `cargo xtask verify quick` | PASS: generator, fmt/clippy, unit/contract/doctest và repository checks |
| `node apps/vink-web/e2e/browser-smoke.mjs --base http://127.0.0.1:3101 --chrome … --state /tmp/vink101-qualified/state.json --phase editor-chrome` | PASS |
| `cargo xtask verify web` | PASS |
| `cargo xtask ui-preview leptos verify` | PASS |

Phase editor kiểm 320/390/834/1024/1440 × sáng/tối × chữ100/200 cho
clean/dirty/saving/unknown/conflict, target toolbar ≥48px, no overflow, borderless
sheet, labels/descriptions, một live owner, tooltip/Escape, focus/forced-colors,
native detail bằng chuột/Space/CDP touch ở 320/200, selection/undo/IME mô phỏng,
pending/newer edits/lost ACK, GET trước retry, rejected reconciliation không PUT,
conflict/overwrite/reload, preview và leave/discard/late ACK. Bản sửa giữ nguyên
các kiểm cũ, không thay bằng mock persistence. Các lần FAIL trước sửa được giữ
trong log local, gồm ENOSPC, click bị tooltip nhận mất và oracle theme cũ đo nền body nay trong suốt; oracle đã chuyển sang sheet là owner R5; không đổi chúng thành PASS.

## Giới hạn nghiệm thu

OS IME thật, zoom trình duyệt thật, paste Word/Docs, thiết bị/bàn phím mobile thật,
screen reader, Safari/Firefox và CMP/native: NOT_RUN. CDP/root scaling không chứng
minh các lớp này. Baseline DOM geometry JSON bị mất do hết đĩa; trước/sau thị giác
và after geometry có thật, nhưng không tuyên bố đo mức giảm chiều cao đầy đủ.
Không đọc/chờ GitHub CI và không deploy. Issue chỉ tick theo bằng chứng đã có;
những phần cần các lớp kiểm còn thiếu vẫn để mở.
