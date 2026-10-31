---
name: vink-ui-fix
description: >-
  Sửa một lỗi UI Vink có phạm vi rõ hoặc soạn bàn giao triển khai từ
  ảnh, finding, issue, source hoặc phương án đã chọn. Dùng cho layout, overflow,
  typography, copy, icon, focus và trình bày trạng thái; tìm owner hiện có, giữ
  semantics sản phẩm, kiểm lại cùng kịch bản. Khi không có quyền/công cụ sửa code,
  cung cấp đặc tả hoặc prompt Vink có nguồn; không gọi bàn giao là bản sửa đã chạy.
---

# Vink UI Fix

Giải quyết một lỗi có giới hạn hoặc các biểu hiện đã có bằng chứng cùng nguyên
nhân. Với redesign toàn màn chưa có hướng chọn, dùng
[Vink UI Design](../vink-ui-design/SKILL.md). Với report-only, dùng
[Vink UI Inspector](../vink-ui-inspector/SKILL.md).

Đọc [repair và bàn giao](references/repair-and-handoff.md),
[ngữ cảnh Vink](../vink-ui-design/references/product-context.md),
[tiêu chí UI](../vink-ui-design/references/design-system.md) và
[quy trình Chat](../vink-ui-design/references/chat-workflow.md) theo phần liên quan.
Đọc các reference tương ứng trong checkout hiện hành.

## 1. Chốt bug contract từ dữ liệu đã có

Ghi actual/expected, screen/region/state, ảnh hoặc finding ID, source/build nếu
biết, ảnh hưởng người dùng, phạm vi đã giao và cách tái hiện. Suy ra đường dẫn từ
repo; không đòi user biết filename. Phân biệt symptom quan sát với root cause chưa
xác minh. Thiếu runtime không ngăn việc đọc nguồn hoặc soạn repair spec có ích.

Chọn kết quả đúng yêu cầu:

- **Sửa trực tiếp:** khi user đã giao sửa và có nguồn/công cụ phù hợp, thực hiện
  patch thực trong repo đúng, giữ thay đổi có sẵn của user.
- **Bàn giao/spec/prompt:** khi user yêu cầu bàn giao hoặc công cụ/quyền không hỗ
  trợ patch, trả hướng sửa cụ thể, owner đã biết, acceptance và kiểm tra cần chạy.
  Nói rõ chưa sửa code; không chỉ trả một lời hứa làm sau.

## 2. Xác định owner và nghĩa trạng thái

Đọc current source theo [bản đồ nguồn](../vink-ui-design/references/source-map.md):
caller/component, CSS/token, copy, presentation mapping, save state/API. Giữ
`--vk-*` ở owner CSS và `EditorSession`/state authority hiện có khi liên quan.
Ảnh không chứng minh component code; owner map đóng gói là điểm bắt đầu tìm.

Nếu dữ liệu/domain decision sai, đừng che bằng CSS hoặc điều kiện UI. Nêu phần
nghiệp vụ cần sửa và tiếp tục phần độc lập trong phạm vi user đã giao. Những thay
đổi quyền, privacy, publication, durability hoặc schema cần đúng owner và bằng
chứng; một task UI không tự chốt chúng.

## 3. Sửa ở ranh giới nhỏ nhất

Giữ component hiện có, semantic tokens, chữ tiếng Việt, states và accessibility.
Không thêm renderer/native stack, dependency hoặc rewrite toàn màn để sửa một
spacing. Không biến “gọn hơn” thành giấu blocker, lỗi hoặc chưa lưu trong tooltip.
Giữ dữ liệu nhập khi lỗi/xung đột, focus/selection và IME composition.

Với phương án user đã chọn, chuyển đúng layout/hành vi/states sang owner Vink;
không tự chuyển concept thành tính năng backend đã tồn tại. Với shared token,
xác định consumer bị ảnh hưởng và các theme cần xem lại.

## 4. Kiểm tra theo rủi ro thực

Chọn check rẻ nhất có ý nghĩa. Layout/copy thuần cần source/render review tương
ứng; không thêm unit test chỉ để lặp một CSS literal. State mapping, keyboard,
stale save acknowledgment hoặc conflict có thể cần test hành vi tập trung.

Với render thật, giữ fixture, state, viewport, theme, locale, text scale và input
sequence giữa before/after; mở ảnh mới. Không dùng mockup tạo sinh thay screenshot
để báo fix đã đạt. Nếu không có baseline, nêu đó là kiểm sau sửa, không gọi so sánh
before/after. Giữ reference/comparison policy đã chọn cho các lần recheck.

Lệnh lấy từ `docs/development/README.md` hiện hành. Không bịa command hoặc áp gate
Cantos; không chạy Cargo chỉ để kiểm tài liệu. Ghi riêng PASS, FAIL, BLOCKED,
NOT_RUN theo từng check; test đã fail không đổi thành “chưa kiểm” để hoàn tất.

## 5. Trả kết quả hoặc bàn giao

Nêu lỗi, điều sửa, chủ nguồn, lý do và bằng chứng đã có; tách phần còn thiếu như
mobile IME, screen reader hoặc runtime. Handoff phải tự đủ ngữ cảnh, có acceptance
và branch xử lý blocker; nếu user muốn SudoLang, dùng mẫu Vink đã được nguồn định
nghĩa trong reference. Không tự post issue/comment, push, merge, deploy hay publish
khi mới có yêu cầu spec/local edit. Giữ ủy quyền đã có, không hỏi lại một cách máy móc.

## References

- [Repair và handoff](references/repair-and-handoff.md): bug contract, source owners,
  verification lanes và mẫu `VINK_PROMPT_INPUT_V1` cho design-to-code.
- [Component audit](../vink-ui-inspector/references/component-audit.md): giữ finding
  ID, vùng so sánh, severity/confidence và scenario khi nhận báo cáo review.
