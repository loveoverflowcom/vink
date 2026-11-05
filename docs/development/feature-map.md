# Bản đồ tính năng và đường kiểm chứng Vink

**Đối chiếu mã nguồn:** 31/10/2026, nền `20635b45c856990638c06eaab011b9940da852dc`; hàng font tự host
thêm ngày 01/11/2026 cùng baseline Lavender; các hàng Bàn viết (CodeMirror, guard rời trang, xem
trước, export) đối chiếu lại ngày 01/11/2026 trên nền `c9d1e74` cho [#6](https://github.com/loveoverflowcom/vink/issues/6).
Shell/appearance và assertions liên quan được cập nhật ngày 01/11/2026 trên nền `6648200`
cho [#7](https://github.com/loveoverflowcom/vink/issues/7). Dashboard Bàn viết và assertions
`draftDashboard` đối chiếu ngày 01/11/2026 trên nền `cfad6a2` cho
[#8](https://github.com/loveoverflowcom/vink/issues/8).
Khám phá/reader và assertions `publicReading` đối chiếu ngày 01/11/2026 trên nền `c6a8a29`
cho [#9](https://github.com/loveoverflowcom/vink/issues/9), theo cập nhật v3 của #14.
Logo/theme và assertions `brandAssets`/`assertBrand` đối chiếu lại ngày 03/11/2026 trên nền
`cbcbe0d` cho [#34](https://github.com/loveoverflowcom/vink/issues/34), thay asset #10.
Search công khai và ngữ cảnh reader đối chiếu ngày 01/11/2026 trên nền `75ccb41c` cho lát cắt A
của [#14](https://github.com/loveoverflowcom/vink/issues/14); [spec r2](../design/public-reading.md#layoutspec-r2--tìm-trong-danh-sách-và-giữ-ngữ-cảnh-14-lát-cắt-a).
Mật độ Explore và assertions hình học/search/byline trong `publicReading` đối chiếu
ngày 03/11/2026 trên nền `0e69412`; [LayoutSpec r3 và bằng chứng UI-REPAIR-02](../design/ui-repair-02.md).
Phân cấp Explore, nhịp header reader và hàng bản thảo gọn đối chiếu ngày 04/11/2026 trên nền
`27927c4`; [LayoutSpec và bằng chứng UI-REPAIR-14/15/16](../design/ui-repair-14-16.md).
Readiness, provisioning PostgreSQL theo owner và role tách đối chiếu ngày 02/11/2026 trên nền
`5afb00b` cho commit 1 của [ranh giới PostgreSQL/Identity](../architecture/identity-postgres-boundaries.md);
hàng Identity đối chiếu ngày 02/11/2026 trên nền `6357922` (commit 2); hàng App ↔ Identity và
backup/restore trên nền `255ce77` (commit 3).
Mục UI System preview thêm ngày 02/11/2026 trên nền `b358794` cho
[#19](https://github.com/loveoverflowcom/vink/issues/19) (UIS-01); hàng trạng thái/phản hồi thêm ngày
02/11/2026 trên nền `f3ca042` cho [#21](https://github.com/loveoverflowcom/vink/issues/21) (UIS-03);
hàng shell/điều hướng và mục UIS-04 đối chiếu ngày 03/11/2026 trên nền `3a02409` cho
[#22](https://github.com/loveoverflowcom/vink/issues/22).
Ranh giới lưu nháp/hash/ID và assertions RUST-CORE-01 đối chiếu ngày 02/11/2026 trên nền
`3a024091df6eb1da962ad3fa08750954c0b83a4f`; [HOW/WHY và bất biến](literary-domain-core.md).
Chrome editor, retry có đối chiếu và assertions `editorChrome` đối chiếu ngày 03/11/2026
trên nền `0fc8c04`; [LayoutSpec và bằng chứng UI-REPAIR-05](../design/ui-repair-05.md).
State/recovery và focus capture đối chiếu ngày 03/11/2026 trên nền `8045d45`;
[UI-REPAIR-06/07](../design/ui-repair-06-07.md) ghi source fingerprint và kết quả
từng renderer/host, tách ảnh nguyên bản, specimen và ảnh native.
CTA chính của panel recovery Explore/Reader đối chiếu ngày 04/11/2026 trên nền `c6aa486`;
[UI-REVIEW-19](../design/ui-review-19-recovery-views.md).
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

Logo selected #34 trong brand rail/mobile là ảnh trang trí của link trang chủ hiện có, cùng
accessible name/dirty guard. Owner: `shell.rs`, `appearance.rs`, `vink.css`,
[assets/brand](../../apps/vink-web/assets/brand/README.md) và pipeline `xtask`.
`brandAssets` kiểm SVG chuẩn/compact, PNG thật/MIME/decode 24–512px, manifest và bootstrap trước hydration;
việc chặn `/pkg/vink.js` sinh đúng hai lỗi `import()` bị từ chối, được kiểm rồi loại khỏi
điều kiện “không có console error” (mọi lỗi khác vẫn làm smoke thất bại). Smoke giả lập
`prefers-color-scheme: light` làm hệ thống mặc định, nên không phụ thuộc giao diện máy chạy.
`assertBrand` kiểm cặp theme/geometry qua các lựa chọn, reload, OS và storage denied trong
`adaptiveShell`. Ảnh `brand-before-hydration-*` và các ảnh shell dùng cùng fixture local;
phải mở chúng để nhận xét thị giác. Manifest chưa chứng minh cài app/launcher trên OS thật.

| Luồng | Đường đi / thao tác | Kết quả và trạng thái hiện có | Owner |
| --- | --- | --- | --- |
| Thử biểu cảm · Mẫu | `/thu-nghiem/bieu-cam` → chọn một hình để phát một lần | 16 poster tĩnh; Unicode nguyên bản, cap 2, reduced/hidden/offscreen và lỗi quay về poster/Unicode; không lưu reaction/sticker. Không có picker sản phẩm. | [expressions.rs](../../apps/vink-web/src/expressions.rs), [catalog/policy](../../packages/expressions/README.md), [LayoutSpec/registry](expressions.md) |
| Khám phá tác phẩm công khai | `/`, form GET “Tìm trong danh sách” → `/?q=…`; chọn link “Đọc tác phẩm” | Luồng v4 hữu hạn tối đa 12 bài theo ngày đăng; công cụ gọn, scope nối với input, avatar/tên/ngày cùng byline; tìm title/tên tác giả có/không dấu, Đ/đ, NFC/NFD trong cards đã tải, giữ thứ tự/chữ gốc và ghi rõ phạm vi. Loading/rỗng/không kết quả/unavailable riêng, có điểm dừng. Chưa có full-library/tags/intent/curation/feedback/bookmark | `Home`, `PublicListView`, `WorkCardView` trong [pages/mod.rs](../../apps/vink-web/src/pages/mod.rs), [public_list.rs](../../apps/vink-web/src/public_list.rs), [search.rs](../../apps/vink-web/src/search.rs) |
| Đọc tác phẩm | `/tac-pham/{id}?q=…` → “Khám phá” về `/?q=…#work-{id}`; Back/Forward, reload | Giữ query và đích hàng đọc bằng URL; sau khi list mount thì cuộn/focus về hàng còn khớp, không nhận đích ngoài. Title dài wrap, đoạn/khổ/dòng/thụt lề giữ nguyên. Public HTML/metadata/canonical trước JS; canonical bỏ q/hash. Thiếu/chưa công khai `404`, lỗi dữ liệu `503`, retry giữ URL/query | `Reader`, `PublicationView` và restore/remember position trong [pages/mod.rs](../../apps/vink-web/src/pages/mod.rs), [DocumentView](../../apps/vink-web/src/document.rs) |
| Điều hướng thích ứng | Rail: Khám phá → Bàn viết, Tôi ở chân; mobile: Khám phá → Bàn viết → Tôi | `/` vẫn là mặc định. Desktop 220px; tablet/editor 96px; thu/mở nhớ trên thiết bị và ghi đè ngữ cảnh. Mobile ẩn bar khi viết. Route lạ (`/ban-vietnam`, `/tac-pham/a/b`) là 404, không sáng đích. Back/Forward khi Tôi mở chỉ đóng Tôi, kể cả lịch sử fragment nội trang ([UI-REVIEW-20](../design/ui-review-20.md)) | [shell.rs](../../apps/vink-web/src/shell.rs) (`Destination`), primitive [vink-ui navigation](../../packages/ui-system/leptos/src/navigation.rs), [spec shell](../design/README.md#shell-thích-ứng-7) |
| Tôi và chọn giao diện | Web/tablet: Tôi mở overlay; Mở trang Tôi tới `/toi`; mobile: Tôi tới `/toi` | Hai vùng Tài khoản/Giao diện dùng chung `AccountStatus`/`ThemePicker`, avatar trung tính và help phạm vi thiết bị ([UI-REPAIR-04](../design/ui-repair-04.md)). Sáng/Tối/Hệ thống, lưu preference local; OS đổi được theo dõi bằng CSS khi chọn Hệ thống. Storage lỗi vẫn dùng được trong phiên. Không có dữ liệu profile/auth thật. Theme/rail/overlay không lưu hay rời editor; chuyển route dùng guard | [appearance.rs](../../apps/vink-web/src/appearance.rs), [me.rs](../../apps/vink-web/src/pages/me.rs), [NavigationGuard](../../apps/vink-web/src/navigation.rs) |
| Tìm và viết tiếp | “Bàn viết” → `/ban-viet` → “Viết tiếp” hoặc “Mở bản thảo” → `/ban-viet/{id}` | Resume là nháp mới cập nhật nhất theo API; row có title/version/updated_at. Tiêu đề resume dài hơn 60 Unicode scalar hiển thị tối đa ba dòng khi thẻ hẹp, nút mở/thu gọn giữ nguyên chữ; thẻ rộng hiện toàn bộ title và CTA bên cạnh. Tìm có/không dấu, Đ/đ và NFC/NFD chỉ trong tối đa 100 nháp đã tải; đổi sort/tìm không đổi resume. Loading, rỗng, không kết quả, lỗi, 401 và 403 riêng; query/sort giữ qua tải lại/retry | `DeskIndex`, `DraftListView` trong [desk.rs](../../apps/vink-web/src/pages/desk.rs), [draft_list.rs](../../apps/vink-web/src/draft_list.rs), `DraftResume` trong [vink-patterns](../../packages/vink-patterns/leptos/src/lib.rs) |
| Tạo và đối chiếu kết quả | Heading “Tạo bản thảo”; khi rỗng “Viết bản thảo đầu tiên” → editor. Mất phản hồi → “Tải lại danh sách” → mở nháp tìm thấy hoặc chủ động “Tạo thêm bản thảo” | Một POST đang bay; lỗi 401/403/validation tách khỏi outcome chưa rõ (network, 503, bất thường). Không tự retry POST. Chỉ GET thành công mới mở khóa tạo thêm; GET không chứng minh POST nào đã ghi. Phản hồi muộn sau khi rời dashboard không điều hướng | `CreateDraftButton`, `CreateNotice`, `DeskIndex`; `CreateState` trong [draft_list.rs](../../apps/vink-web/src/draft_list.rs) |
| Viết và lưu nháp | `/ban-viet/{id}` → sửa “Tiêu đề”, “Nội dung” (CodeMirror 6) → “Lưu nháp”; có Enter ở tiêu đề và Ctrl/Cmd+S trong form (chờ IME commit) | `unsaved` → `saving` → `saved` khi server xác nhận; `failed`/outcome chưa rõ có chữ, retry/export; `conflict` giữ nội dung. Mọi lần thử lưu từ Failed đọc lại version trước: GET lỗi không PUT, version khác mở conflict, cùng version mới gửi PUT có CAS. Toolbar wrap, sticky chỉ khi editor đủ rộng/cửa sổ đủ cao; hướng dẫn ngắt dòng trong details. Lưu thủ công, chưa autosave | `DraftEditor`, `Editor` trong [desk.rs](../../apps/vink-web/src/pages/desk.rs), [API client](../../apps/vink-web/src/api.rs), [EditorSession](../../apps/vink-web/src/desk_state.rs), [BodyEditor](../../apps/vink-web/src/editor.rs), [runtime](../../apps/vink-web/editor/README.md), [UI-REPAIR-05](../design/ui-repair-05.md) |
| Xử lý xung đột | Mở cùng nháp ở hai tab, tab B lưu trước, tab A lưu bản cũ → “Lưu đè bằng nội dung đang viết” khi muốn giữ bản A | Save cũ nhận `409`, không ghi; bản A còn trong editor. Lưu đè dùng version được báo và vẫn có thể xung đột lần nữa. Muốn mở bản server thì “Tải bản đang viết” trước khi reload | `Editor`, `EditorSession`; owner/version được kiểm trong [PgStore](../../services/vink-app/src/persistence/postgres.rs) |
| Xem trước và tải `.txt` | “Xem trước” hoặc “Tải .txt” trên thanh công cụ editor | Hộp thoại đọc dùng `DocumentView` từ chữ hiện tại (kể cả chưa lưu); tệp `Tiêu đề.txt` gồm tiêu đề, dòng trống, nội dung đúng như đang viết. Không lưu, không gửi duyệt, không đổi trạng thái | `Editor`, [document.rs](../../apps/vink-web/src/document.rs) (`export_text`, `export_file_name`) |
| Rời editor khi chưa lưu | Link “Khám phá”/“Bàn viết”/“Tôi”, brand, “← Bàn viết”, back/forward của trình duyệt khi còn chữ chưa được xác nhận | Hộp thoại “Ở lại” (Escape, trả focus về trường đang viết) / “Lưu và rời trang” (chỉ đi sau ack của bản mới nhất). Khi lưu lỗi, xung đột hoặc còn treo: “Tải bản đang viết (.txt)” và “Rời mà không lưu…” có bước xác nhận. Reload/đóng tab dùng `beforeunload` của trình duyệt | [LeaveGuard](../../apps/vink-web/src/desk_state.rs), [NavigationGuard](../../apps/vink-web/src/navigation.rs), `Editor` |
| Kiểm tra tiến trình | `/health/live`, `/health/ready` | Liveness tách khỏi kết nối/schema database; readiness `200` chỉ khi lịch sử migration trùng migration nhúng (`pending_migrations`, `diverged` là `503`); báo `checks.role`, production từ chối role `privileged`; Identity chỉ là thông tin, không làm App mất readiness. Không khẳng định có auth, moderation, Discourse hay job | [health.rs](../../services/vink-app/src/http/health.rs), [vink-store-postgres](../../packages/store-postgres/src/migration.rs) |
| Bàn viết khi có Identity | Request API riêng tư với `Authorization: Bearer` khi App cấu hình `VINK_IDENTITY_SOCKET` | Không credential `401`; credential mà chưa authority/bị từ chối/chưa có binding `401`; Identity dừng, quá hạn, trả lời hỏng hoặc socket do user khác phục vụ `503 dependency_unavailable`. Đọc công khai không gọi Identity. Local fixture (local + loopback) vẫn quyết định khi được bật | [identity](../../services/vink-app/src/identity/mod.rs), [client](../../services/vink-app/src/identity/client.rs) |
| Identity riêng, chưa có authority | `vink-identity serve` trên `VINK_IDENTITY_SOCKET`; `GET /identity/v1/capabilities`, `POST /identity/v1/verify-credential` | Caller đúng uid: capabilities `not_configured`, verify `501 authority_not_configured`; sai/thiếu caller `401`; payload sai `400`/`413`; version khác `404`; quá 2 s `503`. Readiness cần DB đúng schema và caller đã cấu hình | [vink-identity](../../services/vink-identity/README.md), [contract v1](../../services/vink-identity/contracts/rpc/v1/README.md) |
| Chuẩn bị PostgreSQL theo owner | `services/postgres/provision.sh [--check]`, `vink-app migrate` | DB/role App và Identity; DB cũ được nhận quyền; chạy lại không đổi password; migrate chỉ bằng `VINK_MIGRATE_DATABASE_URL`, thiếu quyền thoát 1 | [services/postgres](../../services/postgres/README.md), [main.rs](../../services/vink-app/src/main.rs) |

Các điểm bám dashboard: `draft-loading`, `draft-content` (`aria-busy`), `draft-empty`,
`draft-resume`/`resume-draft`, `draft-row`, `draft-search`, `draft-sort`, `draft-scope`,
`draft-result-count`, `draft-no-results`, `clear-draft-search`, `refresh-drafts`,
`retry-drafts`, `create-notice`, `reconcile-drafts`. Spec và delta nằm ở
[dashboard #8](../design/README.md#dashboard-bàn-viết-8).

Các action đã migrate có ID trên button (`#create-draft`, `#save`, `#preview`, `#export`,
`#retry-save`, `#keep-mine`, `#leave-stay`, `#leave-save`, `#leave-export`, `#leave-discard`,
`#discard-back`, `#discard-confirm`, `#preview-close`, `#profile-close`, `#open-me`);
field có ID `#draft-search`/`#draft-sort`/`#draft-title`. Trạng thái vẫn dùng `data-testid`:
`save-status`, `editor-toolbar`, `save-failed`, `conflict`, `desk-error`, `editor-failed`;
`data-testid=editor-body` là host CodeMirror (`data-state=loading|ready|failed`, chữ ở
`.cm-content`, từng dòng `.cm-line`); `input[name=title]` là trường tiêu đề. Dialog có
`data-testid=leave-editor|draft-preview`, lời giải thích/phục hồi có `leave-message|leave-recovery`;
button bên trong dùng các ID đã liệt kê. Dashboard refresh/retry/reconcile có ID
`#refresh-drafts`, `#retry-drafts`, `#reconcile-drafts`, `#clear-draft-search`.
Shell có ID `navigation-toggle`, `navigation-primary`, `navigation-tooltip`, `profile-trigger`,
`bottom-navigation` và `data-testid=profile-menu`; nút tiện ích dùng `#profile-close`, `#open-me`.
radio theme có `name=overlay-theme|page-theme` và `value=light|dark|system`. `save-status` có `data-status` là `saved`, `unsaved`, `saving`, `failed`, `conflict`.
Đây là điểm tìm phần tử, không thay thế việc kiểm nội dung, version và dữ liệu bền vững.

## Assertion hiện có và phần còn thiếu

Nguồn kiểm trình duyệt là [browser-smoke.mjs](../../apps/vink-web/e2e/browser-smoke.mjs);
`serverHtml`, `adaptiveShell`, `accountAppearance`, `appearanceFirstPaint`, `shellWithDirtyEditor`, `mainPhase`, `afterRestartPhase` là các phần trong script, không phải lệnh CLI
mới. Nguồn kiểm PostgreSQL là [tests/postgres.rs](../../services/vink-app/tests/postgres.rs),
[store tests](../../packages/store-postgres/tests/postgres.rs) và [provision.rs](../../xtask/src/provision.rs).
Chọn lệnh chạy từ hướng dẫn phát triển, không chạy thao tác phá dữ liệu trên database sử dụng thật.

| Kịch bản / điều cần quan sát | Assertion hiện có | Giới hạn cần giữ |
| --- | --- | --- |
| Raw save chỉ vào port sau validate, không đổi HTTP contract | `application::tests` kiểm actor → version → title → document, invalid không gọi store, NFD tới port là NFC và outcomes giữ riêng; `invalid_http_save_input_never_changes_the_durable_draft` (`postgres`) kiểm `422`/field và dữ liệu không đổi sau mỗi reject, body sai của anonymous vẫn `401` | RecordingStore không chứng minh CAS; HTTP chạy trên PostgreSQL thật với test resolver, chưa có auth authority |
| Validated type không bị dựng từ JSON/raw hoặc trộn ID | Doctests compile-fail của `vink-app` trong `quick`; `domain::ids::tests` kiểm UUID sai và giữ v4/v7 hiện có | Không áp thêm nil/version policy; type không thay owner/version predicate |
| Hash canonical và working draft chưa có title | `canonical_hash_matches_fixed_v1_vector_for_nfc_and_nfd`, `hash_keeps_title_line_lengths_case_and_indentation_significant`, tests document/title/`revision_requires_title` (`quick`); `empty_untitled_working_draft_can_be_saved_and_loaded` (`postgres`) | Hash format/schema không đổi; chưa có stored-hash reader hay submit command |
| Nháp thay đổi nhưng người đọc vẫn thấy snapshot công khai, không có dòng riêng | `public_read_serves_the_published_snapshot_not_the_draft`, `server_html_has_public_content_and_never_private_drafts` (`postgres`) kiểm draft save và revision chưa được trỏ không đổi public API/list/SSR; `serverHtml` (`web`) kiểm lại sau PUT thật, HTML reader/Khám phá và khung Bàn viết không có chữ riêng | Seed tạo publication; chưa chứng minh gửi duyệt/xuất bản/thu hồi theo ý định tác giả |
| Đọc công khai có điều kiện (READ-01) | `http::conditional::tests` (`quick`) kiểm grammar `If-None-Match` (`*`, `W/`, list/OWS/phần tử rỗng, dấu phẩy trong tag, nhiều dòng header), field sai/quá cỡ bị bỏ qua, so khớp đúng tag và 304 không body; `database_outage_is_reported_not_hidden` kiểm `*` lúc mất PostgreSQL vẫn 503 `no-store`. `conditional_reads::*` (`postgres`) kiểm 200→304, GET/HEAD, nháp riêng không đổi tag, đổi tên tác giả/con trỏ revision/thêm-bớt/thứ tự/hòa thời điểm/danh sách rỗng ra tag mới, tag cũ sau rút bài là 404, rename đồng thời không ghép tag với body khác. `conditionalPublicReads` (`web`) kiểm wire 304 qua CDP và application vẫn nhận 200 đủ byte | Rút bài/đổi tên là SQL fixture vì chưa có lệnh publish/withdraw; 304 vẫn chạy SQL. Chỉ Chrome headless; số đo cục bộ ở [READ-01](public-conditional-reads.md) |
| Đọc công khai cache-first trong tab (READ-02) | `freshness::tests` (`quick`) kiểm ngưỡng 119/120/180 s, đọc không gia hạn, spread/backoff/`Retry-After` có trần, single-flight, manual/invalidate, 404 xóa và response muộn không hồi sinh, reset scope, `max_display_age`, GC/trần entry, raw 304 → một lần không điều kiện, chuỗi ngẫu nhiên. `cacheFirstPublicReads` (`web`) kiểm hydration không request, quay lại render ngay không loading/request, tín hiệu lifecycle đồng thời → một request có validator (wire 304), tab ẩn không request, 503 giữ danh sách, reader báo bản mới thay vì thay chữ, 404 xóa chữ và card | Đồng hồ `performance.now` và `visibilityState` do test điều khiển; timer 120 s thật chỉ kiểm ở unit. Chưa có mutation để gọi `invalidate`; CMP/native NOT_IMPLEMENTED. Xem [READ-02](public-read-freshness.md) |
| Khám phá hữu hạn, trạng thái và reader | `public_list::tests` (`quick`) kiểm title/author/NFC/NFD/Đ/đ/order/chữ gốc, link q encoding và đích nội bộ; test `draft_list` giữ hồi quy helper. `no_publications_is_an_empty_collection_and_not_an_outage` (`postgres`); `database_outage_is_reported_not_hidden` (`quick`) kiểm SSR có q vẫn 503, echo query và canonical. `serverHtml` kiểm SSR tìm title/author/no-results, public HTML/metadata/404/privacy; `publicReading` kiểm hydrated GET, clear/retry, query/hash/focus qua reader, Back/Forward/reload, không fetch thêm list/manuscript khi tìm; loading/rỗng/no-results/503 riêng | Browser reload/back dùng seed thật; nhiều tác giả/title dài/lỗi API là HTTP fixtures có nhãn. Source assertion không là bằng chứng đã chạy; không có contract cho tương tác cộng đồng |
| Reader/Explore responsive và chữ an toàn | `publicReading` kiểm markup là text, khổ/thụt lề/đoạn, không overflow tại 1440/1024/834/390/320px, hai theme, root text 100/200%; Reader hẹp giữ H1 đầy đủ/chọn được với phân cấp tối đa 1,4 lần chữ đọc; fixture title không ngắt từ và document rỗng dưới reduced motion; Tab từ return link qua ba đích dưới, focus và cuối trang không bị bar che; `bottomNavigationGeometry` kiểm nhãn tối đa hai dòng trong target ≥44px và bar ≤18% chiều cao viewport 900px; `textContrast` lấy các cặp public UI; PNG + contexts có geometry. [Bằng chứng UI-REPAIR-01](../design/ui-repair-01.md) | Ảnh chỉ thành bằng chứng thị giác sau khi mở; root text scale không phải browser zoom; headless focus do CDP giữ visible khi Chrome mở tab cập nhật. Không chứng minh screen reader hay thiết bị thật |
| Dashboard, tìm trong phạm vi và trạng thái | `vietnamese_search_preserves_source_and_provider_order` (`quick`); `draftDashboard` (`web`) dùng fixtures rỗng/1/100 row/title dài NFD, tên trống/lặp/từ liền; kiểm work_id/version/time, scope, tìm Đ/đ, sort, không kết quả, giữ query/sort qua 503/retry, 401/403, không N+1; sửa title/lưu thật rồi trở lại giữ work và version đã xác nhận; `serverHtml` kiểm index không có title/body riêng | Fixtures HTTP chỉ thuộc test, không phải data sản phẩm; không kiểm tìm toàn thư viện hoặc profile/publication |
| Tạo nháp khi phản hồi mất hoặc đến muộn | `uncertain_create_needs_reconciliation_and_a_new_intent`, `explicit_pre_write_rejections_remain_distinct` (`quick`); `draftDashboard` giữ POST, chặn submit lặp, cho PostgreSQL commit thật rồi bỏ ack 201, kiểm GET lỗi không mở khóa, GET thành công tìm đúng một nháp không replay; response đến sau khi rời trang không kéo về editor | GET không xác định chắc chắn chủ nhân của một POST mất ack; chưa có idempotency key hay auth session thật |
| Dashboard narrow/theme/keyboard | `draftDashboard` kiểm mở/thu gọn bằng Enter/Space, full title trong AX tree, thứ tự resume → search → sort bằng Tab, focus-visible và CTA clear bottom bar. Reflow 1440/1024/834/390/320px × sáng/tối × chữ 100/200%; title gọn ba dòng và công cụ trong viewport đầu ở 390/100%. PNG + `contexts.json` có geometry; `textContrast` đo các cặp chữ dashboard hai theme; [bằng chứng UI-REPAIR-03](../design/ui-repair-03.md) | Phải mở PNG trước kết luận thị giác; AX tree không phải một lượt dùng screen reader, root text scale không phải zoom hay điện thoại thật |
| Hydration và chuyển từ reader sang Bàn viết | `mainPhase` kiểm dấu hydration, giữ marker JS qua điều hướng, không có console error/exception | Một Chrome headless desktop; không bao phủ mọi link hoặc trình duyệt |
| Gõ tiếng Việt, giữ dòng/khổ/thụt lề, lưu rồi reload | `vietnamese_text_lines_and_stanzas_round_trip` (`postgres`); `mainPhase` kiểm CodeMirror mount từ `/pkg/vink-editor.js` (trang đọc không tải), không còn textarea, so chữ trong `.cm-line`, document server và version sau lưu/reload; chờ window cũ được thay và editor mới hydrated trước khi đọc lại | CDP dùng `Input.insertText`, thử `imeSetComposition` có fallback; không phải IME hệ điều hành. Chưa kiểm Word/Docs paste hay thiết bị thật |
| Chỉ báo đã lưu cho đúng nội dung được xác nhận | Unit tests của [desk_state.rs](../../apps/vink-web/src/desk_state.rs) kiểm phản hồi cũ, sửa tiếp khi request đang bay, một save đang bay và lỗi; `mainPhase` kiểm phiên bản 2, Ctrl+S, failed; `editorChrome` giữ PUT, click/shortcut lặp chỉ một request, gõ thêm trước ack vẫn unsaved và GET xác nhận snapshot cũ | Smoke chưa kiểm Enter ở tiêu đề; lỗi mạng là mô phỏng ở tầng Fetch của trình duyệt |
| Retry sau kết quả lưu chưa rõ | `editorChrome` làm hỏng request, retry sau GET cùng version; giữ response sau commit thật rồi bỏ ack, đối chiếu 403 không PUT, retry tiếp vẫn GET và conflict không phát PUT; explicit keep-local rồi reload. Giữ GET đối chiếu rồi xác nhận bỏ: GET muộn không khởi động PUT, không điều hướng hay đổi version server | Không có idempotency key; GET không xác nhận saved thay cho ack cũ. CAS vẫn là ranh giới chống ghi đè nếu có writer chen giữa GET/PUT |
| Chrome editor, reflow và dialog | `editorChrome`: 1440/1024/834/390/320 × light/dark × root text 100/200%, clean/dirty/saving/failed/conflict và preview/leave; target ≥44px, không overflow, recovery còn đường tới, Tab công cụ/title/body và nút an toàn trong dialog, đo tương phản sau transition hữu hạn, cửa sổ thấp và title focus không bị sticky che. PNG/manifest ghi geometry | Ảnh phải mở riêng; CSS text 200% không phải browser zoom. Viewport thấp không là bàn phím mềm thật; không xác nhận OS IME, Safari/Firefox hay screen reader |
| Một EditorView giữ trạng thái | `shellWithDirtyEditor` đánh dấu `.cm-editor`, thu/mở rail, đổi theme qua radio, mở/đóng Tôi, resize mobile; so chữ/selection/dirty và không có PUT/POST, Tab trở lại và native blur/focus qua `EditorView.focus()`; `mainPhase` đổi `data-theme`, mở/đóng xem trước và hộp thoại rời trang, rồi kiểm cùng view, selection (`getSelection`), vị trí cuộn trang sau khi đóng xem trước, Ctrl+Z/Ctrl+Shift+Z | Selection/undo/cuộn đo qua DOM của một Chrome desktop; chưa kiểm cuộn khi bàn phím ảo mở |
| Xem trước và export dùng chữ hiện tại | `mainPhase` so khối trong `draft-preview` với khổ chưa lưu và kiểm version server không đổi; `afterRestartPhase` tải `.txt` thật qua `Browser.setDownloadBehavior` và so nội dung; tệp khôi phục từ hộp thoại được kiểm trong `mainPhase` | Mỗi phase chỉ tải một tệp vì Chrome hỏi trước lần tải thứ hai; chưa kiểm mở tệp trên Windows/macOS |
| Version cũ không ghi, giữ nội dung để người viết chọn | `stale_expected_version_conflicts_without_writing`, `concurrent_saves_of_one_version_have_exactly_one_winner` (`postgres`); `mainPhase` kiểm conflict và nút giữ bản local | Smoke mô phỏng bên ghi cạnh tranh bằng PUT trong cùng trang, chưa thao tác hai tab thật; test PostgreSQL chạy 8 save đồng thời |
| Quyền đọc/ghi riêng và lỗi API không bị gộp thành thành công | `another_actor_cannot_read_list_or_write_a_private_draft`, `api_distinguishes_outcomes_and_keeps_drafts_private` (`postgres`); `private_routes_are_closed_without_an_auth_authority`, `database_outage_is_reported_not_hidden` trong [http/mod.rs](../../services/vink-app/src/http/mod.rs) (`quick`) | Resolver header chỉ nằm trong test; fixture loopback không chứng minh session, logout/ban hoặc ACC-01/02 |
| Role tách và database cũ còn nguyên sau provisioning | [tests/postgres.rs](../../packages/store-postgres/tests/postgres.rs) (lifecycle, `postgres`); [provision.rs](../../xtask/src/provision.rs) (`postgres`): snapshot dữ liệu + lịch sử trước/sau, runtime đọc lại tiêu đề NFD, thử DDL/TEMP/ghi `_sqlx_migrations`/`CREATE EXTENSION`/kết nối chéo bằng chính role, ca âm cấp `CREATE` thừa | Server tạm do test harness tạo; Compose thật và DB do binary base ghi chỉ có bằng chứng thủ công trong [verification](identity-postgres-verification.md); lane `web` vẫn chạy app bằng credential test harness |
| Identity đóng khi không chắc chắn, không lộ credential | `http::tests` của Identity (`quick`: caller theo peer, header/subject giả, payload, version, deadline, fixtures, redaction); [identity tests/postgres.rs](../../services/vink-identity/tests/postgres.rs) và [xtask identity.rs](../../xtask/src/identity.rs) (`postgres`: binary thật, socket thật, role đã provision) | Không có authority thật: chỉ fixture test có nhãn trả `authenticated`; App chưa tiêu thụ contract; chế độ mạng không tồn tại |
| App không cấp quyền khi Identity lỗi | `identity_tests` trong [http/mod.rs](../../services/vink-app/src/http/mod.rs) (`quick`: Identity giả trên socket thật — rejected/authenticated/no-authority `401`, 503/caller/malformed/outcome lạ/chậm `503`, sai uid server và mất socket `503`, header giả không đổi gì); `app_integration`, `app_refusals` trong [xtask identity.rs](../../xtask/src/identity.rs) (`postgres`: hai binary thật) | Không có authority thật; carrier Bearer là tạm thời; không có binding subject → principal |
| Backup/restore theo owner | `backup_restore` trong [provision.rs](../../xtask/src/provision.rs) (`postgres`): `pg_dump`/`pg_restore` App và Identity vào database khác, so dữ liệu/ID/Unicode/con trỏ/lịch sử/owner | Cùng cluster; không kiểm restore sang host khác hay khôi phục tại một thời điểm chung cho hai owner |
| Dữ liệu đã xác nhận còn sau restart, process dừng đúng | `confirmed_save_survives_a_new_connection_pool` (`postgres`); `afterRestartPhase` và orchestration trong [web.rs](../../xtask/src/web.rs) (`web`) | Không phải crash/replay/backup-restore; worker chỉ có lifecycle, không có job |
| Font tự host được phục vụ và tải | `mainPhase` kiểm trang đọc có Be Vietnam Pro 400 và Literata ở trạng thái `loaded`, file font trả `font/woff2` (`web`); repository checks so hash từng face/license với [provenance](../../apps/vink-web/assets/fonts/provenance.json) và kiểm URL font trong CSS/shell (`quick`) | Không chứng minh vị trí dấu, fallback trên máy thiếu font hay hiển thị trên thiết bị thật; kiểm theme trong `adaptiveShell` là bằng chứng riêng của shell |
| Shell, preference và focus | `adaptiveShell` kiểm rail 220/96, tên/`aria-expanded` của toggle và nút ≥44px không bị che khi thu, mặc định/ghi đè/reload, thứ tự/active route, link thật không phải tab, skip link focus main, modifier click để trình duyệt xử lý, 404 cho route lạ không sáng đích, ba lần bấm dồn dập chỉ còn đích cuối, tooltip focus/hover/Escape ngoài rail, Tôi overlay focus đầu/Tab/Escape/outside/Đóng/resize, Back/Forward khi Tôi mở chỉ đóng Tôi (kể cả fragment-only), Back/Forward không overlay vẫn đổi fragment tự nhiên, traversal bị giữ phục hồi scroll khác0 sau native history, rồi Back sau mới rời, ba đích mobile, theme system/override/reload/storage denied, không overflow và bottom clearance ở 320px/chữ 200%; unit `shell::tests` (đích theo segment, ngữ cảnh editor, thứ tự/href), `public_list::tests` (return context round-trip qua parser của router, link lạ chỉ dựng URL nội bộ); `vietnamese_initials_use_first_and_last_words` kiểm tên tiếng Việt/NFC/blank | Một Chrome headless Linux, resize CSS và cỡ chữ root; không phải thiết bị mobile/zoom trình duyệt/screen reader; initials fixture không là profile |
| Tôi: tài khoản, radio và overlay có thể cuộn | `accountAppearance` (`web`) kiểm cùng copy thật/neutral avatar, ID duy nhất và nhóm native tách tên, ArrowLeft/ArrowRight/Space và theme intents liên tiếp đồng bộ hai view, focus/checked marker; overlay 1440/834 expanded/compact cao 900/360 và 1024 compact cao 900, ba theme × chữ 100/200%, không tràn ngang, Tab/Shift+Tab cuộn lộ hai đầu và Escape trả trigger. `adaptiveShell` kiểm stale close/reopen và aria-expanded, storage denied đồng bộ hai nơi; `brandAssets` thêm System/OS sáng và tối trước hydration; `/toi` 1440/1024/834/390/320 ba theme × chữ 100/200%, DOM Range kiểm từng từ tiếng Việt nguyên vẹn. `appearanceFirstPaint` ([UI-REVIEW-21](../design/ui-review-21-me-profile.md)) chặn bundle, đối chiếu root và cả hai checked native trước hydration tại 1440/320 × chữ 100/200%, explicit theo OS ngược và System theo OS sáng/tối; reload thường lấy mẫu từng frame tới hydration. `shellWithDirtyEditor` giữ route/hash/editor khi overlay tiêu thụ Back, fragment không overlay không mở guard rời trang, version server giữ nguyên; `mainPhase` thử Mở trang Tôi khi save treo và conflict vẫn giữ guard/chữ | Chrome headless, root text scale và input chương trình; ảnh cần mở riêng. Không xác nhận auth/profile, thiết bị thật hoặc screen reader |
| Rời editor khi chưa lưu | Unit tests `LeaveGuard` (hủy khi đang lưu, ack muộn, đổi đích, chữ mới hơn, lỗi/409, xác nhận bỏ, bỏ khi save treo); `mainPhase` kiểm chữ đã xác nhận rời không hỏi, brand và back bị giữ, Escape/“Ở lại” giữ URL/chữ/focus, lưu rồi rời sau ack, hủy chuyển sang Tôi khi PUT bị giữ rồi thả không điều hướng muộn, lỗi → tải bản đang viết → rời mà không lưu → ack muộn không điều hướng lần nữa và không đánh dấu nháp khác đã lưu | `beforeunload` (reload/đóng tab) chưa có assertion; Chrome desktop headless, back qua `history.back()`; chưa kiểm cử chỉ vuốt back trên mobile |

## Công cụ và bằng chứng ngoài smoke

Web runner chuẩn bị database thử, server và Chrome; khi chạy tới bước tương ứng, nó ghi
`target/verify-web/server.log` và `smoke-state.json`. Thư mục này được làm mới ở lần chạy kế
tiếp; cần giữ bằng chứng có ích trước khi chạy lại nếu nhiệm vụ cần bàn giao. `adaptiveShell`, `publicReading`, `editorChrome`
và `shellWithDirtyEditor` ghi PNG tại `target/verify-web/screenshots/` (1440/1024/834/390/320px,
sáng/tối, 320px chữ 200%, editor dirty và overlay). Manifest `contexts.json` ghi route, viewport, DPR, theme, root font và user agent. Không có video hay performance trace.
Assertions DOM và ảnh chưa mở không chứng minh bố cục đẹp.

Với yêu cầu visual QA, dùng công cụ browser có sẵn để chụp **và mở** ảnh, ghi viewport/theme/
text scale; xem [vink-ui-inspector](../../.agents/skills/vink-ui-inspector/SKILL.md).
IME thật, accessibility với screen reader, mobile thật và hiệu năng cần phiên kiểm riêng có
môi trường và quan sát cụ thể. Không tự đổi chúng thành `PASS` vì lane `web` thành công.

### UI System preview (UIS-01, UIS-02, UIS-03, UIS-04)

UI-REPAIR-06/07 bổ sung `StatePanel` action slot ở hai renderer. `feedback` SSR và
`VinkPanelTest` kiểm typed recovery cùng callback/link của caller; `publicReading`
và `draftDashboard` kiểm action nằm trong panel, clear trả focus về search,
GET retry giữ query/hash và reload document thật, 401/403/503 cùng uncertain-create
vẫn riêng. `captureRecovery` ghi 220 ảnh theo 11 state × 5 width × hai theme ×
text100/200; input chương trình và cỡ chữ CSS không là OS IME/browser zoom.

`focusedCaptures` trong preview smoke opt-in qua development guide: 36 selected
records liên kết raw ↔ các tile specimen; geometry tests gồm tọa độ phân số,
padding/clamp, specimen sau mốc 6000px, chia tile theo viewport và geometry sai.
Chrome kiểm PNG/ảnh không trắng, ID/theme, giữ bounds và phục hồi viewport/scroll/
focused DOM node. Manifest chỉ ghi CAPTURED; inspection/readback riêng.

Không phải route của app: hai preview chạy riêng, không gọi service. Owner và lệnh ở
[packages/ui-system](../../packages/ui-system/README.md).

| Luồng | Đường đi / thao tác | Assertion hiện có | Giới hạn |
| --- | --- | --- | --- |
| Token source → file sinh ra | Sửa `shared/tokens.json`, `cargo xtask ui-tokens` | `ui_tokens::tests` (vòng alias, alias treo/sai type, thiếu theme, đơn vị sai, field lạ, tương phản thấp, sinh hai lần không đổi, file commit khớp); `ui-tokens --check` trong `verify quick` | Tương phản là số học token |
| Preview Leptos | `cargo xtask ui-preview leptos run` → `http://127.0.0.1:3019/#<entry>/<scenario>`; tìm, chọn kịch bản, đổi Giao diện/Khung/Cỡ chữ/Chuyển động | [preview-smoke.mjs](../../packages/ui-system/leptos/preview/e2e/preview-smoke.mjs) (`structure`, `themes`, `densityAndMotion`, `search`, `reflow`, `captures`); test crate `vink-ui`/`vink-ui-preview` | Một Chrome headless, DPR 1; ảnh ở `target/ui-preview/leptos/captures` phải được mở |
| Preview CMP | `cargo xtask ui-preview cmp run` (cửa sổ desktop) | `VinkThemeTest` (common), `VinkScopeTest`, `PreviewTest`, `CaptureTest` (desktop) | JVM desktop off-screen; Android/iOS chỉ compile khi bật |
| Font CMP Desktop ([PR 08](../design/ui-repair-08.md)) | Be Vietnam Pro 400/600 và Literata TTF từ cùng WOFF2 Web, classpath `vink-fonts/`; fallback hệ thống có lý do khi thiếu/hỏng | `PreviewTest.selfHostedFacesDecodeOnDesktop` vẫn yêu cầu 3 face; `FontTest` kiểm metadata/trục/glyph tiếng Việt, đo font thật qua Compose 400/600 và missing/corrupt từng face; `FontCaptureTest` chụp cùng specimen với WOFF2 trước và TTF sau; `checks::desktop_font_derivative_checks_detect_corruption_and_unowned_files` kiểm hash | CI `ui-cmp` Linux và `ui-cmp-macos` chạy cùng lane; glyph/đo chữ không thay shaping/IME hay native acceptance; ảnh phải mở mới là visual evidence |
| Web dùng token sinh ra | `/pkg/vink.css` = `tokens.css` + `vink-ui.css` + `vink.css` | `verify web` hiện có (contrast hai theme, theme/reload/OS/storage denied, reflow) chạy trên CSS mới | Không thêm assertion web mới cho UIS-01 |
| Component nền UIS-02 ([#20](https://github.com/loveoverflowcom/vink/issues/20)) | Preview Leptos `#<họ>.<component>/<kịch bản>` (ví dụ `#action.button/pending`, `#field.form/invalid`, `#identity.avatar/image-error`); preview CMP chọn cùng ID | Test `vink-ui` (`action`, `field`, `media`, `identity` golden vectors, `layout`, `text`), `vink-ui-preview` (mọi kịch bản render, không ID trùng); `components` trong [preview-smoke.mjs](../../packages/ui-system/leptos/preview/e2e/preview-smoke.mjs); CMP `VinkIdentityTest`, `PreviewTest` (dispatch, selection, semantics), `CaptureTest` | Chưa có màn app dùng; IME thật, screen reader, Android/iOS runtime và hover CMP chưa chạy; contrast là màu resolve trên Chrome/Skia |
| Trạng thái/phản hồi UIS-03 ([#21](https://github.com/loveoverflowcom/vink/issues/21)) | Preview `#state.<họ>/<kịch bản>` (ví dụ `#state.error/partial`, `#state.dialog/session-expired`); sân thử `#state.playground/<recipe>` (đồng hồ thủ công, bước, nhật ký) cho 8 recipe | Generator `ui_states` + vectors chung (`state::tests` Rust, `VinkStateTest` JVM/iOS simulator); test `feedback` (render vùng/panel/notice/dialog), `vink-ui-preview` (`states::tests`: reducer có ticket bỏ double-click và ack muộn); `states` trong [preview-smoke.mjs](../../packages/ui-system/leptos/preview/e2e/preview-smoke.mjs) (vùng, intent, announce, snackbar theo đồng hồ, 8 recipe, focus dialog, motion giảm/không hoạt động/tab ẩn qua `visibilitychange`, frame có timestamp); CMP `StatePreviewTest`, `DrivenCaptureTest` | Chưa màn app nào dùng; lifecycle Android/iOS chưa có host; TalkBack/VoiceOver, giảm chuyển động của OS thật chưa chạy |
| Shell/điều hướng UIS-04 ([#22](https://github.com/loveoverflowcom/vink/issues/22)) | Preview `#navigation.<họ>/<kịch bản>` (ví dụ `#navigation.scaffold/toggle`, `#navigation.overlay/busy`); CMP thêm `#navigation.adapter/*` (back stack, deep link, chữ chưa lưu) | Generator `ui_navigation` + vectors chung (`navigation::tests` Rust, `VinkNavigationTest` JVM/iOS simulator khi bật native); `navigation` trong [preview-smoke.mjs](../../packages/ui-system/leptos/preview/e2e/preview-smoke.mjs) (thứ tự, link thật, toggle 96→220→96 bằng chuột/Enter/Space giữ trang/chữ/selection, tooltip, một modal, Back đóng lớp phủ trước, Escape trả focus); CMP `NavigationPreviewTest` (deep link round-trip/từ chối link ngoài, Back đóng lớp phủ trước, chữ chưa lưu giữ Back/điều hướng, đổi khung không remount, toggle giữ selection, thứ tự thanh dưới, một modal); app thật trong `adaptiveShell` (`web`) | Android predictive back, iOS swipe-back, lifecycle, TalkBack/VoiceOver chưa chạy; adapter CMP là host chẩn đoán, không phải app native |
| Initials của app | `appearance.rs::initials` → `vink_ui::identity::initials` (giữ `•` khi neutral) | `vietnamese_initials_use_first_and_last_words` (`quick`); `verify web` của shell | App hiện chỉ gọi với `None`; tên thật chưa có nguồn |

Chưa có đường đi cho đăng nhập thật, gửi duyệt, moderation, publish/withdraw, tuyển chọn Tạp Chí,
UI và kiểm duyệt bình luận, AI job, export ngoài `.txt` và nhiều chương. Trạng thái chi tiết và điều kiện làm tiếp
ở [giới hạn skeleton](README.md) và [S-01–S-05](../architecture/decisions.md).

Khi một route, thao tác hoặc assertion đổi, cập nhật hàng liên quan cùng source owner;
[vink-maintain-verification](../../.agents/skills/vink-maintain-verification/SKILL.md) hướng dẫn
phân biệt tài liệu cũ, thiếu coverage và lỗi sản phẩm. Không cần cập nhật theo lịch cố định.

## Bình luận hai tầng D2T-01/D2T-02a/D2T-02b (#41, #43)

Lát cắt 1 ngày 03/11/2026 trên nền `8045d45`; D2T-02a/b trên nền `31a62b1`. Có route API v1,
UI đọc thảo luận hai tầng D2T-03 bổ sung tại reader; production trả `401` cho lệnh ghi, LocalFixture chỉ là harness cục bộ. Owner và registry/gap: [bình luận hai tầng](discussion-core.md),
[ADR ownership đã chốt](../architecture/discussion-ownership.md).

| Điểm vào | Assertion có trong code/runner | Giới hạn |
| --- | --- | --- |
| `DiscussionThread::create_root/create_reply` | [Unit tests](../../services/vink-app/src/domain/discussion.rs): root A, reply B/C/D cùng root; context canonical; nhiều root; foreign/missing target; duplicate/self-reference, reject atomic và 20^4 chuỗi lệnh | State thuần, append-only, chưa restore/removal/auth/body/store/request/API; ID và thread/work binding bền vững phải kiểm tại owner thật |
| Model → trace → core | [Behavior owner](../../services/vink-app/behavior/discussion/README.md): Quint typecheck/scenarios/simulation/mutation/drift; `quint_generated_inputs_replay_against_the_production_creation_seam`, `quint_lifecycle_inputs_replay_against_the_production_decisions` (gồm receipt/retry) và `quint_reaction_inputs_replay_against_the_production_decisions` chỉ feed command input rồi so outcome/state; lane đòi ba test replay có tên được chọn | Một thread, ID nhỏ, bounds/seed hữu hạn. Chưa model checking; không proof PostgreSQL hay UI |
| Domain vòng đời (`domain/discussion/*`) | Unit tests từng module (`quick`): body NFC/emoji/bidi, mode generation, lifecycle 4 bước, visibility mọi trạng thái × người xem, quyền tác giả, bảng `decide_submission`, cursor, reaction desired-state/giới hạn/tally; doctest ranh giới type có lawful twin; [Kani](../../services/vink-app/src/domain/discussion/proofs.rs) (`kani`): kiểm duyệt/gỡ/chế độ với mọi version `i64`, projection không lộ bình luận ẩn, `PageLimit`, `reply_placement`, receipt trước mọi gate, reaction và tally, miền cursor | Thuần; Kani kiểm một quyết định, không chuỗi lệnh; không chứng minh transaction hay transport; reaction chưa có store/API/UI |
| Use case `Vink::{submit_comment, remove_own_comment, set_comments_mode, moderate_comment, discussion, replies}` | [discussion_tests.rs](../../services/vink-app/src/application/discussion_tests.rs) với store ghi nhận (`quick`) | Không HTTP route; `ModeratorGrant` chưa có authority cấp |
| Kiểm duyệt `GET /api/v1/moderation/comments`, `POST …/{id}/decisions`; lệnh `vink-app moderator grant\|revoke\|list` | Domain `moderation`/`policy` (`quick`); `moderation_authority_is_a_revocable_grant_rechecked_at_commit`, `moderators_recuse…`, `rejection_reasons_reach_only_their_writer…`, `the_queue_lists_pending_present_comments_oldest_first`, `the_http_moderation_api_keeps_every_outcome_distinct` (`postgres`); parse đối số lệnh (`quick`, bin tests) | Chưa có UI kiểm duyệt, cảnh báo backlog, report/appeal; production trả 401 |
| `PgStore` discussion + migration | [tests/postgres/discussion.rs](../../services/vink-app/tests/postgres/discussion.rs) (`postgres`): SQL trực tiếp, khóa tường minh, 8 retry đồng thời, audit, neo/context, keyset ties, SQL ↔ domain | Superuser harness; chưa có lệnh rút bài thật, benchmark hay EXPLAIN |
| `GET/POST/DELETE/PUT /api/v1/works/{id}/discussion…` ([contract](../../services/vink-app/contracts/http/v1/README.md#discussion-d2t-02b)) | Fixture tests `http::contract::tests::discussion` (provider) và `decodes_discussion_fixtures`/`classifies_discussion_error_fixtures` (`vink-web`) trong `quick`; `the_http_api_keeps_every_discussion_outcome_distinct` qua router trên PostgreSQL (`postgres`) | Resolver test lấy principal từ header; production chưa có auth nên ghi trả 401; chưa browser/UI, chưa rate limit |

### Consumer D2T-03 (#45)

[LayoutSpec và state matrix](../design/discussion-ui.md) ghi vùng, semantics và dependency gaps.
Các hàng dưới là assertion đã viết, không tự xác nhận kết quả chạy.

| Điểm vào | Assertion có trong code/runner | Giới hạn |
| --- | --- | --- |
| Reader `#feedback-region` | `api::discussion_resource`, `PublicReadsAdapter::discussion` gọi viewer `None`; `discussionReadingAndWrites` kiểm SSR không chứa own-pending/rejected sau commit thật | Anonymous SSR; private projection chỉ qua no-store API sau hydration. Không auth production |
| `DiscussionController` | Unit tests D2T-03: double send, new typing/IME, scope đổi, replay key/payload và physical attempt riêng, late ACK/error, Closed, context revoked | Rust consumer production seam; không phải Quint/Kani, OS IME hoặc chứng minh DB |
| Root/reply paging và context | Adapter giữ cursor opaque, hai tầng, neutral anchor/unavailable, ID-keyed rows; unit tests projection/dedupe/cursor và mutation/read epoch fencing | Không endpoint context ngoài trang, ranking/count tổng/reaction/report |
| Gửi/gỡ/author toggle cục bộ | `discussionReadingAndWrites` trong browser smoke: POST commit rồi mất ACK, exact keyed replay, pending receipt, giữ typing, PUT generation, DELETE own | LocalFixture + scratch PostgreSQL; không production auth/moderation E2E. Runtime result phải đọc log |
| Web/CMP patterns | `sharing-open`, `sharing-closed`, `discussion-flat`, `discussion-anchor` cùng catalog IDs; Web render tests và CMP Desktop tests giữ text/selection qua policy | CMP là composition mẫu; chưa native service consumer, OS IME/screen reader/performance |
| Responsive/target revocation | Browser smoke captures light/dark 320/390/834/1024/1440 và 200% ở 320/390; context fixture revoke giữ peers/draft | Capture labels tách real local PostgreSQL khỏi intercepted provider fixture; ảnh cần mở riêng |

## Adoption UIS-05 (#23)

Đối chiếu 03/11/2026 trên nền `25e3d10` + diff PR UIS-05. Mapping của năm route → composition →
primitive → token/adapter nằm trong [spec/evidence](ui-system-adoption-evidence.md), không đổi
quyền, transport hoặc save state. [ui/adapters.rs](../../apps/vink-web/src/ui/adapters.rs) chuyển
public/draft cards sang view facts; [vink-patterns](../../packages/vink-patterns/README.md) không
import DTO, service hay fixture. `DraftMetadata`/nhãn UTC được composition sở hữu.

- Browser `adaptiveShell`/`draftDashboard`/`publicReading`/`mainPhase`: gọi primitives qua DOM mới,
  giữ kiểm tương phản, tab order, SSR/private HTML, no N+1, zero utility writes, one EditorView,
  selection/undo, delayed save/create, conflict/export/leave. Button blocked được kiểm qua
  `aria-disabled` (vẫn focus được) và số request thực.
- Hai gallery đăng ký `screen.explore`, `screen.reader`, `screen.desk`, `screen.editor`,
  `screen.me`, `composition.feedback`; mỗi màn có trạng thái áp dụng ở catalog. Fixtures chỉ ở
  consumer preview. Web `screenPatterns` kiểm callback thật của reader/composition, giữ
  text/focus/selection qua local status/theme/resize, pending/auth/uncertain không submit,
  sample intent không claim saved/sent và không gọi mạng. `meSelection` kiểm radio Tôi:
  Hệ thống mặc định, một lựa chọn, keyboard/focus, giữ qua resize/OS theme, reset khi remount,
  không đổi host theme hay storage. CMP `ScreenPreviewTest` kiểm các recipe và
  controlled TextFieldValue, selection/focus, callback/blocked states và radio Tôi qua
  keyboard/recomposition/remount. `ScreenCaptureTest` chụp consumer riêng ở 360/834/1280dp;
  thêm Tôi (default/auth-required/long), reader feedback và composition feedback
  (default/pending/error/uncertain) tại 320/390dp × light/dark × font scale 1/2.
  Web `captures` giữ raw gallery và specimen tiles cùng ma trận hẹp đó. CMP có mật độ 1,
  padding 16dp, OS inset 0; `ParticipantsTest` kiểm khử trùng người theo stable key/thứ tự, không bịa người thiếu ID.
- `CaptureTest` CMP và `preview-smoke` Leptos render các kịch bản vào captures; ảnh cần mở mới là
  bằng chứng thị giác. Native compile/common test không phải native screen runtime. Kết quả chạy
  thuộc evidence, không được suy từ hàng coverage này. UI bình luận thật thuộc #45,
  chưa nối vào discussion API và chưa có auth authority. Feedback intent và quyền
  tác giả bật/tắt bình luận là hai trục độc lập; recipe không nghiệm thu quan hệ này.
  Bằng chứng và giới hạn lần kiểm UI-REPAIR-12/13 ở [repair report](../design/ui-repair-12-13.md).

## Qualification UIS-06 (#24)

Coverage trên nền `204d319` + UIS-06 diff; kết quả thực và readiness từng target ở
[qualification evidence](ui-system-qualification-evidence.md).

| Claim / failure | Owner / assertion | Boundary |
| --- | --- | --- |
| Public API thêm nhưng không có specimen, admission map cũ, preview vào production | `xtask/src/ui_qualification.rs` so component declarations với `shared/api-coverage.json`; negative tests API thiếu/mapping cũ/dependency | Source admission, không tự chứng minh render |
| Literal style/weight lặp lại, app quay lại native controls | Cùng guard scan scoped CSS/Kotlin/Rust app; exceptions exact owner/reason/removal | Không thay review dynamic selectors/build assets |
| Catalogue có unsupported nhưng không giải thích | `ui_tokens.rs` yêu cầu `unsupported.<renderer>`; generator tests | Schema/metadata |
| Control lồng/không tên, forced-colors mất focus, chữ rail vỡ ở 200% | `preview-smoke::structure/forcedColors/compactVietnameseWords`, `browser-smoke::adaptiveShell` | Actual Chrome gallery + app tách riêng |
| Native preview khác Desktop, launch recipe lạ, search tiếng Việt | `:diagnostics/commonMain` dùng chung Specimen; `ReplayTest` duyệt mọi ID/scenario và reject option lạ | Common/JVM/iOS common; không UI runtime |
| Native input mất qua theme/recreation; background không ngừng hoạt động; double pending | Android `DiagnosticTest`, iOS `DiagnosticTests` | Actual instrumentation/XCTest trên target ghi trong evidence; chưa là OS IME/VoiceOver/TalkBack |

Native diagnostic không là app production, không nối auth/save/feedback backend.
Extension và compatibility/removal owners ở [UI guide](../../packages/ui-system/EXTENDING.md).

## Preview điều hướng: UI-REPAIR-09

`navigation.rail` chỉ kiểm rail 220/96; `navigation.scaffold` phân biệt canvas ép
cố định (cuộn ngang có nhãn) với `adaptive`/`adaptive-editor` đo frame thật. Các ID
cũ giữ nguyên. Owner/scenario và đơn vị:
[navigation contract](../../packages/ui-system/shared/navigation.md#qualification-preview-ui-repair-09).

`preview-smoke.mjs::navigationFit` kiểm width/role/scroll bằng bàn phím, hai frame
320/800/1200 CSS px trong cùng viewport 1440, preference/context, input node +
chữ/vùng chọn riêng, route/overlay/focus return, hidden mount và observer disposal.
Matrix 320/390/834/1440 × sáng/tối × chữ 100/200 đo frame riêng và kiểm nguyên từ
cho nhãn thường; rail96/long-labels vẫn là stress có giới hạn ghi rõ.
`NavigationPreviewTest` kiểm counterpart dp, input/vùng chọn đã sửa và overlay qua
resize trên JVM. Các lane ghi bằng chứng riêng; coverage này không xác nhận
Android/iOS runtime, OS IME hay screen reader. Kết quả và ảnh ở
[UI-REPAIR-09](../design/ui-repair-09.md).


## Coverage EMO-01A

Nền `27927c4`, nguồn/assertion cho #25: `expressionPilot` được `verify web` gọi trong main phase,
kiểm SSR/no-JS/hydration không có engine trên route tĩnh; phát thật Software qua keyboard/touch;
shared Rust budget, callback sau hủy/unmount, motion/offscreen/tab hidden, fallback engine/WASM/
JSON/poster, CSP cùng origin được inject ở test. Các ảnh `expressions-*` dùng context capture
hiện hữu. Chữ 200% ở 320px; năm viewport × hai theme. Source invariants/fixtures/model và bounds
được chỉ mục trong [registry EMO](expressions.md#registry-emo--một-model-lifecycle).

CMP `NotoExpressionTest` dùng fixture sinh từ cùng manifest, chỉ kiểm lookup/Unicode trên JVM.
Đây là coverage intent; không phải tuyên bố lượt chạy PASS, heap leak-free, IME thật, screen reader,
mobile native, full benchmark hoặc #45 integration. SHA/kết quả chạy phải xem evidence của PR.


## LVE-PR-01 — endorsement core (05/11/2026)

| Điểm vào / scope | Owner / assertions | Coverage limit |
| --- | --- | --- |
| Chưa có UI/HTTP; pure work revision + root/flat reply | `domain::endorsement::decide_endorsement`, target facts, retained state, accepted order, receipt, checked count; 8 regression + Quint replay 33 inputs, sáu Kani obligations được registry chọn | Pure slice; [registry](endorsement-core.md) ghi kết quả/gap; auth/transaction/API/UI và epic AC-03 chưa hoàn tất |
