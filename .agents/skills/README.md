# Vink skills

Các skills được viết riêng cho Vink, kế thừa có chọn lọc hướng dẫn Cantos và draft về workflow
coding agent, rồi đối chiếu với skeleton đã triển khai. Skills là phương pháp làm việc;
yêu cầu nằm trong [prd/](../../prd/vision.md), quyết định trong
[docs/architecture](../../docs/architecture/decisions.md), còn cấu trúc code, lệnh và giới hạn
hiện tại nằm ở [docs/development](../../docs/development/README.md) — nguồn lệnh duy nhất.
Đọc [AGENTS.md](../../AGENTS.md), rồi chỉ nạp skill và reference phù hợp với nhiệm vụ.

| Skill | Dùng khi |
| --- | --- |
| [vink-how](vink-how/SKILL.md) | Lần theo và giải thích cơ chế đang có từ UI/API tới nơi giữ dữ liệu và kiểm thử |
| [vink-why](vink-why/SKILL.md) | Tìm lý do thiết kế, phương án từng cân nhắc và điều kiện xem xét lại từ nguồn có dẫn chứng |
| [vink-recall](vink-recall/SKILL.md) | Tìm lại ngữ cảnh chat/handoff liên quan rồi đối chiếu với checkout hiện tại |
| [vink-architect](vink-architect/SKILL.md) | So sánh thiết kế cho thay đổi lớn và chia lát cắt có thể kiểm chứng |
| [vink-prototype](vink-prototype/SKILL.md) | Thử một giả thuyết hoặc phần của S-01–S-05 trong phạm vi rõ, ghi kết quả và giới hạn |
| [vink-maintain-verification](vink-maintain-verification/SKILL.md) | Bảo trì bản đồ tính năng, coverage và hướng dẫn/công cụ kiểm chứng theo code thật |
| [vink-work-item](vink-work-item/SKILL.md) | Triển khai feature, issue, sửa lỗi hoặc tiếp tục một lát cắt có thể nghiệm thu |
| [vink-engineering](vink-engineering/SKILL.md) | Sửa domain, application, PostgreSQL, API/contract, worker, tích hợp và chọn cách kiểm chứng |
| [vink-publication](vink-publication/SKILL.md) | Sửa revision, kiểm duyệt AI/người, ý định xuất bản, rút bài hoặc tuyển chọn |
| [vink-behavior](vink-behavior/SKILL.md) | Phân tích state/guard/race, viết hoặc review model Quint có phạm vi rõ |
| [vink-leptos-web](vink-leptos-web/SKILL.md) | Làm Leptos SSR/hydration, route, Bàn viết, editor, autosave và browser state |
| [vink-layout-spec](vink-layout-spec/SKILL.md) | Mô tả cây bố cục, responsive/theme/state trước sinh UI; dùng chung cho thiết kế, sửa nhỏ và đối chiếu |
| [vink-ui-design](vink-ui-design/SKILL.md) | Thiết kế/review M3 Expressive, token, typography tiếng Việt, trải nghiệm đọc và accessibility |
| [vink-ui-fix](vink-ui-fix/SKILL.md) | Sửa lỗi UI có phạm vi rõ hoặc bàn giao thiết kế để triển khai |
| [vink-ui-inspector](vink-ui-inspector/SKILL.md) | Kiểm tra UI theo từng thành phần, kịch bản và bằng chứng |
| [vink-code-review](vink-code-review/SKILL.md) | Review diff, commit, branch hoặc PR; mặc định trả findings và không sửa |
| [vink-prompt](vink-prompt/SKILL.md) | Viết prompt triển khai, chia scope, review thành prompt sửa hoặc prompt tiếp tục |

## Định tuyến nhanh

