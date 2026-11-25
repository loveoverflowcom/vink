# Lưu bài · registry triển khai và bàn giao

| Chiều | Trạng thái sau #387 |
| --- | --- |
| Sản phẩm | SAV-01..04 (21/11), D1–D12, vòng đời và phạm vi DEFERRED (25/11) đã được chủ sản phẩm duyệt tại [Epic #384 §8](https://github.com/loveoverflowcom/vink/issues/384). [PRD §18](../../prd/requirements.md#18-lưu-bài-để-đọc-lại-saved-works) sở hữu quy tắc và ví dụ SAV-INV-01..05. |
| Registry quyết định | [VDR-SAVED-WORKS@r1](../../decisions/proposals/VDR-SAVED-WORKS.r1.toml) · `PENDING` · SHA-256 `7ebf41f88b6b3f91bf2aed1dd10d0f221b133ce30d55027e57d36d3d6e468b45`. Epic ghi “DUYỆT: Duyệt D1–D12 theo phương án đề xuất” và “DUYỆT: Duyệt quy tắc vòng đời dữ liệu”. Chưa có lời duyệt chỉ đích danh revision/hash này theo [quy tắc signoff](../../decisions/signoffs/VDR-DECISION-SIGNOFF.r1.toml). |
| Kiến trúc | [Contract v1](../../services/vink-app/contracts/http/v1/saved-works.md) là `DESIGN_PROPOSED` cho một owner trong Vink App. CHOICE-01 dùng một dòng bền cho mỗi cặp người lưu và tác phẩm. |
| Triển khai | [Domain Lưu bài](../../services/vink-app/src/domain/saved_work.rs) và [adapter PostgreSQL](../../services/vink-app/src/persistence/saved_works.rs) đạt mức `persistence-api`. Web thuộc #388–389. [LayoutSpec](../design/saved-works.md) cùng [delta Pen Flight](../design/pen-flight-r2.md) mô tả đích cho Reader và hồ sơ. CMP/native thuộc DEFERRED. |
| Kiểm chứng | [#385](https://github.com/loveoverflowcom/vink/issues/385) có bằng chứng tài liệu và registry. [Các ca unit #386](../../services/vink-app/src/domain/saved_work/tests.rs) chạy bằng `cargo xtask verify unit app --filter domain::saved_work`. Các [test tích hợp #387](../../services/vink-app/tests/postgres/saved_works.rs) đi qua Axum và PostgreSQL. Kết quả, SHA và giới hạn bằng chứng được ghi ở [#387](https://github.com/loveoverflowcom/vink/issues/387). |

## Truy vết quyết định và phần cần chứng minh

| PR và owner | Luật / oracle cho lát tiếp theo | Ranh giới bằng chứng cần có |
| --- | --- | --- |
| PR1 · [#386](https://github.com/loveoverflowcom/vink/issues/386) · domain | SAV-01, SAV-INV-01/03/04, D6/D7/D8/D12. Đặt desired state; phân biệt lần mới, no-op mới, retry và stale. Mô phỏng lưu → bỏ lưu → lưu, stale trái chiều, đứng sát cap và toàn bộ bảng vòng đời. | Unit thuần; lấy [reaction generation](../../services/vink-app/src/domain/discussion/reaction.rs) làm mẫu, độc lập HTTP/SQL. |
| PR1 · [#387](https://github.com/loveoverflowcom/vink/issues/387) · App/PG/HTTP | SAV-01..03, SAV-INV-01..05, D6–D10/D12. Hai WorkId tranh suất cuối, đổi standing/quyền công khai trong lúc ghi, bỏ lưu bài không đọc được, xóa chủ và fence. Hai trang xen Gone/cùng thời điểm; đổi quyền trước trang sau. So byte/ETag công khai và header riêng; kiểm không phát thông báo/ranking. | PostgreSQL thật + API v1, contract fixture thành công/lỗi; kết quả retry không làm mới saved_at. |
| PR2 · [#388](https://github.com/loveoverflowcom/vink/issues/388) · Reader | SAV-04, D1–D5/D11. Một cặp bằng nhau, “Thấm 12” cạnh Lưu; sheet trả focus, không auto-save; no-op/ACK trễ, lỗi cap và mất ACK. | Web CSR standalone và SSR/hydration đang ship; bàn phím, screen reader, hai theme, reflow theo LayoutSpec. |
| PR3 · [#389](https://github.com/loveoverflowcom/vink/issues/389) · hồ sơ | SAV-01..04 và SAV-INV-01..05. Hành trình Lưu → Đã lưu → đọc publication mới → Back → Bỏ lưu. F5, yêu cầu trực tiếp, đổi phiên, phân trang, neutral/omit, NotFound tương đương với khách/người khác. | Web + hành trình tích hợp với API thật; đối chiếu DOM/SSR, public cache và quyền tại lần đọc. |

## Giao dịch và cursor của #387

**CHOICE-01.** Domain tăng generation cả khi ý định mới trùng trạng thái. Vì retry có thể đến sau khi bỏ lưu, PostgreSQL giữ generation cùng `saved_at` trên một dòng có khóa chính `(principal_id, work_id)`. Chỉ dòng có `saved_at` nằm trong index danh sách và chiếm suất. Khóa principal của người lưu tuần tự hóa cả hai WorkId tranh suất cuối. PUT khóa con trỏ công khai trước, rồi lấy khóa principal theo thứ tự UUID để việc xóa tài khoản tác giả và người lưu có cùng thứ tự với Follow. Domain quyết định kết quả sau khi adapter đọc lại quyền và đếm trong giao dịch. [Migration](../../services/vink-app/migrations/20261204000000_saved_works.sql) · [Domain](../../services/vink-app/src/domain/saved_work.rs).

GET danh sách dùng một snapshot PostgreSQL cho standing, retirement, publication, thẻ thể loại và byline. Truy vấn lọc Gone trước khi lấy tối đa 21 dòng hiện được. Cursor ký biên thời gian và WorkId bằng khóa ngẫu nhiên của cặp, gắn với chủ và thứ tự v1. Vì fence giữ khóa qua bỏ lưu, cursor vẫn xác minh được cả khi người dùng đã bỏ lưu hoặc lưu lại bài ở cuối trang trước. [Truy vấn](../../services/vink-app/src/persistence/saved_works/list.sql) · [Cursor](../../services/vink-app/src/persistence/saved_works/cursor.rs).

Khi App ghi fence xóa tài khoản hoặc thu hồi handle theo đường cũ, trigger dọn các dòng do tài khoản đó sở hữu ngay trong giao dịch. Những người từng lưu bài của tác giả đã xóa thấy hàng trung tính theo quyền hiện tại. Việc Account/Identity phát lệnh xóa đã xác minh và cấp trạng thái chỉ hạn chế đăng/bình luận lần lượt thuộc SOC-DEP-01 và SOC-DEP-02. [Ranh giới Account](../architecture/social-graph.md).

**Bàn giao Web.** [PR #527](https://github.com/loveoverflowcom/vink/pull/527) đưa đặc tả và domain vào develop. #387 cung cấp API cho Reader #388 và hồ sơ #389. Sau khi mất ACK, consumer đọc lại đúng cặp của phiên hiện hành. [Hợp đồng và bảng lỗi](../../services/vink-app/contracts/http/v1/saved-works.md).
