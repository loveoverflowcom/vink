# HOW · Bàn viết tìm, đọc và khôi phục danh sách

Trace gốc tại `develop@2712145b08a6a4c7a12c1d57033ecd2a5bd33b26`. Người viết đã vào `/ban-viet`,
mở kính lúp, nhập “đường về”, chọn sắp xếp và mở một bản thảo. Nếu mạng rớt sau khi đã tải,
họ vẫn xem được kết quả trước đó trong cùng phiên và có cách thử lại.

1. **SSR.** [Session::provide](../../../apps/vink-web/src/account.rs#L78) khởi tạo phiên `Unknown`,
   chưa có scope riêng. [DeskIndex](../../../apps/vink-web/src/pages/desk.rs#L40) render heading,
   thông báo `noscript` và [DeskLoading](../../../apps/vink-web/src/pages/desk.rs#L131). Skeleton
   có trong markup nhưng ẩn. HTML không serialize dữ kiện của tác giả.
   **Sau hydrate.** `browser::watch` kiểm phiên. [DeskSession](../../../apps/vink-web/src/pages/desk.rs#L79)
   dùng `DeskAccess` cho khách, người thiếu bút danh, phiên bị khóa hoặc không kiểm tra được. Khi
   xác nhận scope riêng, `DeskIndex` mount `DeskWorks` theo scope và mới phát GET danh sách.
2. [DeskWorks](../../../apps/vink-web/src/pages/desk.rs) giữ `query` của ô nhập, `applied`
   của danh sách và `sort` trong scope phiên. Theo [delta #531](layout-spec.md#51-header-tìm-và-phục-hồi),
   mở dock đưa focus vào `#draft-search` ở hàng riêng. Nút mở/đóng, chọn định dạng, tạo và sort
   vẫn ở vị trí cũ. Form đóng thu về hàng cao 0 và dùng `inert` để giữ node, buffer và thứ tự Tab. Submit sau composition
   mới đổi scope đọc hoặc refresh cùng scope. Nút đóng, Escape và Back giữ nguyên hai chuỗi.
   `#draft-filter` hiện cụm đã áp dụng dù người dùng đang sửa ô nhập hoặc đã thu gọn.
   “Bỏ lọc” trả danh sách về đầu với sort đang chọn. Khi form đã đóng, focus phục hồi về
   launcher, hoặc H1 nếu công cụ bị ẩn. Một thao tác lấy focus sau đó được ưu tiên.
3. [read_list](../../../apps/vink-web/src/pages/desk.rs#L202) nhận ticket `ReadOwner` cho
   scope/intent và cursor. [Client](../../../apps/vink-web/src/api.rs#L814) gửi GET tới
   `/api/v1/author/works`. [HTTP](../../../services/vink-app/src/http/api.rs#L401) lấy owner
   từ phiên, parse query, gọi [application](../../../services/vink-app/src/application/author_works.rs#L55)
   rồi [PostgreSQL](../../../services/vink-app/src/persistence/author_works.rs#L198). HTTP dùng
   no-store với dữ kiện riêng. Cursor là vị trí đọc chứ không cấp quyền. Đổi account hoặc scope
   làm kết quả muộn mất hiệu lực. Tìm/sắp xếp và phân trang vẫn do provider thực hiện.
4. [DraftListView](../../../apps/vink-web/src/pages/desk.rs#L627) render danh sách đã đọc và
   `AuthorWorkRow` key bằng Work ID. Trang 24 mục, tối đa 240 mục mỗi phần. Hàng mở bản thảo;
   không còn liên kết “Đọc” trên thẻ theo `D-02` ([LayoutSpec Lượt xem r3](../work-views.md)),
   hàng số liệu theo thứ tự `Thấm → Nhận xét → Lượt xem` (`D-01`) kèm giải thích `#views-explainer`
   và [registry Lượt xem](../../development/work-views.md). [Registry tác giả](../../development/desk-author-feedback.md)
   sở hữu quy tắc Thấm/Nhận xét và quyền.
5. [Desk R5](layout-spec.md#54-desk-r5-614) tái dùng Button và Select. Hai trạng thái dùng
   chung nút “Viết” và chọn định dạng. Nút Viết, định dạng và sắp xếp dùng pill tại Desk.
   Khi mất ACK, “Tải lại danh sách” và “Viết thêm” giữ thứ tự đối soát của luồng tạo.

## Mở chi tiết kiểm duyệt (#477)

Từ hàng có ngữ cảnh, “Xem kiểm duyệt” mở `/ban-viet?review={WorkId}`. [Tín hiệu ngữ cảnh](../../development/desk-author-feedback.md#ngữ-cảnh-kiểm-duyệt-614)
đi cùng trang tác phẩm và gồm lịch sử qua nhiều revision, kể cả khi revision mới nhất chưa được gửi.
Cùng một chi tiết nhận liên kết từ Editor, URL cũ và đích thông báo có thêm `revision` cùng `decision`. Component
[DeskReview](../../../apps/vink-web/src/pages/desk_review.rs) tải một tác phẩm theo phiên đã
xác nhận, độc lập với phần danh sách đang hiển thị. Khi đóng, bộ lọc và vị trí trên Bàn viết
vẫn thuộc owner danh sách. [LayoutSpec R4.4](desk-moderation.md) ghi trạng thái và cách điều hướng.

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
đổi scope, query riêng, giữ node và simulated composition.
có SHA và môi trường riêng. Không chạy lại browser hoặc IME hệ điều hành trong lượt tài liệu.
