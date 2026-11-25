# HOW · Khám phá tìm, lọc và quay lại kết quả

Người đọc mở `/?q=...`, thêm một thẻ rồi vào Reader. Đường dẫn giữ ngữ cảnh đã áp dụng,
kể cả khi mở trực tiếp hoặc dùng Back. Chữ đang soạn và lựa chọn chưa áp dụng sống ở giao diện.

1. [DiscoveryPage và DiscoverShell](../../../apps/vink-web/src/pages/discovery.rs) đọc mode,
   q, tag và cursor từ router. `ExploreModes` giữ một bộ link thật cho Tất cả, Đang theo dõi
   (theo feature flag) và Tác giả. Link hiện hành mang `aria-current="page"`. Panel kết quả
   đổi theo URL, còn các node thanh tab và logo sống trong shell chung. `DiscoverySearch`
   lấy buffer ban đầu từ q. Ở Tất cả, `/?q=…` mở dock dưới thanh tab ngay từ SSR. Context q,
   hash tìm hoặc trạng thái mở từ nút tìm đều có thể mở dock. Trang `/the/:slug` giữ ô
   “Tìm trong thẻ” riêng.
2. [works_resource](../../../apps/vink-web/src/api.rs#L579) ở SSR gọi `SharedPublicReads` trực
   tiếp. [PublicReadsAdapter::works](../../../services/vink-app/src/http/ssr.rs#L152) gọi
   [discover_works](../../../services/vink-app/src/application/discovery.rs#L176) để đọc snapshot
   công khai qua store. SSR không gọi HTTP vòng về chính server. Trình duyệt hydrate dùng
   resource và adapter public-read. Lần điều hướng sau có thể đọc HTTP `GET /api/v1/works`
   qua [gateway](../../../services/vink-app/src/http/api.rs#L363) với chính sách
   [READ-01/02](../../development/public-read-freshness.md).
3. [Search submit](../../../apps/vink-web/src/pages/discovery.rs) thay q, reset cursor và
   điều hướng. [FilterForm](../../../apps/vink-web/src/pages/discovery.rs) dùng GET để Áp
   dụng, với tag draft tách riêng. Preview lựa chọn và “Bỏ chọn” trong sheet thay draft; thay đổi
   áp dụng mới thành URL. Giữ tối đa 4 thẻ theo AND ở [hợp đồng discovery](../../development/discovery-api.md).
4. [Rows](../../../apps/vink-web/src/pages/discovery.rs) dựng liên kết Reader mang ngữ
   cảnh. [Registry Khám phá](../../development/discovery-ui.md) mô tả context quay lại, focus hàng
   và đích dự phòng khi hàng biến mất. [Đóng dock](../../../apps/vink-web/src/pages/discovery.rs)
   tạo href bỏ q/cursor, giữ các tag và khôi phục focus/scroll theo ngữ cảnh.

## Failure và phục hồi

- Không kết quả vì query/tag khác kho công khai rỗng. Trạng thái có link xóa tìm/xóa lọc phù hợp.
  [DiscoveryFailure](../../../apps/vink-web/src/pages/discovery.rs) phân loại 422 cursor
  không dùng được để quay về trang đầu với cùng bộ lọc, 422 đầu vào để bỏ bộ lọc lỗi, và lỗi
  không tải được để thử lại. Catalog lỗi có link GET hoặc nút retry khi hydrate.
- Đọc lại có thể giữ snapshot công khai gần nhất kèm trạng thái, theo [READ-02](../../development/public-read-freshness.md).
  Khi authority nhận 404 cho một publication, cache gỡ card liên quan. Không mở rộng cơ chế
  cache công khai sang query riêng của Bàn viết.

[Discovery smoke](../../../apps/vink-web/e2e/discovery-smoke.mjs) kiểm 422 input/cursor,
SSR trước JavaScript, đóng dock bằng nút và Escape khi URL có q/tag/cursor, và thứ tự Tab/cây
trợ năng ở các mức chữ theo #427. [Author Search smoke](../../../apps/vink-web/e2e/author-search-smoke.mjs)
kiểm cùng document và node khi đổi chế độ, chờ API, Back và Forward.
