---
name: vink-shape
description: Định hình tính năng Vink, phát hiện khoảng trống nghiệp vụ, phân loại quyết định sản phẩm vs lựa chọn kỹ thuật và hỏi chủ sản phẩm đúng điểm cần chốt. Dùng cho ý tưởng mới, thay đổi UX/quyền/trạng thái/dữ liệu hoặc khi skill khác gặp quy tắc OPEN/mâu thuẫn.
---

# Vink shape

Làm rõ nghiệp vụ trước khi đặc tả hoặc triển khai. Bắt đầu từ một tình huống sử dụng cụ thể:
**Ai đang làm gì, với đối tượng nào, mong đợi kết quả gì, và điều gì xảy ra khi thao tác không thành công?**
Tra ID hoặc miền liên quan trong [kho TOML](../../../decisions/README.md). Đối chiếu kết quả
với code hiện hành, [requirements](../../../prd/requirements.md),
[decision log](../../../docs/architecture/decisions.md),
[registry triển khai và bằng chứng](../../../docs/development/product-decisions-registry.md) và
[bản đồ miền Vink](references/domain-lenses.md). Chỉ dùng revision lịch sử để giải thích bối cảnh.
Khi kết quả là `OPEN`, giữ câu hỏi và ghi rõ phần việc phụ thuộc vào lựa chọn đó.

Không biến mọi tác vụ thành thủ tục hỏi đáp: một bản sửa lỗi giao diện rõ nghĩa, refactor kỹ thuật
hoặc lát cắt đã có đủ quyết định đi thẳng tới [vink-work-item](../vink-work-item/SKILL.md) hoặc
[vink-ui-fix](../vink-ui-fix/SKILL.md).

## 1. Quét theo sáu nhóm, chỉ đào sâu nhóm liên quan

Đây là khung suy xét của agent; chỉ chuyển thành câu hỏi cho chủ sản phẩm những điểm thực sự tác
động đến phần việc đang làm:

| Nhóm | Những điều cần làm rõ |
| --- | --- |
| Mục tiêu và phạm vi | Người dùng đang giải quyết việc gì? Bản đầu phải làm tới đâu? Điều gì được hoãn (`DEFERRED`)? |
| Quyền và trách nhiệm | Ai được thực hiện hành động? Trên nội dung của ai? Trong điều kiện nào? Ai được thay đổi quyết định trước đó? |
| Vòng đời nội dung | Sửa, gửi, duyệt, công khai, chuyển riêng tư, xóa, khôi phục có quan hệ thế nào? |
| Trải nghiệm | Người dùng thấy gì, làm gì tiếp, xử lý chờ/lỗi/xung đột/mất kết nối ra sao? |
| Dữ liệu | Thu thập gì, vì mục đích gì, ai được đọc, có gửi ra ngoài (như AI provider) không, lưu và xóa thế nào? |
| Vận hành | Ai xử lý phần AI không quyết định được? Hệ thống có thực sự gửi thông báo, có hàng đợi và có cam kết phản hồi không? |

## 2. Phân biệt quyết định của chủ sản phẩm với lựa chọn kỹ thuật của agent

| Loại vấn đề | Cách xử lý |
| --- | --- |
| Đã có quyết định rõ, còn hiệu lực (`PRODUCT_CONFIRMED`, ADR) | Áp dụng và dẫn nguồn chính xác |
| Lựa chọn kỹ thuật thông thường trong phạm vi đã giao | Agent tự quyết dựa trên code, ranh giới kiến trúc và bằng chứng |
| Chi tiết có thể đảo ngược, không đổi quyền hoặc hành vi chính | Chọn theo chuẩn hiện có, ghi giả định (`assumptions`) kèm điều kiện xem lại |
| Lựa chọn làm đổi quyền, phạm vi công khai, nghĩa dữ liệu hoặc hành vi chính | Hỏi chủ sản phẩm nếu chưa có quyết định (`open_product`) |
| Điểm mở thuộc tính năng chưa làm tới | Ghi lại cùng điều kiện cần xem xét, tiếp tục phần độc lập hiện tại |
| Hai nguồn quyết định hoặc code và PRD mâu thuẫn | Đối chiếu nguồn, thời điểm và phạm vi thay thế; chỉ hỏi phần mâu thuẫn còn lại |

