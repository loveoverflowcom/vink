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
