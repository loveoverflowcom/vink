# Vink — Việt Ink · bản đồ tài liệu

**Vink giúp người viết Việt duy trì việc viết, tìm được người đọc phù hợp và tiến bộ qua phản hồi
tử tế.** Trải nghiệm ưu tiên văn bản, tiếng Việt và cộng đồng có chủ đích; thiết kế theo Material 3
Expressive nhưng đủ yên tĩnh để đọc dài.

Vink sở hữu tác phẩm, vòng đời xuất bản và thảo luận trong PostgreSQL. Bình luận hai tầng do Vink
tự xây bằng Rust core ([ownership](architecture/discussion-ownership.md), chốt 03/11/2026);
Discourse chỉ là nguồn tham khảo. Hiện trạng triển khai theo miền ở [AGENTS.md](../AGENTS.md) và
các registry trong [development](development/README.md).


## Đọc theo thứ tự nào

| Tài liệu | Câu hỏi được giải quyết |
| --- | --- |
| [Vision](../prd/vision.md) | Vink tồn tại để làm gì và trải nghiệm nào đáng đầu tư? |
| [Requirements](../prd/requirements.md) | Phạm vi, yêu cầu có ID và tiêu chí nghiệm thu |
| [Kho quyết định TOML](../decisions/README.md) | Quyết định nào đang có hiệu lực, revision nào đang chờ signoff? |
| [Tech stack](architecture/tech-stack.md) | Chọn công nghệ nào, vì sao và khi nào đổi hướng? |
| [Decision log](architecture/decisions.md) | D-01, bất biến INV-01–INV-08, spike S-01–S-05 |
| [Development](development/README.md) | Setup, lệnh, cấu trúc code, lane kiểm chứng |
| [Feature map](development/feature-map.md) | Hành trình người dùng → route → owner → phép kiểm |
| [Thiết kế](design/README.md) | LayoutSpec hiện hành và owner của token/asset |

## Bốn chiều trạng thái

Giữ tách bạch trong tài liệu, review và bàn giao: quyết định sản phẩm (`PRODUCT_CONFIRMED`,
`DEFERRED`, `OPEN`), quyết định kiến trúc (`DESIGN_PROPOSED`, `Provisional`, `Accepted`), mức triển
khai (`NOT_IMPLEMENTED`, `core`, `persistence-api`, `worker-provider`, `web`, `cmp`) và bằng chứng
kiểm chứng (lệnh, SHA, ranh giới). Nhập tài liệu, qua check hay merge code không làm một đề xuất
thành `Accepted`.

## Bản đồ theo nhóm

| Nhóm | Owner |
| --- | --- |
| Sản phẩm | [Vision](../prd/vision.md), [Requirements](../prd/requirements.md) |
| Quyết định sản phẩm 10–11/11/2026 | [DecisionDelta](architecture/product-decisions-20261110.md), [xuất bản/kiểm duyệt/tombstone](architecture/publication-moderation.md), [pilot AI](architecture/moderation-ai-pilot.md), [sửa bình luận](architecture/comment-editing.md), [bộ sưu tập](architecture/collections.md), [Đề xuất và Top 100](architecture/discovery-ranking.md), [hashtag](architecture/hashtags.md), [registry](development/product-decisions-registry.md), [model đích](../services/vink-app/behavior/target/README.md) |
| Kiến trúc nền | [Tech stack](architecture/tech-stack.md), [decision log](architecture/decisions.md), [PostgreSQL/Identity](architecture/identity-postgres-boundaries.md), [Auth D-05](architecture/auth-authority.md), [frontend Cloudflare](architecture/edge-frontend.md), [độ hạt lưu văn bản](architecture/content-storage-rfc.md), [document canonical](architecture/document-source-format.md), [tiêu đề theo định dạng](architecture/editor-title-authority.md) |
| Thảo luận | [ownership](architecture/discussion-ownership.md), [admission](architecture/discussion-admission.md), [reads](architecture/discussion-reads.md), [outbox](architecture/discussion-outbox.md), [core](development/discussion-core.md), [LayoutSpec](design/discussion-ui.md) |
| Ghi nhận văn học, phân loại | [LVE](architecture/literary-endorsement.md), [taxonomy](architecture/work-taxonomy.md) |
| Điểm Vink (#600, backend-only, chưa triển khai runtime) | [PRD §19](../prd/requirements.md#19-điểm-vink-reputation-v1), [đặc tả](architecture/reputation.md), [registry](development/reputation.md), [SR-01](development/reputation-science.md) |
| Social, thông báo, lượt xem, tài khoản | [Social Graph](architecture/social-graph.md), [notifications](architecture/notifications.md), [work views](architecture/work-views.md), [xóa tài khoản](architecture/account-deletion.md), [khôi phục mật khẩu](architecture/password-recovery.md) |
| API | [App HTTP v1](../services/vink-app/contracts/http/v1/README.md), [Identity RPC v1](../services/vink-identity/contracts/rpc/v1/README.md) |
| Vận hành | [production](../infra/production/README.md), [Cloudflare edge](../infra/production/cloudflare-edge.md), [auth operations](development/auth-operations.md) |
| Hướng dẫn agent | [AGENTS.md](../AGENTS.md), [skills](../.agents/skills/README.md) |
| Nguồn nghiên cứu nền | [sources](references/sources.md) |

## Quy ước lưu trữ

Mỗi thông tin có một owner. Repository giữ những gì cần để build, chạy, kiểm và hiểu quyết định
hiện hành; lịch sử quá trình nằm ở nơi khác.

| Nơi lưu | Nội dung |
| --- | --- |
| Git | Source, đặc tả và LayoutSpec hiện hành, quyết định (TOML, ADR), test, fixture, regression baseline, runtime asset kèm provenance, tóm tắt bằng chứng tối thiểu trong registry owner |
| Google Drive | Concept, canvas, artwork gốc, ZIP thiết kế, ảnh preview, ảnh trước/sau, visual evidence ([vink-ui-evidence-drive](../.agents/skills/vink-ui-evidence-drive/SKILL.md)) |
| GitHub Actions artifact | Output thô của lane kiểm chứng (log, trace, capture, manifest) |
| GitHub Issue/PR | Lịch sử triển khai, review, handoff, nghiệm thu theo lượt |
| Máy local (`target/`, ignored) | File tạm, capture, log, cache |

Không thêm thư mục archive hay evidence vào Git; Git history đã giữ lịch sử. `cargo xtask verify
static repo` từ chối file theo dõi quá cỡ, ảnh/archive/log ngoài allowlist có lý do và thư mục
evidence/archive (xem [repository guards](development/scoped-workflows.md#guard-chống-phình-repository)).
Khi đổi hành vi, cập nhật registry owner bằng trạng thái hiện hành thay vì nối thêm changelog.
