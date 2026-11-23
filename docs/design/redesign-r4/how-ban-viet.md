# HOW · Bàn viết tìm, đọc và khôi phục danh sách

Source tại `develop@2712145b08a6a4c7a12c1d57033ecd2a5bd33b26`. Người viết đã vào `/ban-viet`,
mở kính lúp, nhập “đường về”, chọn sắp xếp và mở một bản thảo. Nếu mạng rớt sau khi đã tải,
họ vẫn xem được kết quả trước đó trong cùng phiên và có cách thử lại.

1. [DeskIndex / DeskSession](../../../apps/vink-web/src/pages/desk.rs#L40) phân luồng phiên.
   SSR có khung, trạng thái phù hợp và dùng `DeskAccess` khi thiếu quyền. Facts của tác giả
   không được serialize cùng HTML. `DeskWorks` chỉ phát đọc riêng từ phía hydrate với phiên đủ quyền.
2. [DeskWorks](../../../apps/vink-web/src/pages/desk.rs#L149) giữ `query` (ô nhập), `applied`
   (query đã submit), `sort` và dock state. Mở dock focus `#draft-search`. Form được ẩn bằng
   `hidden` khi đóng. Enter/submit đang composition bị chặn. Submit lấy `query`, đổi scope đọc
   hoặc refresh cùng scope. [Đóng dock](../../../apps/vink-web/src/pages/desk.rs#L162) bằng nút,
   Esc hoặc Back trả focus về launcher và hiện vẫn giữ `query`/`applied`. Đó là khác biệt với
   đề xuất #414 cho kho rỗng và đóng dock, chưa có kết quả runtime mới hay quyết định áp dụng
   hết các delta.
3. [read_list](../../../apps/vink-web/src/pages/desk.rs#L202) nhận ticket `ReadOwner` cho
   scope/intent và cursor. [Client](../../../apps/vink-web/src/api.rs#L814) gửi GET tới
   `/api/v1/author/works`. [HTTP](../../../services/vink-app/src/http/api.rs#L401) lấy owner
   từ phiên, parse query, gọi [application](../../../services/vink-app/src/application/author_works.rs#L55)
   rồi [PostgreSQL](../../../services/vink-app/src/persistence/author_works.rs#L198). HTTP dùng
   no-store với dữ kiện riêng. Cursor là vị trí đọc chứ không cấp quyền. Đổi account hoặc scope
   làm kết quả muộn mất hiệu lực. Tìm/sắp xếp và phân trang vẫn do provider thực hiện.
4. [DraftListView](../../../apps/vink-web/src/pages/desk.rs#L627) render danh sách đã đọc và
   `AuthorWorkRow` key bằng Work ID. Trang 24 mục, tối đa 240 mục mỗi phần. Hàng mở bản thảo,
   liên kết công khai là sibling và gắn ngữ cảnh bản công khai. [Registry tác giả](../../development/desk-author-feedback.md)
   sở hữu quy tắc các số liệu và quyền.

## Failure và phục hồi

- [Nhánh hoàn thành](../../../apps/vink-web/src/pages/desk.rs#L270) bỏ response sai ticket/phiên.
  Refresh tạm lỗi giữa cùng phiên giữ kết quả và ô tìm, thông báo dữ liệu cũ, có `#retry-drafts`
  khi có thể thử lại. 401/403 gỡ dữ kiện riêng. First use chưa viết khác query không có kết quả.
- [Lifecycle](../../../apps/vink-web/src/pages/desk.rs#L333) refresh khi tab hiện và `wake.due`;
  không gửi mutation vì reconnect. Phân trang cursor `Invalid` có `#restart-drafts`, lỗi append
  còn thử tải tiếp, `#first-draft-page` về phần đầu. `NotFound` có nhánh riêng trong code hiện
  tại, cần tái hiện nếu muốn mở rộng CTA restart cho ca này.
- [CreateNotice](../../../apps/vink-web/src/pages/desk.rs#L573) giữ “Tải lại danh sách” cho POST
  tạo bản thảo chưa xác nhận. GET đối soát giúp mở bản tìm thấy hoặc cho phép chủ động tạo thêm,
  không tự lặp POST và không chứng minh request ban đầu đã ghi. [Đích focus](../../../apps/vink-web/src/pages/desk.rs#L311)
  hiện gồm restart, heading khi sang phần, input/launcher khi phục hồi, hoặc khôi phục anchor
  khi refresh nền.

[Smoke source](../../../apps/vink-web/e2e/browser-smoke.mjs#L3490) có mở/đóng dock, UI lỗi,
đổi scope, query riêng, giữ node và simulated composition. [Bằng chứng #411](../../development/evidence/desk-states-411/README.md)
có SHA và môi trường riêng. Không chạy lại browser hoặc IME hệ điều hành trong lượt tài liệu.
