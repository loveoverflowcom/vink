---
name: vink-ui-capture
description: >-
  Chụp và lập báo cáo hiện trạng UI/UX Vink từ app, component và preview đang chạy;
  ghi navigation, states, tokens, nguồn và coverage để bàn giao review hoặc upload
  Drive. Dùng khi cần snapshot triển khai có thể tái hiện; review ảnh có sẵn dùng
  vink-ui-inspector, sửa UI dùng vink-ui-fix.
---

# Vink UI Capture

Tạo bộ bằng chứng của checkout/runtime được giao, có ảnh thật, metadata và khoảng
trống rõ ràng. Đọc [ma trận thu thập](references/capture-surfaces.md) khi khám phá
surface và [contract bàn giao](references/evidence-contract.md) trước khi ghi output.

## Phạm vi và nguồn

- Pin repo, branch, full SHA, dirty status và runtime/build thực tế trước khi chụp.
  Mặc định đối chiếu `develop`, nhưng không checkout đè công việc hiện tại. Nếu user
  giao branch/worktree khác, chụp đúng nó. App đang chạy khác checkout hoặc không
  xác minh được build phải ghi rõ; không gắn SHA đọc từ Git cho process chưa xác định.
- Khám phá route, navigation, component, catalog/scenario và token từ source hiện
  hành. Giữ ID qua các run; không biến PRD hay ảnh thiết kế thành màn đã triển khai.
- Tách `product`, `component-preview`, `screen-recipe`, `diagnostic-host` và
  `design-reference`. Web responsive, CMP Desktop/off-screen và Android/iOS là các
  môi trường riêng. Compile native không chứng minh có app native chạy được.
- Dùng dữ liệu tổng hợp và fixture cục bộ được repo hỗ trợ. `VINK_DEV_PRINCIPAL`
  là local fixture, không phải đăng nhập thật. Auth/login chưa có ghi
  `NOT_IMPLEMENTED`; không dựng auth giả để đủ ma trận.

## Thu thập

1. Lập inventory và ma trận dự kiến trước khi chạy. Với scope toàn UI, liệt kê cả
   màn app, overlay/dialog, component/catalog, preview, recipes và token owner;
   chọn các biến thể theo [ma trận](references/capture-surfaces.md). Mục chưa chụp
   vẫn nằm trong manifest với lý do. Không gọi một mẫu nhỏ là toàn bộ coverage.
2. Dùng setup, runner và capture hiện có sau khi đọc
   [hướng dẫn phát triển](../../../docs/development/README.md). Chỉ chuẩn bị runtime
   cục bộ cần thiết; không reset DB, đổi fixture/seed dùng chung hoặc chạy migration
   trên môi trường khác để tạo trạng thái đẹp. Không thêm capture runner vào repo
   chỉ vì lần chụp này. Thiếu browser/emulator/fixture thì tiếp tục phần độc lập.
3. Đợi app-ready/hydration, font và layout ổn định; ghi điều kiện chờ, motion, thời
   điểm, viewport/density/text scale, theme yêu cầu và theme hiệu lực. Chụp PNG
   viewport; thêm full-page khi phần cuộn có ý nghĩa, giữ hai loại riêng.
4. Tái hiện click-path và ghi trạng thái navigation trước/sau: route, selected
   destination, rail/bar, overlay stack, Back/Escape/close và guard đang áp dụng.
   Cạnh thấy qua tương tác là `observed`; cạnh chỉ đọc source là `source-only`.
   Không suy ra lưu dữ liệu, accessibility hay IME thành công từ một ảnh.
5. Với component/preview, giữ catalog ID + scenario của repo, renderer và source
   owner; liên kết nơi dùng trong màn app khi xác minh được. Recipe minh họa vẫn
   mang nhãn mẫu. Với tokens, lưu snapshot nguồn/version/hash và giá trị hiệu lực
   đọc được từ renderer, override/scope/đơn vị. Không sửa token để khớp ảnh thiết kế.
6. Mở từng ảnh dùng trong báo cáo; kiểm không trắng, sai màn, sai theme hoặc lẫn
   prototype. `CAPTURED` chỉ nói đã có ảnh; ghi riêng ảnh đã mở hay chưa.
   Ảnh lỗi thật giữ đúng state lỗi, không gắn nhãn default/success.

## Output và bàn giao

Ghi output mặc định tại `target/ui-evidence/<run_id>/` hoặc thư mục user chỉ định,
không commit binary/log/draft riêng vào repo. Theo
[contract](references/evidence-contract.md), tạo `manifest.json`,
`reports/REPORT.md`, `reports/navigation.mmd` + adjacency list, ảnh và snapshot
nguồn UI an toàn. Báo cáo tiếng Việt, ghi số dự kiến/đã chụp/đã mở và các gap theo
surface, renderer, theme, state; kèm cách tái hiện, vị trí component và token owner.

Nếu user giao upload Drive hoặc cả capture → upload, tiếp tục
[$vink-ui-evidence-drive](../vink-ui-evidence-drive/SKILL.md) với chính bundle này,
không hỏi lại quyền đã có. Nếu chỉ giao capture, trả đường dẫn local và bước upload
phù hợp. Không tự xoá bundle/worktree khi bản local còn là bản duy nhất.

Đối chiếu thiết kế/audit chuyên sâu thuộc
[$vink-ui-inspector](../vink-ui-inspector/SKILL.md). Nếu user cũng giao compare,
tiếp tục bằng manifest và reference đúng phạm vi; không tự sửa UI, tạo issue,
merge, deploy hay đăng bình luận từ một yêu cầu capture.
