# Fixture Lượt xem — #341

[Luật](../../../../../docs/architecture/work-views.md) và
[hiện trạng, hợp đồng cho #342/#343](../../../../../docs/development/work-views.md#7-đường-ghi-341).
Dữ liệu kiểm thử của đường ghi; không phải payload HTTP. Thời gian là UTC, ngày của tổng theo
ngày tính theo `Asia/Ho_Chi_Minh`.

- `decisions.json`: 28 ca một quyết định. Mỗi ca cho người đọc, thời điểm trong UUIDv7 của sự
  kiện, đồng hồ quyết định, trạng thái công khai, receipt và cửa sổ đã có, rồi quyết định mong
  đợi (`count`, `skip`, `replay`, `refuse`) với căn cứ publication, ngày và cửa sổ được thay. Ca
  gắn E-xx của đặc tả và VIEW-ID. `G*` là mã khách (UUID v4), các tên khác là ID cố định.
  Thực thi: `domain::work_view::tests::shared_decision_vectors`.
- `scenario.json`: 26 bước dòng thời gian trên một tác phẩm (xem, gửi lại, thay revision, chuyển
  riêng tư, công khai lại, dọn hạn) với kết quả và tổng sau mỗi bước, cùng trạng thái cuối (tổng,
  tổng theo ngày, số cửa sổ và receipt còn lại, mốc bắt đầu đếm). Nhãn sự kiện được sinh UUIDv7 ở
  bước đầu tiên dùng nó và dùng lại ở các bước sau. Thực thi:
  `application::work_view_tests::shared_timeline_oracle_in_memory` (store trong bộ nhớ) và
  `work_view::shared_timeline_oracle_on_postgres` trong `tests/postgres.rs` (adapter thật,
  publication thật).

HTTP thu nhận (#343) dùng lại hai file bằng cách ánh xạ người đọc sang phiên hay cookie khách;
API đọc (#342) dùng trạng thái cuối của `scenario.json` làm oracle.
