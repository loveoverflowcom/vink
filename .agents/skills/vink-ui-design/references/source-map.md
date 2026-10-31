# Nguồn và bản đồ chủ sở hữu

Snapshot đọc ngày **31/10/2026**. Đây là provenance của plugin 1.0.0, không là lịch
đồng bộ tự động. Đọc lại nguồn hiện hành trước các khẳng định “mới nhất”, code patch
hoặc quyết định phụ thuộc trạng thái repo. Link pin dưới đây phục vụ tái lập; không
reset checkout hiện hành về SHA này.

## Nguồn Vink

Repo `loveoverflowcom/vink`, nhánh `develop`, SHA
`85cc0b2763aaf87461a6f0382a71388251ad9614`.

| Vai trò | Nguồn đã đọc |
| --- | --- |
| Chỉ dẫn và reality | [AGENTS.md](https://github.com/loveoverflowcom/vink/blob/85cc0b2763aaf87461a6f0382a71388251ad9614/AGENTS.md) |
| Lời hứa sản phẩm, ba không gian | [prd/vision.md](https://github.com/loveoverflowcom/vink/blob/85cc0b2763aaf87461a6f0382a71388251ad9614/prd/vision.md) |
| WR/PUB/MOD/COM/ED/UX/ACC/OPS | [prd/requirements.md](https://github.com/loveoverflowcom/vink/blob/85cc0b2763aaf87461a6f0382a71388251ad9614/prd/requirements.md) |
| Quyết định còn mở và invariants | [decisions.md](https://github.com/loveoverflowcom/vink/blob/85cc0b2763aaf87461a6f0382a71388251ad9614/docs/architecture/decisions.md) |
| Đề xuất stack / asset / editor | [tech-stack.md](https://github.com/loveoverflowcom/vink/blob/85cc0b2763aaf87461a6f0382a71388251ad9614/docs/architecture/tech-stack.md) |
| Baseline Vink UI | [vink-ui-design/SKILL.md](https://github.com/loveoverflowcom/vink/blob/85cc0b2763aaf87461a6f0382a71388251ad9614/.agents/skills/vink-ui-design/SKILL.md) |
| Tokens và component CSS | [apps/vink-web/style/vink.css](https://github.com/loveoverflowcom/vink/blob/85cc0b2763aaf87461a6f0382a71388251ad9614/apps/vink-web/style/vink.css) |
| Web/SSR/editor owner map | [vink-leptos-web/SKILL.md](https://github.com/loveoverflowcom/vink/blob/85cc0b2763aaf87461a6f0382a71388251ad9614/.agents/skills/vink-leptos-web/SKILL.md) |
| Nghĩa publication và quyền | [vink-publication/SKILL.md](https://github.com/loveoverflowcom/vink/blob/85cc0b2763aaf87461a6f0382a71388251ad9614/.agents/skills/vink-publication/SKILL.md) |
| Lệnh thật và giới hạn từng lane | [docs/development/README.md](https://github.com/loveoverflowcom/vink/blob/85cc0b2763aaf87461a6f0382a71388251ad9614/docs/development/README.md) |
| Bàn giao prompt | [vink-prompt/SKILL.md](https://github.com/loveoverflowcom/vink/blob/85cc0b2763aaf87461a6f0382a71388251ad9614/.agents/skills/vink-prompt/SKILL.md) và [prompt-template.md](https://github.com/loveoverflowcom/vink/blob/85cc0b2763aaf87461a6f0382a71388251ad9614/.agents/skills/vink-prompt/references/prompt-template.md) |
| Lịch sử chuyển Cantos → Vink | [skill-adaptation.md](https://github.com/loveoverflowcom/vink/blob/85cc0b2763aaf87461a6f0382a71388251ad9614/docs/references/skill-adaptation.md) |

Vink đã chuyển tám repo skills từ Cantos ở SHA
`0901b20b9a97d3b10780bef91ec93e1c3b43f13c`. Plugin này kế thừa bản Vink, bổ sung
phương pháp inspect/fix từ Cantos mới hơn, và viết lại cho Chat. Không phải thao tác
đổi tên hàng loạt, không thay source skills trong hai repo.

## Phương pháp lấy từ Cantos

Repo `loveoverflowcom/cantos`, nhánh `develop`, SHA
`1c2c867114f181207950e02989dde113da587309`.

| Nguồn | Phần được kế thừa |
| --- | --- |
| [cantos-ui-design](https://github.com/loveoverflowcom/cantos/blob/1c2c867114f181207950e02989dde113da587309/.agents/skills/cantos-ui-design/SKILL.md) | Ý định → intensity → semantic tokens → states → quan sát → kiểm lại cùng kịch bản. |
| [cantos-ui-inspector](https://github.com/loveoverflowcom/cantos/blob/1c2c867114f181207950e02989dde113da587309/.agents/skills/cantos-ui-inspector/SKILL.md) | Inventory thành phần, kiểm từng thành phần, quan hệ, so sánh reference khi yêu cầu, độ chắc chắn. |
| [cantos-ui-fix](https://github.com/loveoverflowcom/cantos/blob/1c2c867114f181207950e02989dde113da587309/.agents/skills/cantos-ui-fix/SKILL.md) | Bug contract, tìm owner nhỏ nhất, sửa có phạm vi, regression phù hợp, before/after đúng scenario. |
| [typography.md](https://github.com/loveoverflowcom/cantos/blob/1c2c867114f181207950e02989dde113da587309/.agents/skills/cantos-ui-design/references/typography.md) | Dấu tiếng Việt, tên dài, cỡ chữ, coverage và tính toàn vẹn văn bản. |
| [contextual-help.md](https://github.com/loveoverflowcom/cantos/blob/1c2c867114f181207950e02989dde113da587309/.agents/skills/cantos-ui-design/references/contextual-help.md) | Copy theo vai trò, bỏ lặp, giữ lý do thiết yếu và đường trợ giúp tiếp cận được. |
| [components.md](https://github.com/loveoverflowcom/cantos/blob/1c2c867114f181207950e02989dde113da587309/.agents/skills/cantos-ui-inspector/references/components.md) | Mô hình component, dimensions, coverage và finding. |

Không kế thừa brand lavender, player/audio/QC, CMP/native, hai-renderer admission,
schema/registry/lệnh Cantos, localization bắt buộc vi/en hay chính sách ảnh Cantos.
Tài liệu nguồn có cả đề xuất lịch sử: chỉ mang phương pháp phù hợp, không coi mọi
đoạn trong reference là capability thực.

## Owner map cho bàn giao code

Những đường dẫn sau được nêu trong owner docs tại snapshot; đọc file thực trước
khi khẳng định root cause, line hoặc patch cụ thể. Tất cả đường dẫn `src/...` dưới
đây đều có gốc `apps/vink-web/`.

| Phần | Owner được nguồn ghi nhận |
| --- | --- |
| Shell, routes, hydration | `apps/vink-web/src/lib.rs` |
| Home/reader/not found | `apps/vink-web/src/pages/mod.rs` |
| Bàn viết | `apps/vink-web/src/pages/desk.rs` |
| Pure save state | `apps/vink-web/src/desk_state.rs` |
| Document/textarea/render | `apps/vink-web/src/document.rs` |
| Consumer API, transport, SSR read seam | `apps/vink-web/src/contract.rs`, `src/api.rs`, `src/server.rs` |
| CSS semantic tokens/components | `apps/vink-web/style/vink.css` |
| Browser smoke | `apps/vink-web/e2e/browser-smoke.mjs` |
| Domain/application/HTTP/persistence | `services/vink-app/src/` theo owner trong docs development |

Vink là SSR/hydration, không sao chép CSR/Trunk của Cantos. Không mang Turso,
Compose hoặc services Cantos sang. Dùng source hiện tại để tìm component trước
khi tạo mới; giữ authority/state machine hiện có.

## Bảo trì trong repo

Khi cập nhật các skill, đối chiếu nguồn thực và decisions hiện hành; giữ provenance của
plugin và upstream Cantos, cập nhật index, metadata và links. Snapshot lịch sử không
phải là nguồn quyền lực cao hơn source hiện hành.
