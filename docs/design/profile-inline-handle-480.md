# Địa chỉ tác giả trong Chỉnh hồ sơ · #480

Delta Web ngày 24/11/2026 dựa trên [Social R2 §3](social-289.md#handle) và
[Lavender R3](lavender-r3-336/README.md). Chủ sản phẩm yêu cầu “Phần này nên để chung
trong chỉnh sửa tôi / thông tin cá nhân chứ? Không cần 1 màn riêng”.
[Nguồn yêu cầu](https://github.com/loveoverflowcom/vink/issues/480) và
[proposal r2](../../decisions/proposals/VDR-SOCIAL-GRAPH.r2.toml) (`PENDING`).
Mốc UI mới chưa có ảnh app được duyệt.

- `#dia-chi-tac-gia` cạnh bút danh. Giá trị đã đọc, số lượt, lối thiết lập hoặc đổi khi đủ quyền.
  Hết lượt hiển thị địa chỉ, trang tác giả và lịch sử liên kết. Nếu tải lỗi, Thử lại chỉ tác động hàng này.
- Một `Dialog` M3 cho setup/đổi, có tên trường, địa chỉ dạng @, quyết định quota/liên kết cũ và
  hành động xác nhận. Form dialog là anh em của form Chỉnh hồ sơ. Hủy, Escape và Back trả focus về hành
  động hoặc hàng nếu hết lượt. IME chưa commit thì Enter không gửi.
- Chọn CHOICE-01 A. Controller Handle riêng sử dụng `profile_scope` của form, không thay nháp Profile.
  Refresh Handle sau Profile ACK và khi nhận thông báo thay đổi từ tab khác. Receipt và lần đọc
  riêng xác nhận địa chỉ/quota trước khi bỏ pending. Chỉ gửi lại đúng payload sau khi chưa tìm thấy receipt.
- Chọn CHOICE-02 A. `/toi/dia-chi` trả 308 no-store tới `/toi/chinh-ho-so#dia-chi-tac-gia`,
  router cũng replace tới đích khi điều hướng được hydrate. Chính chủ thấy hàng có focus khi
  đến bằng fragment nếu chưa chọn control khác. Khách dùng đường Tài khoản của Chỉnh hồ sơ.
- Provider giữ nguyên version, quota, CAS, quyền và `/@handle`. Lưu hồ sơ dùng version khác
  với đổi địa chỉ. Giữ token Lavender R3 và thành phần chung.

Build, browser và đối chiếu responsive trên app thật `NOT_RUN` trong lượt code draft.
Kiểm chứng qua `cargo xtask verify quick` và `cargo xtask verify web --scenario password`.
