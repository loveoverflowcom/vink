# Ví dụ và tình huống kiểm workflow

Các nội dung dưới là **mẫu đề xuất**, chưa sinh UI hay kiểm runtime. Nguồn đã đối
chiếu: `develop` tại `65bd6008692bbece8387c75f0c8efb9c644dbb51`,
[CSS](../../../../apps/vink-web/style/vink.css) và
[Bàn viết](../../../../apps/vink-web/src/pages/desk.rs). Kiểm lại source hiện hành
khi dùng. Bàn viết có danh sách/tạo nháp, editor lưu thủ công và conflict; chưa
có nguồn phản hồi/Discourse. Dashboard dưới không là mô tả màn đang chạy.

## A. Dashboard Bàn viết mới và prototype

Yêu cầu mẫu: “Thiết kế dashboard mới và làm prototype.” Vai trò tác giả; mục tiêu
viết tiếp và tìm phản hồi. Cùng dữ liệu giả được ghi nhãn: “Mưa ở đầu ngõ” và
“Những ngày bà để cửa mở chờ một người đi xa”; phản hồi mẫu không có số đếm tương
tác. Cùng viewport giả định 1440/390 CSS px, light/dark, state có bản thảo;
trạng thái khó chung là không tải được danh sách. Không giả hiện đã có API phản hồi.

**A — Tiếp tục một bản thảo:** ưu tiên quay lại bài gần nhất.

```text
Screen #desk — tiếp tục viết
  Column #page — đọc từ trên xuống
    Region #header — tiêu đề, Tạo bản thảo phụ
    Region #resume — bài gần nhất, CTA Viết tiếp nổi bật
    Row #secondary — phần sau việc chính
      Region #draft-list — các bản thảo khác
      Region #feedback — phản hồi mẫu, mức nhấn thấp
```

Màn hẹp: resume → draft-list → feedback. Lỗi tải: thông báo tại vùng danh sách,
không biến thành “Chưa có bản thảo”; chỉ hiện resume khi có dữ liệu đáng tin.
Đánh đổi: một bài chiếm nhiều diện tích, đổi bài cần đi xa hơn.

**B — Danh sách và phản hồi bên cạnh:** ưu tiên chọn bài và tìm ngữ cảnh.

```text
Screen #desk — chọn bản thảo để viết tiếp
  Column #page — ngữ cảnh trước nội dung
    Region #header — tiêu đề, Tạo bản thảo phụ
    Grid #workspace — chính linh hoạt, phụ hữu hạn
      Column #drafts — bản thảo là trọng tâm
        Region #resume — bài gần nhất, CTA Viết tiếp
        Region #draft-list — hàng gọn cho các bài khác
      Region #feedback — phản hồi mẫu hỗ trợ
```

Màn hẹp: resume → draft-list → feedback, giữ action cạnh bài. Lỗi tải: vùng drafts
hiển thị lỗi và cách thử lại; feedback không nhận vị trí CTA chính. Đánh đổi:
feedback nằm thấp trên mobile. **Khuyến nghị B** cho tác giả thường đổi bài và
đọc góp ý; tiếp tục đặc tả/prototype vì yêu cầu đã giao cả hai.

**C — Tổng quan tuyến tính:** ưu tiên đọc và thao tác trên màn hẹp.

```text
Screen #desk — một luồng việc gọn
  Column #page — không chia cột
    Region #header — tiêu đề, Tạo bản thảo phụ
    Region #draft-list — bài gần nhất đầu danh sách, CTA Viết tiếp trong hàng
    Region #feedback — phản hồi mẫu sau danh sách
```

Màn hẹp giữ thứ tự; title xuống dòng, action dưới title khi thiếu chỗ. Lỗi tải:
thay nội dung danh sách bằng lỗi/thử lại, giữ heading. Đánh đổi: phản hồi dễ nằm
ngoài màn hình đầu khi có nhiều bản thảo. C gộp resume vào danh sách, khác A/B
về phân cấp chứ không chỉ đổi màu.

