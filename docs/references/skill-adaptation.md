# Nguồn và cách chuyển skills sang Vink

**Ngày:** 30/10/2026. Các skills nằm trong [`.agents/skills/`](../../.agents/skills/README.md)
và được định tuyến qua [AGENTS.md](../../AGENTS.md).

Nguồn được đọc từ checkout Cantos tại commit
[`0901b20b9a97d3b10780bef91ec93e1c3b43f13c`](https://github.com/loveoverflowcom/cantos/commit/0901b20b9a97d3b10780bef91ec93e1c3b43f13c).
Các file skills nguồn không có thay đổi cục bộ khi đối chiếu. Đây là mốc chuyển skills;
khác với commit Cantos `a8e2b6f…` trong [nghiên cứu foundation](sources.md), vốn được giữ nguyên.

## Phần được chọn và viết lại

| Nguồn Cantos tại commit trên | Skill Vink | Điều chỉnh chính |
| --- | --- | --- |
| [cantos-engineering](https://github.com/loveoverflowcom/cantos/blob/0901b20b9a97d3b10780bef91ec93e1c3b43f13c/.agents/skills/cantos-engineering/SKILL.md) | [vink-engineering](../../.agents/skills/vink-engineering/SKILL.md) | Giữ phân lớp, kiểu dữ liệu, bất biến và bằng chứng; chuyển persistence sang PostgreSQL, thêm ranh giới Discourse và không giả định runner có sẵn |
| [cantos-publication](https://github.com/loveoverflowcom/cantos/blob/0901b20b9a97d3b10780bef91ec93e1c3b43f13c/.agents/skills/cantos-publication/SKILL.md) | [vink-publication](../../.agents/skills/vink-publication/SKILL.md) | Chuyển từ audio release/QC sang revision văn bản, ý định tác giả, moderation, publication, curation và thu hồi discussion |
| [cantos-behavior](https://github.com/loveoverflowcom/cantos/blob/0901b20b9a97d3b10780bef91ec93e1c3b43f13c/.agents/skills/cantos-behavior/SKILL.md) | [vink-behavior](../../.agents/skills/vink-behavior/SKILL.md) | Giữ phân loại semantic impact, reachability, negative controls và giới hạn model; bỏ yêu cầu registry/catalog/conformance tool chưa tồn tại ở Vink |
| [cantos-leptos-web](https://github.com/loveoverflowcom/cantos/blob/0901b20b9a97d3b10780bef91ec93e1c3b43f13c/.agents/skills/cantos-leptos-web/SKILL.md) | [vink-leptos-web](../../.agents/skills/vink-leptos-web/SKILL.md) | Từ CSR Studio/Theatre sang đề xuất SSR/hydration, reader và editor tiếng Việt; tập trung autosave, xung đột, private HTML và JS bridge |
| [cantos-ui-design](https://github.com/loveoverflowcom/cantos/blob/0901b20b9a97d3b10780bef91ec93e1c3b43f13c/.agents/skills/cantos-ui-design/SKILL.md) | [vink-ui-design](../../.agents/skills/vink-ui-design/SKILL.md) | Giữ M3 Expressive, typography, accessibility và ảnh đã mở; thiết kế theo Bàn viết/Xưởng Viết/Tạp Chí, bỏ brand/player/CMP và không ép song ngữ |
| [cantos-code-review](https://github.com/loveoverflowcom/cantos/blob/0901b20b9a97d3b10780bef91ec93e1c3b43f13c/.agents/skills/cantos-code-review/SKILL.md) | [vink-code-review](../../.agents/skills/vink-code-review/SKILL.md) | Giữ review theo invariant, đường gọi thực và độ mới bằng chứng; thay các pass audio/native bằng publication văn bản, privacy, PostgreSQL, Discourse và editor |
| [cantos-work-item](https://github.com/loveoverflowcom/cantos/blob/0901b20b9a97d3b10780bef91ec93e1c3b43f13c/.agents/skills/cantos-work-item/SKILL.md) | [vink-work-item](../../.agents/skills/vink-work-item/SKILL.md) | Giữ scope có thể nghiệm thu và bàn giao theo lớp; bỏ phụ thuộc queue/Project, không tự thêm approval, tôn trọng yêu cầu commit trực tiếp vào develop |
| [cantos-prompt](https://github.com/loveoverflowcom/cantos/blob/0901b20b9a97d3b10780bef91ec93e1c3b43f13c/.agents/skills/cantos-prompt/SKILL.md) | [vink-prompt](../../.agents/skills/vink-prompt/SKILL.md) | Giữ pin nguồn, chia scope, review/resume và blocker branches; tạo mẫu VINK_PROMPT_INPUT_V1 gọn, không phụ thuộc plugin hay contract Cantos |

Skills được viết lại theo [requirements](../../prd/requirements.md) và
[decision log](../architecture/decisions.md), không phải thay tên hàng loạt. References cũ
được thay bằng chủ tài liệu Vink hoặc hai references mới phục vụ verification và mẫu prompt;
không mang theo các liên kết đến module hay lệnh Cantos không có trong repo này.

## Phạm vi và cách bảo trì

Không đưa audio production/listening/Script IR, Compose Multiplatform, Turso, Kani, native
Identity, CI và deployment đặc thù Cantos vào workflow Vink. Bài học về bất biến, retry và
publication được áp dụng có chọn lọc cho văn bản và thảo luận.

Skills là hướng dẫn phương pháp, không tạo quyền hành động ra ngoài, không bắt người dùng
xác nhận lại điều đã giao và không biến đề xuất Hybrid/SSR/auth thành quyết định Accepted.
Metadata UI dùng tên `vink-*`, prompt mẫu tương ứng và cơ chế khám phá mặc định; không yêu cầu
cài plugin hay kết nối tài khoản ngoài để đọc skills trong checkout.

## Cập nhật theo skeleton (30/10/2026)

Khi skeleton được triển khai, skills được đối chiếu lại với code và lệnh thật:

- [README skills](../../.agents/skills/README.md) có bảng định tuyến nhanh (prompt, review,
  skeleton, copy/layout, lưu nháp/xung đột, thiếu dependency, publication).
- `vink-engineering` và `vink-leptos-web` có bảng owner → đường dẫn của code đã tồn tại;
  [verification](../../.agents/skills/vink-engineering/references/verification.md) chọn lane
  `cargo xtask verify` theo loại thay đổi.
- Lệnh chỉ được định nghĩa ở [docs/development](../development/README.md); skills, README, AGENTS
  và CI dẫn tới đó. `verify quick` kiểm frontmatter, `agents/openai.yaml`, danh sách skills,
  link Markdown và ranh giới import.
- `vink-publication`, `vink-behavior`, `vink-ui-design`, `vink-code-review` và `vink-prompt` ghi
  rõ phần đã có (revision bất biến, con trỏ publication, token CSS, lane thật) và phần chưa có.

Khi Vink có implementation, cập nhật mô tả repository reality trong AGENTS, hướng dẫn của
owner liên quan và lệnh kiểm thực sự có sẵn. Chỉ bổ sung references khi có nhu cầu cụ thể;
không đồng bộ tự động các ràng buộc mới của Cantos sang Vink.

## Bổ sung từ draft workflow coding agent (31/10/2026)

Nguồn đầu vào là bản người dùng cung cấp trong chat, tiêu đề **“Cách kỹ sư Grok & Cursor dùng
AI Agent để đưa hàng nghìn PR vào production”**, có dòng ghi người chia sẻ Jack Vĩ và mô tả
các workflow pstack. Lần này chỉ dùng draft làm gợi ý phương pháp rồi đối chiếu repository
Vink tại `20635b45c856990638c06eaab011b9940da852dc`; không đọc hay nhập mã nguồn pstack.
Các thông tin về nhân sự, số PR, mức tăng năng suất và phiên bản pstack trong draft chưa được
xác minh và không được dùng làm bằng chứng hay cam kết hiệu quả của bộ skills này.

| Ý tưởng từ draft | Phần áp dụng cho Vink | Điều chỉnh theo phạm vi thực tế |
| --- | --- | --- |
| `/how` | [vink-how](../../.agents/skills/vink-how/SKILL.md) | Lần từ route qua SSR/hydration, application và PostgreSQL; tách code, assertion và kết quả chạy |
| `/why` | [vink-why](../../.agents/skills/vink-why/SKILL.md) | D/INV/S, Git history và PR liên quan; không coi foundation hay merge là quyết định Accepted |
| `/recall` | [vink-recall](../../.agents/skills/vink-recall/SKILL.md) | Tool lịch sử là tùy môi trường; đối chiếu chat cũ với checkout mới, không chuyển quyền từ task cũ |
| `/architect`, thiết kế từ trải nghiệm/API usage | [vink-architect](../../.agents/skills/vink-architect/SKILL.md) | So phương án theo owner, quyền dữ liệu và bằng chứng; chia lát cắt, tiếp tục triển khai nếu đã được giao |
| Prototype để giải quyết bất định | [vink-prototype](../../.agents/skills/vink-prototype/SKILL.md) | Thử ngay tại seam của skeleton, câu hỏi và điều kiện dừng rõ; một demo không hoàn tất S-01–S-05 |
| Feature Map, bảo trì verification, “Build the Lever” | [vink-maintain-verification](../../.agents/skills/vink-maintain-verification/SKILL.md) và [feature map](../development/feature-map.md) | Dùng `xtask`/browser smoke hiện có, ghi rõ assertion và khoảng trống; chỉ thêm tool cho nhu cầu lặp lại có thật |
| `/teach` | Tích hợp vào `vink-how`, `vink-why`, `vink-architect` | Giải thích bằng ví dụ, nguồn và đánh đổi; chưa cần một skill riêng trùng chức năng |

Không biến đề xuất chạy nhiều agent/model, cloud workspace, bảo trì mỗi ngày hoặc tổ chức lại
toàn bộ tài liệu thành yêu cầu mặc định. Cách viết từ trải nghiệm người dùng được giữ trong
kiến trúc/prototype; chưa cần thêm package hay bộ tài liệu Diátaxis cho skeleton hiện tại.
Triển khai, review và chọn lớp kiểm vẫn do `vink-work-item`, `vink-code-review` và
`vink-engineering` sở hữu, tránh tạo bản sao của các workflow này.

Bản đồ đầu tiên được lập bằng việc đọc route, component, contract, fixture và assertion.
Không bổ sung CLI, không chạy lại PostgreSQL/browser, không nâng mức hoàn thiện sản phẩm.
Khi cần kiểm chứng runtime hoặc sửa công cụ, thực hiện theo lớp bằng chứng được ghi trong
hướng dẫn phát triển và verification guide.
