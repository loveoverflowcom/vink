# Auth R2 trong Reader · #328

- Base: `ea6a4025a46ebca680f5083517e36a9de466b548` (#330). Source đã chạy:
  [`23718e8`](https://github.com/loveoverflowcom/vink/commit/23718e8de1fdf42b4467e7b9b365e823b4a41d38),
  tree `c3abd135403f5726954d812fd500753a364181fe`.
- Lệnh: `cargo xtask verify web --scenario auth --scenario password`. App và Identity chạy thật
  trên loopback, PostgreSQL 17.11 scratch, mã email qua outbox cục bộ, Chrome
  157.0.8103.79 headless/CDP. Phase auth còn khởi động App thật với cổng đăng nhập tắt.
- Kết quả: PASS `auth`, `reader-auth-closed`, `auth-password`. Log hoàn chỉnh SHA-256:
  `5183136c020445afd6153b47f107a62d7709a36862b6908d2d246f8b0c49a8c5`.
- Chữ 200% là phóng font gốc, không phải zoom. Mốc thị giác đã mở:
  [Reader R2, mobile tối 390×844](https://drive.google.com/file/d/1Uef6IlEtkxQ_trMB8rw0JLz2nh9RlqZb/view).
  [LayoutSpec Reader](../../auth-reader-web.md) ghi phần kế thừa và hành vi.

## Nghiệm thu trên runtime thật

| AC | Bằng chứng |
| --- | --- |
| 01 | Root và reply cùng giữ DOM, text và selection có hướng. Identity xác minh mã thật; App ghi bút danh rồi đọc lại phiên/principal. Khi vùng nhận xét riêng xuất hiện, textarea phản hồi ở 480.75 → 480.16 CSS px so với đỉnh viewport. Không thấy POST nhận xét suốt Auth; một POST thật trả 201 sau khi bấm Gửi. |
| 02 | Escape, Back nội bộ, Đóng khi có request, lỗi mã và ACK 200 thật giữ/thả qua CDP. Back giữ chính xác scrollY 599 và selection 3→12 hướng lùi. Đóng trả focus CTA; xác nhận principal trả focus đúng textarea. ACK muộn không mở lại form hoặc giành focus. Hồi quy cùng pha kiểm A→B và A→khách→B, gồm ACK nhận xét thật giữ/thả: không rò chữ, receipt hay phần riêng. |
| 03 | Bài thật đã tắt nhận xét và App thật tắt đăng nhập đều không có CTA Auth. Desktop là native dialog; mobile là sheet. 320px/chữ 200% không tràn ngang; đủ trường và nút cuối qua Tab, cuộn thật với viewport thấp 500 CSS px, keyboard trap hai chiều và reduced motion. |

Fixture SQL chỉ dựng một root công khai và một bài đã khóa nhận xét trong DB scratch.
Request xác thực, bút danh, quyền và nhận xét đi qua API thật. Không khẳng định đã kiểm duyệt
root fixture qua luồng vận hành. Những ca dựng lỗi cũ khác của pha auth vẫn được ghi nhãn riêng.

## Ảnh Chrome viewport thật

| Ảnh | Đã đối chiếu |
| --- | --- |
| [390×844 tối 100%](reader-auth-390-dark-100.png) | Sheet neo đáy, nền Reader và nháp còn thấy sau scrim; logo/Đóng, tiêu đề, hai trường tonal, nút Filled và hai hành động text đúng thứ tự R2. Mép trên y≈398 so với y≈389 ở mốc; nội dung nền và focus ring là từ app thật, không so từng pixel. |
| [1280×844 sáng 100%](reader-auth-1280-light-100.png) | Dialog căn giữa viewport, cùng thứ tự, rail/Reader giữ phía sau. |
| [320×800 tối 200%](reader-auth-320-dark-200.png) · [320×500 tới nút cuối](reader-auth-320-dark-200-end.png) | Tiêu đề và Đóng trước, không thu nhỏ chữ, cuộn dọc; ảnh cuối từ Tab thật tới “Dùng mã email”. |

| Chiều | Kết quả trong phạm vi local |
| --- | --- |
| Functional | PASS: luồng root/reply, bút danh, gate và dữ liệu riêng theo scope. |
| Geometry | PASS: viewport anchor, sheet chạm đáy, dialog căn giữa, reflow và scrollport. |
| Visual | PASS (self-review): mở cả bốn ảnh trên và so phân cấp 390×844 với mốc duyệt. |
| Accessibility / responsive | PASS trong Chrome cho tên dialog, chữ 200%, thứ tự Tab, focus và nút cuối; chưa đo lại tương phản riêng của sheet hoặc dùng screen reader. |
| Interaction | PASS cho thao tác Chrome được liệt kê, gồm Escape, Back trong lịch sử SPA và giữ/thả ACK thật. |

Không kiểm Back giữa hai tài liệu (trước khi vào SPA), Safari/Firefox, điện thoại thật, IME và screen
reader. Quick và lane PostgreSQL đầy đủ để lượt rà soát cuối; PostgreSQL scratch phía trên là
bằng chứng browser runtime, không thay lane tích hợp đầy đủ. Không CI GitHub, deploy hoặc bật production.
