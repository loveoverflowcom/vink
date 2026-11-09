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
Logo/theme và assertions `brandAssets`/`assertBrand` migrate sang [V-pen R1 #117](../design/brand/vpen-r1/README.md) ngày 08/11/2026; baseline lịch sử 03/11/2026 trên nền
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
backup/restore trên nền `255ce77` (commit 3). Các hàng Identity, App ↔ Identity, đăng nhập và đình
chỉ đối chiếu lại ngày 07/11/2026 trên nền `9dc4991` cho spike D-05/S-03
([ADR](../architecture/auth-authority.md), [bằng chứng](auth-authority-s03-evidence.md)).
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
Assertions hài hòa thị giác sau R2 ([#82](https://github.com/loveoverflowcom/vink/pull/82): icon avatar Tôi,
viền focus editor, status thảo luận rỗng), smoke Explore không JavaScript và các hàng thảo luận/LVE
đối chiếu lại ngày 05/11/2026 trên nền `e9d8316`.
Search R3 đối chiếu ngày 08/11/2026 trên nền `f7bd0c4` cho #90: `SearchVariant::Bar`
dùng chung Khám phá/Bàn viết; `searchBarGeometry`/`searchBarFocus` trong `publicReading`
kiểm anatomy/focus, raw NFD/caret qua theme/resize, native GET/clear và reader return.
`draftDashboard` giữ last-success/query/input identity qua 503, xóa snapshot khi 401/403;
thứ tự Tab Viết tiếp → submit → input → sort khi query rỗng. [LayoutSpec R3](../design/search-r3.md).
Delta [R5.1](../design/search-r5.md) trên nền `6c97917` giữ GET/IME/private query,
chuyển Bar sang hidden native label + placeholder một dòng và short scope ngoài shell;
tonal full-row links có gap thay divider/ListEnd. `quietListSurfaces` kiểm resting
surfaces trong matrix, `draftDashboard` kiểm một settled result status và named refresh
tooltip; preview `components` kiểm shared tooltip hover/focus/Escape và native details
keyboard/no-JS. Source assertions không tự là bằng chứng chạy; xem [run R5.1](search-r5-evidence.md).
Bàn viết R6 (#133), trên nền `e2294a2`, giữ thứ tự resume trước search;
`draftR6Hierarchy` kiểm fixture ba nháp NFD, khối resume ≤250px và search lên ≥48px
ở 390/100, một scope/kết quả, primary link/disclosure focus thường + forced-colors,
list chung nền, full title/equal inset/exact IDs tại 320–1440/chữ100–200.
`draftDashboard` tiếp tục kiểm 100-item cap, query/IME, sort, quyền, last-success và
uncertain-create. [LayoutSpec R6](../design/desk-r6.md) thay phân cấp/list grouping
của R5.1 cho Desk; Khám phá không đổi. [Bằng chứng local](desk-r6-evidence.md).
Lõi phân loại tác phẩm (chưa có điểm vào người dùng) thêm ngày 08/11/2026 trên nền `6c97917` cho
[#95](https://github.com/loveoverflowcom/vink/issues/95); xem mục TAG-PR-01 cuối trang.
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

Logo V-pen R1 #117 nằm góc trên bên phải của Khám phá (`/`). Owner: `pages/mod.rs`,
`appearance.rs`, `vink.css`, [assets/brand](../../apps/vink-web/assets/brand/README.md)
và pipeline `xtask`. `serverHtml` kiểm brand đúng trang trước JS. `brandAssets` kiểm
SVG/PNG thật (MIME và decode 24–512px), manifest và mark trong bốn cấu hình
Light/Dark/System trước hydration. `assertBrandPlacement` kiểm hiện diện duy nhất
trên Khám phá và vắng ở các đích khác sau hydration. `assertExploreHeader` kiểm vị trí
logo với ô tìm và H1 ở hai độ rộng rail và trên mobile 320–430px với chữ 100/200%.
`adaptiveShell` kiểm đổi rail giữ vị trí logo và nội dung đang nhập trong ô tìm.
`railReview` đo trục icon, vùng bấm, chữ tiếng Việt và focus khi cửa sổ thấp.
Xem ảnh `brand-before-hydration-*`, `rail-r1-*` và `explore-real-*` từ runner Web.

| Luồng | Đường đi / thao tác | Kết quả và trạng thái hiện có | Owner |
| --- | --- | --- | --- |
| Thử biểu cảm · Mẫu | `/thu-nghiem/bieu-cam` → chọn một hình để phát một lần | 16 poster tĩnh; Unicode nguyên bản, cap 2, reduced/hidden/offscreen và lỗi quay về poster/Unicode; không lưu reaction/sticker. Không có picker sản phẩm. | [expressions.rs](../../apps/vink-web/src/expressions.rs), [catalog/policy](../../packages/expressions/README.md), [LayoutSpec/registry](expressions.md) |
| Khám phá tác phẩm công khai | /?q=…&tag=…&tag=…&cursor=…; /the/:slug | Discovery toàn thư viện qua API #98, AND tối đa 4 thẻ, GET/SSR, danh sách hữu hạn và link trang kế. Lựa chọn thử chỉ áp dụng sau Áp dụng; alias canonical theo server; lỗi khác rỗng. | [pages/discovery.rs](../../apps/vink-web/src/pages/discovery.rs), [discovery.rs](../../apps/vink-web/src/discovery.rs), [TAG-PR-06](discovery-ui.md) |
| Đọc tác phẩm | /tac-pham/:id?q=…&tag=…&cursor=…&from_tag=… | Giữ toàn bộ ngữ cảnh discovery/trang thẻ và focus hàng; nếu hàng mất thì focus heading. Canonical reader bỏ query; metadata thẻ lấy từ snapshot công khai. | [pages/mod.rs](../../apps/vink-web/src/pages/mod.rs), [discovery UI](discovery-ui.md) |
| Điều hướng thích ứng | Rail: Khám phá → Bàn viết, Tôi ở chân; mobile: Khám phá → Bàn viết → Tôi | `/` vẫn là mặc định. Desktop 220px; tablet/editor 96px; thu/mở nhớ trên thiết bị và ghi đè ngữ cảnh. Mobile ẩn bar khi viết. Route lạ (`/ban-vietnam`, `/tac-pham/a/b`) là 404, không sáng đích. Back/Forward khi Tôi mở chỉ đóng Tôi, kể cả lịch sử fragment nội trang ([UI-REVIEW-20](../design/ui-review-20.md)) | [shell.rs](../../apps/vink-web/src/shell.rs) (`Destination`), primitive [vink-ui navigation](../../packages/ui-system/leptos/src/navigation.rs), [spec shell](../design/README.md#shell-thích-ứng-7) |
| Tôi và chọn giao diện | Web/tablet: Tôi mở overlay; Mở trang Tôi tới `/toi`; mobile: Tôi tới `/toi` | Hai vùng Tài khoản/Giao diện dùng chung `AccountStatus`/`ThemePicker`, avatar trung tính và help phạm vi thiết bị ([UI-REPAIR-04](../design/ui-repair-04.md)). Sáng/Tối/Hệ thống, lưu preference local; OS đổi được theo dõi bằng CSS khi chọn Hệ thống. Storage lỗi vẫn dùng được trong phiên. Không có dữ liệu profile/auth thật. Theme/rail/overlay không lưu hay rời editor; chuyển route dùng guard | [appearance.rs](../../apps/vink-web/src/appearance.rs), [me.rs](../../apps/vink-web/src/pages/me.rs), [NavigationGuard](../../apps/vink-web/src/navigation.rs) |
| Tìm và viết tiếp | “Bàn viết” → `/ban-viet` → “Viết tiếp” hoặc “Mở bản thảo” → `/ban-viet/{id}` | Resume là nháp mới cập nhật nhất theo API; row có title/version/updated_at. Tiêu đề resume dài hơn 60 Unicode scalar hiển thị tối đa ba dòng khi thẻ hẹp, nút mở/thu gọn giữ nguyên chữ; thẻ rộng hiện toàn bộ title và CTA bên cạnh. Tìm có/không dấu, Đ/đ và NFC/NFD chỉ trong tối đa 100 nháp đã tải; đổi sort/tìm không đổi resume. Loading, rỗng, không kết quả, lỗi, 401 và 403 riêng; query/sort giữ qua tải lại/retry | `DeskIndex`, `DraftListView` trong [desk.rs](../../apps/vink-web/src/pages/desk.rs), [draft_list.rs](../../apps/vink-web/src/draft_list.rs), `DraftResume` trong [vink-patterns](../../packages/vink-patterns/leptos/src/lib.rs) |
| Tạo và đối chiếu kết quả | Heading “Tạo bản thảo”; khi rỗng “Viết bản thảo đầu tiên” → editor. Mất phản hồi → “Tải lại danh sách” → mở nháp tìm thấy hoặc chủ động “Tạo thêm bản thảo” | Một POST đang bay; lỗi 401/403/validation tách khỏi outcome chưa rõ (network, 503, bất thường). Không tự retry POST. Chỉ GET thành công mới mở khóa tạo thêm; GET không chứng minh POST nào đã ghi. Phản hồi muộn sau khi rời dashboard không điều hướng | `CreateDraftButton`, `CreateNotice`, `DeskIndex`; `CreateState` trong [draft_list.rs](../../apps/vink-web/src/draft_list.rs) |
| Viết và lưu nháp | `/ban-viet/{id}` → sửa “Tiêu đề”, “Nội dung” (CodeMirror 6) → “Lưu nháp”; có Enter ở tiêu đề và Ctrl/Cmd+S trong form (chờ IME commit) | `unsaved` → `saving` → `saved` khi server xác nhận; `failed`/outcome chưa rõ có chữ, retry/export; `conflict` giữ nội dung. Mọi lần thử lưu từ Failed đọc lại version trước: GET lỗi không PUT, version khác mở conflict, cùng version mới gửi PUT có CAS. Toolbar wrap, sticky chỉ khi editor đủ rộng/cửa sổ đủ cao; hướng dẫn ngắt dòng trong details. Lưu thủ công, chưa autosave | `DraftEditor`, `Editor` trong [desk.rs](../../apps/vink-web/src/pages/desk.rs), [API client](../../apps/vink-web/src/api.rs), [EditorSession](../../apps/vink-web/src/desk_state.rs), [BodyEditor](../../apps/vink-web/src/editor.rs), [runtime](../../apps/vink-web/editor/README.md), [UI-REPAIR-05](../design/ui-repair-05.md) |
| Xử lý xung đột | Mở cùng nháp ở hai tab, tab B lưu trước, tab A lưu bản cũ → “Lưu đè bằng nội dung đang viết” khi muốn giữ bản A | Save cũ nhận `409`, không ghi; bản A còn trong editor. Lưu đè dùng version được báo và vẫn có thể xung đột lần nữa. Muốn mở bản server thì “Tải bản đang viết” trước khi reload | `Editor`, `EditorSession`; owner/version được kiểm trong [PgStore](../../services/vink-app/src/persistence/postgres.rs) |
| Xem trước và tải `.txt` | “Xem trước” hoặc “Tải .txt” trên thanh công cụ editor | Hộp thoại đọc dùng `DocumentView` từ chữ hiện tại (kể cả chưa lưu); tệp `Tiêu đề.txt` gồm tiêu đề, dòng trống, nội dung đúng như đang viết. Không lưu, không gửi duyệt, không đổi trạng thái | `Editor`, [document.rs](../../apps/vink-web/src/document.rs) (`export_text`, `export_file_name`) |
| Rời editor khi chưa lưu | Link “Khám phá”/“Bàn viết”/“Tôi”, “← Bàn viết”, back/forward của trình duyệt khi còn chữ **hoặc thẻ** chưa được xác nhận | Hộp thoại “Ở lại” (Escape, trả focus về trường đang viết, kể cả ô tìm chủ đề) / “Lưu và rời trang” (chỉ đi sau ack của mọi phần nó chờ: chữ, thẻ hoặc cả hai). Khi lưu lỗi, bị từ chối, xung đột hoặc còn treo: “Tải bản đang viết (.txt)” và “Rời mà không lưu…” có bước xác nhận. Reload/đóng tab dùng `beforeunload` của trình duyệt | [LeaveGuard](../../apps/vink-web/src/desk_state.rs) (generic qua `Guarded`, `DraftSession`), [NavigationGuard](../../apps/vink-web/src/navigation.rs), `Editor` |
| Gắn thẻ cho bản nháp (TAG-PR-05) | `/ban-viet/{id}` → mục “Thông tin tác phẩm” sau trang viết → chip “Dạng bài”, ô “Tìm chủ đề” → gợi ý/“Xem các chủ đề có sẵn” → “Lưu” trên toolbar (hoặc Ctrl/Cmd+S) | Chỉ trình duyệt đọc thẻ nháp và danh mục (SSR không có mục này). Một dạng bài, ≤3 chủ đề từ catalog; chip có nút bỏ có tên riêng; thẻ ngừng dùng được giữ, ghi bằng chữ, “Thay bằng …” khi có kế nhiệm; refusal của server gắn vào đúng chip. Thay đổi chỉ thẻ là `unsaved`, toolbar gộp trạng thái chữ + thẻ, “Lưu” gửi riêng phần đã đổi. `409` của thẻ: “Dùng thẻ đang lưu”/“Lưu đè bằng lựa chọn trên trang”; Mất ACK → “Thử lại” gửi lựa chọn hiện tại với version cũ. Trả về lựa chọn cũ khi còn lệnh chờ hoặc chưa rõ kết quả vẫn giữ guard rời trang. Lưu thẻ không công khai gì; production `401` (D-05) | [work_info.rs](../../apps/vink-web/src/pages/work_info.rs), [tag_state.rs](../../apps/vink-web/src/tag_state.rs), `DraftSession` trong [desk_state.rs](../../apps/vink-web/src/desk_state.rs), adapter [ui/adapters.rs](../../apps/vink-web/src/ui/adapters.rs), `TagSelector`/`TagChip` trong [vink-patterns](../../packages/vink-patterns/leptos/src/tags.rs), [LayoutSpec](../design/tag-selector.md) |
| Kiểm tra tiến trình | `/health/live`, `/health/ready` | Liveness tách khỏi kết nối/schema database; readiness `200` chỉ khi lịch sử migration trùng migration nhúng (`pending_migrations`, `diverged` là `503`); báo `checks.role`, production từ chối role `privileged`; Identity chỉ là thông tin, không làm App mất readiness. Không khẳng định có auth, moderation, Discourse hay job | [health.rs](../../services/vink-app/src/http/health.rs), [vink-store-postgres](../../packages/store-postgres/src/migration.rs) |
| Bàn viết khi có Identity | Request API riêng tư với cookie `__Host-vink_session` hoặc `Authorization: Bearer` khi App cấu hình `VINK_IDENTITY_SOCKET` | Identity xác minh phiên mỗi request, App đọc binding và đình chỉ của mình: chỉ principal đã bind, không bị đình chỉ được hành động; không credential/bị từ chối/hết hạn/đã thu hồi/chưa bind/bị đình chỉ `401`; Identity hoặc App DB không quyết được, trả lời hỏng, socket do user khác phục vụ `503 dependency_unavailable`. Đọc công khai không gọi Identity. Local fixture (local + loopback) vẫn quyết định khi được bật | [identity](../../services/vink-app/src/identity/mod.rs), [client](../../services/vink-app/src/identity/client.rs) |
| Đăng nhập bằng mã email (spike S-03, chưa có UI) | `POST /api/v1/auth/sign-in/challenges`, `POST /api/v1/auth/sign-in/complete`, `GET`/`DELETE /api/v1/auth/session`, `POST /api/v1/auth/principal` | Mã gửi vào outbox cục bộ của Identity; handle và credential chỉ ở cookie `__Host-` HttpOnly; chủ thể mới là `needs_principal` cho tới khi tạo principal bằng bút danh (một principal/subject kể cả đồng thời); mã sai `422 code_rejected`, challenge đóng `409`, tài khoản bị disable/principal bị đình chỉ `403` và không đặt cookie; đăng xuất chỉ xóa cookie khi Identity đã thu hồi (Identity sập `503`, cookie giữ lại) | [auth.rs](../../services/vink-app/src/http/auth.rs), [API v1](../../services/vink-app/contracts/http/v1/README.md#sign-in-d-05s-03-spike) |
| Đình chỉ principal / disable tài khoản (vận hành) | `vink-app principal suspend <uuid> <lý do>`, `reinstate`, `list`; `vink-identity account disable|enable <subject>` | Đình chỉ đóng principal ở request kế tiếp dù Identity xác nhận phiên, gỡ thì phục hồi; disable thu hồi mọi phiên trong cùng transaction và chặn đăng nhập, enable không khôi phục phiên. Không route HTTP nào làm việc này | [main.rs](../../services/vink-app/src/main.rs), [principal.rs](../../services/vink-app/src/persistence/principal.rs), [accounts.rs](../../services/vink-identity/src/accounts.rs) |
| Identity riêng, có/không authority | `vink-identity serve` trên `VINK_IDENTITY_SOCKET`; `GET /identity/v1/capabilities`, `POST /identity/v1/verify-credential`; với `VINK_IDENTITY_AUTHORITY=email-code` thêm `sign-in/*`, `sessions/revoke`, `email-change/*` | Caller đúng uid: không authority thì capabilities `not_configured`, mọi thao tác `501`; email-code: verify trả subject + phiên hoặc `rejected{reason}`; sai/thiếu caller `401`; payload sai `400`/`413`; email sai `422`; throttle `429`; version khác `404`; quá 2 s `503`. Readiness cần DB đúng schema và caller đã cấu hình, báo `mail_transport` và `production_authentication: not_implemented` | [vink-identity](../../services/vink-identity/README.md), [contract v1](../../services/vink-identity/contracts/rpc/v1/README.md) |
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
`serverHtml`, `conditionalPublicReads`, `cacheFirstPublicReads`, `brandAssets`, `adaptiveShell` (gọi
`shellFragmentHistory`), `accountAppearance`, `appearanceFirstPaint`, `shellWithDirtyEditor`,
`draftDashboard`, `nativeExploreSearch`, `publicReading`, `editorChrome`, `mainPhase` (gọi
`expressionPilot`), `discussionReadingAndWrites` và `afterRestartPhase` là các phần trong script,
không phải lệnh CLI mới. Nguồn kiểm PostgreSQL là [tests/postgres.rs](../../services/vink-app/tests/postgres.rs),
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
| Khám phá hữu hạn, trạng thái và reader | `public_list::tests` (`quick`) kiểm title/author/NFC/NFD/Đ/đ/order/chữ gốc, link q encoding và đích nội bộ; test `draft_list` giữ hồi quy helper. `no_publications_is_an_empty_collection_and_not_an_outage` (`postgres`); `database_outage_is_reported_not_hidden` (`quick`) kiểm SSR có q vẫn 503, echo query và canonical. `serverHtml` kiểm SSR tìm title/author/no-results, public HTML/metadata/404/privacy; `nativeExploreSearch` tắt JavaScript trang và chặn bundle/WASM: Enter trong form GET thật ra đúng một `q` (bỏ tham số trùng/cũ), link xóa về `/` không submit, nút submit ra no-results SSR; `publicReading` kiểm hydrated GET, clear/retry, query/hash/focus qua reader, Back/Forward/reload, không fetch thêm list/manuscript khi tìm; loading/rỗng/no-results/503 riêng | Browser reload/back dùng seed thật; nhiều tác giả/title dài/lỗi API là HTTP fixtures có nhãn. Source assertion không là bằng chứng đã chạy; không có contract cho tương tác cộng đồng |
| Reader/Explore responsive và chữ an toàn | `publicReading` kiểm markup là text, khổ/thụt lề/đoạn, không overflow tại 1440/1024/834/390/320px, hai theme, root text 100/200%; Reader hẹp giữ H1 đầy đủ/chọn được với phân cấp tối đa 1,4 lần chữ đọc; fixture title không ngắt từ và document rỗng dưới reduced motion; Tab từ return link qua ba đích dưới, focus và cuối trang không bị bar che; `bottomNavigationGeometry` kiểm nhãn tối đa hai dòng trong target ≥44px và bar ≤18% chiều cao viewport 900px; `textContrast` lấy các cặp public UI; PNG + contexts có geometry. [Bằng chứng UI-REPAIR-01](../design/ui-repair-01.md) | Ảnh chỉ thành bằng chứng thị giác sau khi mở; root text scale không phải browser zoom; headless focus do CDP giữ visible khi Chrome mở tab cập nhật. Không chứng minh screen reader hay thiết bị thật |
| Dashboard, tìm trong phạm vi và trạng thái | `vietnamese_search_preserves_source_and_provider_order` (`quick`); `draftDashboard` (`web`) dùng fixtures rỗng/1/100 row/title dài NFD (chờ animation hữu hạn xong trước khi chụp mỗi theme), tên trống/lặp/từ liền; kiểm work_id/version/time, scope, tìm Đ/đ, sort, không kết quả, giữ query/sort qua 503/retry, 401/403, không N+1; sửa title/lưu thật rồi trở lại giữ work và version đã xác nhận; `serverHtml` kiểm index không có title/body riêng | Fixtures HTTP chỉ thuộc test, không phải data sản phẩm; không kiểm tìm toàn thư viện hoặc profile/publication |
| Tạo nháp khi phản hồi mất hoặc đến muộn | `uncertain_create_needs_reconciliation_and_a_new_intent`, `explicit_pre_write_rejections_remain_distinct` (`quick`); `draftDashboard` giữ POST, chặn submit lặp, cho PostgreSQL commit thật rồi bỏ ack 201, kiểm GET lỗi không mở khóa, GET thành công tìm đúng một nháp không replay; response đến sau khi rời trang không kéo về editor | GET không xác định chắc chắn chủ nhân của một POST mất ack; chưa có idempotency key hay auth session thật |
| Dashboard narrow/theme/keyboard | `draftDashboard` kiểm mở/thu gọn bằng Enter/Space, full title trong AX tree, thứ tự resume → search → sort bằng Tab, focus-visible và CTA clear bottom bar. Reflow 1440/1024/834/390/320px × sáng/tối × chữ 100/200%; title gọn ba dòng và công cụ trong viewport đầu ở 390/100%. Recent row giữ toàn title/metadata/href, dùng hết inner measure khi hẹp, cùng trục heading/resume/tools/list; Enter/CDP touch mở cùng nháp thật. Phase `desk-layout` chạy riêng dashboard + public reading trên server scratch đã chuẩn bị. PNG + `contexts.json` có width/line count/row height; [evidence #66](../design/ui-repair-66-draft-rows.md); `textContrast` đo các cặp chữ dashboard hai theme; [bằng chứng UI-REPAIR-03](../design/ui-repair-03.md) | Phải mở PNG trước kết luận thị giác; AX tree không phải một lượt dùng screen reader, root text scale không phải zoom hay điện thoại thật |
| Hydration và chuyển từ reader sang Bàn viết | `mainPhase` kiểm dấu hydration, giữ marker JS qua điều hướng, không có console error/exception; node `[role=status]` rỗng trong `.vk-discussion` là `display: none` (không tạo khoảng grid chết) | Một Chrome headless desktop; không bao phủ mọi link hoặc trình duyệt |
| Gõ tiếng Việt, giữ dòng/khổ/thụt lề, lưu rồi reload | `vietnamese_text_lines_and_stanzas_round_trip` (`postgres`); `mainPhase` kiểm CodeMirror mount từ `/pkg/vink-editor.js` (trang đọc không tải), không còn textarea, so chữ trong `.cm-line`, document server và version sau lưu/reload; chờ window cũ được thay và editor mới hydrated trước khi đọc lại | CDP dùng `Input.insertText`, thử `imeSetComposition` có fallback; không phải IME hệ điều hành. Chưa kiểm Word/Docs paste hay thiết bị thật |
| Chỉ báo đã lưu cho đúng nội dung được xác nhận | Unit tests của [desk_state.rs](../../apps/vink-web/src/desk_state.rs) kiểm phản hồi cũ, sửa tiếp khi request đang bay, một save đang bay và lỗi; `mainPhase` kiểm phiên bản ACK thật trong Chi tiết, trạng thái ngắn luôn thấy, Ctrl+S, failed; `editorChrome` giữ PUT, click/shortcut lặp chỉ một request, gõ thêm trước ack vẫn unsaved và GET xác nhận snapshot cũ | Smoke chưa kiểm Enter ở tiêu đề; lỗi mạng là mô phỏng ở tầng Fetch của trình duyệt |
| Retry sau kết quả lưu chưa rõ | `editorChrome` làm hỏng request, retry sau GET cùng version; giữ response sau commit thật rồi bỏ ack, đối chiếu 403 không PUT, retry tiếp vẫn GET và conflict không phát PUT; explicit keep-local rồi reload. Giữ GET đối chiếu rồi xác nhận bỏ: GET muộn không khởi động PUT, không điều hướng hay đổi version server | Không có idempotency key; GET không xác nhận saved thay cho ack cũ. CAS vẫn là ranh giới chống ghi đè nếu có writer chen giữa GET/PUT |
| Chrome editor, reflow và dialog | `editorChrome` (cũng có phase độc lập `editor-chrome`): 1440/1024/834/390/320 × light/dark × root text 100/200%, clean/dirty/saving/failed/conflict và preview/leave; toolbar target ≥48px, mặt viết R5 chung title/body không resting stroke/halo/divider, một live status owner không lặp khi gõ; recovery/backup visible, native detail, tooltip/focus/forced-colors và composition guard; không overflow, recovery còn đường tới, Tab công cụ/title/body (body không viền, một focus outline 2px offset −2px) và nút an toàn trong dialog, đo tương phản sau transition hữu hạn, cửa sổ thấp và title focus không bị sticky che. PNG/manifest ghi geometry; [evidence #101](../design/ui-editor-r5.md) | Ảnh phải mở riêng; CSS text 200% không phải browser zoom. Viewport thấp không là bàn phím mềm thật; không xác nhận OS IME, Safari/Firefox hay screen reader |
| Một EditorView giữ trạng thái | `shellWithDirtyEditor` đánh dấu `.cm-editor`, thu/mở rail, đổi theme qua radio, mở/đóng Tôi, resize mobile; chờ hai frame sau resize, so chữ/selection/dirty và không có PUT/POST, Tab trở lại và native blur/focus qua `EditorView.focus()`; `mainPhase` đổi `data-theme`, mở/đóng xem trước và hộp thoại rời trang, rồi kiểm cùng view, selection (`getSelection`), vị trí cuộn trang sau khi đóng xem trước, Ctrl+Z/Ctrl+Shift+Z | Selection/undo/cuộn đo qua DOM của một Chrome desktop; chưa kiểm cuộn khi bàn phím ảo mở |
| Xem trước và export dùng chữ hiện tại | `mainPhase` so khối trong `draft-preview` với khổ chưa lưu và kiểm version server không đổi; `afterRestartPhase` tải `.txt` thật qua `Browser.setDownloadBehavior` và so nội dung; tệp khôi phục từ hộp thoại được kiểm trong `mainPhase` | Mỗi phase chỉ tải một tệp vì Chrome hỏi trước lần tải thứ hai; chưa kiểm mở tệp trên Windows/macOS |
| Version cũ không ghi, giữ nội dung để người viết chọn | `stale_expected_version_conflicts_without_writing`, `concurrent_saves_of_one_version_have_exactly_one_winner` (`postgres`); `mainPhase` kiểm conflict và nút giữ bản local | Smoke mô phỏng bên ghi cạnh tranh bằng PUT trong cùng trang, chưa thao tác hai tab thật; test PostgreSQL chạy 8 save đồng thời |
| Quyền đọc/ghi riêng và lỗi API không bị gộp thành thành công | `another_actor_cannot_read_list_or_write_a_private_draft`, `api_distinguishes_outcomes_and_keeps_drafts_private` (`postgres`); `private_routes_are_closed_without_an_auth_authority`, `database_outage_is_reported_not_hidden` trong [http/mod.rs](../../services/vink-app/src/http/mod.rs) (`quick`) | Resolver header chỉ nằm trong test; fixture loopback không chứng minh session, logout/ban hoặc ACC-01/02 |
| Role tách và database cũ còn nguyên sau provisioning | [tests/postgres.rs](../../packages/store-postgres/tests/postgres.rs) (lifecycle, `postgres`); [provision.rs](../../xtask/src/provision.rs) (`postgres`): snapshot dữ liệu + lịch sử trước/sau, runtime đọc lại tiêu đề NFD, thử DDL/TEMP/ghi `_sqlx_migrations`/`CREATE EXTENSION`/kết nối chéo bằng chính role, ca âm cấp `CREATE` thừa | Server tạm do test harness tạo; Compose thật và DB do binary base ghi chỉ có bằng chứng thủ công trong [verification](identity-postgres-verification.md); lane `web` vẫn chạy app bằng credential test harness |
| Identity đóng khi không chắc chắn, không lộ credential | `http::tests`, `email_code::tests`, `mail::tests` của Identity (`quick`: caller theo peer, header/subject giả, payload, version, deadline, fixtures, redaction; chuẩn hóa NFC, mã, handle, credential, bảng quyết định thử mã vét cạn; thao tác email-code bị đóng khi không có authority, kiểm request trước storage); [identity tests/postgres.rs](../../services/vink-identity/tests/postgres.rs) và [email_code.rs](../../services/vink-identity/tests/postgres/email_code.rs) (`postgres`: mã một lần/gắn handle/giới hạn thử, 8 lần trả lời đồng thời một phiên, 4 lần bị ép chờ cùng khóa hàng một phiên, 6 lần đăng nhập đầu đồng thời một tài khoản, throttle đồng thời, logout, hết hạn, disable, đổi email giữ subject và địa chỉ được giải phóng không thừa kế); [xtask identity.rs](../../xtask/src/identity.rs) (`postgres`: binary thật, socket thật, role đã provision) | Mail chỉ là outbox cục bộ; không rate limit theo IP; chế độ mạng không tồn tại |
| App không cấp quyền khi Identity lỗi | `identity_tests` trong [http/mod.rs](../../services/vink-app/src/http/mod.rs) (`quick`: Identity giả trên socket thật — rejected/authenticated chưa bind/suspended/no-authority `401`, 503/caller/malformed/outcome lạ/subject sai/directory lỗi/chậm `503`, cookie và Bearer như nhau, sai uid server và mất socket `503`, header giả không đổi gì; đăng nhập: Origin, cookie `__Host-`, không lộ secret trong body, revoke phiên khi bị đình chỉ/directory lỗi; logout); [principal.rs](../../services/vink-app/tests/postgres/principal.rs) (`postgres`: 12 lần provisioning đồng thời một principal không mồ côi, subject khác không chung principal, binding bất biến/không gắn subject thứ hai, đổi bút danh giữ principal và nháp, đình chỉ/gỡ); `app_integration`, `app_refusals`, `email_code_end_to_end` trong [xtask](../../xtask/src/identity/email_code.rs) (`postgres`: hai binary thật) | Không có UI hay trình duyệt thật; không fence đình chỉ tại commit; native chưa có |
| Backup/restore theo owner | `backup_restore` trong [provision.rs](../../xtask/src/provision.rs) (`postgres`): `pg_dump`/`pg_restore` App và Identity vào database khác, so dữ liệu/ID/Unicode/con trỏ/lịch sử/owner | Cùng cluster; không kiểm restore sang host khác hay khôi phục tại một thời điểm chung cho hai owner |
| Dữ liệu đã xác nhận còn sau restart, process dừng đúng | `confirmed_save_survives_a_new_connection_pool` (`postgres`); `afterRestartPhase` và orchestration trong [web.rs](../../xtask/src/web.rs) (`web`) | Không phải crash/replay/backup-restore; worker chỉ có lifecycle, không có job |
| Font tự host được phục vụ và tải | `mainPhase` kiểm trang đọc có Be Vietnam Pro 400 và Literata ở trạng thái `loaded`, file font trả `font/woff2` (`web`); repository checks so hash từng face/license với [provenance](../../apps/vink-web/assets/fonts/provenance.json) và kiểm URL font trong CSS/shell (`quick`) | Không chứng minh vị trí dấu, fallback trên máy thiếu font hay hiển thị trên thiết bị thật; kiểm theme trong `adaptiveShell` là bằng chứng riêng của shell |
| Shell, preference và focus | `adaptiveShell` kiểm rail 220/96, tên/`aria-expanded` của toggle và nút ≥44px không bị che khi thu, mặc định/ghi đè/reload, thứ tự/active route, link thật không phải tab, skip link focus main, modifier click để trình duyệt xử lý, 404 cho route lạ không sáng đích, ba lần bấm dồn dập chỉ còn đích cuối, tooltip focus/hover/Escape ngoài rail, Tôi overlay focus đầu/Tab/Escape/outside/Đóng/resize, Back/Forward khi Tôi mở chỉ đóng Tôi (kể cả fragment-only), Back/Forward không overlay vẫn đổi fragment tự nhiên, traversal bị giữ phục hồi scroll khác0 sau native history, rồi Back sau mới rời, ba đích mobile, theme system/override/reload/storage denied, không overflow và bottom clearance ở 320px/chữ 200%; unit `shell::tests` (đích theo segment, ngữ cảnh editor, thứ tự/href), `public_list::tests` (return context round-trip qua parser của router, link lạ chỉ dựng URL nội bộ); `vietnamese_initials_use_first_and_last_words` kiểm tên tiếng Việt/NFC/blank | Một Chrome headless Linux, resize CSS và cỡ chữ root; không phải thiết bị mobile/zoom trình duyệt/screen reader; initials fixture không là profile |
| Bottom bar: icon và dòng đầu cùng trục (#88) | `bottomNavigationAlignment` đo icon-centreY và dòng đầu bằng DOM Range sau `fonts.ready`, so lệch ≤1 CSS px, target đều và cùng resolved font/line-height; được gọi trong ma trận `adaptiveShell`. Phase `bottom-navigation` kiểm riêng 320/390 × chữ100/200 × light/dark × ba route, focus/Tab, clearance sau resize, rail 834px và editor ẩn bar. [Before/after](../design/bottom-nav-88.md) | Một Chrome headless/macOS, root text scale; chưa thiết bị thật, screen reader, safe-area khác0 hoặc native/CMP |
| Tôi/panel: bề rộng chữ và trục nội dung (#67 AC-11–13) | `accountLayoutGeometry` dùng bounds từ capture helper: heading/account/choices cùng content axes; account text dùng trọn cột bên trong nhóm nền hiện hành; gutter16 theo layout viewport; account320/390 chữ200 xếp dọc và dùng toàn measure; panel inset đối xứng trong client box và CTA cùng trục. `adaptiveShell` thêm ảnh cuối trang và helper/footer tới được trên navigation. [Recapture và số đo](../design/ui-repair-67-me-panel-geometry.md) | Recapture R2 giữ như lịch sử; R5 bổ sung nền account và helper thu gọn, đo theo content box mới, không bù scrollbar bằng margin. Chrome headless/CDP, không suy native hay screen reader |
| Tôi/panel R4–R5: surface và disclosure (#67 AC-14–21) | `preferenceSurfaces` kiểm account/copy thật, selected fill + check khác idle, không resting frame/rim/divider; native details dùng chung có Enter/Space/Tab/CDP touch và no-JS. Panel834×360/text200 mở help vẫn một scroller, CTA cuối tới được. `appearanceFocusContrast` đo vòng focus selected ≥3:1 cả sáng/tối/forced-colors sau transition. Reader Tab qua một build disclosure trước nav; footer bỏ rule. [Layout, captures và kiểm local](../design/me-preferences-r5.md) | Native details mở trong luồng; Escape ở panel đóng modal cha và trả trigger. Root text/CDP không chứng minh zoom thật, OS IME, screen reader hoặc CMP/native |
| Tôi: tài khoản, radio và overlay có thể cuộn | `accountAppearance` (`web`) kiểm cùng copy thật/neutral avatar (icon ở tâm, rộng 60% avatar, sai số 1px), ID duy nhất và nhóm native tách tên, ArrowLeft/ArrowRight/Space và theme intents liên tiếp đồng bộ hai view, focus/checked marker; overlay 1440/834 expanded/compact cao 900/360 và 1024 compact cao 900, ba theme × chữ 100/200%, không tràn ngang, Tab/Shift+Tab cuộn lộ hai đầu và Escape trả trigger. `adaptiveShell` kiểm stale close/reopen và aria-expanded, storage denied đồng bộ hai nơi; `brandAssets` thêm System/OS sáng và tối trước hydration; `/toi` 1440/1024/834/390/320 ba theme × chữ 100/200%, DOM Range kiểm từng từ tiếng Việt nguyên vẹn. `appearanceFirstPaint` ([UI-REVIEW-21](../design/ui-review-21-me-profile.md)) chặn bundle, đối chiếu root và cả hai checked native trước hydration tại 1440/320 × chữ 100/200%, explicit theo OS ngược và System theo OS sáng/tối; reload thường lấy mẫu từng frame tới hydration. `shellWithDirtyEditor` giữ route/hash/editor khi overlay tiêu thụ Back, fragment không overlay không mở guard rời trang, version server giữ nguyên; `mainPhase` thử Mở trang Tôi khi save treo và conflict vẫn giữ guard/chữ | Chrome headless, root text scale và input chương trình; ảnh cần mở riêng. Không xác nhận auth/profile, thiết bị thật hoặc screen reader |
| Rời editor khi chưa lưu | Unit tests `LeaveGuard` (hủy khi đang lưu, ack muộn, đổi đích, chữ mới hơn, lỗi/409, xác nhận bỏ, bỏ khi save treo); `mainPhase` kiểm chữ đã xác nhận rời không hỏi, link Khám phá của rail và Back bị giữ, Escape/“Ở lại” giữ URL/chữ/focus, lưu rồi rời sau ack, hủy chuyển sang Tôi khi PUT bị giữ rồi thả không điều hướng muộn, lỗi → tải bản đang viết → rời mà không lưu → ack muộn không điều hướng lần nữa và không đánh dấu nháp khác đã lưu | `beforeunload` (reload/đóng tab) chưa có assertion; Chrome desktop headless, back qua `history.back()`; chưa kiểm cử chỉ vuốt back trên mobile |

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
| Preview CMP | `cargo xtask ui-preview cmp run` (cửa sổ desktop) | `VinkThemeTest` (common), `VinkScopeTest`, `PreviewTest`, `CaptureTest` (desktop) | JVM desktop off-screen; native có lệnh diagnostic riêng |
| Font CMP Desktop ([PR 08](../design/ui-repair-08.md)) | Be Vietnam Pro 400/600 và Literata TTF từ cùng WOFF2 Web, classpath `vink-fonts/`; fallback hệ thống có lý do khi thiếu/hỏng | `PreviewTest.selfHostedFacesDecodeOnDesktop` vẫn yêu cầu 3 face; `FontTest` kiểm metadata/trục/glyph tiếng Việt, đo font thật qua Compose 400/600 và missing/corrupt từng face; `FontCaptureTest` chụp cùng specimen với WOFF2 trước và TTF sau; `checks::desktop_font_derivative_checks_detect_corruption_and_unowned_files` kiểm hash | CI `ui-cmp` Linux và `ui-cmp-macos` chạy cùng lane; glyph/đo chữ không thay shaping/IME hay native acceptance; ảnh phải mở mới là visual evidence |
| Web dùng token sinh ra | `/pkg/vink.css` = `tokens.css` + `vink-ui.css` + `vink.css` | `verify web` hiện có (contrast hai theme, theme/reload/OS/storage denied, reflow) chạy trên CSS mới | Không thêm assertion web mới cho UIS-01 |
| Component nền UIS-02 ([#20](https://github.com/loveoverflowcom/vink/issues/20)) | Preview Leptos `#<họ>.<component>/<kịch bản>` (ví dụ `#action.button/pending`, `#field.form/invalid`, `#identity.avatar/image-error`); preview CMP chọn cùng ID | Test `vink-ui` (`action`, `field`, `media`, `identity` golden vectors, `layout`, `text`), `vink-ui-preview` (mọi kịch bản render, không ID trùng); `components` trong [preview-smoke.mjs](../../packages/ui-system/leptos/preview/e2e/preview-smoke.mjs); CMP `VinkIdentityTest`, `PreviewTest` (dispatch, selection, semantics), `CaptureTest` | Chưa có màn app dùng; IME thật, screen reader, Android/iOS runtime và hover CMP chưa chạy; contrast là màu resolve trên Chrome/Skia |
| Ảnh nội dung ([#115](https://github.com/loveoverflowcom/vink/issues/115), [#116](https://github.com/loveoverflowcom/vink/issues/116)) | Preview `identity.image-frame/default\|image-error` và `identity.avatar/image-error`. Web có data URL và `/fixtures/sample.webp`; CMP có bytes đóng gói cho ảnh mẫu và ảnh chi tiết | [Fixture/profile chung](../../packages/ui-system/shared/content-images.md); Web smoke kiểm HTTP/decode và hydration. CMP `ContentImagesTest` kiểm container, kích thước, codec và hủy. `PreviewTest`, Android `DiagnosticTest` và iOS `DiagnosticTests` kiểm render/fallback | [Bằng chứng Android/iOS/desktop](ui-system-qualification-evidence.md#lossy-webp-trong-cmp-116). Ảnh bìa production `NOT_IMPLEMENTED` |
| Trạng thái/phản hồi UIS-03 ([#21](https://github.com/loveoverflowcom/vink/issues/21)) | Preview `#state.<họ>/<kịch bản>` (ví dụ `#state.error/partial`, `#state.dialog/session-expired`); sân thử `#state.playground/<recipe>` (đồng hồ thủ công, bước, nhật ký) cho 8 recipe | Generator `ui_states` + vectors chung (`state::tests` Rust, `VinkStateTest` JVM/iOS simulator); test `feedback` (render vùng/panel/notice/dialog), `vink-ui-preview` (`states::tests`: reducer có ticket bỏ double-click và ack muộn); `states` trong [preview-smoke.mjs](../../packages/ui-system/leptos/preview/e2e/preview-smoke.mjs) (vùng, intent, announce, snackbar theo đồng hồ, 8 recipe, focus dialog, motion giảm/không hoạt động/tab ẩn qua `visibilitychange`, frame có timestamp); CMP `StatePreviewTest`, `DrivenCaptureTest` | Chưa màn app nào dùng; lifecycle Android/iOS chưa có host; TalkBack/VoiceOver, giảm chuyển động của OS thật chưa chạy |
| Shell/điều hướng UIS-04 ([#22](https://github.com/loveoverflowcom/vink/issues/22)) | Preview `#navigation.<họ>/<kịch bản>` (ví dụ `#navigation.scaffold/toggle`, `#navigation.overlay/busy`); CMP thêm `#navigation.adapter/*` (back stack, deep link, chữ chưa lưu) | Generator `ui_navigation` + vectors chung (`navigation::tests` Rust, `VinkNavigationTest` JVM/iOS simulator khi bật native); `navigation` trong [preview-smoke.mjs](../../packages/ui-system/leptos/preview/e2e/preview-smoke.mjs) (thứ tự, link thật, toggle 96→220→96 bằng chuột/Enter/Space giữ trang/chữ/selection, tooltip, một modal, Back đóng lớp phủ trước, Escape trả focus); CMP `NavigationPreviewTest` (deep link round-trip/từ chối link ngoài, Back đóng lớp phủ trước, chữ chưa lưu giữ Back/điều hướng, đổi khung không remount, toggle giữ selection, thứ tự thanh dưới, một modal); app thật trong `adaptiveShell` (`web`) | Android predictive back, iOS swipe-back, lifecycle, TalkBack/VoiceOver chưa chạy; adapter CMP là host chẩn đoán, không phải app native |
| ToggleAction + EndorsementAction (LVE [#73](https://github.com/loveoverflowcom/vink/issues/73), UI System) | Preview `#action.toggle-action/<kịch bản>` (gồm `lifecycle`: gửi, mất phản hồi, Kiểm tra lại, từ chối) và `#composition.endorsement/<kịch bản>`; CMP chọn cùng ID | Generator `ui_toggle` + vectors `toggle-contract-v1` (`toggle::tests` Rust, `VinkToggleTest` JVM; `ui_toggle::tests` từ chối hàng phát lệnh mù/unavailable thành số); `vink-patterns` (`endorsement_copy…`, `EndorsementTest`); `vink-ui-preview` (`endorsement::tests`); `toggles` trong [preview-smoke.mjs](../../packages/ui-system/leptos/preview/e2e/preview-smoke.mjs) (click đúp một lần, không optimistic, mất phản hồi chặn bấm, Kiểm tra lại, Enter/Space, focus 2px, số không khả dụng, lý do chặn, bốn đích độc lập, khách); CMP `PreviewTest`/`CaptureTest` qua catalog ([evidence](ui-system-endorsement-evidence.md)) | Reader Web dùng qua [adapter LVE](#lve-web-adapter-73-wp-04); glyph CHOICE-07 OPEN; TalkBack/VoiceOver/screen reader thật, Android/iOS runtime chưa chạy |
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
| Model → trace → core | [Behavior owner](../../services/vink-app/behavior/discussion/README.md): Quint typecheck/scenarios/simulation/mutation/drift; `quint_generated_inputs_replay_against_the_production_creation_seam`, `quint_lifecycle_inputs_replay_against_the_production_decisions` (gồm receipt/retry), `quint_reaction_inputs_replay_against_the_production_decisions` (cùng hai counterexample no-op của bật/tắt và reaction phải bị production từ chối) và `quint_endorsement_inputs_replay_against_production` ([LVE](literary-endorsement-core.md); route ở mục LVE API bên dưới) và `quint_admission_inputs_replay_against_the_production_decisions` (#120, cùng ba counterexample admission phải bị production từ chối) chỉ feed command input rồi so outcome/state; lane đòi tám test replay có tên được chọn | Một thread, ID nhỏ, bounds/seed hữu hạn. Chưa model checking; không proof PostgreSQL hay UI |
| Domain vòng đời (`domain/discussion/*`) | Unit tests từng module (`quick`): body NFC/emoji/bidi, mode generation, lifecycle 4 bước, visibility mọi trạng thái × người xem, quyền tác giả, bảng `decide_submission`, cursor, reaction desired-state/giới hạn/tally; doctest ranh giới type có lawful twin; [Kani](../../services/vink-app/src/domain/discussion/proofs.rs) (`kani`): kiểm duyệt/gỡ/chế độ với mọi version `i64`, projection không lộ bình luận ẩn, `PageLimit`, `reply_placement`, receipt trước mọi gate, reaction và tally, miền cursor, fence hai yêu cầu của chế độ và reaction (yêu cầu đã xác nhận chặn yêu cầu cũ hơn), admission có version/shadow/hậu kiểm (#120, [quyết định](../architecture/discussion-admission.md)) | Thuần; Kani kiểm một quyết định hoặc một cặp yêu cầu, không chuỗi lệnh dài; không chứng minh transaction hay transport; reaction chưa có store/API/UI |
| Use case `Vink::{submit_comment, remove_own_comment, set_comments_mode, moderate_comment, discussion, replies}` | [discussion_tests.rs](../../services/vink-app/src/application/discussion_tests.rs) với store ghi nhận (`quick`) | Store ghi nhận không chứng minh SQL/khóa; HTTP và grant kiểm duyệt ở các hàng dưới |
| Kiểm duyệt `GET /api/v1/moderation/comments`, `POST …/{id}/decisions`; lệnh `vink-app moderator grant\|revoke\|list` | Domain `moderation`/`policy` (`quick`); `moderation_authority_is_a_revocable_grant_rechecked_at_commit`, `moderators_recuse…`, `rejection_reasons_reach_only_their_writer…`, `the_queue_lists_pending_present_comments_oldest_first`, `the_http_moderation_api_keeps_every_outcome_distinct` (`postgres`); parse đối số lệnh (`quick`, bin tests) | Chưa có UI kiểm duyệt, cảnh báo backlog, report/appeal; production trả 401 |
| `PgStore` discussion + migration | [tests/postgres/discussion.rs](../../services/vink-app/tests/postgres/discussion.rs) (`postgres`): SQL trực tiếp, khóa tường minh, 8 retry đồng thời, audit, neo/context, keyset ties, SQL ↔ domain; provenance admission giữ bằng schema, đình chỉ tại commit (hai thứ tự race) và nâng cấp dữ liệu cũ (#120) | Superuser harness; chưa có lệnh rút bài thật, benchmark hay EXPLAIN |
| `GET/POST/DELETE/PUT /api/v1/works/{id}/discussion…` ([contract](../../services/vink-app/contracts/http/v1/README.md#discussion-d2t-02b)) | Fixture tests `http::contract::tests::discussion` (provider) và `decodes_discussion_fixtures`/`classifies_discussion_error_fixtures` (`vink-web`) trong `quick`; `the_http_api_keeps_every_discussion_outcome_distinct` qua router trên PostgreSQL (`postgres`) | Resolver test lấy principal từ header; production chưa có auth nên ghi trả 401; UI/browser ở mục Consumer D2T-03; chưa rate limit |

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
| Lõi thuần work revision + root/flat reply (API/PostgreSQL ở mục dưới) | `domain::endorsement::decide_endorsement`, target facts, retained state, accepted order, receipt, checked count; 8 regression + Quint replay 33 inputs, sáu Kani obligations được registry chọn | Pure slice; [registry](endorsement-core.md) ghi kết quả/gap; API/PostgreSQL ở mục dưới; auth authority, UI và epic AC-03 chưa hoàn tất |

## LVE API + PostgreSQL (#73, PR #81)

Owner, data flow, lock order và gap: [runtime](literary-endorsement-runtime.md). Consumer Web ở mục
[LVE Web adapter](#lve-web-adapter-73-wp-04); chưa có consumer native; production trả `401` cho ghi và
trạng thái riêng.

| Điểm vào | Assertion hiện có | Còn thiếu |
| --- | --- | --- |
| Use case `Vink::{set_work_endorsement, set_comment_endorsement, endorsement_summaries, own_endorsements}` | [endorsement_tests.rs](../../services/vink-app/src/application/endorsement_tests.rs) (`quick`): xác thực trước kiểm input, lệnh thuần đúng, accepted/replayed/conflict/key reuse/refusal riêng, batch ≤ 100 | Store ghi lại; không chứng minh SQL |
| `PUT /api/v1/works/{id}/endorsement/me`, `PUT …/discussion/comments/{id}/endorsement/me`, `GET …/endorsements`, `GET …/endorsements/me` ([contract](../../services/vink-app/contracts/http/v1/README.md#literary-endorsement-lve-73)) | `http::contract::tests::endorsement`; `http::tests::endorsement_routes_fail_closed_and_report_outages`; `the_http_api_keeps_every_endorsement_outcome_distinct` trên PostgreSQL (`postgres`) | Resolver test lấy principal từ header; consumer `vink-web` ở mục dưới |
| `PgStore` endorsement + migrations `20261105000000`, `20261105120000` (parity selected/version) | [tests/postgres/endorsement.rs](../../services/vink-app/tests/postgres/endorsement.rs) (`postgres`): SQL trực tiếp (gồm parity), order/receipt, rollback khi lỗi sau ghi state, hydrate fail-closed, race hai thứ tự commit, khóa lệnh trên mọi đích, khóa với kiểm duyệt/rút-tái bản, quyền riêng tư, đối soát; `xtask` kiểm role runtime và App + Identity | Lệnh publish/withdraw thật; tài khoản; plan đo trên dataset tổng hợp ([bằng chứng](evidence/lve-postgres/README.md)) |

## LVE Web adapter (#73, WP-04)

Reader `/tac-pham/{id}`: “Đánh giá cao” sau văn bản và “Hữu ích” trên từng bình luận gốc/phản hồi
phẳng công khai. Owner, bảng ánh xạ lỗi và CHOICE-10 A: [runtime](literary-endorsement-runtime.md#web-adapter-wp-04);
kết quả chạy: [web evidence](literary-endorsement-web-evidence.md). Production trả `401`; ghi thật chỉ
qua LocalFixture loopback.

| Điểm vào | Assertion hiện có | Còn thiếu |
| --- | --- | --- |
| `EndorsementController` ([endorsement_state.rs](../../apps/vink-web/src/endorsement_state.rs)) | `endorsement_state::tests` (`quick`, 13 `lve_web_*`): chạm đúp một lệnh, không optimistic, ACK/đọc trễ và đảo thứ tự theo order, count stamp, mất ACK → Uncertain → replay đúng lệnh/attempt mới, replay hiện trạng thái hiện tại, Unknown → đọc lại, `409` không gửi lại ý định cũ, đổi session/quay lại, khách, `401`/`403`/`404`/`422`/`500`/`503`, revision đổi, advisory own_content, batch đích công khai | Thuần Rust; không phải Quint/Kani; transport và hydration ở hàng dưới |
| DTO/đường dẫn | `contract::tests::{decodes_endorsement_fixtures, classifies_endorsement_error_fixtures}`, `api::discussion_tests::endorsement_paths…` | Fixture provider là hợp đồng, không chứng minh server |
| SSR action | `pages::endorsement::tests` (khách: tổng số nhóm `1.234` + lý do, không link; revision khác/đọc lỗi không thành số; fixture `Unknown`) | Render Rust không phải trình duyệt |
| Trình duyệt, tác giả (`main`) | `endorsementAsAuthor` ([endorsement-smoke.mjs](../../apps/vink-web/e2e/endorsement-smoke.mjs)): vị trí sau văn bản trong SSR, tổng số SSR, `Unknown` trước own read, lý do “chính mình” sau hydration, bấm không gửi lệnh, `403 self_endorsement` | LocalFixture = tác giả |
| Trình duyệt, người đọc (`endorsement`) | `endorsementAsReader` với server thứ ba (principal mẫu thứ hai + grant kiểm duyệt): duyệt gốc + phản hồi, SSR “Hữu ích”, chạm đúp/pending/xác nhận sau ACK, mất ACK sau commit → replay `replayed`, tab khác → `409` hiện trạng thái quan sát, Enter/Space trên phản hồi giữ focus, không cascade, không đổi thứ tự, số trong PostgreSQL, ma trận 320–1440 sáng/tối + 200%, own read `401` → lý do đăng nhập | Interception chỉ giữ/bỏ phản hồi thật và trả `401` cho own read; không auth production, screen reader thật, native |


## TAG-PR-01/02/03 — taxonomy core, PostgreSQL và API (#95/#96/#97, 08/11/2026)

Điểm vào là HTTP v1 (đọc catalog công khai; thẻ nháp riêng tư, production `401` tới D-05) và lệnh operator
`vink-app tag …`; từ TAG-PR-05 (#99) Bàn viết là consumer Web đầu tiên của thẻ nháp và catalog (mục
TAG-PR-05). Owner, lựa chọn và gap: [decision record](../architecture/work-taxonomy.md),
[registry](taxonomy-core.md), [storage](taxonomy-postgres.md), [API](taxonomy-api.md), [discovery](discovery-api.md) (#98);
Explore/reader/trang thẻ ở #100.

| Điểm vào / scope | Owner / assertions | Coverage limit |
| --- | --- | --- |
| Chữ thẻ tiếng Việt: nhãn, slug giữ dấu, search key bỏ dấu, mô tả, truy vấn | `domain::taxonomy::text`; `domain::taxonomy::tests` (`quick`): NFC/NFD, `ma/má/mà/mả/mã/mạ` sáu slug chung khóa `ma`, chữ viết khác/dạng tương thích/`İ`/dấu lạc, biên 40 ký tự, slug parse ↔ derive | Unit/golden viết tay; không Kani cho Unicode |
| Catalog curated: tạo, đổi tên (slug cũ thành redirect), mô tả, alias, ngừng dùng có gợi ý kế nhiệm; resolve slug; xếp gợi ý | `decide_catalog_change`, `resolve_slug`, `rank_matches`; unit tests catalog/lookup; bốn harness Kani `taxonomy_catalog_*` (tạo/mô tả, đổi tên, alias, ngừng dùng) | Unique slug do PostgreSQL giữ. Điểm vào curator là lệnh operator ở dưới |
| Thẻ nháp: owner/standing/version, facet, ≤ 3 chủ đề, thẻ ngừng dùng giữ được nhưng không thêm mới, lệnh cũ → xung đột kèm observed | `decide_tag_change`; unit tests (TEACH: Tản văn/Ký ức/Gia đình, alias, tab cũ, mất ACK, fence lệnh trùng trạng thái); khử trùng vét cạn 3 280 input; Kani `taxonomy_tag_change_respects_owner_version_slots_bounds_and_retired_tags`, `taxonomy_version_tokens_are_total` | Facts nạp dưới khóa thật ở dòng PostgreSQL bên dưới |
| Snapshot metadata và chiếu công khai/nháp | `MetadataDigest` (golden độc lập), `MetadataSnapshot::restore`, `public_tags`, `draft_tags`; test hash v1 không đổi | Binding và rút bài chỉ qua fixture; workflow publish thật là của owner publication |
| Use case `Vink::{work_tags, save_work_tags, public_work_tags}` + port `WorkTagStore`, `PublicTagStore` | [taxonomy_tests.rs](../../services/vink-app/src/application/taxonomy_tests.rs) (`quick`): store giả gọi policy thật; chưa xác thực/input sai không chạm store; refusal/xung đột/outage giữ riêng; hai tab; thẻ công khai chỉ từ snapshot được bind, ẩn thẻ ngừng dùng, thiếu catalog là `Internal` | Store giả không chứng minh SQL, HTTP hay quyền production |
| Lưu PostgreSQL (TAG-PR-02): schema/constraint, `PgStore` (`WorkTagStore`, `PublicTagStore`), `change_catalog` (từ #97 nhận `CuratorAuthority` và ghi audit), seam `capture_metadata_snapshot` | [tests/postgres/taxonomy.rs](../../services/vink-app/tests/postgres/taxonomy.rs) (`postgres`): SQL trực tiếp cho mọi luật cấu trúc, digest SQL = Rust = golden, nâng cấp không backfill, CAS/8 lệnh một thắng, riêng tư chủ/không tồn tại, race hai thứ tự (ngừng dùng ↔ gắn, đình chỉ ↔ lưu, chụp ↔ lưu) chờ quan sát bằng `pg_blocking_pids`, rollback có fault injection, slug giữ với thẻ qua alias/đổi tên/đổi về, thẻ công khai qua sửa nháp/thay/rút | Publish/rút bài qua SQL fixture có nhãn. Điểm vào curator là lệnh operator, còn thẻ nháp phụ thuộc authority |
| Đọc catalog công khai (TAG-PR-03): `GET /api/v1/tags`, `/tags/suggestions?q=`, `/tags/slugs/{slug}` | `Vink::{catalog_tags, search_tags, resolve_tag}` → `CatalogReadStore`; [catalog_tests.rs](../../services/vink-app/src/application/catalog_tests.rs) (`quick`): input sai không đọc store, `ma`/`má` không gộp, alias, thẻ ngừng dùng không gợi ý, 200 + `truncated`, resolve fail closed; `http::tests::tag_routes_validate_report_outages_and_fail_closed` (DB sập → `503 no-store`, `405` cho mọi ghi); fixture `tags.*`, `tag.resolve.*`; [taxonomy_api.rs](../../services/vink-app/tests/postgres/taxonomy_api.rs) (`postgres`): `catalog_reads_are_public_ranked_and_survive_an_identity_outage` (Identity sập: catalog `200`, thẻ nháp `503`; NFD/hoa, alias/tên cũ redirect, retired kèm kế nhiệm) | Bàn viết dùng danh mục và gợi ý từ #99. Resolve slug chờ #100. Mỗi request nạp catalog đang hoạt động, trả `no-store` (điều kiện xem lại ở [API](taxonomy-api.md)) |
| Thẻ nháp qua HTTP: `GET`/`PUT /api/v1/drafts/{work_id}/tags` | `api::{get_draft_tags, save_draft_tags}` → `Vink::{work_tags, save_work_tags}`; fixture `draft.tags.*`, `error.{tag_version_conflict,unknown_tag,tag_retired,wrong_facet,too_many_themes}`; `tag_request_requires_every_field_and_refuses_authority_fields`; PG: `draft_tags_round_trip_privately_with_presence_and_catalog_rules` (chủ khác `404`, thiếu field/`owner_id` `422` không ghi, Origin, khử trùng, từng refusal, tab cũ `409` kèm `tags`, `null`/`[]` xóa, lưu trùng trạng thái tăng version, lưu chữ không đụng thẻ, publication không đổi), `lost_acknowledgements_and_parallel_tabs_never_overwrite_a_newer_choice`, `standing_is_rechecked_when_the_tag_save_commits` | Resolver header có nhãn trong test API. Bàn viết dùng LocalFixture loopback trong smoke. Đối soát bằng trạng thái, không có receipt (CHOICE-03 A) |
| Quản trị catalog: `vink-app tag list|history|create|rename|describe|alias|retire` | `parse_tag` + `ChangeCatalog::validate` (trước cấu hình), `Vink::change_catalog` → `PgStore` + `tag_catalog_changes`; unit `tag_changes_are_validated_by_the_application_before_any_configuration`, `http_never_holds_curator_authority`; PG: `catalog_governance_runs_through_the_use_case_and_audits_every_step` (audit mỗi version, refusal/conflict không ghi, trigger từ chối bước version không audit/sửa xóa audit/authority lạ, fault injection rollback cả thay đổi), `comment_moderators_get_no_catalog_authority`, `upgrading_an_unaudited_catalog_keeps_it_and_audits_from_then_on`; binary thật: `the_operator_command_changes_and_reports_the_catalog` (stdout, exit 0/1/2, input sai trước kết nối, `history`) | Audit quy về credential database, không về người; không HTTP admin (CHOICE-02 A) |

## TAG-PR-04 — khám phá công khai theo chữ và thẻ (#98, 08/11/2026)

Điểm vào tìm toàn thư viện là `GET /api/v1/works`. SSR trang chủ dùng request mặc định. UI hiện tìm trong danh sách đã tải, còn bộ lọc thẻ và phân trang chờ #100.
Lựa chọn, plan, đo đạc và gap: [discovery](discovery-api.md); hợp đồng:
[HTTP v1](../../services/vink-app/contracts/http/v1/README.md#public-discovery-tag-pr-04).

| Điểm vào / scope | Owner / assertions | Coverage limit |
| --- | --- | --- |
| Luật thuần: `q` (giữ chữ gõ, kiểm độ dài, có chữ hoặc số, từ chối NUL), `tag` (slug chuẩn hóa, ≤ 4, 8 giá trị thô), `limit` 1..24, cursor gắn phạm vi, `filter_tags` (chỉ thẻ đang hoạt động là facet), `cut_page` | `domain::discovery`; `domain::discovery::tests` (`quick`): rỗng = vắng, NFC/NFD/Đ, biên ký tự/byte, khử trùng, truth table resolve → ràng buộc, phạm vi theo query/thẻ canonical, layout cursor viết tay, một cách viết hex | Unit; không Kani (Unicode/chuỗi cursor như các lát trước) |
| Use case `Vink::{discover_works, publication}` + port `PublicationStore` | [discovery_tests.rs](../../services/vink-app/src/application/discovery_tests.rs) (`quick`): input sai không chạm store theo thứ tự q → tag → limit → cursor, mặc định = trang chủ 12, cắt trang + cursor sau alias, cursor của tìm kiếm khác → `CursorScope`, không có/ngừng dùng → rỗng có lý do, thẻ card chỉ đang hoạt động theo thứ tự snapshot, catalog thiếu/slug lệch → `Internal`, outage → `Unavailable` | Store giả không chứng minh SQL, snapshot hay plan |
| `GET /api/v1/works?q=&tag=&limit=&cursor=`, `GET …/publication` (thẻ công khai) | `api::{works_query, list_works}`; `http::tests::work_search_validates_before_reading_and_reports_outages` (`quick`: mọi tham số sai `422` đúng field trước database, outage `503 no-store` không ETag); fixture `works.{list,search,search.unmatched}.json`, `publication.get.json` (provider + `decodes_public_fixtures` của web) | Consumer web chỉ decode; UI ở #100 |
| PostgreSQL thật qua router | [tests/postgres/discovery.rs](../../services/vink-app/tests/postgres/discovery.rs) (`postgres`): ngoài 12 đầu + mọi trang với `limit` 1/5/7/24 qua ties; truth table chữ/thẻ/alias/đổi tên viết tay; nháp/chưa công khai/đã rút không vào JSON/HTML/kết quả, sửa thẻ nháp vẫn `304`; 200→304, đổi nhãn → 200, body/ETag nhất quán dưới rename đồng thời; cursor sai/lệch phạm vi `422`, thay đổi dữ liệu giữa hai trang, sigma Hy Lạp kiểm cursor dùng đúng khóa SQL của bộ lọc. Khóa SQL = Rust trong các ca tiếng Việt/Latin của test; `discovery_measurement` (`#[ignore]`, 20 000 tác phẩm, `EXPLAIN ANALYZE BUFFERS`) | Publication là fixture SQL có nhãn (một bài qua seam capture thật); rename đồng thời là best-effort; đo warm trên một máy |

## TAG-PR-05 — bộ chọn thẻ và Thông tin tác phẩm (#99, 09/11/2026)

Consumer Web của thẻ nháp; component dùng chung cho #100. Luồng HOW/WHY/TEACH, lựa chọn và bằng chứng:
[taxonomy-editor](taxonomy-editor.md); bố cục: [LayoutSpec](../design/tag-selector.md).

| Điểm vào / scope | Owner / assertions | Coverage limit |
| --- | --- | --- |
| Consumer contract thẻ: DTO, phân loại lỗi, đường dẫn | `TagDto`, `TagList`, `TagSuggestions`, `DraftTags`, `SaveDraftTagsRequest`, `SavedDraftTags`, `ApiError::{TagConflict, TagRefused}` trong [contract.rs](../../apps/vink-web/src/contract.rs): `decodes_tag_fixtures` (round-trip từng field, `form: null`/`themes: []` luôn gửi), `classifies_tag_error_fixtures`; `tag_paths_keep_the_raw_query_and_the_work_in_their_own_parameter` (`quick`) | `tag.resolve.*` chưa có consumer (#100) |
| Trạng thái thẻ nháp và query | `TagSession`, `QueryState` trong [tag_state.rs](../../apps/vink-web/src/tag_state.rs): test thứ tự có kiểm soát (ACK muộn/trùng, mất ACK và `409` khớp lựa chọn, version tăng hơn một vẫn xung đột khi chỉ khớp lựa chọn cũ, 401, từ chối thẻ, composition/answer cũ/echo khác) (`quick`) | Pure, không DOM/transport |
| Chữ + thẻ khi lưu và rời trang | `DraftSession`, `LeaveGuard<T: Guarded>` trong [desk_state.rs](../../apps/vink-web/src/desk_state.rs): test chỉ thẻ, chờ cả hai, lưu một phần, conflict chặn. Trả về lựa chọn cũ khi đang lưu vẫn giữ điều hướng. Khi mất ACK, thử lại chỉ gửi thẻ với version cũ (`quick`) | Hai request riêng; không giao dịch chung chữ + thẻ (TAG-PR-03 CHOICE-01 B) |
| Adapter DTO → facts | `TagLabels`, `tag_catalog`, `tag_options`, `exact_option`, `tag_refusal_text` trong [ui/adapters.rs](../../apps/vink-web/src/ui/adapters.rs): fixture `draft.tags.get`, `tags.list`, `tags.suggestions` (`quick`) | Nhãn chỉ từ dữ liệu đã đọc; ID chưa đọc hiện là “Thẻ chưa tải được tên” |
| `TagSelector`, `TagChip` (Leptos) | [tags.rs](../../packages/vink-patterns/leptos/src/tags.rs): SSR tests tên nút bỏ, trạng thái viết bằng chữ, nhóm/đếm/gợi ý (`quick`); preview `composition.tag-selector` 9 kịch bản + smoke: tên nút bỏ, legend/đếm, status polite, ↓/↑/Escape, refusal gắn chip, thay thẻ ngừng dùng, Enter khi composition không chọn, Enter sau composition chọn exact, focus sau bỏ chip (`ui-preview leptos verify`) | Composition mô phỏng bằng CDP, không phải IME hệ điều hành; cây accessibility không thay screen reader |
| `TagSelector`, `TagChip` (CMP) | [Tags.kt](../../packages/vink-patterns/cmp/src/commonMain/kotlin/vink/patterns/Tags.kt), `TagsTest` (common), specimen `TagSpecimens.kt` cùng 9 kịch bản (`ui-preview cmp verify`) | Desktop off-screen; không native product app/consumer, không runtime Android/iOS |
| Editor thật: lưu/đọc lại thẻ, IME, conflict, thẻ ngừng dùng. Giữ guard khi tác giả hoàn tác lựa chọn lúc đang lưu. Mất ACK rồi đối soát qua `409` vẫn giữ lựa chọn mới trên trang và version chữ | `draftTags` + `retiredTagAfterRestart` trong [browser-smoke.mjs](../../apps/vink-web/e2e/browser-smoke.mjs); `verify web` seed catalog bằng `vink-app tag` và ngừng dùng Đô thị giữa hai phase ([web.rs](../../xtask/src/web.rs)) | LocalFixture loopback có nhãn; production `401` (D-05); một trình duyệt Chrome headless |

## TAG-PR-06 — giao diện discovery #100

[LayoutSpec](../design/tag-discovery.md) và [HOW/WHY/TEACH + evidence](discovery-ui.md).
Browser assertions ở [discovery-smoke.mjs](../../apps/vink-web/e2e/discovery-smoke.mjs): GET/no-JS, Apply, query/tag/page, canonical/unknown tag, reader return và matrix reflow; phase discovery-corpus dùng SQL publication fixture có nhãn để tìm ngoài 12 bài, AND/pagination và race đảo thứ tự response. READ-02 cache dùng query/filter/cursor làm key; 404 reader gỡ khỏi mọi snapshot và fence response đang bay.