Ví dụ: thêm một bảng PostgreSQL hay chọn cấu trúc index là lựa chọn kỹ thuật của agent; nhưng việc
bảng đó lưu toàn văn nháp riêng, ai được đọc và dữ liệu có gửi sang provider AI hay không là quyết
định sản phẩm cần căn cứ rõ. Một baseline kỹ thuật trong pure core cho mục đang `OPEN` (như `O-04`
về bình luận chờ duyệt khi xóa bài, hay `O-03` trên tombstone trước khi `O-03@r2` chốt, hay `O-12` trước khi
`O-12@r2` chốt cách đếm ký tự mật khẩu)
**không** có nghĩa chủ sản phẩm đã chốt; khi làm tới tính năng phụ thuộc trực tiếp vào nó, phải nêu
rõ và hỏi chủ sản phẩm.

Giữ nguyên các quyết định nền tảng đã chốt trừ khi chủ sản phẩm yêu cầu xem lại: hai mức riêng tư
(“Chỉ mình tôi” / “Công khai”), quản trị viên đứng trên kết quả của trợ lý AI (`MOD-09`), nháp riêng
không gửi sang AI chỉ vì vừa lưu (`MOD-05`, `INV-08`), và nhóm tuổi / nhiều chương / Tạp Chí đang
hoãn (`DEFERRED`).

## 3. Cách hỏi: một tình huống, hai phương án, một đề xuất

Khi cần hỏi chủ sản phẩm, chỉ hỏi **1–3 câu mỗi lượt**, ưu tiên câu có tác động lớn nhất trước (dùng
công cụ hỏi trắc nghiệm của môi trường nếu phù hợp, hoặc trình bày ngắn gọn bằng văn bản). Mỗi câu
phải nêu:

1. **Một tình huống cụ thể của Vink** (không dùng câu hỏi trừu tượng kiểu “bạn muốn mô hình trạng thái nào?”).
2. **Hai phương án (A / B)** kèm hệ quả dễ thấy với người đọc, tác giả hoặc quản trị viên.
3. **Một khuyến nghị** kèm lý do gắn với nguyên tắc Vink hiện có.

Ví dụ chuẩn:
> **Tình huống:** Một bình luận đã có 20 lượt “Hữu ích”. Sau đó người viết sửa nội dung bình luận và bản sửa được duyệt.
> - **Phương án A:** Giữ nguyên 20 lượt “Hữu ích” trên bình luận sau khi sửa (phù hợp sửa chính tả, nhưng nếu đổi hẳn ý thì 20 lượt cũ tạo cảm giác cộng đồng đồng tình với nội dung mới).
> - **Phương án B (Khuyến nghị):** Gắn “Hữu ích” với đúng phiên bản đã được ghi nhận; khi bản sửa mới hiển thị công khai, số đếm công khai bắt đầu lại cho phiên bản đó, còn dữ liệu cũ vẫn giữ để đối chiếu (nhất quán với `LVE-FR-04` và `O-06`).

## 4. Đầu ra và chuyển tiếp

Kết thúc bước định hình bằng bản tóm tắt gọn khớp với hợp đồng bàn giao `DECISIONS` của
[VINK_PROMPT_INPUT_V1](../vink-prompt/references/prompt-template.md):

- **Phạm vi tính năng & tình huống chính:** làm gì ở lượt này, hoãn gì.
- **`confirmed`:** các quy tắc đã chốt và nguồn dẫn.
- **`open_product`:** câu hỏi thuộc quyền quyết định của chủ sản phẩm (`decision_owner: product`,
  `affected_requirements`, `blocks`).
- **`open_technical` & `assumptions`:** lựa chọn kỹ thuật do agent xử lý và giả định có thể đảo ngược.

Khi chủ sản phẩm đã trả lời hoặc khi cần ghi nhận kết quả định hình vào tài liệu sở hữu, chuyển sang
[vink-specify](../vink-specify/SKILL.md). Nếu tác vụ đã giao gồm thiết kế, triển khai hoặc viết
prompt, tiếp tục qua [vink-architect](../vink-architect/SKILL.md),
[vink-ui-design](../vink-ui-design/SKILL.md), [vink-work-item](../vink-work-item/SKILL.md) hoặc
[vink-prompt](../vink-prompt/SKILL.md) mà không dừng lại chờ xin phép thêm cho phần việc độc lập.
