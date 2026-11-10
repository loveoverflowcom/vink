# Editor desktop R1.4 — phần chênh sau R5 (#101)

Base `develop@a90a54e3dc43131d755c53770190a4082c3ae6a9` sau PR #137;
branch `codex/editor-r14-alignment`. Đã đọc issue #101 và comments (không có)
ngày 09/11/2026 UTC. Không có PR editor đang mở. [R5](ui-editor-r5.md) đã
merge qua #104, nên tiếp tục code hiện có.

[LayoutSpec R1.4](https://drive.google.com/file/d/1-SBgur-9EmDVKQ1CKLm7pyIMPJr_jDF7/view)
và [ảnh Editor](https://drive.google.com/file/d/1MIOE10ZYJLxbH5c7Npn4Ai6XhtpCcbMl/view)
đã đọc/mở. Đây là prototype, không phải app hoặc bằng chứng save server. Màu,
measure và controls lấy từ tokens/source hiện hành, không đo màu hoặc chép
textarea/sessionStorage/dropdown của prototype.

## Phần đã có và delta đã chọn

R5 đã có rail 96px mặc định, một cột giữa, toolbar tonal 64px ở 1440/chữ100,
title/body chung mặt viết 720px, và Thông tin tác phẩm sau mặt viết. Giữ nguyên
những owner này; giữ sticky khi đủ rộng/cao theo contract #101. Ở chữ200/màn hẹp,
toolbar trở về document flow. Không đổi palette, measure hoặc save protocol.

```text
Screen #editor — tác giả sửa nháp riêng, WR-01/02/05, UX-01, INV-08
  Row #context — về Bàn viết; H1 Bản nháp (chữ UI)
  Region #toolbar — status + Chi tiết; preview/export icon; Lưu primary
  Region #recovery — có điều kiện; cảnh báo + actions luôn thấy
  Region #paper — label/tiêu đề Literata; label/nội dung CodeMirror
  Region #work-info — H2 UI; status; quyền riêng tư; tên thẻ hiện tại
    Disclosure #work-tags-edit — Chỉnh thẻ; TagSelector hiện có
    Region #tag-recovery — lỗi/xung đột + actions ngoài disclosure
```

Tiêu đề dùng token headline (28px/600), giữ native input, giới hạn 200 ký tự,
selection/IME và cuộn ngang của trường. Hai label dùng caption UI cùng phân cấp.
Ngữ cảnh Bản nháp và heading metadata dùng Be Vietnam Pro thay vì Literata.
Chỉnh thẻ dùng native `SupportingDetails` từ #90; Enter/Space và focus native,
không thêm overlay, cuộn con, renderer hay state authority. Selector/query vẫn
mounted khi đóng. Tên lựa chọn và status nằm ngoài; tải lỗi, từ chối, unknown và
conflict không bị giấu. Ở lại trả focus về trigger nếu đang đóng, query nếu mở.

Owners: app `style/vink.css`, `pages/work_info.rs`, đoạn restore-focus trong
`pages/desk.rs`; dùng lại shared presentation. Không sửa CodeMirror runtime,
TagSession/DraftSession, API/DB/auth hoặc save/reconciliation/export/leave guards.

## Bằng chứng local

Kiểm local trên macOS, Chrome 157, PostgreSQL 17.11 scratch ở loopback, LocalFixture
có nhãn. Dữ liệu so ảnh là văn xuôi mẫu “Những ngày mưa ở khuỷu đường cũ”, cùng
body, trạng thái clean/unknown, theme, viewport và root font CSS. Ảnh toàn trang
chụp scrollY=0; ảnh metadata cuộn đầu vùng theo cùng chính sách. Không so số
absolute scrollY giữa các vùng có chiều cao khác nhau.


| Cùng fixture | Trước | Sau |
| --- | ---: | ---: |
| Toolbar 1440/sáng hoặc tối/chữ100 | 64px | 64px |
| Metadata 1440/tối/chữ100 | 342.52px | 190.77px |
| Metadata 1440/sáng/chữ200 | 676.48px | 359.14px |
| Metadata 320/tối/chữ200 | 1314.94px | 645.09px |

Không tuyên bố toolbar được làm nhỏ hơn: R5 đã giải quyết phần đó. Các giá trị
metadata là trạng thái đóng sau sửa, catalog rỗng trong DB so ảnh. Khi mở,
TagSelector đầy đủ vẫn ở cùng luồng cuộn. Browser regression riêng dùng catalog
thật do lệnh operator seed, ghi và đọc lại trên PostgreSQL scratch.

| Ảnh cùng điều kiện đã mở | Trước | Sau |
| --- | --- | --- |
| 1440/tối/chữ100, scrollY=0 | [Ảnh](../development/evidence/editor-r14/before-r14-clean-1440-100-dark.png) | [Ảnh](../development/evidence/editor-r14/after-r14-clean-1440-100-dark.png) |
| 1440/sáng/chữ200, scrollY=0 | [Ảnh](../development/evidence/editor-r14/before-r14-clean-1440-200-light.png) | [Ảnh](../development/evidence/editor-r14/after-r14-clean-1440-200-light.png) |
| Metadata 1440/tối/chữ100 | [Ảnh](../development/evidence/editor-r14/before-r14-clean-1440-100-dark-metadata.png) | [Ảnh](../development/evidence/editor-r14/after-r14-clean-1440-100-dark-metadata.png) |
| Metadata 320/tối/chữ200 | [Ảnh](../development/evidence/editor-r14/before-r14-clean-320-200-dark-metadata.png) | [Ảnh](../development/evidence/editor-r14/after-r14-clean-320-200-dark-metadata.png) |

[Geometry/context](../development/evidence/editor-r14/contexts.json) và
[hash source/ảnh](../development/evidence/editor-r14/manifest.json). Chỉ các ảnh
chọn lọc này cùng một số ảnh app 390/unknown được mở; không gọi toàn bộ matrix
capture là đã review thị giác.

## Kiểm chứng và tự review

09–10/11/2026 UTC, Darwin 24.6.0 arm64, Rust 1.99.1, Node 22.15.0,
Chrome 157.0.8103.40, PostgreSQL 17.11. Cwd checkout Vink. Lệnh Cargo dùng
`CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0`,
`CARGO_BUILD_JOBS=4`; npm cache trong `/tmp`. Web nhận `VINK_CHROME` và
`VINK_TEST_DATABASE_URL` của cluster scratch riêng. Không dùng production data.

| Lệnh | Kết quả / boundary |
| --- | --- |
| `cargo xtask verify quick` | PASS — fmt, clippy SSR/hydrate/preview, unit/contract/doctest, tokens và repository checks |
| `node apps/vink-web/e2e/browser-smoke.mjs --base http://127.0.0.1:3101 --chrome … --state … --phase editor-chrome` | PASS — catalog thật; 320/390/834/1024/1440 × sáng/tối × root font100/200; trạng thái và save races |
| `cargo xtask verify web` | PASS — full SSR/privacy, hydration, editor, thẻ, discovery, restart và endorsement regression |
| `node --check apps/vink-web/e2e/browser-smoke.mjs`, diff/links/JSON, tự review | PASS — không còn finding có thể hành động trong phạm vi |

Ca mới kiểm cùng trục toolbar/paper/metadata; Chỉnh thẻ mở/đóng bằng Enter/Space,
target ≥48px, query và EditorView giữ identity; tên thẻ khi đóng; Ở lại trả về
trigger đang thấy; reload giữ tên được ACK; conflict và lưu đè ngoài disclosure.
Giữ các ca cũ về simulated composition/undo, pending+new edits, mất ACK/GET
reconciliation, explicit overwrite, export, leave/discard/late ACK, sticky/caret.
`shellWithDirtyEditor` kiểm rail 96/220 và Giao diện thật giữ chữ chưa lưu,
selection và một EditorView. Các checks này không chứng minh IME hệ điều hành.

Lượt web đầu **FAIL** vì phép chờ animation của harness giữ promise `finished`
cho chip nằm trong native details đóng; probe ghi `idle`, `currentTime=null`.
Chỉ lọc trạng thái lúc lấy promise vẫn không đủ. Bản cuối poll animation hữu hạn
đang chạy và visible, có timeout. Focused và full web sau sửa đều PASS. Các probe
caret bổ sung ban đầu dùng DOM focus/Tab thiếu đúng chu kỳ native; không lấy chúng
làm kết luận mất chữ, dùng ca hồi quy bàn phím có sẵn nêu trên. Log FAIL được giữ.

## Giới hạn và bàn giao

NOT_RUN: OS IME, zoom browser thật, paste Word/Docs, screen reader có tên,
Safari/Firefox, thiết bị mobile thật và CMP/native. Root font CSS/CDP không chứng
minh các lớp này. `verify postgres`, Quint, Kani và UI preview runtime không chạy
vì không đổi store/domain/contract hoặc shared implementation. Không kiểm/chờ
GitHub CI; không deploy; issue #101 vẫn mở cho acceptance cần bằng chứng còn thiếu.

Ảnh/log/context đầy đủ liên quan được giữ local tại
`/Users/manhblue/Documents/Codex/2026-11-10/task-2/evidence/vink-101`.
Các output build được dọn sau khi giữ bằng chứng và xác minh merge remote.
