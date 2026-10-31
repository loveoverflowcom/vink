# Hệ thiết kế Vink

## Chủ nguồn và baseline

Chủ tokens/component hiện hành: `apps/vink-web/style/vink.css` trong Vink, snapshot
`85cc0b2763aaf87461a6f0382a71388251ad9614`. Dùng `--vk-*` và `.vk-*`; không tạo
palette thứ hai trong feature. Các giá trị dưới đây là baseline đã triển khai,
không phải tuyên bố mọi giá trị đã được chốt thương hiệu vĩnh viễn.

| Vai trò | Light | Dark |
| --- | --- | --- |
| `--vk-surface` | `#fbf8f3` | `#141312` |
| `--vk-surface-container` | `#f0eae1` | `#22201e` |
| `--vk-on-surface` | `#1d1b19` | `#e9e3da` |
| `--vk-on-surface-variant` | `#4c463f` | `#d0c7bb` |
| `--vk-primary` | `#2f3b73` | `#bac3ff` |
| `--vk-on-primary` | `#ffffff` | `#1a2457` |
| `--vk-primary-container` | `#dde1ff` | `#353f78` |
| `--vk-on-primary-container` | `#16204f` | `#dde1ff` |
| `--vk-tertiary` | `#9c3a26` | `#ffb4a3` |

Giữ cảm giác **giấy ấm – mực chàm – đỏ son**. Accent đỏ son là điểm nhấn thương hiệu,
không đồng nghĩa trạng thái lỗi; lỗi dùng token semantic riêng cùng icon/chữ. Thiết
kế dark có phân cấp nền riêng, không đảo màu ảnh light. Không mang Lavender Stage,
audio player, CMP parity hoặc yêu cầu AVIF-only của Cantos vào Vink.

Nếu thay role chung, liệt kê các consumer bị ảnh hưởng và đo lại những cặp màu thực
tế trong cả hai theme. Xem cả hover/focus/selected/disabled, nền ảnh và scrim nếu có.
Không suy ra tương phản đạt từ tên token hay một preview tạo sinh.

## Typography và tính toàn vẹn văn bản

Baseline reading stack: `Literata`, `Noto Serif`, `Source Serif 4`, `Source Serif Pro`,
`Times New Roman`, serif. UI stack: `Be Vietnam Pro`, `Noto Sans`, `Segoe UI`, Roboto,
`Helvetica Neue`, Arial, system-ui, sans-serif. Đây là **local fallback stacks**;
không khẳng định các font đầu đã được tải hay nhúng. Thêm web font là một quyết định
asset riêng, gồm license, dung lượng, coverage và fallback.

Reading hiện dùng `clamp(1.0625rem, 0.98rem + 0.35vw, 1.25rem)`, leading `1.8`, measure
`36rem`. Giữ chúng làm điểm xuất phát, rồi kiểm với nội dung/thiết bị thật. UI và
văn bản đọc có vai trò khác nhau; không buộc toàn bộ app dùng serif hoặc title cực lớn.

- Dùng mẫu đầy đủ dấu, dấu kết hợp, tên dài, hội thoại, thơ và văn xuôi. Kiểm glyph
  fallback theo weight; tránh chiều cao cố định cắt dấu hoặc dấu chấm dưới.
- Cho nội dung dài wrap tự nhiên; không `break-all` để cứu layout. Tránh title đẩy
  nút ra khỏi màn. Cho truy cập tên đầy đủ nếu truncation có lý do.
- Giữ paragraph, hard line break và khoảng khổ thơ có chủ ý. Không tạo card cho mỗi
  đoạn, không áp layout chương truyện lên thơ. Không chuẩn hóa lại giọng/tả của tác giả.
- Editor giữ IME composition, caret, selection, undo và scroll anchor. Không format,
  gửi hoặc ghi đè đang gõ bằng server echo giữa composition.
- Bố cục thích nghi ở màn hẹp và khi tăng chữ; bề rộng đọc hữu hạn ở desktop, không
  kéo văn bản thành một dòng dài theo chiều rộng dashboard.

## Hình, khoảng cách, điều hướng

Baseline radius `8/12/20/28px` và pill; spacing `0.25/0.5/0.75/1/1.5/2/3rem`.
Chọn scale nhất quán, dùng khác biệt hình/kích thước để nói vai trò. Nút hành động
và vùng đọc không cần cùng radius. Ưu tiên surface separation và khoảng trắng;
focus ring hoặc viền cần để hiểu control được giữ.

Navigation ưu tiên định hướng và việc tiếp tục. Chọn top navigation, rail, drawer
hay bottom bar từ số mục, chiều rộng, input và phân cấp thực tế; không mặc định sao
chép quyết định rail/drawer của Cantos. Preferences gọn, tên ngắn, có accessible name.
Search/overlay phải có anchor, dismiss, keyboard/focus và responsive được mô tả.
Đừng để overlay hoặc bàn phím che hành động/lỗi đang cần; không đổi bố cục gây mất caret.

## Copy và component states

Phân loại copy: **thiết yếu**, **hỗ trợ**, **hướng dẫn**, **dư thừa**. Giữ action,
trạng thái, lỗi, giới hạn bắt buộc, blocker và cách khôi phục ở nơi có thể tìm thấy.
Bỏ đoạn lặp lại tiêu đề hoặc giải thích control quá quen. Tooltip/disclosure chỉ
dùng cho phần tùy chọn; không giấu lý do duy nhất của nút disabled trong hover/long-press.

Ma trận theo component: default, hover khi phù hợp, focus, pressed, selected,
disabled+reason, loading, empty, error, pending và success. Chỉ đưa trạng thái có
nghĩa. Error không được giống “không có nội dung”; retry không làm mất bản thảo hoặc
âm thầm gửi hai lần. Trạng thái luôn có chữ/icon/semantics ngoài màu.

Mẫu nhãn có thể dùng khi khớp nghiệp vụ: `Viết tiếp`, `Tạo bản thảo`, `Lưu bản nháp`,
`Đang lưu…`, `Đã lưu`, `Chưa lưu thay đổi`, `Có bản sửa mới hơn`, `Chờ kiểm duyệt`,
`Gửi để đăng khi được duyệt`, `Bản đang công khai`, `Xem lý do`, `Gửi khiếu nại`.
Không viết `Đã lưu` khi chỉ có local recovery. Không coi những nhãn này là API enum.

## Accessibility và motion

Kiểm heading/landmark, named controls, trạng thái accessible, focus rõ, thứ tự đọc,
keyboard, dialog focus containment/return và phương án thay drag/gesture. Cỡ vùng
bấm phải phù hợp cảm ứng, không chỉ tăng icon. Cho text enlargement/reflow; tránh
truyền thông tin chỉ bằng màu, animation hoặc vị trí. Đo contrast từ màu resolve;
đối chiếu tiêu chuẩn accessibility mà dự án chọn, không tự tuyên bố chứng nhận WCAG.

Motion baseline: `150ms/280ms`, curve `cubic-bezier(0.2, 0, 0, 1)`; dùng theo ý định,
không là thời lượng bắt buộc cho mọi thao tác. Input dispatch ngay, motion trang trí
phản hồi; cho interrupt/reverse, giữ target ổn định và final state rõ. Respect reduced
motion, không chạy animation lặp trang trí khi đọc/viết. Video hoặc tương tác runtime
mới chứng minh hành vi; ảnh tĩnh không chứng minh độ mượt.
