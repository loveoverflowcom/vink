# Nguồn và giới hạn bằng chứng cho Vink

**Ngày đối chiếu:** 30/10/2026 (UTC+07). Tài liệu này giúp phân biệt ràng buộc người dùng, quan sát từ source và khuyến nghị kiến trúc của bộ đề xuất v0.2.

## 1. Tài liệu đầu vào

| Nguồn | Vai trò |
| --- | --- |
| [README.md người dùng cung cấp](https://chatgpt.com/library?file_id=libfile_2f8a8eefd180819186f916f98317a55b&account_id=personal) | Định vị Vink, M3 Expressive, định hướng Discourse/Leptos/PostgreSQL và quan hệ độc lập với Cantos |
| [vision.md người dùng cung cấp](https://chatgpt.com/library?file_id=libfile_c26024f5b8508191ae3e6bb2efcfa19a&account_id=personal) | Vision ban đầu, trải nghiệm viết/đọc/học, câu hỏi AI và roadmap |
| [Pasted text.txt người dùng cung cấp](https://chatgpt.com/library?file_id=libfile_bd149ad558f08191a44c5d3bfc8e52c5&account_id=personal) | Đề xuất Hybrid chi tiết, literary content ownership, worker, SSO và state models để phản biện |
| Yêu cầu trong hội thoại ngày 30/10/2026 | Ưu tiên vision/tech stack, PostgreSQL độc lập, so sánh A/B/C, tham khảo Cantos và AI từ MVP |

Định vị thị trường, nhóm beta, phạm vi MVP, lựa chọn editor và mức tự động hóa là đề xuất thiết kế. Bộ tài liệu không tuyên bố đã khảo sát người dùng, đo thị trường hoặc chứng minh nhu cầu thương mại.

## 2. Cantos được đọc tại một commit cố định

**Repository:** loveoverflowcom/cantos · **Nhánh đối chiếu:** develop · **Commit:** [a8e2b6f40b58e0e72a2bdab2d4f44240cb6a0330](https://github.com/loveoverflowcom/cantos/commit/a8e2b6f40b58e0e72a2bdab2d4f44240cb6a0330).

| ID | Nguồn | Điều nguồn hỗ trợ |
| --- | --- | --- |
| C1 | [ADR 0046](https://github.com/loveoverflowcom/cantos/blob/a8e2b6f40b58e0e72a2bdab2d4f44240cb6a0330/docs/decisions/0046-service-owned-contracts.md) | Service-owned schema, consumer-owned representations; retirement của packages/contracts; còn giới hạn phân lớp được ghi trong ADR |
| C2 | [Theatre Web Cargo.toml](https://github.com/loveoverflowcom/cantos/blob/a8e2b6f40b58e0e72a2bdab2d4f44240cb6a0330/apps/cantos-theatre/web/Cargo.toml) | Leptos CSR cho WASM, SSR dưới dev-dependencies native |
| C2S | [Studio Web Cargo.toml](https://github.com/loveoverflowcom/cantos/blob/a8e2b6f40b58e0e72a2bdab2d4f44240cb6a0330/apps/cantos-studio/web/Cargo.toml) | Cùng cách phân biệt CSR runtime/SSR test cho Studio |
| C3 | [AGENTS.md](https://github.com/loveoverflowcom/cantos/blob/a8e2b6f40b58e0e72a2bdab2d4f44240cb6a0330/AGENTS.md) | Nguyên tắc modular monolith, boundaries và tránh abstraction không có nhu cầu |
| C4 | [Architecture overview](https://github.com/loveoverflowcom/cantos/blob/a8e2b6f40b58e0e72a2bdab2d4f44240cb6a0330/docs/architecture/overview.md) | Runtime, surfaces, owner dữ liệu và kiến trúc hiện tại |
| C5 | [Store Turso manifest](https://github.com/loveoverflowcom/cantos/blob/a8e2b6f40b58e0e72a2bdab2d4f44240cb6a0330/packages/store-turso/Cargo.toml) | Dependency embedded turso hiện tại; không dựa vào mô tả libsql cũ |
| C6 | [Identity auth v1](https://github.com/loveoverflowcom/cantos/blob/a8e2b6f40b58e0e72a2bdab2d4f44240cb6a0330/services/cantos-server/contracts/auth/identity-auth/v1/README.md) | Identity authentication facts; Server-owned sessions; Web cookie/native Bearer; giới hạn acceptance |
| C7 | [ADR 0054](https://github.com/loveoverflowcom/cantos/blob/a8e2b6f40b58e0e72a2bdab2d4f44240cb6a0330/docs/decisions/0054-publication-generation-and-access.md) | Publication generation tách editing head, policy/access riêng; implementation còn theo từng phần |
| C8 | [ADR 0027](https://github.com/loveoverflowcom/cantos/blob/a8e2b6f40b58e0e72a2bdab2d4f44240cb6a0330/docs/decisions/0027-quint-executable-behavior.md) | Quint, bằng chứng mô phỏng có bounds và giới hạn kết luận |
| C9 | [Repository checks workflow](https://github.com/loveoverflowcom/cantos/blob/a8e2b6f40b58e0e72a2bdab2d4f44240cb6a0330/.github/workflows/repository-checks.yml) | CI thực tế và qualification lane thủ công; không suy rộng Quint cho mọi PR |
| C10 | [Metadata publication contract](https://github.com/loveoverflowcom/cantos/blob/a8e2b6f40b58e0e72a2bdab2d4f44240cb6a0330/services/cantos-server/contracts/studio/metadata-publication/v1/README.md) | Phạm vi backend publication đã triển khai và những acceptance còn riêng |

Source code và ADR không tự chứng minh production hiện chạy đúng, UI đã nhận thay đổi hay mọi test đều PASS. Không sao chép toolchain/version pins Cantos thành cam kết tương thích cho Vink.

## 3. Discourse và các điểm tích hợp

Code được kiểm tại **[fda60103f6db3ab23431062a930735eff857152d](https://github.com/discourse/discourse/commit/fda60103f6db3ab23431062a930735eff857152d)**, snapshot của main ngày nghiên cứu. **Đây không phải release triển khai đã được chọn cho Vink.** Tài liệu Meta có thể tiếp tục thay đổi; integration phải được thử lại trên bản runtime được pin.

| ID | Nguồn | Điều nguồn hỗ trợ |
| --- | --- | --- |
| D1 | [Discourse README](https://github.com/discourse/discourse/blob/fda60103f6db3ab23431062a930735eff857152d/README.md) | Rails, Ember, PostgreSQL, Redis và yêu cầu runtime tại snapshot |
| D2 | [Introducing Horizon](https://meta.discourse.org/t/introducing-horizon-our-newest-theme/369608) | Horizon là theme của Discourse |
| D3 | [External moderation integration](https://meta.discourse.org/t/how-to-integrate-a-third-party-moderation-service-with-discourse/415566/2) | Review API, actions/version, queue phụ thuộc pending/flags, thông tin ngày 23/10/2026 |
| D4 | [PostsController](https://github.com/discourse/discourse/blob/fda60103f6db3ab23431062a930735eff857152d/app/controllers/posts_controller.rb) | Create và update đi qua các đường khác nhau |
| D5 | [PostRevisor](https://github.com/discourse/discourse/blob/fda60103f6db3ab23431062a930735eff857152d/lib/post_revisor.rb) | Sửa live Post, lưu revision và phát các sự kiện liên quan |
| D6 | [NewPostManager](https://github.com/discourse/discourse/blob/fda60103f6db3ab23431062a930735eff857152d/lib/new_post_manager.rb) | Cơ chế xử lý post mới, approval và ngoại lệ |
| D7 | [ReviewableQueuedPost](https://github.com/discourse/discourse/blob/fda60103f6db3ab23431062a930735eff857152d/app/models/reviewable_queued_post.rb) | Actions của queued post; không đồng nhất mọi reviewable |
| D8 | [ReviewablesController](https://github.com/discourse/discourse/blob/fda60103f6db3ab23431062a930735eff857152d/app/controllers/reviewables_controller.rb) | Quyền, version và xung đột khi perform review action |
| D9 | [DiscourseConnect specification](https://meta.discourse.org/t/setup-discourseconnect-official-single-sign-on-for-discourse-sso/13045) | Signed SSO, nonce, external ID, verified email và account linking |
| D10 | [API key configuration](https://meta.discourse.org/t/create-and-configure-an-api-key/230124) | All Users/Single User, Api-Username và giới hạn scopes |
| D11 | [Webhook configuration](https://meta.discourse.org/t/configure-webhooks-that-trigger-on-discourse-events-to-integrate-with-external-services/49045) | Chữ ký, payload và khả năng chứa dữ liệu staff-only |
| D12 | [Import base](https://github.com/discourse/discourse/blob/fda60103f6db3ab23431062a930735eff857152d/script/import_scripts/base.rb) | Import có đường PostCreator và cờ bỏ kiểm tra |
| D13 | [Email receiver](https://github.com/discourse/discourse/blob/fda60103f6db3ab23431062a930735eff857152d/lib/email/receiver.rb) | Đường email có tùy chọn khác với create thông thường |
| D14 | [Install guide](https://github.com/discourse/discourse/blob/fda60103f6db3ab23431062a930735eff857152d/docs/INSTALL.md) | Hướng cài đặt Docker và yêu cầu phối hợp phiên bản runtime |
| D15 | [Emit webhook event job](https://github.com/discourse/discourse/blob/fda60103f6db3ab23431062a930735eff857152d/app/jobs/regular/emit_web_hook_event.rb) | Delivery có retry/điều kiện; không suy ra exactly-once |
| D16 | [PostRevision](https://github.com/discourse/discourse/blob/fda60103f6db3ab23431062a930735eff857152d/app/models/post_revision.rb) | History gắn Post; khác publication pointer độc lập do Vink đề xuất |
| D17 | [ReviewablePost](https://github.com/discourse/discourse/blob/fda60103f6db3ab23431062a930735eff857152d/app/models/reviewable_post.rb) | Hậu kiểm và ngoại lệ; review_every_post không là tiền kiểm tuyệt đối |

Không có nguồn nào ở đây bảo đảm một plugin khoảng 80 dòng sẽ đáp ứng toàn bộ phạm vi Vink. Mức độ tùy biến và khả năng giữ mọi đường ghi phải được xác nhận bằng S-02/S-04.

## 4. Công nghệ bổ trợ

| ID | Nguồn chính thức | Phạm vi sử dụng trong đề xuất |
| --- | --- | --- |
| T1 | [Leptos SSR](https://book.leptos.dev/ssr/index.html) | SSR với Axum/Actix là hướng tích hợp có tài liệu |
| T2 | [Leptos page-load lifecycle](https://book.leptos.dev/ssr/22_life_cycle.html) | Phân biệt server SSR build và browser hydration build |
| T3 | [Leptos islands](https://book.leptos.dev/islands.html) | Khả năng giảm hydration theo phần tương tác và tradeoff routing |
| T4 | [ProseMirror guide](https://prosemirror.net/docs/guide/) | Schema, document JSON và transactions; không phải benchmark IME của Vink |
| T5 | [SQLx documentation](https://docs.rs/sqlx/latest/sqlx/) | Driver PostgreSQL, transactions, migrations và API Rust |
| T6 | [PostgreSQL SELECT](https://www.postgresql.org/docs/current/sql-select.html) | SKIP LOCKED phù hợp một số queue-like workloads, có giới hạn tính nhất quán |
| T7 | [PostgreSQL unaccent](https://www.postgresql.org/docs/current/unaccent.html) | Loại dấu; không xác nhận tìm kiếm tiếng Việt hoàn chỉnh |
| T8 | [Quint checking properties](https://quint.sh/docs/checking-properties) | Safety, liveness, simulation và phạm vi formal methods |
| T9 | [Quint model checkers](https://quint.sh/docs/model-checkers) | Phân biệt cách khảo sát trạng thái và giới hạn công cụ |
| T10 | [Leptos JavaScript integration](https://book.leptos.dev/web_sys.html) | Đường tích hợp JS/Web APIs; editor cụ thể vẫn cần spike |

## 5. Các nhận định đã hiệu chỉnh

| Nhận định dễ gây hiểu nhầm | Cách dùng trong bộ đề xuất |
| --- | --- |
| “Leptos SSR là stack Cantos đã triển khai” | Cantos Web được đọc dùng CSR; SSR là quyết định Vink cần thử |
| “Horizon thay Ember” | Horizon là theme, Ember vẫn thuộc Discourse |
| “Discourse Admin quản hết Vink” | Admin xử lý domain Discourse; tác phẩm Vink có màn vận hành riêng |
| “Có webhook là mọi bài đều đã được tiền kiểm” | Owner phải giữ nội dung pending; create/edit và từng cửa ghi cần kiểm chứng |
| “AI Approved tức Published” | Publish cần đúng snapshot, quyền, ý định tác giả và gate hiện tại |
| “Outbox bảo đảm exactly-once” | Cần idempotency/reconciliation và generation fencing phù hợp ở phía đích |
| “Chuyển HumanReview là không còn kẹt” | Cần người phụ trách, khung xử lý, cảnh báo và đo tuổi queue |
| “Custom fields chắc chắn chậm; block ID giải quyết mọi neo” | Chưa có benchmark tương ứng; ownership và schema cần quyết định riêng |
| “Quint PASS chứng minh toàn bộ hệ thống đúng” | Ghi đúng model, bounds, phương pháp và kiểm chứng implementation bổ sung |

## 6. Giới hạn của lần nghiên cứu

Đã đọc ba đầu vào, đối chiếu nguồn Cantos/Discourse và tài liệu chính thức; đã review chéo logic và phạm vi bộ Markdown. Chưa triển khai Vink, cài Discourse, chạy benchmark, thử editor hay chạy model Quint. Các acceptance và spike là công việc cần làm khi triển khai, không được đánh dấu hoàn tất từ nghiên cứu tài liệu.