| Tình huống | Skill | Điều cần giữ |
| --- | --- | --- |
| “Luồng lưu nháp hoạt động thế nào?” | `vink-how` | Lần theo source, chỉ rõ owner và giới hạn; đọc test không đồng nghĩa đã chạy |
| “Vì sao tách nháp khỏi publication?” | `vink-why` | Phân biệt lý do được ghi nhận với suy luận; không tự chấp nhận đề xuất |
| “Lần trước chốt gì, đang vướng đâu?” | `vink-recall` | Tìm đúng chat nếu có tool, kiểm lại code; lịch sử không ghi đè yêu cầu hiện tại |
| “Thiết kế autosave/editor hoặc luồng mới” | `vink-architect` | So sánh theo trải nghiệm, owner và bằng chứng; việc nhỏ không cần vòng kiến trúc |
| “Thử phương án A/B, làm spike S-01” | `vink-prototype` | Nêu câu hỏi và điều kiện dừng; demo không hoàn tất cả spike hay nghiệm thu production |
| “Cập nhật cách agent tự kiểm tra Vink” | `vink-maintain-verification` | Đối chiếu [feature map](../../docs/development/feature-map.md), source và assertion; không tạo runner cạnh tranh |
| Viết prompt cho agent khác | `vink-prompt` | Chỉ soạn prompt; không sửa code, commit, push hay đăng bình luận |
| Review diff/PR | `vink-code-review` | Findings trước, xếp theo mức độ; chỉ sửa khi người dùng giao thêm việc sửa |
| Dựng hoặc mở rộng skeleton | `vink-work-item` + `vink-engineering`; thêm `vink-leptos-web`/`vink-ui-design` khi chạm web | Tiếp tục từ code hiện có, không scaffold bản cạnh tranh; chạy lane theo phần đã đổi |
| Audit ảnh hoặc giao diện | `vink-ui-inspector` | Mở bằng chứng, báo finding và coverage; review không tự cấp quyền sửa |
| Phác thảo bố cục hoặc đặc tả hướng đã chọn | `vink-layout-spec` | Cây vùng là nguồn chính; spec-only dừng đúng phạm vi, không tự sinh UI |
| Màn mới/redesign và prototype UI | `vink-ui-design` + `vink-layout-spec` (+ `vink-prototype` khi thử nghiệm) | 2–3 hướng cấu trúc nếu chưa chọn, spec trước sinh UI; đã giao làm thì tiếp tục không chờ duyệt mặc định |
| Sửa lỗi UI có phạm vi rõ | `vink-ui-fix` (+ `vink-leptos-web` khi chạm component) | Xuất spec vùng rút gọn trước patch, giữ nghĩa trạng thái, kiểm cùng kịch bản |
| Đổi copy hoặc layout | `vink-ui-design` (+ `vink-leptos-web` nếu đổi component) | Không cần model Quint; mở ảnh ở viewport/theme liên quan, chạy `verify quick` và `verify web` nếu hành vi đổi |
| Sửa lưu nháp, version, xung đột | `vink-engineering` (+ `vink-leptos-web` cho UI) | CAS cần `verify postgres`; máy trạng thái UI cần unit test và `verify web` |
| Thiếu Discourse, PostgreSQL hoặc Chrome | Skill đang dùng + [verification](vink-engineering/references/verification.md) | Làm phần độc lập; báo `FAIL`/`NOT_RUN` cho lớp chưa chạy; không báo integration `PASS` |
| Revision, moderation, publish, withdraw | `vink-publication` + `vink-engineering` (+ `vink-behavior` khi có race) | Không đổi D-xx thành Accepted; guard kiểm lại tại commit |

Chỉ chọn skill giải quyết câu hỏi đang có, không bắt mọi thay đổi đi qua cả chuỗi. Phần
“teach” trong draft được tích hợp vào `vink-how`, `vink-why` và `vink-architect`: giải thích
bằng luồng cụ thể, lý do và đánh đổi. `vink-recall` không bắt buộc tool lịch sử; các skill
không tự tạo agent song song, cloud workspace hay lịch bảo trì hằng ngày.

Quy ước bố cục do `vink-layout-spec` sở hữu. Design, Fix, Inspector, frontend,
prototype và prompt dùng nó theo phạm vi; LayoutSpec không gọi ngược các workflow.
Audit không cần thiết kế mới; thay đổi backend/tài liệu không cần đặc tả UI.

Có thể gọi bằng `$vink-work-item`, `$vink-code-review`, `$vink-prompt`… trong môi trường hỗ trợ
repo skills, hoặc yêu cầu agent đọc trực tiếp file `SKILL.md` tương ứng. Các mục `agents/openai.yaml`
cung cấp tên hiển thị và prompt mẫu; không yêu cầu plugin Cantos hay dịch vụ ngoài. Đây là bản
canonical duy nhất: runtime khác nên tham chiếu thư mục này thay vì chép thêm một bộ.
`cargo xtask verify quick` kiểm frontmatter, `agents/openai.yaml`, danh sách ở bảng trên và link.

Nguồn, commit Cantos, draft bổ sung và lý do chọn từng skill nằm tại
[Skill adaptation](../../docs/references/skill-adaptation.md). Skills dẫn tới chủ sở hữu và
không tự chuyển decision đề xuất thành Accepted.
