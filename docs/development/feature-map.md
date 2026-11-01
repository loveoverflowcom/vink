# Bản đồ tính năng và đường kiểm chứng Vink

**Đối chiếu mã nguồn:** 31/10/2026, nền `20635b45c856990638c06eaab011b9940da852dc`; hàng font tự host
thêm ngày 01/11/2026 cùng baseline Lavender; các hàng Bàn viết (CodeMirror, guard rời trang, xem
trước, export) đối chiếu lại ngày 01/11/2026 trên nền `c9d1e74` cho [#6](https://github.com/loveoverflowcom/vink/issues/6).
Shell/appearance và assertions liên quan được cập nhật ngày 01/11/2026 trên nền `6648200`
cho [#7](https://github.com/loveoverflowcom/vink/issues/7). Dashboard Bàn viết và assertions
`draftDashboard` đối chiếu ngày 01/11/2026 trên nền `cfad6a2` cho
[#8](https://github.com/loveoverflowcom/vink/issues/8).
Bản đồ mô tả skeleton trong checkout này; lần lập bản đồ chỉ đọc source và assertions,
không chạy lại ứng dụng. Cột kiểm thử cho biết nơi có assertion, không phải kết quả `PASS`.
Khi code đổi, đối chiếu lại các hàng liên quan và ghi mốc mới; kết quả chạy phải gắn với
revision/diff, môi trường và lệnh của chính lần chạy đó.

[Hướng dẫn phát triển](README.md) sở hữu setup, cấu hình, lệnh và giới hạn các lane.
[Verification guide](../../.agents/skills/vink-engineering/references/verification.md) giúp
chọn lớp bằng chứng; [requirements](../../prd/requirements.md) sở hữu tiêu chí sản phẩm.
Bản đồ này chỉ giúp tìm luồng, thao tác và nơi kiểm chứng, không thay thế các nguồn đó.

## Điều kiện để đi qua các luồng

- Dùng đúng `VINK_PUBLIC_ORIGIN` của môi trường local theo hướng dẫn phát triển. POST/PUT
  từ origin khác bị từ chối; mở được trang chưa chứng minh thao tác ghi hoạt động.
- Fixture trong [dev_seed.rs](../../services/vink-app/src/dev_seed.rs) có tác phẩm
  `[Mẫu] Mưa trên phố cũ`, `SAMPLE_WORK = 33333333-3333-4333-8333-333333333333`.
  Seed chỉ nạp khi gọi rõ ràng và giữ nguyên hàng đã tồn tại; một database local đã sửa
  không nhất thiết còn đúng nội dung mẫu ban đầu. Dùng dữ liệu riêng cho thử nghiệm ghi.
- Bàn viết cần JavaScript/hydration và principal phía server. `VINK_DEV_PRINCIPAL` chỉ là
  fixture local trên loopback; chưa có màn đăng nhập hoặc nguồn xác thực production.
  Không cấu hình authority thì API nháp trả `401`, dù khung HTML vẫn mở được.
- Các đường dẫn dưới đây tương đối với origin đang chạy. Định tuyến web nằm ở
  [App](../../apps/vink-web/src/lib.rs), API ở
  [router](../../services/vink-app/src/http/api.rs); phương thức và lỗi do
  [contract v1](../../services/vink-app/contracts/http/v1/README.md) sở hữu.

## Điểm vào và trạng thái cần nhận biết

| Luồng | Đường đi / thao tác | Kết quả và trạng thái hiện có | Owner |
| --- | --- | --- | --- |
| Khám phá tác phẩm công khai | `/`, chọn thẻ trong “Tác phẩm công khai”; “Khám phá” trên thanh điều hướng trở về `/` | Danh sách snapshot công khai; có trạng thái rỗng và lỗi dữ liệu. Chưa phải Tạp Chí tuyển chọn | `Home`, `WorkCardView` trong [pages/mod.rs](../../apps/vink-web/src/pages/mod.rs) |
| Đọc tác phẩm | `/tac-pham/{id}`, thử `SAMPLE_WORK` trên fixture nguyên trạng | HTML trước JavaScript có nội dung publication, metadata và canonical; không thấy phần nháp. Thiếu/chưa công khai trả `404`, dữ liệu không tới được trả `503` | `Reader`, `PublicationView` trong [pages/mod.rs](../../apps/vink-web/src/pages/mod.rs), [DocumentView](../../apps/vink-web/src/document.rs) |
| Điều hướng thích ứng | Rail: Khám phá → Bàn viết, Tôi ở chân; mobile: Khám phá → Bàn viết → Tôi | `/` vẫn là mặc định. Desktop 220px; tablet/editor 96px; thu/mở nhớ trên thiết bị và ghi đè ngữ cảnh. Mobile ẩn bar khi viết | [shell.rs](../../apps/vink-web/src/shell.rs), [spec shell](../design/README.md#shell-thích-ứng-7) |
| Tôi và chọn giao diện | Web/tablet: Tôi mở overlay; Mở trang Tôi tới `/toi`; mobile: Tôi tới `/toi` | Sáng/Tối/Hệ thống, lưu preference local; OS đổi được theo dõi bằng CSS khi chọn Hệ thống. Storage lỗi vẫn dùng được trong phiên. Không có dữ liệu profile/auth thật. Theme/rail/overlay không lưu hay rời editor; chuyển route dùng guard | [appearance.rs](../../apps/vink-web/src/appearance.rs), [me.rs](../../apps/vink-web/src/pages/me.rs), [NavigationGuard](../../apps/vink-web/src/navigation.rs) |
| Tìm và viết tiếp | “Bàn viết” → `/ban-viet` → “Viết tiếp” hoặc “Mở bản thảo” → `/ban-viet/{id}` | Resume là nháp mới cập nhật nhất theo API; row có title/version/updated_at. Tìm có/không dấu, Đ/đ và NFC/NFD chỉ trong tối đa 100 nháp đã tải; đổi sort/tìm không đổi resume. Loading, rỗng, không kết quả, lỗi và 401 riêng; query/sort giữ qua tải lại/retry | `DeskIndex`, `DraftListView`, `DraftMetadata` trong [desk.rs](../../apps/vink-web/src/pages/desk.rs), [draft_list.rs](../../apps/vink-web/src/draft_list.rs) |
| Tạo và đối chiếu kết quả | Heading “Tạo bản thảo”; khi rỗng “Viết bản thảo đầu tiên” → editor. Mất phản hồi → “Tải lại danh sách” → mở nháp tìm thấy hoặc chủ động “Tạo thêm bản thảo” | Một POST đang bay; lỗi 401/403/validation tách khỏi outcome chưa rõ (network, 503, bất thường). Không tự retry POST. Chỉ GET thành công mới mở khóa tạo thêm; GET không chứng minh POST nào đã ghi. Phản hồi muộn sau khi rời dashboard không điều hướng | `CreateDraftButton`, `CreateNotice`, `DeskIndex`; `CreateState` trong [draft_list.rs](../../apps/vink-web/src/draft_list.rs) |
| Viết và lưu nháp | `/ban-viet/{id}` → sửa “Tiêu đề”, “Nội dung” (CodeMirror 6) → “Lưu nháp”; có Enter ở tiêu đề và Ctrl/Cmd+S trong form (chờ IME commit) | `unsaved` → `saving` → `saved` khi server xác nhận; `failed` (thông báo có “Thử lưu lại”, “Tải bản đang viết”) hoặc `conflict` giữ nội dung trên trang. Lưu thủ công, chưa autosave | `DraftEditor`, `Editor` trong [desk.rs](../../apps/vink-web/src/pages/desk.rs), [EditorSession](../../apps/vink-web/src/desk_state.rs), [BodyEditor](../../apps/vink-web/src/editor.rs), [runtime](../../apps/vink-web/editor/README.md) |
| Xử lý xung đột | Mở cùng nháp ở hai tab, tab B lưu trước, tab A lưu bản cũ → “Lưu đè bằng nội dung đang viết” khi muốn giữ bản A | Save cũ nhận `409`, không ghi; bản A còn trong editor. Lưu đè dùng version được báo và vẫn có thể xung đột lần nữa. Muốn mở bản server thì “Tải bản đang viết” trước khi reload | `Editor`, `EditorSession`; owner/version được kiểm trong [PgStore](../../services/vink-app/src/persistence/postgres.rs) |
| Xem trước và tải `.txt` | “Xem trước” hoặc “Tải .txt” trên thanh công cụ editor | Hộp thoại đọc dùng `DocumentView` từ chữ hiện tại (kể cả chưa lưu); tệp `Tiêu đề.txt` gồm tiêu đề, dòng trống, nội dung đúng như đang viết. Không lưu, không gửi duyệt, không đổi trạng thái | `Editor`, [document.rs](../../apps/vink-web/src/document.rs) (`export_text`, `export_file_name`) |
| Rời editor khi chưa lưu | Link “Khám phá”/“Bàn viết”/“Tôi”, brand, “← Bàn viết”, back/forward của trình duyệt khi còn chữ chưa được xác nhận | Hộp thoại “Ở lại” (Escape, trả focus về trường đang viết) / “Lưu và rời trang” (chỉ đi sau ack của bản mới nhất). Khi lưu lỗi, xung đột hoặc còn treo: “Tải bản đang viết (.txt)” và “Rời mà không lưu…” có bước xác nhận. Reload/đóng tab dùng `beforeunload` của trình duyệt | [LeaveGuard](../../apps/vink-web/src/desk_state.rs), [NavigationGuard](../../apps/vink-web/src/navigation.rs), `Editor` |
| Kiểm tra tiến trình | `/health/live`, `/health/ready` | Liveness tách khỏi kết nối/schema database; readiness không khẳng định có auth, moderation, Discourse hay job | [health.rs](../../services/vink-app/src/http/health.rs) |

Các điểm bám dashboard: `draft-loading`, `draft-content` (`aria-busy`), `draft-empty`,
`draft-resume`/`resume-draft`, `draft-row`, `draft-search`, `draft-sort`, `draft-scope`,
`draft-result-count`, `draft-no-results`, `clear-draft-search`, `refresh-drafts`,
`retry-drafts`, `create-notice`, `reconcile-drafts`. Spec và delta nằm ở
[dashboard #8](../design/README.md#dashboard-bàn-viết-8).

Các điểm bám automation đã có trong Bàn viết: `data-testid=create-draft`, `save`, `save-status`,
`editor-toolbar`, `preview`, `export`, `save-failed`, `retry-save`, `conflict`, `keep-mine`,
`desk-error`, `editor-failed`; `data-testid=editor-body` (host CodeMirror, có `data-state`
`loading|ready|failed`; chữ ở `.cm-content`, từng dòng là `.cm-line`); `input[name=title]`;
hộp thoại `leave-editor` (`leave-message`, `leave-stay`, `leave-save`, `leave-recovery`,
`leave-export`, `leave-discard`, `discard-back`, `discard-confirm`) và `draft-preview`
(`preview-close`). Shell có `rail-toggle`, `profile-trigger`, `profile-menu`, `profile-close`, `open-me`;
radio theme có `name=overlay-theme|page-theme` và `value=light|dark|system`. `save-status` có `data-status` là `saved`, `unsaved`, `saving`, `failed`, `conflict`.
Đây là điểm tìm phần tử, không thay thế việc kiểm nội dung, version và dữ liệu bền vững.

## Assertion hiện có và phần còn thiếu

Nguồn kiểm trình duyệt là [browser-smoke.mjs](../../apps/vink-web/e2e/browser-smoke.mjs);
`serverHtml`, `adaptiveShell`, `shellWithDirtyEditor`, `mainPhase`, `afterRestartPhase` là các phần trong script, không phải lệnh CLI
mới. Nguồn kiểm PostgreSQL là [tests/postgres.rs](../../services/vink-app/tests/postgres.rs).
Chọn lệnh chạy từ hướng dẫn phát triển, không chạy thao tác phá dữ liệu trên database sử dụng thật.

| Kịch bản / điều cần quan sát | Assertion hiện có | Giới hạn cần giữ |
| --- | --- | --- |
| Nháp thay đổi nhưng người đọc vẫn thấy snapshot công khai, không có dòng riêng | `public_read_serves_the_published_snapshot_not_the_draft`, `server_html_has_public_content_and_never_private_drafts` (`postgres`); `serverHtml` (`web`) kiểm HTML reader và khung Bàn viết không có `DRAFT_ONLY_LINE` | Seed tạo publication; chưa chứng minh gửi duyệt/xuất bản/thu hồi theo ý định tác giả |
| Dashboard, tìm trong phạm vi và trạng thái | `vietnamese_search_preserves_source_and_provider_order` (`quick`); `draftDashboard` (`web`) dùng fixtures rỗng/100 row/title dài NFD, kiểm work_id/version/time, scope, tìm Đ/đ, sort, không kết quả, giữ query/sort qua 503/retry, 401, không N+1; `serverHtml` kiểm index không có title/body riêng | Fixtures HTTP chỉ thuộc test, không phải data sản phẩm; không kiểm tìm toàn thư viện hoặc profile/publication |
| Tạo nháp khi phản hồi mất hoặc đến muộn | `uncertain_create_needs_reconciliation_and_a_new_intent`, `explicit_pre_write_rejections_remain_distinct` (`quick`); `draftDashboard` giữ POST, chặn submit lặp, cho PostgreSQL commit thật rồi bỏ ack 201, kiểm GET lỗi không mở khóa, GET thành công tìm đúng một nháp không replay; response đến sau khi rời trang không kéo về editor | GET không xác định chắc chắn chủ nhân của một POST mất ack; chưa có idempotency key hay auth session thật |
| Dashboard narrow/theme/keyboard | `draftDashboard` kiểm thứ tự resume → search → sort bằng Tab, focus-visible, reflow 1440/1024/390/320px và 320px chữ 200%; PNG + `contexts.json`; `textContrast` đo các cặp chữ dashboard hai theme | Phải mở PNG trước kết luận thị giác; không chứng minh screen reader, zoom hay điện thoại thật |
| Hydration và chuyển từ reader sang Bàn viết | `mainPhase` kiểm dấu hydration, giữ marker JS qua điều hướng, không có console error/exception | Một Chrome headless desktop; không bao phủ mọi link hoặc trình duyệt |
| Gõ tiếng Việt, giữ dòng/khổ/thụt lề, lưu rồi reload | `vietnamese_text_lines_and_stanzas_round_trip` (`postgres`); `mainPhase` kiểm CodeMirror mount từ `/pkg/vink-editor.js` (trang đọc không tải), không còn textarea, so chữ trong `.cm-line`, document server và version sau lưu/reload; chờ window cũ được thay và editor mới hydrated trước khi đọc lại | CDP dùng `Input.insertText`, thử `imeSetComposition` có fallback; không phải IME hệ điều hành. Chưa kiểm Word/Docs paste hay thiết bị thật |
| Chỉ báo đã lưu cho đúng nội dung được xác nhận | Unit tests của [desk_state.rs](../../apps/vink-web/src/desk_state.rs) kiểm phản hồi cũ, sửa tiếp khi request đang bay, một save đang bay và lỗi; `mainPhase` kiểm phiên bản 2 sau nút lưu, Ctrl+S, và trạng thái `failed` khi request bị làm lỗi bằng CDP Fetch | Smoke chưa kiểm Enter ở tiêu đề; lỗi mạng là mô phỏng ở tầng Fetch của trình duyệt |
| Một EditorView giữ trạng thái | `shellWithDirtyEditor` đánh dấu `.cm-editor`, thu/mở rail, đổi theme qua radio, mở/đóng Tôi, resize mobile; so chữ/selection/dirty và không có PUT/POST, Tab trở lại và native blur/focus qua `EditorView.focus()`; `mainPhase` đổi `data-theme`, mở/đóng xem trước và hộp thoại rời trang, rồi kiểm cùng view, selection (`getSelection`), vị trí cuộn trang sau khi đóng xem trước, Ctrl+Z/Ctrl+Shift+Z | Selection/undo/cuộn đo qua DOM của một Chrome desktop; chưa kiểm cuộn khi bàn phím ảo mở |
| Xem trước và export dùng chữ hiện tại | `mainPhase` so khối trong `draft-preview` với khổ chưa lưu và kiểm version server không đổi; `afterRestartPhase` tải `.txt` thật qua `Browser.setDownloadBehavior` và so nội dung; tệp khôi phục từ hộp thoại được kiểm trong `mainPhase` | Mỗi phase chỉ tải một tệp vì Chrome hỏi trước lần tải thứ hai; chưa kiểm mở tệp trên Windows/macOS |
| Version cũ không ghi, giữ nội dung để người viết chọn | `stale_expected_version_conflicts_without_writing`, `concurrent_saves_of_one_version_have_exactly_one_winner` (`postgres`); `mainPhase` kiểm conflict và nút giữ bản local | Smoke mô phỏng bên ghi cạnh tranh bằng PUT trong cùng trang, chưa thao tác hai tab thật; test PostgreSQL chạy 8 save đồng thời |
| Quyền đọc/ghi riêng và lỗi API không bị gộp thành thành công | `another_actor_cannot_read_list_or_write_a_private_draft`, `api_distinguishes_outcomes_and_keeps_drafts_private` (`postgres`); `private_routes_are_closed_without_an_auth_authority`, `database_outage_is_reported_not_hidden` trong [http/mod.rs](../../services/vink-app/src/http/mod.rs) (`quick`) | Resolver header chỉ nằm trong test; fixture loopback không chứng minh session, logout/ban hoặc ACC-01/02 |
| Dữ liệu đã xác nhận còn sau restart, process dừng đúng | `confirmed_save_survives_a_new_connection_pool` (`postgres`); `afterRestartPhase` và orchestration trong [web.rs](../../xtask/src/web.rs) (`web`) | Không phải crash/replay/backup-restore; worker chỉ có lifecycle, không có job |
| Font tự host được phục vụ và tải | `mainPhase` kiểm trang đọc có Be Vietnam Pro 400 và Literata ở trạng thái `loaded`, file font trả `font/woff2` (`web`); repository checks so hash từng face/license với [provenance](../../apps/vink-web/assets/fonts/provenance.json) và kiểm URL font trong CSS/shell (`quick`) | Không chứng minh vị trí dấu, fallback trên máy thiếu font hay hiển thị trên thiết bị thật; kiểm theme trong `adaptiveShell` là bằng chứng riêng của shell |
| Shell, preference và focus | `adaptiveShell` kiểm rail 220/96, mặc định/ghi đè/reload, thứ tự/active route, tooltip focus/Escape, Tôi overlay focus đầu/Tab/Escape/outside/Đóng/resize, ba đích mobile, theme system/override/reload/storage denied, không overflow và bottom clearance ở 320px/chữ 200%; `vietnamese_initials_use_first_and_last_words` kiểm tên tiếng Việt/NFC/blank | Một Chrome headless Linux, resize CSS và cỡ chữ root; không phải thiết bị mobile/zoom trình duyệt/screen reader; initials fixture không là profile |
| Rời editor khi chưa lưu | Unit tests `LeaveGuard` (hủy khi đang lưu, ack muộn, đổi đích, chữ mới hơn, lỗi/409, xác nhận bỏ, bỏ khi save treo); `mainPhase` kiểm chữ đã xác nhận rời không hỏi, brand và back bị giữ, Escape/“Ở lại” giữ URL/chữ/focus, lưu rồi rời sau ack, hủy chuyển sang Tôi khi PUT bị giữ rồi thả không điều hướng muộn, lỗi → tải bản đang viết → rời mà không lưu → ack muộn không điều hướng lần nữa và không đánh dấu nháp khác đã lưu | `beforeunload` (reload/đóng tab) chưa có assertion; Chrome desktop headless, back qua `history.back()`; chưa kiểm cử chỉ vuốt back trên mobile |

## Công cụ và bằng chứng ngoài smoke

Web runner chuẩn bị database thử, server và Chrome; khi chạy tới bước tương ứng, nó ghi
`target/verify-web/server.log` và `smoke-state.json`. Thư mục này được làm mới ở lần chạy kế
tiếp; cần giữ bằng chứng có ích trước khi chạy lại nếu nhiệm vụ cần bàn giao. `adaptiveShell`
và `shellWithDirtyEditor` ghi PNG tại `target/verify-web/screenshots/` (1440/1024/390/320px,
sáng/tối, 320px chữ 200%, editor dirty và overlay). Manifest `contexts.json` ghi route, viewport, DPR, theme, root font và user agent. Không có video hay performance trace.
Assertions DOM và ảnh chưa mở không chứng minh bố cục đẹp.

Với yêu cầu visual QA, dùng công cụ browser có sẵn để chụp **và mở** ảnh, ghi viewport/theme/
text scale; xem [vink-ui-inspector](../../.agents/skills/vink-ui-inspector/SKILL.md).
IME thật, accessibility với screen reader, mobile thật và hiệu năng cần phiên kiểm riêng có
môi trường và quan sát cụ thể. Không tự đổi chúng thành `PASS` vì lane `web` thành công.

Chưa có đường đi cho đăng nhập thật, gửi duyệt, moderation, publish/withdraw, tuyển chọn Tạp Chí,
bình luận Discourse, AI job, export ngoài `.txt` và nhiều chương. Trạng thái chi tiết và điều kiện làm tiếp
ở [giới hạn skeleton](README.md) và [S-01–S-05](../architecture/decisions.md).

Khi một route, thao tác hoặc assertion đổi, cập nhật hàng liên quan cùng source owner;
[vink-maintain-verification](../../.agents/skills/vink-maintain-verification/SKILL.md) hướng dẫn
phân biệt tài liệu cũ, thiếu coverage và lỗi sản phẩm. Không cần cập nhật theo lịch cố định.
