# D2T-03 — phạm vi triển khai và bằng chứng

Issue #45; branch `codex/discussion-ui`, PR #59. Checkpoint local dưới đây đã tích hợp
`develop@27927c462244f2a70e36a686a37c8c95690775b6`.
Source checkpoint local: `0f46f817160c2507861e0c508cfd3dbb4c1b008b`.
Cây source tương ứng trên GitHub: [`c09eeeb8`](https://github.com/loveoverflowcom/vink/commit/c09eeeb8d55b1ddf4bbfc6b4aa21ead9ac6b43ff);
cả hai có Git tree `7b6b9f653d03d88901ba5966b3c65126ad2589be`. Commit tài liệu
sau đó cập nhật LayoutSpec/evidence; kết quả local và sidecar giữ SHA gốc.
PR tiếp tục tích hợp `develop@3b95ca09ec9e134f9b80d2c7add677ca9d1cc028`
(PR #57 Noto pilot và #58 UI). Kết quả revalidation trên base mới, head và CI run
được ghi trong PR; không suy PASS cho cây mới từ các ảnh/lệnh checkpoint cũ.
Checkpoint triển khai ban đầu `c6222a7` xuất phát từ `develop@14c33c9`; bảng checkpoint
bên dưới là lịch sử, không thay kết quả kiểm cuối.
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
thuộc feature owner, không thuộc button/ô trả lời có thể được thu gọn. Nhánh ngoài
page giữ cả receipt sau khi ACK xóa chữ đã gửi; editor chỉ được giữ khi còn chữ
hoặc một lần gửi chưa rõ kết quả.

## Kết quả checkpoint local trước PR #57/#58

Rust 1.99.1, Chrome 156.0.8078.80, PostgreSQL 17.11, macOS ARM64; CMP dùng JDK
21.0.7. Cargo local dùng `CARGO_INCREMENTAL=0`, `CARGO_BUILD_JOBS=2`,
`CARGO_PROFILE_DEV_DEBUG=0`, `CARGO_PROFILE_TEST_DEBUG=0`, `CARGO_NET_OFFLINE=true`.
Socket/loopback tests được chạy với quyền cho IPC cục bộ và database dùng một lần.

| Kết quả trên source `0f46f817` | Boundary | Giới hạn |
| --- | --- | --- |
| PASS — `cargo xtask verify quick`, 8/8 | Tokens/drift, fmt, clippy SSR/hydrate/preview, doctests, unit/contract, source boundaries; Web 45 tests, Rust patterns 2 | Không DB/browser/model proof |
| PASS — `cargo xtask verify web`, 7/7, main + restart | Server/PostgreSQL thật, LocalFixture POST commit → mất ACK → keyed replay → own-pending, SSR privacy, close/reopen CAS, own DELETE; save/conflict/export/SIGTERM/restart | LocalFixture không phải production auth; CDP input không phải OS IME |
| PASS — `cargo xtask ui-preview leptos verify`, `VINK_UI_FOCUS_CAPTURE=1` | Renderer, callbacks/focus/selection, mọi scenario reflow ở 320/390/834/1024/1440 và 100/200%; 761 raw contexts cùng specimen tiles | Gallery không persistence |
| PASS — `cargo xtask ui-preview cmp verify` | Common/Desktop, 78 tests, 639 PNG | JVM off-screen, không native product |
| PASS — mutation sensitivity ở controller production | Tạm giữ physical-attempt cũ nhưng giữ nguyên payload/key: test FAIL tại `!complete(old_ack)`; khôi phục đúng SHA-256 rồi full quick PASS | Một negative control của Rust consumer, không Kani/Quint |

`postgres`/`discussion` không chạy lại local vì lát này không đổi domain/SQL/model;
CI của PR chạy các lane hiện hành, kết quả head cuối và link run nằm trong PR.
Model core/lifecycle không là proof/conformance của consumer D2T-03.

Giữ log FAIL trước khi sửa: hết disk khi build archive, sandbox chặn Unix socket,
fixture no-hydration để WASM preload không có consumer, oracle Tab của reader cũ,
và helper DOM-focus đua selection CodeMirror. Đã đối chiếu network/input trace,
dùng native Tab có sẵn trong harness, kiểm hẹp restart 10/10 rồi chạy full Web.
Giữ nguyên deadline, exact append/export và focus-ring assertions; production
editor không đổi trong lát này.

Web product lane giữ 874 captures, mọi sidecar pin `0f46f817`; 29 ảnh discussion
tách PostgreSQL/LocalFixture khỏi intercepted contract fixtures. Đã mở 15 ảnh Web,
10 CMP và 10 Leptos tiles chứa pattern mới; 2 tile header bổ sung không tính vào
coverage pattern. Mẫu đã xem không còn finding thị giác cần sửa; không khẳng định
đã mở mọi ảnh hoặc nghiệm thu screen reader/OS IME/FPS từ ảnh tĩnh.

CMP discussion dùng canvas nội dung 5000/10000 px ở density 1, preview font scale
1/2, OS insets 0; đây không phải chiều cao viewport native. Consumer sidecar không
có SHA. Provenance ngoài sidecar ghi run source `4418f2b`, các input CMP/shared/
fonts không đổi so với `0f46f817`; không gán fingerprint riêng của font test cho
consumer. Leptos có source guard/fingerprint và kiểm source không đổi. Log, traces,
manifest, SHA-256 và danh sách ảnh đã mở được giữ trong evidence của task.

## Checkpoint triển khai trước tích hợp

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
- PR #57 đã đưa manifest 16 asset, Rust policy và Web pilot của #25 vào base mới.
  Tích hợp picker/composer, caret/IME và focus return vẫn chưa được triển khai;
  không còn ghi thiếu manifest là blocker. API typed sticker/reaction vẫn chưa có.
  Unicode gõ trực tiếp được giữ nguyên; không tạo competing manifest hay nút giả.
- API chưa có ranking, context lookup ngoài page, reports/appeals hay rate limits.
  Paging không phải snapshot; refresh thay projection đã tải, không realtime thu hồi
  mọi nội dung khi không có invalidation source.
- CMP có pattern và các scenario cùng catalog IDs, Common/Desktop đã kiểm như bảng
  cuối; chưa có native product/service consumer. Android/iOS, OS IME, screen reader,
  safe-area và native lifecycle NOT_RUN; long-thread/motion/memory budgets chưa qualified.
- Quint hiện tại vẫn là model core/lifecycle #41. Chưa có model interleavings consumer,
  Kani hoặc conformance model → controller cho D2T-03. Negative control local nêu
  trên không thay các lane đó.
- Draft trong bộ nhớ trang, không phải persistence/recovery qua reload. Không lưu text
  vào storage chung; đổi route/scope không chuyển text sang tác phẩm hoặc actor khác.
