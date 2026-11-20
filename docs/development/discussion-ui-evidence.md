# D2T-03 — phạm vi triển khai và bằng chứng

## Nhận xét R4 trên Linux · PR #366

Base `ac77a0fa1fed42a8f25a1795991d253b64cb4ff8`. Commit [10ea20b](https://github.com/loveoverflowcom/vink/commit/10ea20ba63aa0c08edd072c6a7f32d3a5bf846c6) sửa FieldMode, root segment/geometry, composer Sending, clipboard và read-only. Commit [d2092b6](https://github.com/loveoverflowcom/vink/commit/d2092b67de3e0699921876dd1460a64e41875530) giữ tác phẩm trong DOM trong khi lần GET nhận xét đầu tiên đang chờ. [LayoutSpec R4](../design/discussion-ui.md) và [bộ tham chiếu Drive](https://drive.google.com/drive/folders/1OpT6_uY1GG-oGvhSl6b5xsmgSsM7hVdE). Đã mở 12 ảnh 01/01b, 02/02b, 03/03b, 04–08 và 10. Ảnh 04 cho phép gõ trong Sending, lần gửi giữ snapshot bất biến.

### Trang ẩn và số Ưng

Cùng ca thực trên base và PR: tác giả gửi nhận xét, quản trị viên duyệt rồi bấm Ưng. Trước khi ẩn, public summary trả HTTP 200, count 1. Khi mở mới Reader sau khi rút qua kiểm duyệt hoặc tác giả đặt riêng tư, summary của nhận xét trả HTTP 404. DOM của candidate chỉ hiện “Ưng”. Cả hai trang đều ẩn ô soạn, Trả lời và nút Ưng. Tác giả có `can_remove=true` mở được xác nhận gỡ; người đọc khác không có nút. [Provider đọc count](../../services/vink-app/src/application/endorsement.rs) đòi revision đang công khai. Giữ test đòi “Ưng · 1” trên cả hai đường. #360 AC-04 còn mở.

### Đo trên browser

Debian Linux, Rust 1.99.1, wasm-bindgen 0.2.129, Node 24.22.3, PostgreSQL 17.11 và Chromium Headless Shell 156.0.8078.79. Mỗi lượt seed cluster loopback riêng. Ca mất ACK là POST thật đã commit, đối soát trả HTTP 200 trên đúng key/body/target và chỉ có một own-pending. Các ca revoked và own-overlay thay GET bằng fixture của provider; các ảnh của chúng chứng minh cách hiển thị và tương tác ở Reader. Gallery Leptos dựng specimen không có endorsement của ứng dụng.

| Bề mặt | Số đo và phạm vi đã mở |
| --- | --- |
| Uncertain, dark | Cùng fixture 390/100% và 320/200%. Base dùng `#f0e7f8` trên `#2b2434`, 12,45:1. Candidate dùng `--vk-on-info-container` (`#d6e6ff`) trên `--vk-info-container` (`#22466d`), 7,68:1 cho cả tiêu đề và đoạn giải thích. Assertion cũ đòi màu của body. [Ảnh và số đo CI trước sửa](https://github.com/loveoverflowcom/vink/actions/runs/44698129614/artifacts/14611799914) · [Ảnh và số đo CI trên PR](https://github.com/loveoverflowcom/vink/actions/runs/44718676473/artifacts/14620678053). |
| Cụm root và reply | Ở 1440/100%, cạnh tên, thân, nhóm Phản hồi và ô reply cùng x=554,5 CSS px. Ở 1440/200% cùng x=364. Kiểm cả 1280, 390 và 320; root segment ≥48 px, các nút còn lại ≥44 px, thử thứ tự, góc trong 6 px và một tầng reply. |
| Đã đóng, mất đích, Bạn đã gửi | So 224 PNG trước/sau trong bảy trạng thái × 320/390/1280/1440 × sáng/tối × chữ 100/200%. Mỗi SHA có 64 ảnh riêng cho pending/rejected/approved/removed. Candidate không tràn ngang; riêng các chip own đạt 5,05–13,26:1, nút đo được tối thiểu 153×48 px. “Tải lại nhận xét” trên ApprovedOffPage phát sinh đúng một GET public mới. |
| Trang ẩn / rút sau kiểm duyệt | 64 PNG trước/sau; không tràn ngang ở 64/64. Nhãn Ưng tĩnh đạt 5,40:1 sáng và 7,68:1 tối. Ma trận là text scale trong DOM và Chromium trên Linux. OS IME, screen reader, thiết bị thật và browser zoom chưa chạy. |

[Ảnh trước/sau của Uncertain](../design/evidence/discussion-r4-366/uncertain.jpg) · [Căn cột 1280](../design/evidence/discussion-r4-366/desktop-1280.jpg) · [Own approved ở 390](../design/evidence/discussion-r4-366/own-approved-390.jpg) · [Trang ẩn ở 390](../design/evidence/discussion-r4-366/hidden-390.jpg). Các ảnh này giữ đúng nguồn base `ac77a0f` và candidate `10ea20b`.

### Lệnh và giới hạn nghiệm thu

| Lệnh | Kết quả tại checkpoint |
| --- | --- |
| `cargo xtask verify quick` | PASS trên [CI d2092b6](https://github.com/loveoverflowcom/vink/actions/runs/44720543672/job/136235158730). Linux local qua fmt, clippy SSR/hydrate/preview và SSR boundary; các test Identity cần Unix socket bị EPERM. |
| `cargo xtask ui-preview leptos verify` | Build renderer và media fixture thành công. Chromium desktop local bị chặn Unix socket; phép kiểm riêng với Headless Shell mở 48 cảnh thật của screen.reader và composition.feedback, chiều cao Gửi tối thiểu 48 px. |
| `VINK_TEST_DATABASE_URL=<scratch> cargo xtask verify web --scenario main --scenario endorsement --scenario auth --scenario review` | Main local trên base dừng ở bước đổi tab ẩn; main của PR dừng khi chờ hydration trang tiện ích, trước phần nhận xét. Chạy riêng endorsement trên `d2092b6`: PASS, gồm 20 px/48 px và aria-hidden cho Pen Flight/heart, phím Enter/Space, mất ACK, 409/401. Auth local dừng ở Identity Unix socket (EPERM). Review chạy độc lập phát hiện count 404 trên cả hai trang ẩn. |
| `cargo xtask verify postgres` và CMP | Dùng cluster PostgreSQL scratch cho kiểm local. Gradle offline local thiếu cache. Cả hai runner CMP trên [CI d2092b6](https://github.com/loveoverflowcom/vink/actions/runs/44720543672) đã qua cả build và kiểm renderer. |
| CSS, link, API coverage, icon parity | Repository checks nằm trong quick. Danh sách tên và path của 34 icon khớp ở registry, Rust và Kotlin. |

[Web CI d2092b6](https://github.com/loveoverflowcom/vink/actions/runs/44720543672/job/136235158753) giữ GET đầu: ở 120 ms chưa có skeleton, khoảng 320 ms thấy card tĩnh. Lỗi đầu và retry qua GET mới đều đạt. Sau đó phép đo ngay lúc sang dark gặp 4,36:1. Đo từng frame cùng Reader thấy nhãn field xuống 1,14:1, hai pill xuống 1,14:1 và 1,01:1; sau chuyển màu ổn định đạt 10,83/6,45/8,23:1. Bản sửa đổi nền và chữ cùng lúc, giữ chuyển động vòng focus; reduced motion tắt nội suy trong Nhận xét. Đối chứng CSS trên cùng binary cho kết quả tối thiểu 10,83/6,45/8,23:1 cho field/Ưng/Trả lời suốt 400 ms, recovery 7,68:1 ở hai kích thước trên hai lượt độc lập. Browser smoke kiểm ngưỡng 4,5:1 xuyên suốt lần đổi theme.

Ảnh guest cho thấy SignInAvailable mở dialog và Escape giữ nháp. SignInClosed có đúng lý do qua aria-describedby nhưng thiếu icon khóa theo ảnh 04; glyph đã được nối vào nút Gửi của riêng trạng thái này. Ca Refused 403 với transport bị chặn trong trình duyệt giữ chữ và nêu lý do.

PR giữ Draft trong khi #360 AC-04 còn FAIL. Trạng thái từng AC và các lần chạy mới hơn được cập nhật trên [PR #366](https://github.com/loveoverflowcom/vink/pull/366). Bằng chứng auth/CDP không chứng minh IME của hệ điều hành hoặc screen reader.

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

## UI-REVIEW-17 — rà reader thảo luận trên `develop@eb41d4b` (sau PR #59)

Phạm vi: so Design (Reader desktop/mobile, Guided Feedback mobile) với HEAD, và kiểm riêng retry
refetch, closed/open, auth-required, pending, uncertain, giữ chữ. Ảnh chụp bằng Chrome headless
(CDP) vào thư mục ngoài repo; không commit binary. Các ảnh có dữ liệu thảo luận dùng provider
fixture chặn ở trình duyệt (nhãn `intercepted-provider-contract-fixture`), không phải persistence.
Chưa tải ảnh lên Drive và chưa kiểm OS IME/screen reader.

| Kết quả | Chứng cứ | Giới hạn |
| --- | --- | --- |
| P2 đã sửa — “Tải lại nhận xét” khi đọc ban đầu lỗi là link cùng URL + `#feedback-region`: bấm khi server đã khỏe lại tạo 0 request, lỗi vẫn còn; chỉ reload cả trang mới hết (đúng như chú thích “same-URL link … only scrolls” ở `Unavailable`). Sau sửa là Button `resource.refetch()`: 2 GET và vùng nhận xét tải lại | Gỡ tạm `SELECT` của role runtime trên `comments`/`discussion_threads` ở cluster scratch để SSR lỗi | Một lỗi SSR mô phỏng bằng quyền DB, không phải mọi nguyên nhân |
| P2 đã sửa — lỗi “Xem thêm nhận xét” hiện ở `#comments-mode`, cách nút ~2.270px (y 746 vs 3014 ở 390px) nên người đọc không thấy; lời lỗi lại nói “chưa rõ kết quả” cho một lần đọc. Sau sửa: trạng thái ngay trên nút, phần đã tải giữ nguyên, bấm lại gọi đúng cursor cũ; lỗi “Xem thêm phản hồi” dùng cùng copy đọc | Fixture chặn 503 ở GET có `cursor` | Không có nhánh mạng thật bị đứt |
| Không lỗi — closed/open (Closed chặn gửi, giữ chữ), 401, `comments_closed`, pending, mất kết nối/chưa rõ: biên nhận đúng, chữ được giữ; đối soát gửi lại đúng body + `command_key` (2 POST giống hệt) và chỉ khi đó mới có biên nhận; auth-required: gửi disabled có lý do, chữ ở root và reply được giữ | Fixture POST chặn ở trình duyệt, LocalFixture `VINK_DEV_PRINCIPAL` và server không principal | Không chứng minh auth production, persistence, moderation |
| Không lỗi — reflow 320/390/834/1440, sáng/tối, chữ 100/200%: không tràn ngang, không có tầng reply thứ ba | `scrollWidth ≤ innerWidth` ở 16 tổ hợp × 5 trạng thái | Ảnh Design là mẫu có `3 cảm nhận mẫu`, “Gửi cảm nhận mẫu”, “Dẫn một câu trong bài”; không thêm số đếm/xếp hạng/sticker giả |

Ghi nhận, chưa sửa (P3, cần owner quyết): nhãn “Nhận xét” lặp ba lần (tiêu đề, nhãn intent
`Unspecified`, nhãn textarea) vì `FeedbackComposer` dùng chung Web/CMP; chữ nút “Trả lời/Gỡ”
lệch vào trong ~13px so với thân bình luận do padding nút văn bản; composer chưa có thẻ nền và CTA
chưa căn phải như Design; thân bình luận gốc ở desktop chưa thụt theo tên như Design. Đây là khác
biệt trình bày so với mẫu, không phải lỗi dữ liệu hay mất chữ.

### Review bổ sung trước tích hợp — 04/11/2026

Rà từ head PR #60 `c5606c7` trên base `eb41d4b`: lỗi làm mới cả vùng vẫn dùng
thông báo kết quả ghi chưa rõ, còn assertion retry kết thúc ngay lúc xóa lỗi cũ,
trước khi biết request hoàn tất. Bản sửa dùng copy riêng cho đọc/làm mới và giữ
lý do phiên/quyền. Browser smoke nay kiểm GET mới sau lỗi đọc ban đầu qua điều
hướng client, refresh lỗi giữ danh sách/chữ, hai GET phân trang dùng cùng cursor,
và nhận xét trang kế chỉ xuất hiện sau response thành công. Fixture mạng vẫn không
thay bằng chứng persistence hoặc auth production. Lượt browser đầu của review mới
bắt được mẫu chặn URL chưa bao phủ query cursor: sửa wildcard và đếm request của
cả lần lỗi lẫn retry, tránh dùng phản hồi lỗi từ server thật cho cursor giả làm
bằng chứng 503 fixture. Kết quả chạy gắn với head cuối
được ghi trong PR, không suy PASS từ các assertion vừa thêm.

PR #61 xử lý F3 đã ghi ở trên: `Unspecified` không còn nhãn intent ở cả Leptos/CMP,
nhãn textarea và các intent có tên vẫn giữ nguyên. Ghi nhận F3 ở đợt review ban đầu
là lịch sử trước sửa, không còn là việc đang mở sau khi tích hợp #61. Test render
Leptos và common test CMP kiểm hai trường hợp có/không có nhãn; gallery vẫn dùng
intent có tên, CMP vẫn là mẫu không nối dịch vụ.


## Nhận xét R3.1 / desktop R1.3 — #134, 10/11/2026

Base `48e0ec50314a915f730d768d5cf6cad6c636620a` (sau PR #138), source cuối
`cae0782` trên branch `codex/reader-r31-discussion-spacing`.
Đây là delta Web của [#134](https://github.com/loveoverflowcom/vink/issues/134),
không nghiệm thu toàn bộ #128/#129/#73 và không đóng issue.

Root/neo ở nền trang → một nhóm Phản hồi tonal → composer cùng hai biên →
khối lần gửi trước. Pattern giữ anatomy/footer DTO-free; app giữ cursor,
authority, command key, payload và attempt fence. Gộp lý do theo enum/facts,
không theo copy; count/value/disabled vẫn riêng từng target. `TextArea` Web thêm
variant opt-in, native composition owner không đổi. Context/thu ô nằm trong
composer, mở/thu trả focus đúng chỗ; ảnh sau đã được mở kiểm.

### Điều kiện kiểm

macOS ARM64, Rust 1.99.1, Node 22.15.0, Chrome headless 157.0.8103.40, PostgreSQL
17.11. Dùng `CARGO_INCREMENTAL=0`, `CARGO_PROFILE_DEV_DEBUG=0`,
`CARGO_PROFILE_TEST_DEBUG=0`; quick cuối dùng `CARGO_BUILD_JOBS=1`.
PostgreSQL riêng trong `/tmp`, không dùng database người viết.

| Check | Kết quả / boundary |
| --- | --- |
| `cargo xtask verify quick` | PASS — 9/9 nhóm; Web 122 tests, patterns 9, UI 54; log `/tmp/vink134-quick-serial.log` |
| `cargo xtask verify web` | PASS — SSR/hydration, PostgreSQL thật qua LocalFixture, commit → mất ACK → replay đúng key/body/target, pending, close/reopen, own remove, cursor retry, stale scope; LVE real writes/double tap/409/keyboard/401, discovery và restart. Log `/tmp/vink134-web-complete.log` |
| `cargo xtask ui-preview leptos verify` | PASS — assertions, 784 contexts / 1.151 PNG gồm tiles; giữ specimen discussion/textarea và contexts tại `/tmp/vink134-preview-evidence`. Log `/tmp/vink134-preview-serial.log` |
| `node apps/vink-web/e2e/browser-smoke.mjs --base http://127.0.0.1:3014 --chrome '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome' --state /tmp/vink134-after-final/state.json --phase discussion-only` | PASS trên `cae0782` — thêm geometry, descriptions, contrast và ảnh đúng vùng sau đổi chữ. Log `/tmp/vink134-after-final.log` |
| Self review | Không tìm thấy lỗi cần sửa còn lại trong diff cuối. Không đổi quyết định gửi/đối soát, admission, schema hoặc endorsement gate |

Web toàn luồng chạy trên source `eddc712`; các commit sau chỉ chỉnh focus/scroll/màu
ổn định khi chụp ảnh chữ lớn; lần `discussion-only` cuối chạy lại toàn bộ seam #134 với helper mới.
Đây là input CDP và LocalFixture, không phải production auth hoặc OS IME.

### Hình học và ảnh đã mở

GET fixture là `discussion.get.json` ở provider, chặn GET trong browser; không
lấy phản hồi mô phỏng làm bằng chứng persistence. Ca ghi/mất ACK riêng dùng
PostgreSQL thật: server đã commit `201`, đối soát trả `200`, chỉ một own-pending,
ACK muộn giữ chữ mới. HTML SSR được kiểm không chứa own-pending/rejected.

- Before: `/tmp/vink134-before/screenshots`, app tại base #138; PASS từ harness
  discussion cũ. Đã mở `discussion-contract-fixture-flat-390-light.png`,
  `...-1440-dark.png`, `...-320-200-dark.png`. Root/reply body lệch 13 CSS px.
- After: `/tmp/vink134-after-final/screenshots`, cùng fixture. Đã mở các ảnh
  `discussion-contract-fixture-flat-390-light.png`, `...-1440-dark.png`,
  `...-1440-200-dark.png`, `discussion-composer-390-100-dark.png`,
  `discussion-composer-320-200-dark.png`,
  `discussion-local-uncertain-390-100-dark.png`,
  `discussion-local-uncertain-320-200-dark.png`.
- 390/100: root/group/composer đều x=16..359, inset hai phía và text field 16px.
  320/200: đều x=12..293, inset 12px. Scrollbar 15px được trừ khỏi client width.
  Mép các vùng lệch 0px; tên/body cùng trục, nhãn action trong tolerance 1px
  của transparent border. Reply gap thực 16px, recipe giữa nhánh 32px.
- 320/390/834/1024/1440 sáng/tối; chữ 200% tại 320/390/1440: không tràn ngang,
  không tầng phản hồi thứ ba; từng control có description tồn tại. Uniform block
  có đúng một câu chung cho nhánh, target vẫn giữ count và unknown khác 0.
  Mixed/missing facts/Pending/Uncertain/Failed được kiểm riêng ở pure projection.
- Contrast CSS đã resolve của body/metadata/label: thấp nhất 5,40:1 sáng và
  7,68:1 tối. Composer 28px, delivery 20px. Uncertain chỉ một primary khả dụng,
  Gửi bị chặn có lý do; click thật tới Kiểm tra lần gửi ở 320/chữ200/reduced-motion.
  Focus field và nút đã mở xem; không suy screen-reader từ ảnh.

### Giới hạn / tiếp tục nghiệm thu

AC-01 và phần geometry của AC-02/05/07 có ảnh/metrics Web; AC-03 có pure projection,
DOM descriptions và LVE 401; AC-04 có controller/Web/PG regression; AC-06 có
recovery, focus, chữ lớn và reduced-motion. Không tick cả AC rộng hơn từ các bằng
chứng này. Pen Flight chưa có registry ở base; glyph vẫn thuộc #73 theo giao việc,
nên phần glyph AC-07 chưa hoàn tất. Giữ nhãn Hữu ích/count và slot hiện hữu.

NOT_RUN: thiết bị thật, OS IME tiếng Việt thật, VoiceOver/TalkBack hoặc screen reader
khác, native Android/iOS/CMP consumer, production auth/write readiness. Không chạy
lại model/Kani/lane PostgreSQL rộng vì không đổi domain/SQL; `verify web` vẫn dùng
PostgreSQL thật cho các ca ghi nêu trên. Không kiểm/chờ GitHub CI và không deploy.

Các lỗi setup ban đầu (database SQL_ASCII và PATH Node), assertion copy cũ, oracle
LVE chỉ tìm note riêng và lỗi ENOSPC khi ghi preview đã được giữ trong log cũ;
kết quả cuối ở bảng trên. Dọn đúng generated output/cache do task tạo để tiếp tục,
không thay ngưỡng contrast, permission, idempotency hoặc assertion dữ liệu để né lỗi.

Specimen Leptos đã mở: `composition.feedback--discussion-flat--dark--390--text100--focus--part1.png`, `--part2.png`, `--part3.png` trong thư mục evidence preview nêu trên. Đây là gallery có nhãn Mẫu, không phải consumer production.
