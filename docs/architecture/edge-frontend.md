# Frontend Web độc lập trên Cloudflare

Ngày 24/11/2026, chủ sản phẩm chọn SPA Leptos CSR cho [#504](https://github.com/loveoverflowcom/vink/issues/504).
Thiết kế phát hành ở trạng thái `DESIGN_PROPOSED`. [VDR-EDGE-FRONTEND@r2](../../decisions/proposals/VDR-EDGE-FRONTEND.r2.toml)
đang `PENDING`. [Revision r1](../../decisions/proposals/VDR-EDGE-FRONTEND.r1.toml) lưu đề xuất SSR trước khi đổi hướng.

| Yêu cầu | Owner | Hành vi |
| --- | --- | --- |
| Document và deep link | Cloudflare Pages | GET/HEAD chấp nhận HTML nhận cùng index tĩnh, HTTP 200. WASM mount App/router ở URL hiện tại. Trang không tồn tại có thông báo từ router. |
| Home, tìm kiếm, thẻ, tác giả, quan hệ và Reader | Router và resource của `vink-web` | Gọi HTTP API v1. UI cập nhật metadata, pending, trang thay thế, lỗi và điều hướng alias theo câu trả lời. Khi vào lại Reader, chỉ hiển thị thân bài sau khi xác minh lại với API. |
| Tôi, Bàn viết, kiểm duyệt, tương tác theo phiên | API trên VPS và browser | Index chỉ mang cấu hình hiển thị công khai. Cookie host-only, Origin, kiểm tra phiên, quyền và version ở backend. Nội dung riêng chỉ đi vào trang sau khi được API cho phép. |
| JS/WASM/CSS/font/hình/CodeMirror | Pages Static Assets qua inventory hiện hữu | URL tuyệt đối theo revision. Import editor theo URL của module để tab cũ tiếp tục tải đúng code. Đường dẫn asset thiếu trả lỗi. |
| API và health | Worker → ingress VPS → App | Cùng origin, giữ kiểm tra ingress, IP, cookie, ETag, trạng thái và phân loại cache hiện hữu. Identity, worker nghiệp vụ và PostgreSQL ở VPS. |

## Đánh đổi của Reader

Index tĩnh có tiêu đề chung và hướng dẫn bật JavaScript. Người đọc cần tải WASM và nhận câu trả lời của API trước khi thấy văn bản. Trình thu thập chỉ đọc HTML ban đầu sẽ thấy preview chung. HTTP status của document mô tả việc tải shell. API trả trạng thái của tác phẩm, kể cả khi quyền đọc thay đổi hoặc backend ngừng phục vụ. Mốc cần đo là **văn bản đầu tiên thật sự hiển thị** bên cạnh TTFB, FCP, LCP và thời điểm tương tác. [PRD §9](../../prd/requirements.md)

## Phát hành

Giữ Pages hiện hữu và cùng origin để dùng lại hợp đồng private của API. Worker kiểm inventory cả khi phục vụ asset, nên cần đo số invocation, CPU, giới hạn dung lượng và lưu lượng trên account trước cutover. Static hosting khác có thể phục vụ cùng artifact bằng fallback cho document và proxy cùng origin cho API. Gói trả phí và việc đổi production cần quyết định riêng. [Quy trình và trạng thái](../development/edge-frontend.md)
