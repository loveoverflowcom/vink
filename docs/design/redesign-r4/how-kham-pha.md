# HOW · Khám phá tìm, lọc và quay lại kết quả

Source tại `develop@2712145b08a6a4c7a12c1d57033ecd2a5bd33b26`. Người đọc mở `/?q=...`,
thêm một thẻ rồi vào Reader. Đường dẫn giữ ngữ cảnh đã áp dụng, kể cả khi mở trực tiếp hoặc
dùng Back. Chữ đang soạn và lựa chọn chưa áp dụng sống ở giao diện.

1. [Home / DiscoveryPage](../../../apps/vink-web/src/pages/discovery.rs#L193) đọc mode,
   q, tag và cursor từ router. `DiscoveryScope` dispatch Works, Following hoặc Authors.
   [DiscoveryChrome](../../../apps/vink-web/src/pages/discovery.rs#L404) khởi đầu buffer từ
   q. Dock ở Tất cả mở theo context q, hash tìm hoặc trạng thái mở. [ExploreModes](../../../apps/vink-web/src/pages/discovery.rs#L326)
   render link thật cho nhóm và menu, giữ `aria-current` và flag của Đang theo dõi.
2. [works_resource](../../../apps/vink-web/src/api.rs#L579) ở SSR gọi `SharedPublicReads` trực
   tiếp. [PublicReadsAdapter::works](../../../services/vink-app/src/http/ssr.rs#L152) gọi
   [discover_works](../../../services/vink-app/src/application/discovery.rs#L176) để đọc snapshot
   công khai qua store. SSR không gọi HTTP vòng về chính server. Trình duyệt hydrate dùng
   resource và adapter public-read. Lần điều hướng sau có thể đọc HTTP `GET /api/v1/works`
   qua [gateway](../../../services/vink-app/src/http/api.rs#L363) với chính sách
   [READ-01/02](../../development/public-read-freshness.md).
3. [Search submit](../../../apps/vink-web/src/pages/discovery.rs#L537) thay q, reset cursor và
   điều hướng. [FilterForm](../../../apps/vink-web/src/pages/discovery.rs#L943) dùng GET để Áp
   dụng, với tag draft tách riêng. Preview lựa chọn và “Bỏ chọn” trong sheet thay draft; thay đổi
   áp dụng mới thành URL. Giữ tối đa 4 thẻ theo AND ở [hợp đồng discovery](../../development/discovery-api.md).
4. [Rows](../../../apps/vink-web/src/pages/discovery.rs#L1200) dựng liên kết Reader mang ngữ
   cảnh. [Registry Khám phá](../../development/discovery-ui.md) mô tả context quay lại, focus hàng
   và đích dự phòng khi hàng biến mất. [Đóng dock](../../../apps/vink-web/src/pages/discovery.rs#L465)
   tạo href bỏ q/cursor, giữ các tag và khôi phục focus/scroll theo ngữ cảnh.

## Failure và phục hồi

- Không kết quả vì query/tag khác kho công khai rỗng. Trạng thái có link xóa tìm/xóa lọc phù hợp.
  [DiscoveryFailure](../../../apps/vink-web/src/pages/discovery.rs#L1272) phân loại 422 cursor
  không dùng được để quay về trang đầu với cùng bộ lọc, 422 đầu vào để bỏ bộ lọc lỗi, và lỗi
  không tải được để thử lại. Catalog lỗi có link GET hoặc nút retry khi hydrate.
- Đọc lại có thể giữ snapshot công khai gần nhất kèm trạng thái, theo [READ-02](../../development/public-read-freshness.md).
  Khi authority nhận 404 cho một publication, cache gỡ card liên quan. Không mở rộng cơ chế
  cache công khai sang query riêng của Bàn viết.

[Smoke source](../../../apps/vink-web/e2e/browser-smoke.mjs#L1715) phân biệt 422 input/cursor và
đọc SSR trước JS. [#427](https://github.com/loveoverflowcom/vink/issues/427) giữ các ca chưa đủ
bằng chứng: deep link q rồi đóng trên SSR/hydrate, một bộ link hiện trong Tab/AX, ngưỡng sau
#413 và tín hiệu active rail. Browser và ảnh ở source mới `NOT_RUN`.
