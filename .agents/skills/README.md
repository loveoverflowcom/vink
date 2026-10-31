# Vink skills

Các skills được viết lại từ Cantos cho Vink và cập nhật theo skeleton đã triển khai. Skills là
phương pháp làm việc; yêu cầu nằm trong [prd/](../../prd/vision.md), quyết định trong
[docs/architecture](../../docs/architecture/decisions.md), còn cấu trúc code, lệnh và giới hạn
hiện tại nằm ở [docs/development](../../docs/development/README.md) — nguồn lệnh duy nhất.
Đọc [AGENTS.md](../../AGENTS.md), rồi chỉ nạp skill và reference phù hợp với nhiệm vụ.

| Skill | Dùng khi |
| --- | --- |
| [vink-work-item](vink-work-item/SKILL.md) | Triển khai feature, issue, sửa lỗi hoặc tiếp tục một lát cắt có thể nghiệm thu |
| [vink-engineering](vink-engineering/SKILL.md) | Sửa domain, application, PostgreSQL, API/contract, worker, tích hợp và chọn cách kiểm chứng |
| [vink-publication](vink-publication/SKILL.md) | Sửa revision, kiểm duyệt AI/người, ý định xuất bản, rút bài hoặc tuyển chọn |
| [vink-behavior](vink-behavior/SKILL.md) | Phân tích state/guard/race, viết hoặc review model Quint có phạm vi rõ |
| [vink-leptos-web](vink-leptos-web/SKILL.md) | Làm Leptos SSR/hydration, route, Bàn viết, editor, autosave và browser state |
| [vink-ui-design](vink-ui-design/SKILL.md) | Thiết kế/review M3 Expressive, token, typography tiếng Việt, trải nghiệm đọc và accessibility |
| [vink-ui-fix](vink-ui-fix/SKILL.md) | Sửa lỗi UI có phạm vi rõ hoặc bàn giao thiết kế để triển khai |
| [vink-ui-inspector](vink-ui-inspector/SKILL.md) | Kiểm tra UI theo từng thành phần, kịch bản và bằng chứng |
| [vink-code-review](vink-code-review/SKILL.md) | Review diff, commit, branch hoặc PR; mặc định trả findings và không sửa |
| [vink-prompt](vink-prompt/SKILL.md) | Viết prompt triển khai, chia scope, review thành prompt sửa hoặc prompt tiếp tục |

## Định tuyến nhanh

| Tình huống | Skill | Điều cần giữ |
| --- | --- | --- |
| Viết prompt cho agent khác | `vink-prompt` | Chỉ soạn prompt; không sửa code, commit, push hay đăng bình luận |
| Review diff/PR | `vink-code-review` | Findings trước, xếp theo mức độ; chỉ sửa khi người dùng giao thêm việc sửa |
| Dựng hoặc mở rộng skeleton | `vink-work-item` + `vink-engineering`; thêm `vink-leptos-web`/`vink-ui-design` khi chạm web | Tiếp tục từ code hiện có, không scaffold bản cạnh tranh; chạy lane theo phần đã đổi |
| Audit ảnh hoặc giao diện | `vink-ui-inspector` | Mở bằng chứng, báo finding và coverage; review không tự cấp quyền sửa |
| Sửa lỗi UI có phạm vi rõ | `vink-ui-fix` (+ `vink-leptos-web` khi chạm component) | Xác định owner, giữ nghĩa trạng thái, kiểm cùng kịch bản |
| Đổi copy hoặc layout | `vink-ui-design` (+ `vink-leptos-web` nếu đổi component) | Không cần model Quint; mở ảnh ở viewport/theme liên quan, chạy `verify quick` và `verify web` nếu hành vi đổi |
| Sửa lưu nháp, version, xung đột | `vink-engineering` (+ `vink-leptos-web` cho UI) | CAS cần `verify postgres`; máy trạng thái UI cần unit test và `verify web` |
| Thiếu Discourse, PostgreSQL hoặc Chrome | Skill đang dùng + [verification](vink-engineering/references/verification.md) | Làm phần độc lập; báo `FAIL`/`NOT_RUN` cho lớp chưa chạy; không báo integration `PASS` |
| Revision, moderation, publish, withdraw | `vink-publication` + `vink-engineering` (+ `vink-behavior` khi có race) | Không đổi D-xx thành Accepted; guard kiểm lại tại commit |

Có thể gọi bằng `$vink-work-item`, `$vink-code-review`, `$vink-prompt`… trong môi trường hỗ trợ
repo skills, hoặc yêu cầu agent đọc trực tiếp file `SKILL.md` tương ứng. Các mục `agents/openai.yaml`
cung cấp tên hiển thị và prompt mẫu; không yêu cầu plugin Cantos hay dịch vụ ngoài. Đây là bản
canonical duy nhất: runtime khác nên tham chiếu thư mục này thay vì chép thêm một bộ.
`cargo xtask verify quick` kiểm frontmatter, `agents/openai.yaml`, danh sách ở bảng trên và link.

Nguồn, commit Cantos và lý do chọn từng skill nằm tại
[Skill adaptation](../../docs/references/skill-adaptation.md). Skills dẫn tới chủ sở hữu và
không tự chuyển decision đề xuất thành Accepted.