### Đặc tả B trước thao tác sinh UI

`LayoutSpec desk-B/v1 — PROPOSED, agent khuyến nghị theo yêu cầu mẫu.` Cây B ở
trên là cây chính; không duy trì thêm một bản có ID khác.

- **Nội dung/component:** `header` có h1 Bàn viết, nhãn Dữ liệu minh họa, Tạo bản
  thảo là action phụ. `resume` có title, phiên bản mẫu và Viết tiếp; không gán
  “Đã lưu” từ dữ liệu danh sách vì chưa biết text chưa lưu trong editor.
  `draft-list` là ul/li. Tái dùng `.vk-page-title`, `.vk-button--filled` cho Viết
  tiếp, `.vk-button--tonal` cho Tạo bản thảo, `.vk-muted`, `.vk-notice` khi đúng
  vai trò. Wrapper grid/dashboard và nguồn feedback là đề xuất; không giả có
  component Rust hay API tương ứng. Hành động chưa có backend chỉ chạy trên
  fixture prototype trong phạm vi đã giao, không báo thành công production.
- **Phân cấp/kích thước:** header → resume → draft-list → feedback cũng là thứ tự
  DOM/focus. Đề xuất 2 track `minmax(0, 1fr)` và `minmax(16rem, 22rem)` ở màn rộng;
  trần trang 72rem lấy từ `.vk-main`, grid gap `--vk-space-6`. `.vk-desk` hiện chặn
  ở 48rem: cần wrapper riêng cho dashboard nếu triển khai, giữ measure editor.
  Title wrap tự nhiên, không khóa chiều cao/cắt dấu hoặc dùng `break-all`.
- **Responsive:** dưới 64rem chuyển một cột (breakpoint mới **đề xuất**, chưa là
  token/source hiện có). Giữ toàn bộ feedback dưới danh sách, chưa cần drawer.
  Ở 320px và tăng chữ, action xuống hàng nếu cần, tên đầy đủ vẫn đọc được.
- **Cuộn/overlay:** một cuộn trang; không vùng cuộn riêng, sticky/fixed hay overlay.
  Nếu bàn phím xuất hiện, action vẫn ở trong flow; chưa có bằng chứng thiết bị thật.
- **Light/dark:** cùng cây và nghĩa hành động; dùng các role/token bên dưới theo
  `prefers-color-scheme` hiện có, không bịa theme toggle. Focus và disabled được
  kiểm riêng, chưa kết luận contrast từ tên token.

| Vai trò | Token/rule hiện có dùng cho cả light/dark |
| --- | --- |
| Nền / chữ chính | `--vk-surface` / `--vk-on-surface` |
| Vùng hỗ trợ / chữ phụ | `--vk-surface-container-low` / `--vk-on-surface-variant` |
| CTA chính | `--vk-primary` / `--vk-on-primary` |
| Selected nếu có control chọn | `--vk-primary-container` / `--vk-on-primary-container`; mẫu này không thêm selection chỉ để dùng token |
| Focus | `:focus-visible` dùng `--vk-primary` |
| Disabled | `.vk-button:disabled` hiện dùng opacity 0.5, chưa có token disabled riêng; lý do có chữ tại control |
| Lỗi | `--vk-error`, `.vk-notice`; thêm thông tin bằng chữ |

- **States:** loading giữ heading và vùng thông báo; empty có “Chưa có bản thảo”
  và Tạo bản thảo thành CTA chính; error tải dữ liệu dùng thông báo/thử lại, không
  hiển thị danh sách rỗng giả. Pending tạo nháp tránh gửi lặp, failure giữ ngữ cảnh,
  success prototype chỉ cập nhật fixture có nhãn. Feedback empty là lời ngắn;
  failure khác không có phản hồi. Conflict thuộc editor hiện có: bảo toàn text,
  hiện hành động khôi phục đúng owner; dashboard không tự thêm lệnh ghi đè.
