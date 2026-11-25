# Lưu bài · registry triển khai và bàn giao

| Chiều | Trạng thái tại lát SSOT #385 |
| --- | --- |
| Sản phẩm | SAV-01..04 (21/11), D1–D12, vòng đời và phạm vi DEFERRED (25/11) đã được chủ sản phẩm duyệt tại [Epic #384 §8](https://github.com/loveoverflowcom/vink/issues/384). [PRD §18](../../prd/requirements.md#18-lưu-bài-để-đọc-lại-saved-works) sở hữu quy tắc và ví dụ SAV-INV-01..05. |
| Registry quyết định | [VDR-SAVED-WORKS@r1](../../decisions/proposals/VDR-SAVED-WORKS.r1.toml) · `PENDING` · SHA-256 `7ebf41f88b6b3f91bf2aed1dd10d0f221b133ce30d55027e57d36d3d6e468b45`. Epic ghi “DUYỆT: Duyệt D1–D12 theo phương án đề xuất” và “DUYỆT: Duyệt quy tắc vòng đời dữ liệu”. Chưa có lời duyệt chỉ đích danh revision/hash này theo [quy tắc signoff](../../decisions/signoffs/VDR-DECISION-SIGNOFF.r1.toml). |
| Kiến trúc | [Contract v1](../../services/vink-app/contracts/http/v1/saved-works.md) là `DESIGN_PROPOSED` cho một owner trong Vink App. Cách lưu fence qua bỏ lưu còn mở ở #386–387. |
| Triển khai | `NOT_IMPLEMENTED` cho core, persistence/API và Web. [LayoutSpec](../design/saved-works.md) cùng [delta Pen Flight](../design/pen-flight-r2.md) mô tả đích cho Reader và hồ sơ. CMP/native thuộc DEFERRED. |
| Kiểm chứng | #385 chỉ kiểm tài liệu và registry. Unit, PostgreSQL, Web và hành trình tích hợp chưa chạy cho Lưu bài; kết quả thực thi theo commit ghi tại [#385](https://github.com/loveoverflowcom/vink/issues/385). |

## Truy vết quyết định và phần cần chứng minh

| PR và owner | Luật / oracle cho lát tiếp theo | Ranh giới bằng chứng cần có |
| --- | --- | --- |
| PR1 · [#386](https://github.com/loveoverflowcom/vink/issues/386) · domain | SAV-01, SAV-INV-01/03/04, D6/D7/D8/D12. Đặt desired state; phân biệt lần mới, no-op mới, retry và stale. Mô phỏng lưu → bỏ lưu → lưu, stale trái chiều, đứng sát cap và toàn bộ bảng vòng đời. | Unit thuần; lấy [reaction generation](../../services/vink-app/src/domain/discussion/reaction.rs) làm mẫu, độc lập HTTP/SQL. |
| PR1 · [#387](https://github.com/loveoverflowcom/vink/issues/387) · App/PG/HTTP | SAV-01..03, SAV-INV-01..05, D6–D10/D12. Hai WorkId tranh suất cuối, đổi standing/quyền công khai trong lúc ghi, bỏ lưu bài không đọc được, xóa chủ và fence. Hai trang xen Gone/cùng thời điểm; đổi quyền trước trang sau. So byte/ETag công khai và header riêng; kiểm không phát thông báo/ranking. | PostgreSQL thật + API v1, contract fixture thành công/lỗi; kết quả retry không làm mới saved_at. |
| PR2 · [#388](https://github.com/loveoverflowcom/vink/issues/388) · Reader | SAV-04, D1–D5/D11. Một cặp bằng nhau, “Thấm 12” cạnh Lưu; sheet trả focus, không auto-save; no-op/ACK trễ, lỗi cap và mất ACK. | Web CSR standalone và SSR/hydration đang ship; bàn phím, screen reader, hai theme, reflow theo LayoutSpec. |
| PR3 · [#389](https://github.com/loveoverflowcom/vink/issues/389) · hồ sơ | SAV-01..04 và SAV-INV-01..05. Hành trình Lưu → Đã lưu → đọc publication mới → Back → Bỏ lưu. F5, yêu cầu trực tiếp, đổi phiên, phân trang, neutral/omit, NotFound tương đương với khách/người khác. | Web + hành trình tích hợp với API thật; đối chiếu DOM/SSR, public cache và quyền tại lần đọc. |

**Handoff PR1.** Thứ tự là #385 → #386 → #387 trên cùng nhánh tích hợp của Dot. Sau đó PR2 Reader, PR3 Profile và nghiệm thu tích hợp. #376–381 là bộ issue lịch sử trùng; canonical #384–389 theo Epic §9. Trong preview r2, B/G2/đổi nhãn là các nhánh khám phá đã được D1–D3 thay thế. Mốc hình học Thấm bên trong cặp được D5 cập nhật; glyph, count theo revision và quyền Thấm tiếp tục do LVE sở hữu.

**CHOICE-01 kỹ thuật còn mở.** #386–387 có thể dùng trạng thái NotSaved bền cùng owner hoặc tách fence/receipt trong cùng giao dịch. Với cả hai, chỉ membership Saved tính cap, fence có thứ tự do server cấp và tồn tại qua bỏ lưu; cùng một ý định đã chấp nhận không được áp dụng lần hai. Xóa người lưu phải dọn cả dữ liệu chống retry. Việc chọn bảng, khóa và chiến lược giao dịch cần có bằng chứng PostgreSQL.
