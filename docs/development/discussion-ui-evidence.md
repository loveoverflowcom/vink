# D2T-03 — phạm vi triển khai và bằng chứng

Issue #45; base `develop@14c33c95f6ddc77e2990f37b741841df942ea3c5`, branch
`codex/discussion-ui`. Kết quả dưới đây thuộc worktree triển khai ngày 03/11/2026,
trước tích hợp `develop@27927c4` và trước full gates do người review thực hiện.
[LayoutSpec](../design/discussion-ui.md) đã được trình bày trước sửa UI.

Web reader đọc discussion anonymous khi SSR; browser tải projection riêng qua API
`no-store`. Cột nhận xét giữ root/neo và một danh sách replies phẳng. Không dựng
total count, ranking, feedback intent hoặc participant identity mà contract chưa cấp.
Author toggle chỉ từ `viewer_is_work_author`; own remove chỉ từ `can_remove`.
Production chưa có auth authority. Marker opaque của LocalFixture chỉ là tín hiệu
host mẫu, không chứa principal/credential và không thay server resolution/loopback guard.

Controller production giữ work/session/composer, edit generation, command key và
physical attempt. Late ACK không xóa text mới; mất ACK chỉ đối soát đúng payload/key;
401 khóa write readiness riêng, không đổi scope giữa các request đang bay. Target
bị thu hồi mất tên đã cache, vẫn giữ text; draft có root ngoài page hiện tại có vùng
khôi phục riêng. PUT/DELETE fence các read cũ; refresh trong lúc read được xếp lại
khi cần cập nhật sau submit. Deadline HTTP 30 giây là outcome chưa rõ; transport
thuộc feature owner, không thuộc button/ô trả lời có thể được thu gọn.

| Kết quả thực thi tại lượt triển khai | Boundary | Giới hạn |
| --- | --- | --- |
| PASS — `cargo test --offline -p vink-web -p vink-patterns --lib`: 44 Web + 2 patterns | Controller, adapter, cursor/dedupe, mutation/read fencing, pattern HTML/escaping/Sharing Open/Closed/Pending | Không browser/DB/model/native proof |
| PASS — `cargo check --offline -p vink-app` | SSR/provider composition | Compile, không runtime |
| PASS — `cargo check --offline -p vink-web --no-default-features --features hydrate --target wasm32-unknown-unknown` | Hydration/browser transport | Compile, không runtime |
| PASS — `cargo check --offline -p vink-ui-preview` | Gallery adapter | Compile, không render |
| PASS — `cargo xtask ui-tokens` và `cargo xtask ui-tokens --check` | Catalog 0.5.2 sinh từ SSOT; token values không đổi | Không UI runtime |
| PASS — `cargo fmt --all`, `node --check apps/vink-web/e2e/browser-smoke.mjs`, `git diff --check` | Source/format/syntax | Không thay full gates |
| NOT_RUN tại lượt triển khai — full `verify quick/web/postgres/discussion`, CMP verify và captures | Bàn giao người review chạy trên diff cuối cùng và base đã tích hợp | Không khai PASS từ test đã viết |

Log lượt triển khai được giữ ngoài repo trong thư mục evidence của task với tên
`discussion-ui-focused-tests.log`, `discussion-ui-check-ssr.log`,
`discussion-ui-check-hydrate.log`, `discussion-ui-check-preview.log` và
`discussion-ui-catalog-check.log`. Các lỗi compile ban đầu đã được sửa trước các
kết quả PASS; full gate sau bàn giao là nguồn kết quả cuối cùng.

`discussionReadingAndWrites` đã thêm vào browser smoke hiện có: real local-fixture
POST commit rồi mất ACK, keyed replay, own-pending không vào SSR, typing mới, author
close/reopen bằng generation và DELETE own. Các ảnh/thao tác flat context, context
revocation và collapse/reopen khi reply đang gửi dùng intercepted provider fixtures,
được ghi nhãn riêng, không chứng minh persistence/moderation. Ma trận capture gồm
light/dark 320/390/834/1024/1440 và chữ 200% ở 320/390; sidecar giữ boundary/viewport/
theme/build. Ảnh phải được mở để được gọi là inspected.

## Acceptance còn chặn

- Production auth/session và luồng end-to-end auth → moderation → public persistence
  chưa có. Không có nút login giả. UI đặt control tác giả tại reader; chưa có bề mặt
  quản lý tác phẩm đã đăng riêng để nối setting vào đó.
- #25 chưa cung cấp Google manifest/asset/player trong checkout: không dựng picker,
  sticker/reaction hoặc competing manifest. Unicode gõ trực tiếp được giữ nguyên.
- API chưa có ranking, context lookup ngoài page, reports/appeals hay rate limits.
  Paging không phải snapshot; refresh thay projection đã tải, không realtime thu hồi
  mọi nội dung khi không có invalidation source.
- CMP có pattern và các scenario cùng catalog IDs, không có native product/service
  consumer. Common/Desktop/native compile/runtime, OS IME, screen reader, safe-area,
  lifecycle và performance chưa được nghiệm thu ở lượt này.
- Quint hiện tại vẫn là model core/lifecycle #41. Chưa có model interleavings consumer,
  Kani, mutation sensitivity hoặc conformance model → controller cho D2T-03.
- Draft trong bộ nhớ trang, không phải persistence/recovery qua reload. Không lưu text
  vào storage chung; đổi route/scope không chuyển text sang tác phẩm hoặc actor khác.
