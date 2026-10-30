# Vink skills

Các skills được viết lại từ Cantos cho giai đoạn nền tảng của Vink. Đây là hướng dẫn làm việc
trong repo; chúng không chứng minh các module hay lệnh được đề xuất đã tồn tại.
Đọc [AGENTS.md](../../AGENTS.md), rồi chỉ nạp skill và reference phù hợp với nhiệm vụ.

| Skill | Dùng khi |
| --- | --- |
| [vink-work-item](vink-work-item/SKILL.md) | Triển khai feature, issue, sửa lỗi hoặc tiếp tục một lát cắt có thể nghiệm thu |
| [vink-engineering](vink-engineering/SKILL.md) | Sửa domain, API, PostgreSQL, worker, tích hợp và chọn cách kiểm chứng |
| [vink-publication](vink-publication/SKILL.md) | Sửa revision, kiểm duyệt AI/người, ý định xuất bản, rút bài hoặc tuyển chọn |
| [vink-behavior](vink-behavior/SKILL.md) | Phân tích state/guard/race, viết hoặc review model Quint có phạm vi rõ |
| [vink-leptos-web](vink-leptos-web/SKILL.md) | Làm Leptos SSR/hydration, editor JavaScript, autosave và browser state |
| [vink-ui-design](vink-ui-design/SKILL.md) | Thiết kế/review M3 Expressive, typography tiếng Việt, trải nghiệm đọc và accessibility |
| [vink-code-review](vink-code-review/SKILL.md) | Review diff, commit, branch hoặc PR; mặc định trả findings và không sửa |
| [vink-prompt](vink-prompt/SKILL.md) | Viết prompt triển khai, chia scope, review thành prompt sửa hoặc prompt tiếp tục |

Có thể gọi bằng `$vink-work-item`, `$vink-code-review`, `$vink-prompt`… trong môi trường hỗ trợ
repo skills, hoặc yêu cầu agent đọc trực tiếp file `SKILL.md` tương ứng. Các mục `agents/openai.yaml`
cung cấp tên hiển thị và prompt mẫu; không yêu cầu plugin Cantos hay dịch vụ ngoài.

Nguồn, commit Cantos và lý do chọn từng skill nằm tại
[Skill adaptation](../../docs/references/skill-adaptation.md). Các quy tắc sản phẩm tiếp tục nằm
trong [prd/](../../prd/vision.md) và [docs/](../../docs/README.md); skills dẫn tới chủ sở hữu,
không tự chuyển decision đề xuất thành Accepted.