- **Accessibility/đối chiếu:** một h1 trong main của shell, h2 cho vùng có tiêu đề,
  tên link gồm tên bài; tab theo thứ tự cây, focus rõ và không chỉ báo state bằng
  màu. Kiểm cùng dữ liệu ở 1440/390 và trường hợp 320px/tăng chữ, cả hai theme;
  so các ID, CTA và lỗi tải với spec. Render/keyboard/contrast đều **NOT_RUN**
  trong ví dụ này; ảnh không chứng minh lưu nháp hoặc IME.

Grid Areas tùy chọn cho B: wide có 2 track như trên, một hàng `drafts feedback`;
narrow có 1 track, hai hàng `drafts` rồi `feedback`. Header nằm ngoài grid; resume
và draft-list là con của drafts. Ma trận không thêm vùng trái cây.

## B. Sửa title dài đẩy action ra ngoài

Đầu vào mẫu có ảnh chứng minh title đẩy nút; đây không là finding về code Vink
hiện tại. Sau khi trace owner, xuất bản rút gọn này **trước patch**:

```text
Region #draft-row — nhận diện bản thảo và mở để viết tiếp
  Region #title — tên đầy đủ có dấu
  Region #resume-action — Viết tiếp
Before → After: title chiếm ngang đẩy nút → title co/wrap, action xuống hàng khi cần.
Narrow: cùng thứ tự title → action; một luồng cuộn, không cắt tên hoặc ẩn action.
Themes/state: giữ semantic tokens hiện có, focus rõ; không đổi nghĩa trạng thái lưu.
Check: đối chiếu ảnh cùng title dài/viewport/theme, tăng chữ và keyboard; chưa chạy.
```

Không cần thêm ba hướng dashboard. Với “Sửa padding nút Lưu”, chỉ mô tả cây vùng
save-status/save-action, padding trước/sau theo token đã đọc, ảnh hưởng thanh lưu
ở màn hẹp và cách kiểm; không lan sang tất cả `.vk-button` nếu chỉ một nút có lỗi.

## C. Người dùng đã chọn B

“Triển khai phương án B đã chọn thành prototype” → xác nhận bằng lời đang dùng
`desk-B/v1`, trình bày hoặc liên kết spec B còn hiệu lực trước thao tác tạo UI,
ghi phần bổ sung do source mới rồi tiếp tục prototype theo stack đã giao. Không
hỏi chọn lại A/B/C hoặc xin duyệt spec mặc định. Nếu B thiếu trong ngữ cảnh và
không tìm được qua nguồn đã có, hỏi đúng nội dung B; không tự bịa phương án đã chọn.

## Tình huống khi bảo trì skill

| Yêu cầu | Hành vi cần quan sát |
| --- | --- |
| Thiết kế dashboard mới và làm prototype | 2–3 hướng cấu trúc → khuyến nghị/spec → sinh UI, tiếp tục không chờ duyệt mặc định |
| Chỉ phác thảo 3 hướng, chưa viết mã | Đúng ba hướng, cùng input và trạng thái khó để so; dừng ở phác thảo |
| Triển khai phương án B đã chọn | Giữ B, spec trước sinh UI; không mở lại lựa chọn |
| Sửa padding của nút Lưu | Spec rút gọn trước patch, đúng owner; không redesign toàn màn |
| Review ảnh này, chưa sửa | Mở ảnh, quan sát/đối chiếu; thiếu spec không là lỗi; không patch |

Đây là ca kiểm và kỳ vọng, chưa phải kết quả chạy. Khi đánh giá bằng agent độc
lập, cung cấp yêu cầu/nguồn tối thiểu trong nơi tách biệt, giữ trace đầu ra trước
thao tác sửa/sinh UI. Kiểm scope, thứ tự và hành động thực; không chỉ đếm từ khóa
hoặc đánh dấu PASS vì tài liệu đã nhắc quy tắc. Không dùng demo skill làm bằng
chứng runtime Vink hay bảo đảm mọi lần gọi model đều tuân thủ.
