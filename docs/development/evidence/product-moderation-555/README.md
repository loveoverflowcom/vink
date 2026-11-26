# Kiểm duyệt trong Product Web · #555

Ngày 26/11/2026, Chrome trên macOS, App và PostgreSQL scratch. Mốc trước là `develop@3ed0d700` sau #567. Mốc sau là `36500253`. Scenario `review` và các test Rust được chạy ở `9ff49628`. Commit kế tiếp chỉ bổ sung kiểm thử đổi tài khoản và ảnh mobile vào scenario `publication`.

## Kết quả theo acceptance

| AC | Kết quả thực tế |
| --- | --- |
| AC-01 · Route và quyền | Guest, tác giả và admin mở trực tiếp ba URL staff cũ đều nhận HTTP 404. Điều hướng qua liên kết cùng origin ở cả chín tổ hợp đều vào màn không tìm thấy. Trình duyệt không gọi API staff hoặc tra hồ sơ tác giả cho các URL này. API queue trả 401 với guest và 403 với tác giả. Admin có thể cấp quyền, nhận ca và xử lý qua API. Test PostgreSQL xác nhận một phản đối được khử trùng và chỉ admin có thể kết luận. |
| AC-02 · Quy trình tác giả | Editor lưu và gửi đúng snapshot, bản 1 vẫn công khai khi bản 2 bị từ chối. Bàn viết hiển thị lý do, lịch sử, quyền phản đối và kết quả xử lý. URL tác giả cũ về Bàn viết. Mở từ thông báo dẫn đến đúng phiên bản và quyết định lịch sử. Tài khoản staff khác đọc chi tiết riêng của tác giả nhận 404. |
| AC-03 · Đọc lại an toàn | Trong sheet bình thường không còn nút tải lại. Ngắt phản hồi sau khi server ghi phản đối, rồi gây lỗi GET đầu tiên cho kết quả đúng một POST và hai GET (tự đọc, sau đó người dùng bấm “Thử lại”). Tab cũ sau khi rút bài nhận một POST 409 và một GET 200. GET riêng bị giữ đến sau lúc đổi từ tác giả A sang B không đưa dữ liệu A trở lại màn hình và không gửi POST. Mất phiên vẫn giữ chữ chưa lưu trong editor. |
| AC-04 · Guard, artifact, SSOT | Guard định tuyến và năm đồ thị Cargo của Product đạt. Ba test của guard bao gồm negative controls. Quét cả tám file JS/CSS/WASM dev/hydrate không tìm thấy bảy dấu hiệu UI đã gỡ; tìm thấy `desk-review-retry` của tác giả. Hợp đồng response queue cũ vẫn có test trên provider. Registry, feature map và LayoutSpec hiện hành ghi rõ chủ sở hữu và đường đi cho nhân sự. |

**Hai lựa chọn đã kiểm chứng.** CHOICE-01 A gỡ màn hình staff sau khi xác minh hoạt động qua API có phân quyền. CHOICE-02 A dùng scope phiên đã xác nhận, đích đọc và thế hệ request để xử lý kết quả muộn. Nút “Thử lại” chỉ gửi GET.

## Ảnh đối chiếu

Cả hai lần chạy `cargo xtask verify web --scenario publication` đều đạt, từ đăng nhập Identity thật đến lượt assertion cuối. Ở mốc trước chỉ chèn thao tác chụp ảnh và kiểm có nút cũ vào kịch bản. Cả hai bộ ảnh chụp tác phẩm mẫu có bản 1 đang công khai, bản 2 bị từ chối và đủ điều kiện phản đối. Lịch sử đang mở.

| Khung hình | Trước · `3ed0d700` | Sau · `36500253` |
| --- | --- | --- |
| Desktop 1280 × 813 | [Sheet có nút tải lại](before-desk-review-eligible.png) | [Sheet sau thay đổi](desk-review-eligible.png) |
| Mobile 390 × 844 | [Đầu sheet trước thay đổi](before-desk-review-eligible-mobile.png) | [Đầu sheet sau thay đổi](desk-review-eligible-mobile.png) |

Ở mobile, cuối sheet nằm dưới mép khung hình. Assertion DOM kiểm riêng sự hiện diện của nút ở baseline và sự vắng mặt của nó ở mốc sau. Khi lần GET tự động gặp lỗi, sheet hiển thị nút khôi phục theo ngữ cảnh. [Ảnh lỗi đọc trên desktop](desk-review-read-error.png)

## Kiểm tra có thể chạy lại

Các scenario dùng `cargo xtask verify web --scenario publication`, `cargo xtask verify web --scenario review` và `cargo xtask verify web --scenario notifications --capture checks`. `publication` dùng Identity và các phiên độc lập. `review` dùng xác thực loopback của bộ test, App và PostgreSQL thật. `notifications` kiểm điều hướng và hợp đồng giao diện bằng transport điều khiển trong trình duyệt. Quyền được xác minh qua `publication`, API và PostgreSQL.

- `cargo xtask verify postgres app --filter work_appeal_deduplicates_and_only_admin_concludes_with_history` (1 test).
- Các test Rust `web_boundary::tests` (3), `moderation_queue_provider_envelope_and_legacy_continuation_match_fixture` (1) và `every_publication_call_ends_by_deadline` (1).
- [Danh sách file, byte, SHA-256 và dấu hiệu đã quét trong bundle dev/hydrate](artifact-dev.txt).

## Bàn giao #556

Với #552, AC-02 có bằng chứng route, navigation, API và artifact dev; AC-03/04 có bằng chứng tác giả và phục hồi ở trên. Phần kiểm duyệt của AC-01/06/07/08 có inventory, guard, phân loại bằng chứng và kiểm scoped. Ảnh staff cũ là bằng chứng lịch sử; bộ ảnh mới ở đây là Product của tác giả.

**NOT_RUN ở PR3.** Việc so sánh kích thước hai bản release cùng cấu hình, release CSR/OCI và corpus tích hợp Product/Preview thuộc #556. Các ảnh trong PR3 giới hạn ở sheet bị ảnh hưởng. Không có kết luận về kích thước hoặc hiệu năng bản phát hành từ bundle dev.
