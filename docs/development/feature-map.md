# Bản đồ tính năng và đường kiểm chứng Vink

## Hồ sơ tác giả và Theo dõi (#288)

**Mức triển khai Social tại base `c24e5849` là `NOT_IMPLEMENTED`.** Hợp đồng r1 cho #289–#298
được ghim ở [commit cdbdde89](https://github.com/loveoverflowcom/vink/blob/cdbdde896218251cf74b25fe38c585cddbd9a9de/docs/architecture/social-graph.md).
Từ bút danh trên bài/bình luận, người đọc sẽ mở `/@handle`, theo dõi tác giả và đọc bài ở
Khám phá → Đang theo dõi. [Bảng truy vết và bàn giao](social-graph.md) ghi 12 bất biến,
30 tiêu chí, revision registry và phép kiểm của từng owner. [Ma trận quyền và DTO](../../services/vink-app/contracts/http/v1/social-graph.md)
là đầu vào cho LayoutSpec và provider. Mốc #288 có bằng chứng tài liệu/registry. Các lượt
kiểm PostgreSQL, HTTP và browser cho hành trình Social thuộc từng PR triển khai.

**Đối soát có phạm vi ngày 16/11/2026, `develop@6e07806`:** [20 PR gần đây, proposal và gap](verification-reconciliation-20261116.md). Các mốc ở phần mở đầu dưới đây là lịch sử theo lát, không phải ngày kiểm lại toàn bộ bản đồ. [Model/replay ReadOwner/PageChain](../../apps/vink-web/behavior/README.md) bổ sung coverage thuần; không refresh evidence browser/PG cũ.

**Web Perf #234 · baseline opt-in:** [cách đo và giới hạn](web-perf-baseline.md) với 200 works/1.000 comments, 10 cold + 10 warm, history SSR, transport secondary/304 và [trace release hai corpus kèm owner/timer](evidence/web-perf-234-canonical/README.md). Debug lab `997d97d` vẫn là [evidence lịch sử](evidence/web-perf-234/README.md).

**Web Perf #235 · SSR Reader:** [owner, lựa chọn A/B/C và regression](reader-ssr-boundary.md) giữ Async. `main` cover blocked bootstrap, WASM-only delay/failure và fresh Reader handoff; ma trận Leptos/Axum giữ status/privacy trước response. [Ma trận direct/Caddy](evidence/web-perf-235-canonical/README.md) đã kiểm OutOfOrder và rollback Async.

**Web Perf #236 · quay lại Khám phá:** [LayoutSpec](../design/discovery-return-position-236.md) và scenario `return-position` opt-in trên fixture #234. Router ghi fragment vào history; callback chỉ phục hồi vị trí của scope đã settle, bằng hàng hoặc vùng kết quả có tên. [50 vòng với bộ đếm owner và đối chứng rollback](evidence/web-perf-236-canonical/README.md). Không giữ lại view hoặc offset đã rời.

**Web Perf #237 · ngân sách RAM READ-02:** [owner READ-02](public-read-freshness.md#ngân-sách-byte-ram-trong-read-02-237-web-perf-07--choice-01) và [bằng chứng A/B/C](evidence/web-perf-237/README.md) chọn `WEB-PERF-07 CHOICE-01 = B`: `InMemoryCache::with_budget` (`2 MiB` tổng / `1 MiB` mỗi entry proxy UTF-8 + cấu trúc, thứ tự đưa snapshot vào cache) song song `max_entries = 16` / `5 phút`, giữ `NoOpCache` và `RetentionBudget::UNBOUNDED` làm đường rollback theo recipe/type-check trong owner; entry vượt trần vẫn đọc/render đủ và được bỏ khỏi optional retention khi rời view; giữ nguyên `ENABLED = false`, `15 phút`/`24 giờ` và `DESIGN_PROPOSED`. [Đối soát canonical và vòng đời Resource](evidence/web-perf-237/canonical-20261118/README.md).

Với thay đổi liên quan đến quyết định ngày 10/11, tra ID và revision tại
[kho quyết định TOML](../../decisions/README.md). Phạm vi và kiểm chứng của công cụ nằm trong
[bàn giao #164](decision-registry-rollout.md).

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
Bằng chứng lịch sử Bàn viết R6 (#133), trên nền `e2294a2`, giữ thứ tự resume trước search;
`draftR6Hierarchy` kiểm fixture ba nháp NFD, khối resume ≤250px và search lên ≥48px
ở 390/100, một scope/kết quả, primary link/disclosure focus thường + forced-colors,
list chung nền, full title/equal inset/exact IDs tại 320–1440/chữ100–200.
`draftDashboard` giữ geometry, query/IME, sort, quyền, last-success và
uncertain-create. [LayoutSpec R6](../design/desk-r6.md) thay phân cấp/list grouping
của R5.1 cho Desk; Khám phá không đổi. [Bằng chứng local](desk-r6-evidence.md).

Chuẩn bị [action surfaces #225](../design/action-surfaces-desk-discovery-225.md) trên
`develop@69f71f0`: `draftDashboard` thêm native create/Select/Refresh và pending keys;
`discoveryReading` thêm Apply/checkbox/max4/no-JS, `discoveryCorpus` thêm history/set equality/
cursor reset. [R2](action-surfaces-225.md) dùng Filled + Add cho create, Tonal cho Refresh,
FilterList cho launcher và copy AND/max4/page-count gọn. Tests thêm kiểm container/glyph,
0/1/4 thẻ, vượt giới hạn, catalog lỗi và status của trang cuối; giữ guard và native GET hiện có.
[Evidence chuẩn bị](evidence/action-surfaces-225/README.md) giữ riêng baseline lịch sử.

Phân trang nháp [#201](desk-pagination.md) mở rộng `/api/v1/drafts` bằng query/sort/cursor
trên toàn corpus owner, giữ caller không tham số Recent/100. Provider/consumer fixtures và
`tests/postgres/drafts.rs` sở hữu kiểm paging/isolation/Unicode/backfill/query plan. Consumer
Web dùng ReadOwner/PageChain từ #212/#202, submit query rõ ràng, tải 24 mục/trang,
tối đa 10 trang/phần. `draftPagination` kiểm corpus HTTP/PG thật 250 nháp và race transport;
`authWeb` kiểm đổi tài khoản với response thật bị giữ. Kết quả local ghi riêng tại owner #201.

Select Web R1.5 (#135) giữ native control ở Bàn viết và gallery. `selectChevron`
trong cả hai browser runner dùng chung `select-geometry.mjs`: đo 18px/12px/8px,
tâm dọc, vùng chữ bằng canvas, native type-ahead và disabled/invalid/forced-colors
ở 320/390/1440 × hai theme × chữ100/200. [Bằng chứng local](select-r15-evidence.md).

#262 bổ sung `SelectVariant::Inset16` của Leptos và mẫu `#format-select` trong
`field.select`: `formatSelect` trong preview smoke đo icon/inset/reserve 16/16/52 CSS px,
LTR/RTL, 320/390/834/1024/1440 × hai theme × chữ100/200; kiểm hit target click-through, native focus/Tab/type-ahead, AX name và
invalid/disabled/forced-colors. Popup OS và touch thực cần evidence riêng.

Document canonical [#255](document-source-format.md): một shape không version
`{format: "default" | "markdown", source}` với source nguyên văn, một `ContentHash::of`
(`vink-document-content`), migration cutover `20261122000000` (fail-closed khi còn revision cũ —
[lịch sử bất biến](../architecture/document-source-format.md#lịch-sử-bất-biến)) và
`VDR-DOCUMENT-SOURCE-FORMAT@r2` **ACCEPTED**. `POST /api/v1/drafts` nhận body JSON tùy chọn
`{"format": "default" | "markdown"}` (vắng = `default`). Bàn viết tạo và soạn cả `default` lẫn
`markdown` qua một `EditorView` Live Preview (#256), mục lục AST Cách 1 (#257) và luồng chọn/đổi
định dạng có xem trước (#262). Bộ chọn có tại CTA của tài khoản có 0 tác phẩm. Nút hành động chính tách `Gửi` (phiên bản 1) / `Lưu` (phiên bản ≥ 2)
kèm icon 18px Lucide. Markdown vẫn bị chặn xuất bản công khai (`format_not_publishable`) cho đến #264.
Xem [LayoutSpec hợp nhất](../design/editor-live-preview-256-257.md),
[LayoutSpec picker #262](../design/editor-format-picker-262.md) và
[bằng chứng Web cho link nhiều dòng, CTA đầu tiên, lỗi tên và p95 mục lục](evidence/desk-markdown-first-draft/README.md).

#263 [title theo định dạng](../architecture/editor-title-authority.md): Mặc định dùng tên riêng,
Markdown dùng H1 đầu tiên của AST CommonMark (`pulldown-cmark`, không extension), vật chất hóa trong
cùng save/CAS; snapshot giữ title của source đã capture. Corpus `default.json` (12 ca) và
`markdown.json` (30 ca) chạy qua domain/application; `apps/vink-web/editor/scripts/test-markdown.mjs`
kiểm đối xứng cả 30 ca `markdown.json` trên CommonMark parser phía trình duyệt (`@codemirror/lang-markdown`).
Bàn viết hiển thị lỗi title của API cạnh vùng cần sửa. Web main kiểm buffer, sửa và lưu lại,
nháp trống tên và phản hồi `MissingTitle` của snapshot trong PostgreSQL scratch.
Sáu test PostgreSQL (`canonical_document::*`) kiểm save/CAS, snapshot, schema và migration/backout;
không phải một lượt replay toàn bộ corpus qua PostgreSQL.


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
  fixture local trên loopback; [Auth Web](auth-web.md) dùng Identity thật khi deployment mở.
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
| Khám phá tác phẩm công khai | /?q=…&tag=…&tag=…&cursor=…; /the/:slug | Discovery toàn thư viện qua API #98, AND tối đa 4 thẻ, GET/SSR, danh sách hữu hạn và link trang kế. Lựa chọn thử chỉ áp dụng sau Áp dụng; alias canonical theo server; lỗi khác rỗng. Empty #258 dùng WebP app-owned, một câu status, phân biệt corpus/miss/page/tag. | [pages/discovery.rs](../../apps/vink-web/src/pages/discovery.rs), [discovery.rs](../../apps/vink-web/src/discovery.rs), [TAG-PR-06](discovery-ui.md) |
| Đọc tác phẩm | /tac-pham/:id?q=…&tag=…&cursor=…&from_tag=… | Giữ toàn bộ ngữ cảnh discovery/trang thẻ và focus hàng; nếu hàng mất thì focus landmark kết quả có tên (#258). Canonical reader bỏ query; metadata thẻ lấy từ snapshot công khai. | [pages/mod.rs](../../apps/vink-web/src/pages/mod.rs), [discovery UI](discovery-ui.md) |
| Điều hướng thích ứng | Rail: Khám phá → Bàn viết, Tôi ở chân; mobile: Khám phá → Bàn viết → Tôi | `/` vẫn là mặc định. Desktop 220px; tablet/editor 96px; thu/mở nhớ trên thiết bị và ghi đè ngữ cảnh. Mobile ẩn bar khi viết. Route lạ (`/ban-vietnam`, `/tac-pham/a/b`) là 404, không sáng đích. Back/Forward khi Tôi mở chỉ đóng Tôi, kể cả lịch sử fragment nội trang ([UI-REVIEW-20](../design/ui-review-20.md)) | [shell.rs](../../apps/vink-web/src/shell.rs) (`Destination`), primitive [vink-ui navigation](../../packages/ui-system/leptos/src/navigation.rs), [spec shell](../design/README.md#shell-thích-ứng-7) |
| Tôi và chọn giao diện | Rail: Tôi mở panel Giao diện; Mở trang Tôi tới `/toi`; mobile: Tôi tới `/toi` | `/toi` và panel dùng chung lựa chọn màu, cỡ chữ 100%/200% và chế độ rail. Cả ba áp dụng trước hydration và lưu trên thiết bị nếu có storage. Thẻ Tài khoản và điều hướng dùng avatar trung tính trên SSR; sau hydration, avatar chữ lấy từ bút danh của principal qua `GET /api/v1/auth/session/account` (`no-store`). Chuyển route dùng guard của editor | [appearance.rs](../../apps/vink-web/src/appearance.rs), [me.rs](../../apps/vink-web/src/pages/me.rs), [NavigationGuard](../../apps/vink-web/src/navigation.rs) |
| Tìm và viết tiếp | “Bàn viết” → `/ban-viet` → hàng bản thảo → `/ban-viet/{id}` | #214 đọc `/author/works`, gồm works không có draft; trạng thái, Thấm theo public revision và Nhận xét theo public thread, metric lỗi khác zero, nháp ẩn metrics ([owner](desk-author-feedback.md)). #213 bỏ resume/Recent/1; row có full title (mặc định hai dòng, focus/hover mở đủ) + thời gian cập nhật thật, không version nhìn thấy. Tìm có/không dấu, Đ/đ và NFC/NFD toàn corpus; 24 mục/trang, 240 mục/phần rồi xem phần tiếp. Unfiltered empty, query miss, loading, lỗi, 401/403 tách riêng; query/sort/input giữ qua refresh/retry. | `DeskIndex`, `DraftListView` trong [desk.rs](../../apps/vink-web/src/pages/desk.rs), [draft_list.rs](../../apps/vink-web/src/draft_list.rs), `AuthorWorkRow` trong [vink-patterns](../../packages/vink-patterns/leptos/src/lib.rs) |
| Tạo và đối chiếu kết quả | Heading “Tạo bản thảo”; khi rỗng “Viết bản thảo đầu tiên” → editor. Mất phản hồi → “Tải lại danh sách” → mở nháp tìm thấy hoặc chủ động “Tạo thêm bản thảo” | Một POST đang bay; lỗi 401/403/validation tách khỏi outcome chưa rõ (network, 503, bất thường). Không tự retry POST. Chỉ GET thành công mới mở khóa tạo thêm; GET không chứng minh POST nào đã ghi. Phản hồi muộn sau khi rời dashboard không điều hướng | `CreateDraftButton`, `CreateNotice`, `DeskIndex`; `CreateState` trong [draft_list.rs](../../apps/vink-web/src/draft_list.rs) |
| Viết và lưu nháp | `/ban-viet/{id}` → sửa “Tiêu đề”, “Nội dung” (CodeMirror 6) → slot `#save-slot` hợp nhất; có Enter ở tiêu đề và Ctrl/Cmd+S trong form (chờ IME commit) | Clean chỉ hiện `✓ Đã lưu` tĩnh nhẹ + Chi tiết máy chủ (không CTA `Lưu` và không phát `PUT` thừa); dirty/retry mới hiện `#save` (`unsaved` → `saving` → `saved`), phân biệt từng ca partial chữ/thẻ, sửa tiếp khi đang lưu, `unconfirmed` vs `rejected`, `refused` và `conflict` dẫn tới recovery thật; giữ focus không rơi về `<body>` qua các chuyển pha ([#217](../design/ui-editor-unified-save-217.md)). Lưu thủ công, chưa autosave | `DraftEditor`, `Editor` (`project_save_slot`) trong [desk.rs](../../apps/vink-web/src/pages/desk.rs), [API client](../../apps/vink-web/src/api.rs), [`DraftSession`/`EditorSession`](../../apps/vink-web/src/desk_state.rs), [BodyEditor](../../apps/vink-web/src/editor.rs), [runtime](../../apps/vink-web/editor/README.md), [UI-REPAIR-05](../design/ui-repair-05.md) |
| Xử lý xung đột | Mở cùng nháp ở hai tab, tab B lưu trước, tab A lưu bản cũ → “Lưu đè bằng nội dung đang viết” khi muốn giữ bản A | Save cũ nhận `409`, không ghi; bản A còn trong editor. Lưu đè dùng version được báo và vẫn có thể xung đột lần nữa. Nếu cần giữ chữ ở tab khác thì ở lại trang để đối chiếu | `Editor`, `EditorSession`; owner/version được kiểm trong [PgStore](../../services/vink-app/src/persistence/postgres.rs) |
| Xem trước bản đang viết | “Xem trước” trực tiếp trên toolbar ([Paper First #218](../design/ui-editor-paper-first-218.md), [Editor yên tĩnh #254](../design/editor-quiet-254.md)) | Hộp thoại đọc dùng `DocumentView` từ chữ hiện tại (kể cả chưa lưu). Không lưu, không gửi duyệt, không đổi trạng thái. Đã bỏ nút tải bản thảo (`.txt`), hướng dẫn ngắt dòng và footer `#build-info` | `Editor`, [document.rs](../../apps/vink-web/src/document.rs) (`text_to_document`, `DocumentView`) |
| Rời editor khi chưa lưu | Link “Khám phá”/“Bàn viết”/“Tôi”, nút Tonal + icon `Bàn viết` (`.vk-editor-heading__back`), back/forward của trình duyệt khi còn chữ **hoặc thẻ** chưa được xác nhận | `vink_ui::Dialog` tự cuộn với gutter dùng chung (`#leave-editor-context`). “Ở lại” hoặc Escape trả focus và vị trí cuộn về trường đang viết. “Lưu và rời trang” chỉ đi sau ACK của các phần cần lưu, bị khóa (`aria-describedby="leave-editor-context"`) khi lượt lưu đang chờ hoặc có xung đột (lúc xung đột “Ở lại” giữ `Filled`). Khi `can_discard` đúng, nhóm phụ không divider giữ “Rời mà không lưu…” (`DangerText`). Bước sau có “Quay lại” (`Tonal`) và “Bỏ thay đổi và rời” (`Danger` đặc). Reload/đóng tab dùng `beforeunload` | [LeaveGuard](../../apps/vink-web/src/desk_state.rs) (generic qua `Guarded`, `DraftSession`), [NavigationGuard](../../apps/vink-web/src/navigation.rs), `Editor`, [R1 #208](../design/dialog-anatomy-r1-208.md), [Editor yên tĩnh #254](../design/editor-quiet-254.md) |
| Gắn thẻ cho bản nháp (TAG-PR-05) | `/ban-viet/{id}` → mục “Thông tin tác phẩm” sau trang viết → “Chỉnh thẻ” mở tại chỗ → chip “Dạng bài”, ô “Tìm chủ đề” → gợi ý/“Xem các chủ đề có sẵn” → “Lưu” trên toolbar (hoặc Ctrl/Cmd+S) | Chỉ trình duyệt đọc thẻ nháp và danh mục (SSR không có mục này). Tên thẻ và trạng thái luôn thấy; Chỉnh thẻ thu/mở giữ cùng selector, lỗi và phục hồi nằm ngoài disclosure ([desktop R1.4](../design/editor-r14.md)). Một dạng bài, ≤3 chủ đề từ catalog; chip có nút bỏ có tên riêng; thẻ ngừng dùng được giữ, ghi bằng chữ, “Thay bằng …” khi có kế nhiệm; refusal của server gắn vào đúng chip. Thay đổi chỉ thẻ là `unsaved`, toolbar gộp trạng thái chữ + thẻ, “Lưu” gửi riêng phần đã đổi. `409` của thẻ: “Dùng thẻ đang lưu”/“Lưu đè bằng lựa chọn trên trang”; Mất ACK → “Thử lại” gửi lựa chọn hiện tại với version cũ. Trả về lựa chọn cũ khi còn lệnh chờ hoặc chưa rõ kết quả vẫn giữ guard rời trang. Lưu thẻ không công khai gì; production `401` (D-05) | [work_info.rs](../../apps/vink-web/src/pages/work_info.rs), [tag_state.rs](../../apps/vink-web/src/tag_state.rs), `DraftSession` trong [desk_state.rs](../../apps/vink-web/src/desk_state.rs), adapter [ui/adapters.rs](../../apps/vink-web/src/ui/adapters.rs), `TagSelector`/`TagChip` trong [vink-patterns](../../packages/vink-patterns/leptos/src/tags.rs), [LayoutSpec](../design/tag-selector.md) |
| Kiểm tra tiến trình | `/health/live`, `/health/ready` | Liveness tách khỏi kết nối/schema database; readiness `200` chỉ khi lịch sử migration trùng migration nhúng (`pending_migrations`, `diverged` là `503`); báo `checks.role`, production từ chối role `privileged`; Identity chỉ là thông tin, không làm App mất readiness. Không khẳng định có auth, moderation, Discourse hay job | [health.rs](../../services/vink-app/src/http/health.rs), [vink-store-postgres](../../packages/store-postgres/src/migration.rs) |
| Bàn viết khi có Identity | Request API riêng tư với cookie `__Host-vink_session` hoặc `Authorization: Bearer` khi App cấu hình `VINK_IDENTITY_SOCKET` | Identity xác minh phiên mỗi request, App đọc binding và đình chỉ của mình: chỉ principal đã bind, không bị đình chỉ được hành động; không credential/bị từ chối/hết hạn/đã thu hồi/chưa bind/bị đình chỉ `401`; Identity hoặc App DB không quyết được, trả lời hỏng, socket do user khác phục vụ `503 dependency_unavailable`. Đọc công khai không gọi Identity. Local fixture (local + loopback) vẫn quyết định khi được bật | [identity](../../services/vink-app/src/identity/mod.rs), [client](../../services/vink-app/src/identity/client.rs) |
| Đăng nhập bằng mã email (D-05 Provisional, `/toi` → “Dùng mã email”) | `POST /api/v1/auth/sign-in/challenges`, `POST /api/v1/auth/sign-in/complete`, `GET`/`DELETE /api/v1/auth/session`, `POST /api/v1/auth/principal` | Production: hai route mã trả `501` cho tới `VINK_AUTH_SIGN_IN=enabled`; đếm theo mạng nguồn (10 xin/30 trả mã mỗi 15 phút, `X-Forwarded-For` chỉ từ `VINK_TRUSTED_PROXIES`) trước khi gọi Identity, vượt `429` + `Retry-After`, không đếm được `503`; mã được worker Identity gửi sau commit (outbox bền, SMTP hoặc outbox cục bộ); đăng nhập lại thu hồi phiên cũ; handle và credential chỉ ở cookie `__Host-` HttpOnly; chủ thể mới là `needs_principal` cho tới khi tạo principal bằng bút danh (một principal/subject kể cả đồng thời); mã sai `422 code_rejected`, challenge đóng `409`, tài khoản bị disable/principal bị đình chỉ `403` và không đặt cookie; đăng xuất chỉ xóa cookie khi Identity đã thu hồi (Identity sập `503`, cookie giữ lại) | [auth.rs](../../services/vink-app/src/http/auth.rs), [API v1](../../services/vink-app/contracts/http/v1/README.md#sign-in-d-05) |
| Mật khẩu là lựa chọn đầu (ACC-03) | `/toi` → Đăng nhập / Tạo tài khoản → bút danh nếu cần → Bàn viết | Web dùng `POST /api/v1/auth/password/{login,register}`, đọc lại phiên; sai thông tin là lỗi form, tên trùng khi đăng ký là lỗi trường. Mất ACK không gửi lại tự động | [Auth Web](auth-web.md), [LayoutSpec](../design/auth-password-web.md) |
| Thêm username/mật khẩu (ACC-03, `/toi` → “Tên đăng nhập và mật khẩu”) | `POST /api/v1/auth/password/enroll/challenges` → `POST /api/v1/auth/password/enroll`. Đọc lại bằng `GET /api/v1/auth/password/enroll` | Cần phiên `active` hoặc `needs_principal` và email đã xác minh. App kiểm đình chỉ trước RPC. Identity gắn mã với phiên yêu cầu và kiểm hạn phiên sau khi lấy khóa. `code_rejected` và `username_taken` giữ cookie mã để thử tiếp. `challenge_closed` xóa cookie mã. GET trả `{enrolled}` sau khi mất phản hồi. Hai phương thức đăng nhập dùng cùng subject và principal. Đăng ký từ phiên hợp lệ trả `409 already_signed_in` | [Hợp đồng App](../../services/vink-app/contracts/http/v1/README.md#sign-in-d-05) |
| Đình chỉ principal / disable tài khoản (vận hành) | `vink-app principal suspend <uuid> <lý do>`, `reinstate`, `list`. `vink-identity account disable <subject>` hoặc `enable` | Đình chỉ đóng principal ở request kế tiếp dù Identity xác nhận phiên. Gỡ đình chỉ phục hồi quyền của principal. Disable thu hồi mọi phiên trong cùng transaction và chặn đăng nhập. Enable cho phép mở phiên mới. Các thao tác chỉ có qua CLI vận hành | [main.rs](../../services/vink-app/src/main.rs), [principal.rs](../../services/vink-app/src/persistence/principal.rs), [accounts.rs](../../services/vink-identity/src/accounts.rs) |
| Identity riêng, có/không authority | `vink-identity serve` trên `VINK_IDENTITY_SOCKET`. `GET /identity/v1/capabilities`, `POST /identity/v1/verify-credential`. Cấu hình `email-code` mở mã email và đổi địa chỉ, `password` mở đăng ký/đăng nhập bằng mật khẩu, `native` mở cả hai và enrollment | Caller sai uid hoặc thiếu cấu hình nhận `401`. Với caller hợp lệ, thiếu authority hoặc phương thức thì thao tác tương ứng trả `501`. Verify trả subject + phiên hoặc `rejected{reason}`. Payload sai trả `400`/`413`, throttle `429`, version khác `404`, quá deadline `503` (2 s cho đọc, 5 s cho ghi). Readiness cần DB đúng schema và caller đã cấu hình, báo `mail_transport` (`smtp`/`local_outbox`), `mail_queue` và `production_authentication` (`not_accepted` với SMTP, `not_implemented` với outbox cục bộ). Worker gửi thư at-least-once và prune mỗi giờ. CLI có `vink-identity maintenance prune` và `mail status` | [vink-identity](../../services/vink-identity/README.md), [contract v1](../../services/vink-identity/contracts/rpc/v1/README.md) |
| Chuẩn bị PostgreSQL theo owner | `services/postgres/provision.sh [--check]`, `vink-app migrate` | DB/role App và Identity; DB cũ được nhận quyền; chạy lại không đổi password; migrate chỉ bằng `VINK_MIGRATE_DATABASE_URL`, thiếu quyền thoát 1 | [services/postgres](../../services/postgres/README.md), [main.rs](../../services/vink-app/src/main.rs) |

Các điểm bám dashboard: `draft-loading`, `draft-content` (`aria-busy`), `draft-empty`,
`draft-row`, `draft-search`, `draft-sort`, `draft-scope`,
`draft-result-count`, `draft-no-results`, `clear-draft-search`, `refresh-drafts`,
`retry-drafts`, `create-notice`, `reconcile-drafts`. Spec và delta nằm ở
[dashboard #8](../design/README.md#dashboard-bàn-viết-8).

Các action đã migrate có ID trên button (`#create-draft`, `#save`, `#preview`,
`#retry-save`, `#keep-mine`, `#leave-stay`, `#leave-save`, `#leave-discard`,
`#discard-back`, `#discard-confirm`, `#preview-close`, `#profile-close`, `#open-me`);
field có ID `#draft-search`/`#draft-sort`/`#draft-title`. Trạng thái vẫn dùng `data-testid`:
`save-status`, `editor-toolbar`, `save-failed`, `conflict`, `desk-error`, `editor-failed`;
`data-testid=editor-body` là host CodeMirror (`data-state=loading|ready|failed`, chữ ở
`.cm-content`, từng dòng `.cm-line`); `input[name=title]` là trường tiêu đề. Dialog có
`data-testid=leave-editor|draft-preview`, ngữ cảnh dùng `.vk-ui-dialog__context` (`#leave-editor-context` / `#draft-preview-context`);
button bên trong dùng các ID đã liệt kê. Dashboard refresh/retry/reconcile có ID
`#refresh-drafts`, `#retry-drafts`, `#reconcile-drafts`, `#clear-draft-search`.
Shell có ID `navigation-toggle`, `navigation-primary`, `navigation-tooltip`, `profile-trigger`,
`bottom-navigation` và `data-testid=profile-menu`; nút tiện ích dùng `#profile-close`, `#open-me`.
radio theme có `name=overlay-theme|page-theme` và `value=light|dark|system`. `save-status` có `data-status` là `saved`, `unsaved`, `saving`, `failed`, `conflict`.
Đây là điểm tìm phần tử, không thay thế việc kiểm nội dung, version và dữ liệu bền vững.

Editor [#193](../design/editor-writing-reflow-193.md) thêm `#draft-title-details`
(summary “Xem trọn tiêu đề”), bản đọc `#draft-title-full` và nút `#edit-full-title`.
Input một dòng vẫn sở hữu chỉnh sửa; bản đọc wrap dùng trực tiếp cùng title state.
`editorChrome` gọi [editorWriting](../../apps/vink-web/e2e/editor-writing-smoke.mjs)
để kiểm reflow, raw buffer/view, native selection/undo/composition, no-write,
UTF-16 so với trim/NFC/scalar ở API, preview và focus. Cũng chạy trong
`verify web --scenario main`; không là scenario xtask riêng.

## Phần việc còn lại của quyết định sản phẩm 10/11/2026

Bảng ghi các phần đã có điểm vào và các phần đang chờ. Nguồn và bằng chứng model ở
[registry](product-decisions-registry.md).

| Hành vi đích | Yêu cầu | Hiện có trong app | Bằng chứng hiện có |
| --- | --- | --- | --- |
| Chọn “Chỉ mình tôi”/“Công khai”, gửi để kiểm duyệt, đăng đúng bản | AUD-01/02, PUB-01/02 | API tác giả #158 (chụp snapshot, yêu cầu công khai, chuyển riêng tư, đọc trạng thái) + #159: consumer `vink-worker` (tắt mặc định, chỉ evaluator kịch bản local/test) hoặc người duyệt kết luận trên hàng chờ; không có evaluator thì bản mới chờ người duyệt. Web: vùng “Công khai” trong Bàn viết chọn audience, lưu → chụp → gửi, chuyển riêng tư và đối soát ([PUB-AUTHOR-WEB-01](publication-author-web.md)); trang trạng thái/phản đối dùng cùng DTO | [Pure core/replay](publication-core.md); [runtime](publication-runtime.md) + mục PUB-RUNTIME-01 dưới đây |
| Trợ lý AI duyệt, quản trị viên override | MOD-07..09 | Lõi thuần #151; #159 nối lease/claim, thử lại với `EvaluationId` mới và kết luận của admin/moderator ([moderation-runtime](moderation-runtime.md)). Chưa có AI provider thật: `/health` và worker báo `ai_moderation: not_implemented` | [Moderation core/replay](moderation-core.md); replay M1 + `evaluator-vectors.json` |
| Quản trị kiểm duyệt: admin/grant/nhóm, nhận ca, phản đối (cả quyết định moderator), dừng tự động, điều tra riêng | MOD-09..15 | #159 persistence-api: ghế admin (CLI), grant người/nhóm, ca duyệt (nhận, nhả, ghi chú, chuyển admin, tiếp quản, kết luận, mở ca can thiệp), dừng/bật tự động có audit. Sau #170 có store/API phản đối và Web ca tác phẩm/phản đối/chế độ tự động; còn điều tra riêng, ca bình luận và UI grant/nhóm | [ADMIN-CORE-01](moderation-core.md#admin-moderation-core--admin-core-01); replay M4 `governance.qnt` + `governance_tests` |
| Tác giả xóa bài, trang thay thế còn bình luận; trang “Nội dung đã được ẩn” khi bài đang công khai chuyển riêng tư | PUB-04/05/07, COM-11 | Trang sau kiểm duyệt (`O-17@r2`) có ở API/SSR/Web (#159). Trang “Nội dung đã được ẩn” có reader API/SSR/Web, chỉ giữ bình luận đủ điều kiện; tombstone chưa có runtime | Lõi thuần + replay M3 `tombstone.qnt` ([PUB-HIDDEN-01](publication-core.md#trang-ẩn-và-gửi-duyệt-theo-phiên-bản--pub-hidden-01)) |
| Sửa bình luận, dấu “Đã sửa” | COM-10 | Không; chỉ gỡ được bình luận của mình | Model `comment_edit.qnt` |
| Bộ sưu tập cá nhân | COL-01..06 | Không | Model `collections.qnt` |
| Hashtag | HTG-01..06 | Không; chỉ có thẻ catalog | Đặc tả văn bản |
| Đề xuất, Top 100 | RANK, TOP | Không; `/` sắp theo thời gian công khai | Model `discovery.qnt` |
| Giao diện và khôi phục mật khẩu | ACC-03, O-11 | Có backend đăng ký, đăng nhập, thêm mật khẩu và đọc lại trạng thái. Web `/toi` dùng mã email. Form mật khẩu Web/CMP và khôi phục còn thiếu | [Hợp đồng App](../../services/vink-app/contracts/http/v1/README.md#sign-in-d-05) |

## Assertion hiện có và phần còn thiếu

Nguồn kiểm trình duyệt là [browser-smoke.mjs](../../apps/vink-web/e2e/browser-smoke.mjs) cùng
[visual-evidence.mjs](../../apps/vink-web/e2e/visual-evidence.mjs) (manifest v2 kèm `sourceSHA`/`dirtyInputs`/PNG SHA-256,
kiểm tra overflow không lộ `scrollX`, AX Chrome riêng với DOM `textContent`, nhánh modal/recovery và phân tách lane
`app-appearance-control` vs `dom-override-geometry-only` vs `NOT_RUN`, kiểm đơn vị bởi
[visual-evidence.test.mjs](../../apps/vink-web/e2e/visual-evidence.test.mjs));
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
| Nháp thay đổi nhưng người đọc vẫn thấy snapshot công khai, không có dòng riêng | `public_read_serves_the_published_snapshot_not_the_draft`, `server_html_has_public_content_and_never_private_drafts` (`postgres`) kiểm draft save và revision chưa được trỏ không đổi public API/list/SSR; `serverHtml` (`web`) kiểm lại sau PUT thật, HTML reader/Khám phá và khung Bàn viết không có chữ riêng | Seed tạo publication; thu hồi theo ý định tác giả chỉ được kiểm ở API/PostgreSQL (PUB-RUNTIME-01), chưa có UI/browser |
| Đọc công khai có điều kiện (READ-01) | `http::conditional::tests` (`quick`) kiểm grammar `If-None-Match` (`*`, `W/`, list/OWS/phần tử rỗng, dấu phẩy trong tag, nhiều dòng header), field sai/quá cỡ bị bỏ qua, so khớp đúng tag và 304 không body; `database_outage_is_reported_not_hidden` kiểm `*` lúc mất PostgreSQL vẫn 503 `no-store`. `conditional_reads::*` (`postgres`) kiểm 200→304, GET/HEAD, nháp riêng không đổi tag, đổi tên tác giả/con trỏ revision/thêm-bớt/thứ tự/hòa thời điểm/danh sách rỗng ra tag mới, tag cũ sau rút bài là 404, rename đồng thời không ghép tag với body khác. `conditionalPublicReads` (`web`) kiểm wire 304 qua CDP và application vẫn nhận 200 đủ byte | Rút bài/đổi tên là SQL fixture vì chưa có lệnh publish/withdraw; 304 vẫn chạy SQL. Chỉ Chrome headless; số đo cục bộ ở [READ-01](public-conditional-reads.md) |
| Đọc công khai cache-first trong tab (READ-02) | `read_cache::tests`, `read_lifecycle::tests`, `freshness::tests`, `contract::tests` và `public_reads::tests` (`quick`/unit web) kiểm ngưỡng 119/120/180 s, đọc không gia hạn, spread/backoff/`Retry-After` có trần giữ deadline qua `mark_stale` vs wake ngay khi `mutation_confirmed`, single-flight + hủy `WaiterGuard`, phân loại từ chối có kiểu (`status + code + field` cho 422 `cursor`/`q`/`tag`/`limit`/`query`, envelope hỏng/lạ) dừng retry, removal fence chặn seed cũ nhưng không thành tombstone vĩnh viễn khi công khai lại, reset scope `clear`, `max_display_age`, GC/trần mềm 16 entry + **ngân sách trọng số `2 MiB / 1 MiB` proxy + ownership `observe`/`seed`/`edit_all` + ticket fence khi hủy waiter + over-budget skip vẫn đọc đủ + race/eviction/restore/waiter-cancel + đối chứng âm bỏ fence**, raw 304 → một lần không điều kiện, chuỗi ngẫu nhiên. `cacheFirstPublicReads` (`web`) kiểm hydration không request, quay lại render ngay không loading/request, tín hiệu lifecycle đồng thời → một request có validator (wire 304), tab ẩn không request, 503 giữ danh sách, backoff qua focus/online, phục hồi `422` theo field (`#restart-public` vs input/fallback), raw `304`, reader báo bản mới thay vì thay chữ, 404/rút bài xóa chữ & card; release Chrome probe đo A/B/C trên `#234` + `#229` ([evidence #237](evidence/web-perf-237/README.md)) | Đồng hồ `performance.now`, `visibilityState` và lỗi/304 Fetch do test/CDP điều khiển; timer 120 s thật kiểm ở unit. Trọng số `snapshot_weight_bytes` là proxy UTF-8 + cấu trúc, không bảo đảm OS RSS hay bộ nhớ GPU; còn khoảng trống model Quint cho `Store`/`Coordinator`. Đã có caller `invalidate_publication`/`invalidate_withdrawn`; `clear_device_cache` là seam chưa nối UI/logout. Gate IndexedDB tắt; không push withdrawal liên tab. CMP/native `NOT_IMPLEMENTED`. [Owner và giới hạn hiện hành](public-read-freshness.md#ngân-sách-byte-ram-trong-read-02-237-web-perf-07--choice-01) |
| Read lifecycle Web [#202](public-read-freshness.md#lifecycle-web-202) | Memory/NoOp conformance, ticket A→B→A/dispose, PageChain validate-before-cursor và head thay tail; Reader `/reader` cache-first + typed no-store controls; lifecycle hints cho Desk list/public+own discussion. Browser `cacheFirstPublicReads` kiểm offline revisit và 390/1440 chữ100/200; main/review/publication kiểm provider thật. | Scope là Web hiện hữu; #201 sở hữu Desk server pagination, #205 sở hữu moderation lifecycle/paging. Riêng #202 không thêm IndexedDB/SW/offline mutation/native; adapter #203 ở hàng riêng bên dưới. |
| Khám phá hữu hạn, trạng thái và reader | `public_list::tests` (`quick`) kiểm title/author/NFC/NFD/Đ/đ/order/chữ gốc, link q encoding và đích nội bộ; test `draft_list` giữ hồi quy helper. `no_publications_is_an_empty_collection_and_not_an_outage` (`postgres`); `database_outage_is_reported_not_hidden` (`quick`) kiểm SSR có q vẫn 503, echo query và canonical. `serverHtml` kiểm SSR tìm title/author/no-results, public HTML/metadata/404/privacy; `nativeExploreSearch` tắt JavaScript trang và chặn bundle/WASM: Enter trong form GET thật ra đúng một `q` (bỏ tham số trùng/cũ), link xóa về `/` không submit, nút submit ra no-results SSR; `publicReading` kiểm hydrated GET, clear/retry, query/hash/focus qua reader, Back/Forward/reload, không fetch thêm list/manuscript khi tìm; loading/rỗng/no-results/503 riêng | Browser reload/back dùng seed thật; nhiều tác giả/title dài/lỗi API là HTTP fixtures có nhãn. Source assertion không là bằng chứng đã chạy; không có contract cho tương tác cộng đồng |
| Reader/Explore responsive và chữ an toàn | `publicReading` kiểm markup là text, khổ/thụt lề/đoạn, không overflow tại 1440/1024/834/390/320px, hai theme, root text 100/200%; Reader hẹp giữ H1 đầy đủ/chọn được với phân cấp tối đa 1,4 lần chữ đọc; fixture title không ngắt từ và document rỗng dưới reduced motion; Tab từ return link qua ba đích dưới, focus và cuối trang không bị bar che; `bottomNavigationGeometry` kiểm nhãn tối đa hai dòng trong target ≥44px và bar ≤18% chiều cao viewport 900px; `textContrast` lấy các cặp public UI; PNG + contexts có geometry. [Bằng chứng UI-REPAIR-01](../design/ui-repair-01.md) | Ảnh chỉ thành bằng chứng thị giác sau khi mở; root text scale không phải browser zoom; headless focus do CDP giữ visible khi Chrome mở tab cập nhật. Không chứng minh screen reader hay thiết bị thật |
| Public IndexedDB [#203](public-read-freshness.md#indexeddb-203) | `public_persistence::tests`, restore owner suite, `public-storage-smoke.mjs` qua `verify web --scenario storage` hoặc `main`: allowlist, transaction abort/quota, multi-tab/freeze, budget, migration, reopen profile | Adapter dưới gate **tắt**; proposal 24h pending, POLICY-02 OPEN. Không persistence/offline rollout, SW, private storage hoặc native. |
| Nhịp đọc mobile và byline hẹp ([#192 LayoutSpec](../design/tag-discovery.md#192--nhịp-đọc-và-byline-ở-chữ-lớn)) | `public-search` / `publicReading` kiểm tên/ngày dùng hết hàng dưới avatar ở 320px chữ 200% và khoảng cách header → văn bản của bài không thẻ ở 320/390px, viewport cao 480px. Cùng fixture đo cả tọa độ Range dòng đầu, stream và thẻ đầu riêng. | Chrome headless trên PostgreSQL tổng hợp. Các assertion dùng cỡ chữ root, ảnh before/after chỉ dùng cho đúng scenario. |
| Dashboard, tìm trong phạm vi và trạng thái | `vietnamese_search_preserves_source_and_provider_order` (`quick`); `draftDashboard` (`web`) dùng fixtures rỗng/1/100 row/title dài NFD (chờ animation hữu hạn xong trước khi chụp mỗi theme), tên trống/lặp/từ liền; kiểm work_id/version/time, scope, tìm Đ/đ, sort, không kết quả, giữ query/sort qua 503/retry, 401/403, không N+1; sửa title/lưu thật rồi trở lại giữ work và version đã xác nhận; `serverHtml` kiểm index không có title/body riêng | Fixtures HTTP chỉ thuộc test, không phải data sản phẩm; không kiểm tìm toàn thư viện hoặc profile/publication |
| Tạo nháp khi phản hồi mất hoặc đến muộn | `uncertain_create_needs_reconciliation_and_a_new_intent`, `explicit_pre_write_rejections_remain_distinct` (`quick`); `draftDashboard` giữ POST, chặn submit lặp, cho PostgreSQL commit thật rồi bỏ ack 201, kiểm GET lỗi không mở khóa, GET thành công tìm đúng một nháp không replay; response đến sau khi rời trang không kéo về editor | GET không xác định chắc chắn chủ nhân của một POST mất ack; chưa có idempotency key hay auth session thật |
| Dashboard narrow/theme/keyboard | `draftDashboard` kiểm mở/thu gọn bằng Enter/Space, full title trong AX tree, thứ tự resume → search → sort bằng Tab, focus-visible và CTA clear bottom bar. Reflow 1440/1024/834/390/320px × sáng/tối × chữ 100/200%; title gọn ba dòng và công cụ trong viewport đầu ở 390/100%. Recent row giữ toàn title/metadata/href, dùng hết inner measure khi hẹp, cùng trục heading/resume/tools/list; Enter/CDP touch mở cùng nháp thật. Phase `desk-layout` chạy riêng dashboard + public reading trên server scratch đã chuẩn bị. PNG + `contexts.json` có width/line count/row height; [evidence #66](../design/ui-repair-66-draft-rows.md); `textContrast` đo các cặp chữ dashboard hai theme; [bằng chứng UI-REPAIR-03](../design/ui-repair-03.md) | Phải mở PNG trước kết luận thị giác; AX tree không phải một lượt dùng screen reader, root text scale không phải zoom hay điện thoại thật |
| Hydration và chuyển từ reader sang Bàn viết | `mainPhase` kiểm dấu hydration, giữ marker JS qua điều hướng, không có console error/exception; node `[role=status]` rỗng trong `.vk-discussion` là `display: none` (không tạo khoảng grid chết) | Một Chrome headless desktop; không bao phủ mọi link hoặc trình duyệt |
| Gõ tiếng Việt, giữ dòng/khổ/thụt lề, lưu rồi reload | `vietnamese_text_lines_and_stanzas_round_trip` (`postgres`); `mainPhase` kiểm CodeMirror mount từ `/pkg/vink-editor.js` (trang đọc không tải), không còn textarea, so chữ trong `.cm-line`, document server và version sau lưu/reload; chờ window cũ được thay và editor mới hydrated trước khi đọc lại | CDP dùng `Input.insertText`, thử `imeSetComposition` có fallback; không phải IME hệ điều hành. Chưa kiểm Word/Docs paste hay thiết bị thật |
| Chỉ báo đã lưu cho đúng nội dung được xác nhận | Unit tests của [desk_state.rs](../../apps/vink-web/src/desk_state.rs) kiểm phản hồi cũ, sửa tiếp khi request đang bay, một save đang bay và lỗi; `mainPhase` kiểm phiên bản ACK thật trong Chi tiết, trạng thái ngắn luôn thấy, Ctrl+S, failed; `editorChrome` giữ PUT, click/shortcut lặp chỉ một request, gõ thêm trước ack vẫn unsaved và GET xác nhận snapshot cũ | Smoke chưa kiểm Enter ở tiêu đề; lỗi mạng là mô phỏng ở tầng Fetch của trình duyệt |
| Retry sau kết quả lưu chưa rõ | `editorChrome` làm hỏng request, retry sau GET cùng version; giữ response sau commit thật rồi bỏ ack, đối chiếu 403 không PUT, retry tiếp vẫn GET và conflict không phát PUT; explicit keep-local rồi reload. Giữ GET đối chiếu rồi xác nhận bỏ: GET muộn không khởi động PUT, không điều hướng hay đổi version server | Không có idempotency key; GET không xác nhận saved thay cho ack cũ. CAS vẫn là ranh giới chống ghi đè nếu có writer chen giữa GET/PUT |
| Chrome editor, reflow và dialog | `editorChrome` (cũng có phase độc lập `editor-chrome`): 1440/1024/834/390/320 × light/dark × root text 100/200%, clean/dirty/saving/failed/conflict và preview/leave; unified `#save-slot` ([#217](../design/ui-editor-unified-save-217.md)): clean không có `#save` (không `PUT` thừa khi clean/Enter/Ctrl+S), target ≥48px, mặt viết R5 chung title/body không resting stroke/halo/divider, một live status owner không lặp khi gõ; recovery/backup visible, native detail, tooltip/focus/forced-colors và composition guard; không overflow, focus handoff `dirty→saving→saved/failed/conflict` không rơi về `<body>`, `Ctrl/Cmd+S` giữ EditorView, Tab slot/actions/title/body (body không viền, một focus outline 2px offset −2px) và nút an toàn trong dialog, đo tương phản light/dark sau transition hữu hạn, cửa sổ thấp và title focus không bị sticky che. PNG/manifest ghi geometry; [evidence #101](../design/ui-editor-r5.md) | Ảnh phải mở riêng; CSS text 200% không phải browser zoom. Viewport thấp không là bàn phím mềm thật; không xác nhận OS IME, Safari/Firefox hay screen reader |
| Một EditorView giữ trạng thái | `shellWithDirtyEditor` đánh dấu `.cm-editor`, thu/mở rail, đổi theme qua radio, mở/đóng Tôi, resize mobile; chờ hai frame sau resize, so chữ/selection/dirty và không có PUT/POST, Tab trở lại và native blur/focus qua `EditorView.focus()`; `mainPhase` đổi `data-theme`, mở/đóng xem trước và hộp thoại rời trang, rồi kiểm cùng view, selection (`getSelection`), vị trí cuộn trang sau khi đóng xem trước, Ctrl+Z/Ctrl+Shift+Z | Selection/undo/cuộn đo qua DOM của một Chrome desktop; chưa kiểm cuộn khi bàn phím ảo mở |
| Xem trước và export dùng chữ hiện tại | `mainPhase` so khối trong `draft-preview` với khổ chưa lưu và kiểm version server không đổi. `editorChrome` và `mainPhase` tải tệp phục hồi từ hộp thoại trên hai lượt mở trang; `afterRestartPhase` kiểm tệp tải từ toolbar. Runner dùng `Browser.setDownloadBehavior` rồi đọc tệp `.txt` | Chưa kiểm mở tệp trên Windows/macOS |
| Paper First #218 | `editorPaperFirst` trong `editorChrome`: clean/dirty × 320/390/834/1440 × Sáng/Tối × cài đặt app text100/200; native tools Enter/Space/Tab/touch, negative hidden summary, live export/preview và focus return. `editor_tools` kiểm SSR opt-in/default anatomy; save/recovery/races giữ trong main | App text preference thật tách direct DOM matrix hiện hữu; Chrome CDP không là OS IME/mobile device/screen reader. [Spec](../design/ui-editor-paper-first-218.md) |
| Khả năng tới nút phục hồi và rời trang (#194) | `editorChrome` đo rect của cảnh báo và từng nút, kiểm clipping/hit-test, cuối cuộn của dialog, Tab/Shift+Tab và kết quả chọn. Ảnh có kích thước PNG, hash, viewport và số đo theo từng cảnh. `main` kiểm thêm dialog khi thẻ xung đột | Chữ 100/200% mô phỏng bằng CSS, phím/cuộn qua Chrome CDP. Ca mất phản hồi và 503 có fault injection; ACK và version đọc từ PostgreSQL tạm |
| Version cũ không ghi, giữ nội dung để người viết chọn | `stale_expected_version_conflicts_without_writing`, `concurrent_saves_of_one_version_have_exactly_one_winner` (`postgres`); `mainPhase` kiểm conflict và nút giữ bản local | Smoke mô phỏng bên ghi cạnh tranh bằng PUT trong cùng trang, chưa thao tác hai tab thật; test PostgreSQL chạy 8 save đồng thời |
| Quyền đọc/ghi riêng và lỗi API không bị gộp thành thành công | `another_actor_cannot_read_list_or_write_a_private_draft`, `api_distinguishes_outcomes_and_keeps_drafts_private` (`postgres`); `private_routes_are_closed_without_an_auth_authority`, `database_outage_is_reported_not_hidden` trong [http/mod.rs](../../services/vink-app/src/http/mod.rs) (`quick`) | Resolver header chỉ nằm trong test; fixture loopback không chứng minh session, logout/ban hoặc ACC-01/02 |
| Role tách và database cũ còn nguyên sau provisioning | [tests/postgres.rs](../../packages/store-postgres/tests/postgres.rs) (lifecycle, `postgres`); [provision.rs](../../xtask/src/provision.rs) (`postgres`): snapshot dữ liệu + lịch sử trước/sau, runtime đọc lại tiêu đề NFD, thử DDL/TEMP/ghi `_sqlx_migrations`/`CREATE EXTENSION`/kết nối chéo bằng chính role, ca âm cấp `CREATE` thừa | Server tạm do test harness tạo; Compose thật và DB do binary base ghi chỉ có bằng chứng thủ công trong [verification](identity-postgres-verification.md); lane `web` cấp role runtime scratch riêng; provisioning đầy đủ vẫn thuộc lane PostgreSQL |
| Identity đóng khi không chắc chắn, không lộ credential | `http::tests`, `email_code::tests`, `mail::tests` của Identity (`quick`: caller theo peer, header/subject giả, payload, version, deadline, fixtures, redaction; chuẩn hóa NFC, mã, handle, credential, bảng quyết định thử mã vét cạn; thao tác email-code bị đóng khi không có authority, kiểm request trước storage); [identity tests/postgres.rs](../../services/vink-identity/tests/postgres.rs) và [email_code.rs](../../services/vink-identity/tests/postgres/email_code.rs) (`postgres`: mã một lần/gắn handle/giới hạn thử, 8 lần trả lời đồng thời một phiên, 4 lần bị ép chờ cùng khóa hàng một phiên, 6 lần đăng nhập đầu đồng thời một tài khoản, throttle đồng thời, logout, hết hạn, disable, đổi email giữ subject và địa chỉ được giải phóng không thừa kế); [xtask identity.rs](../../xtask/src/identity.rs) (`postgres`: binary thật, socket thật, role đã provision) | Các ca store dùng outbox cục bộ. `email_code_end_to_end` kiểm thêm SMTP loopback. App sở hữu rate limit theo mạng |
| App không cấp quyền khi Identity lỗi | `identity_tests` trong [http/mod.rs](../../services/vink-app/src/http/mod.rs) (`quick`: Identity giả trên socket thật — rejected/authenticated chưa bind/suspended/no-authority `401`, 503/caller/malformed/outcome lạ/subject sai/directory lỗi/chậm `503`, cookie và Bearer như nhau, sai uid server và mất socket `503`, header giả không đổi gì; đăng nhập: Origin, cookie `__Host-`, không lộ secret trong body, revoke phiên khi bị đình chỉ/directory lỗi; logout); [principal.rs](../../services/vink-app/tests/postgres/principal.rs) (`postgres`: 12 lần provisioning đồng thời một principal không mồ côi, subject khác không chung principal, binding bất biến/không gắn subject thứ hai, đổi bút danh giữ principal và nháp, đình chỉ/gỡ); `app_integration`, `app_refusals`, `email_code_end_to_end` trong [xtask](../../xtask/src/identity/email_code.rs) (`postgres`: hai binary thật) | Các ca tích hợp dùng HTTP tới App và Unix socket giữa hai binary. Hành trình trình duyệt nằm ở [Auth Web](auth-web.md) |
| Đăng ký, đăng nhập và thêm mật khẩu (ACC-03) | [password.rs](../../services/vink-identity/tests/postgres/password.rs) kiểm cả hash cũ trước NFKC. [enrollment.rs](../../services/vink-identity/tests/postgres/enrollment.rs) kiểm mã theo mục đích/tài khoản/phiên, khóa phiên hết hạn trước khi phát mã, logout/disable, trùng username, rollback schema và giao dịch. [password_enrollment_end_to_end](../../xtask/src/identity/email_code/password_enrollment.rs) kiểm Origin, cookie/no-store, rate limit, trường username rỗng, dọn cookie mã quá dài, đối soát GET và đọc lại nháp/bình luận bằng cùng principal khi đổi phương thức | `postgres` chạy hai binary thật và outbox cục bộ. `web`: [auth-password-smoke.mjs](../../apps/vink-web/e2e/auth-password-smoke.mjs) kiểm luồng UI, đọc lại nháp cùng principal, mất ACK, tab khác, autofill DOM và responsive; giữ phase mã email riêng. App kiểm đình chỉ trước RPC, Identity giữ khóa tài khoản và phiên trong giao dịch |
| Backup/restore theo owner | `backup_restore` trong [provision.rs](../../xtask/src/provision.rs) (`postgres`): `pg_dump`/`pg_restore` App và Identity vào database khác, so dữ liệu/ID/Unicode/con trỏ/lịch sử/owner | Cùng cluster; không kiểm restore sang host khác hay khôi phục tại một thời điểm chung cho hai owner |
| Dữ liệu đã xác nhận còn sau restart, process dừng đúng | `confirmed_save_survives_a_new_connection_pool` (`postgres`); `afterRestartPhase` và orchestration trong [web.rs](../../xtask/src/web.rs) (`web`) | Không phải crash/replay/backup-restore; worker chỉ có lifecycle, không có job |
| Font tự host được phục vụ và tải | `mainPhase` kiểm trang đọc có Be Vietnam Pro 400 và Literata ở trạng thái `loaded`, file font trả `font/woff2` (`web`); repository checks so hash từng face/license với [provenance](../../apps/vink-web/assets/fonts/provenance.json) và kiểm URL font trong CSS/shell (`quick`) | Không chứng minh vị trí dấu, fallback trên máy thiếu font hay hiển thị trên thiết bị thật; kiểm theme trong `adaptiveShell` là bằng chứng riêng của shell |
| Shell, preference và focus | `adaptiveShell` kiểm rail 220/96, tên/`aria-expanded` của toggle và nút ≥44px không bị che khi thu, mặc định/ghi đè/reload, thứ tự/active route, link thật không phải tab, skip link focus main, modifier click để trình duyệt xử lý, 404 cho route lạ không sáng đích, ba lần bấm dồn dập chỉ còn đích cuối, tooltip focus/hover/Escape ngoài rail, Tôi overlay focus đầu/Tab/Escape/outside/Đóng/resize, Back/Forward khi Tôi mở chỉ đóng Tôi (kể cả fragment-only), Back/Forward không overlay vẫn đổi fragment tự nhiên, traversal bị giữ phục hồi scroll khác0 sau native history, rồi Back sau mới rời, ba đích mobile, theme system/override/reload/storage denied, không overflow và bottom clearance ở 320px/chữ 200%; unit `shell::tests` (đích theo segment, ngữ cảnh editor, thứ tự/href), `public_list::tests` (return context round-trip qua parser của router, link lạ chỉ dựng URL nội bộ); `vietnamese_initials_use_first_and_last_words` kiểm tên tiếng Việt/NFC/blank | Một Chrome headless Linux, resize CSS và cỡ chữ root; không phải thiết bị mobile/zoom trình duyệt/screen reader; initials fixture không là profile |
| Bottom bar: icon và dòng đầu cùng trục (#88) | `bottomNavigationAlignment` đo icon-centreY và dòng đầu bằng DOM Range sau `fonts.ready`, so lệch ≤1 CSS px, target đều và cùng resolved font/line-height; được gọi trong ma trận `adaptiveShell`. Phase `bottom-navigation` kiểm riêng 320/390 × chữ100/200 × light/dark × ba route, focus/Tab, clearance sau resize, rail 834px và editor ẩn bar. [Before/after](../design/bottom-nav-88.md) | Một Chrome headless/macOS, root text scale; chưa thiết bị thật, screen reader, safe-area khác0 hoặc native/CMP |
| Tôi/panel: bề rộng chữ và trục nội dung (#67 AC-11–13) | `accountLayoutGeometry` dùng bounds từ capture helper: heading/account/choices cùng content axes; account text dùng trọn cột bên trong nhóm nền hiện hành; gutter16 theo layout viewport; account320/390 chữ200 xếp dọc và dùng toàn measure; panel inset đối xứng trong client box và CTA cùng trục. `adaptiveShell` thêm ảnh cuối trang và helper/footer tới được trên navigation. [Recapture và số đo](../design/ui-repair-67-me-panel-geometry.md) | Recapture R2 giữ như lịch sử; R5 bổ sung nền account và helper thu gọn, đo theo content box mới, không bù scrollbar bằng margin. Chrome headless/CDP, không suy native hay screen reader |
| Tôi/panel R4–R5: surface và disclosure (#67 AC-14–21) | `preferenceSurfaces` kiểm account/copy thật, selected fill + check khác idle, không resting frame/rim/divider; native details dùng chung có Enter/Space/Tab/CDP touch và no-JS. Panel834×360/text200 mở help vẫn một scroller, CTA cuối tới được. `appearanceFocusContrast` đo vòng focus selected ≥3:1 cả sáng/tối/forced-colors sau transition. Reader Tab qua một build disclosure trước nav; footer bỏ rule. [Layout, captures và kiểm local](../design/me-preferences-r5.md) | Native details mở trong luồng; Escape ở panel đóng modal cha và trả trigger. Root text/CDP không chứng minh zoom thật, OS IME, screen reader hoặc CMP/native |
| Giao diện R1 (#209) | `accountAppearance`, `preferenceSurfaces` và ma trận `adaptiveShell` trong [browser smoke](../../apps/vink-web/e2e/browser-smoke.mjs) kiểm hai consumer, khoảng cách nhóm, lựa chọn tonal, vị trí nhãn, focus, reflow và mở lại panel ở đầu vùng cuộn. [LayoutSpec R1](../design/me-preferences-r1-209.md) | Chữ 100%/200% qua tùy chọn của Web. Tương tác Chrome headless, ảnh và số đo từ app cục bộ |
| Tôi: tài khoản, radio và overlay có thể cuộn | `accountAppearance` (`web`) kiểm bút danh phiên LocalFixture, cùng initials trên thẻ/rail/bottom bar, API `no-store`, ba nhóm radio đồng bộ và ghi nhớ, bàn phím, forced colors, panel cuộn và trả focus. `adaptiveShell` kiểm responsive, storage bị từ chối, stale close/reopen và reflow ở chữ 200%. `appearanceFirstPaint` đối chiếu màu và radio khi chặn hydration, rồi thử đổi giá trị rail trong storage giữa bootstrap và hydration. `shellWithDirtyEditor` kiểm thay tùy chọn và dùng overlay không làm mất chữ, selection hay EditorView. | Chrome headless với input chương trình và PostgreSQL cục bộ. Chưa kiểm thiết bị thật, screen reader hoặc đăng nhập production. |
| Rời editor khi chưa lưu | Unit tests `LeaveGuard` (hủy khi đang lưu, ack muộn, đổi đích, chữ mới hơn, lỗi/409, xác nhận bỏ, bỏ khi save treo); `mainPhase` kiểm chữ đã xác nhận rời không hỏi, link Khám phá của rail và Back bị giữ, Escape/“Ở lại” giữ URL/chữ/focus, lưu rồi rời sau ack, hủy chuyển sang Tôi khi PUT bị giữ rồi thả không điều hướng muộn, lỗi → tải bản đang viết → rời mà không lưu → ack muộn không điều hướng lần nữa và không đánh dấu nháp khác đã lưu | `beforeunload` (reload/đóng tab) chưa có assertion; Chrome desktop headless, back qua `history.back()`; chưa kiểm cử chỉ vuốt back trên mobile |

## Công cụ và bằng chứng ngoài smoke

Ngân sách document lớn [#229](long-document-budget.md): corpus tổng hợp seeded qua validator,
serializer, `Vink<PgStore>`, router SSR và phase CDP `document-budget`. Regression thuần ở
`services/vink-app/tests/document_budget.rs`; biên HTTP và benchmark opt-in ở
`tests/postgres/document_budget.rs`; browser ở `apps/vink-web/e2e/document-budget.mjs`.
[#273](draft-http-budget.md) thêm loopback HTTP/1 + PostgreSQL thật trong
`tests/postgres/draft_http_budget.rs`: cap riêng draft PUT, client corpus, exact readback/hash,
cap ±1 (Content-Length/chunked), malformed, quyền/CAS và non-save route giữ cap cũ.
Phase browser giờ yêu cầu tiny_blocks200k lưu thành công; test opt-in `draft_http_budget_browser`
chạy ba corpus chọn lọc cùng phase, kiểm source/NFD và title trim trên request thật, toàn văn sau 422/409. Cận DTO canonical là 4.199.232 byte.
Đo scalar/UTF-8/JSON/TOAST/HTML/DOM riêng, history 1/10/100 và pglz/LZ4 trong database scratch.
Measurement v2 restore baseline trước từng app-save sample; regression PostgreSQL không ignored
kiểm full content/version sau commit và hai negative controls bắt nhầm changed/no-op. Raw v1
vẫn được giữ, với giới hạn workload ghi ở [đối soát follow-up](long-document-budget.md#đối-soát-follow-up-229).
Đây là coverage phép đo, không thêm chương, autosave hay persistence rollout; kết quả thực thi,
ngưỡng đề xuất và giới hạn host nằm ở báo cáo #229.

Web runner chuẩn bị database thử, server và Chrome; khi chạy tới bước tương ứng, nó ghi
`target/verify-web/<run-id>/server.log` và `smoke-state.json`. Mỗi lượt có thư mục riêng; không xóa bằng chứng lượt trước. `adaptiveShell`, `publicReading`, `editorChrome`
và `shellWithDirtyEditor` ghi PNG tại `target/verify-web/<run-id>/screenshots/` (1440/1024/834/390/320px,
sáng/tối, 320px chữ 200%, editor dirty và overlay). Manifest `contexts.json` ghi route, viewport, DPR, theme, root font và user agent. Không có video hay performance trace.
Assertions DOM và ảnh chưa mở không chứng minh bố cục đẹp.

Với yêu cầu visual QA, dùng công cụ browser có sẵn để chụp **và mở** ảnh, ghi viewport/theme/
text scale; xem [vink-ui-inspector](../../.agents/skills/vink-ui-inspector/SKILL.md).
IME thật, accessibility với screen reader, mobile thật và hiệu năng cần phiên kiểm riêng có
môi trường và quan sát cụ thể. Không tự đổi chúng thành `PASS` vì lane `web` thành công.

### UI System preview (UIS-01, UIS-02, UIS-03, UIS-04)

UI-REPAIR-06/07 bổ sung `StatePanel` action slot ở hai renderer. `feedback` SSR và
`VinkPanelTest` kiểm typed recovery cùng callback/link của caller; `publicReading`
và `draftDashboard` kiểm action thuộc đúng state owner, clear trả focus về search,
GET retry giữ query/hash và reload document thật, 401/403/503 cùng uncertain-create
vẫn riêng. `captureRecovery` ghi 220 ảnh theo 11 state × 5 width × hai theme ×
text100/200; input chương trình và cỡ chữ CSS không là OS IME/browser zoom.

Desk #259 thay hai empty panels bằng composition WebP chung #258, giữ panel ở error/uncertain.
`deskEmptyState` trong `e2e/desk-empty-smoke.mjs`, được gọi bởi `main` và phase `desk-empty`
của runner hiện hữu, kiểm loading/first-use/search miss/clear, node/caret/composition, request
ảnh theo state, HTTP MIME/hash/decode, blocked/corrupt, stale/error và empty continuation
rejection. Ma trận 320/390/834/1440 × light/dark × root text100/200 kiểm art sizing, CTA/privacy
clearance; [evidence và giới hạn](evidence/empty-webp-259/README.md) tách fixtures với real
create/paging trong `draftDashboard`/`draftPagination`. Không đổi Editor hoặc read/create model.
Trong `main`, fixture empty/session chạy trong browser context riêng để BroadcastChannel không
ảnh hưởng ca kế tiếp. Network evidence chỉ tính current document, cho phép decoded-cache reuse
và kiểm đổi theme/viewport/text scale không tải thêm artwork.

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
| Dialog/sheet chữ lớn ([#174 vòng sửa 1](../design/ui-ux-quality-174.md#10-vòng-sửa-1-f-01-và-các-gap-bằng-chứng)) | Web `#state.dialog/default\|sheet`, CMP `composition.interaction/default\|sheet` | `overlayCaptures` trong smoke Leptos: 320×700/390×844 × sáng/tối × chữ 100/200, tiêu đề lúc mở/mở lại, không tràn, Tab tới từng nút, Escape/outside/return focus; `InteractionCaptureTest`: pointer exit, ảnh initial, tile đầu–cuối không gap, bounds/scroll/focus không đổi khi chụp, close/return focus. Clean PNG và overlay cùng lượt | Web root font-size; CMP Desktop off-screen, field tìm bằng accessible label; không glyph/OS IME/screen reader/native qualification. Ảnh phải mở trước kết luận visual |
| Font CMP Desktop ([PR 08](../design/ui-repair-08.md)) | Be Vietnam Pro 400/600 và Literata TTF từ cùng WOFF2 Web, classpath `vink-fonts/`; fallback hệ thống có lý do khi thiếu/hỏng | `PreviewTest.selfHostedFacesDecodeOnDesktop` vẫn yêu cầu 3 face; `FontTest` kiểm metadata/trục/glyph tiếng Việt, đo font thật qua Compose 400/600 và missing/corrupt từng face; `FontCaptureTest` chụp cùng specimen với WOFF2 trước và TTF sau; `checks::desktop_font_derivative_checks_detect_corruption_and_unowned_files` kiểm hash | CI `ui-cmp` Linux và `ui-cmp-macos` chạy cùng lane; glyph/đo chữ không thay shaping/IME hay native acceptance; ảnh phải mở mới là visual evidence |
| Web dùng token sinh ra | `/pkg/vink.css` = `tokens.css` + `vink-ui.css` + `vink.css` | `verify web` hiện có (contrast hai theme, theme/reload/OS/storage denied, reflow) chạy trên CSS mới | Không thêm assertion web mới cho UIS-01 |
| Action surfaces ([#224](https://github.com/loveoverflowcom/vink/issues/224)) | Shared icon/toggle opt-in `ActionSurface::Tonal`; preview `action.icon-button/surfaces`, `action.toggle-action/surfaces`, `tonal-lifecycle`, `tonal-long-label`; wrapper TooltipIconButton/EndorsementAction chuyển tiếp. Default consumers giữ Standard | Rust action/support + patterns wrapper tests; Chrome `actionSurfaces` và `toggles` chạy cả Standard/Tonal; CMP `ActionSurfacesTest` cho semantics, pointer/keyboard, focus/touch bounds và light/dark/text200 captures | Component preview, chưa migrate app pages (#225/#226); Android/iOS và screen reader/OS IME cần evidence riêng; [báo cáo #224](evidence/action-surfaces-224/README.md) |
| Component nền UIS-02 ([#20](https://github.com/loveoverflowcom/vink/issues/20)) | Preview Leptos `#<họ>.<component>/<kịch bản>` (ví dụ `#action.button/pending`, `#field.form/invalid`, `#identity.avatar/image-error`); preview CMP chọn cùng ID | Test `vink-ui` (`action`, `field`, `media`, `identity` golden vectors, `layout`, `text`), `vink-ui-preview` (mọi kịch bản render, không ID trùng); `components` trong [preview-smoke.mjs](../../packages/ui-system/leptos/preview/e2e/preview-smoke.mjs); CMP `VinkIdentityTest`, `PreviewTest` (dispatch, selection, semantics), `CaptureTest` | Chưa có màn app dùng; IME thật, screen reader, Android/iOS runtime và hover CMP chưa chạy; contrast là màu resolve trên Chrome/Skia |
| Ảnh nội dung ([#115](https://github.com/loveoverflowcom/vink/issues/115), [#116](https://github.com/loveoverflowcom/vink/issues/116)) | Preview `identity.image-frame/default\|image-error` và `identity.avatar/image-error`. Web có data URL và `/fixtures/sample.webp`; CMP có bytes đóng gói cho ảnh mẫu và ảnh chi tiết | [Fixture/profile chung](../../packages/ui-system/shared/content-images.md); Web smoke kiểm HTTP/decode và hydration. CMP `ContentImagesTest` kiểm container, kích thước, codec và hủy. `PreviewTest`, Android `DiagnosticTest` và iOS `DiagnosticTests` kiểm render/fallback | [Bằng chứng Android/iOS/desktop](ui-system-qualification-evidence.md#lossy-webp-trong-cmp-116). Ảnh bìa production `NOT_IMPLEMENTED` |
| Trạng thái/phản hồi UIS-03 ([#21](https://github.com/loveoverflowcom/vink/issues/21)) | Preview `#state.<họ>/<kịch bản>` (ví dụ `#state.error/partial`, `#state.dialog/session-expired`); sân thử `#state.playground/<recipe>` (đồng hồ thủ công, bước, nhật ký) cho 8 recipe | Generator `ui_states` + vectors chung (`state::tests` Rust, `VinkStateTest` JVM/iOS simulator); test `feedback` (render vùng/panel/notice/dialog), `vink-ui-preview` (`states::tests`: reducer có ticket bỏ double-click và ack muộn); `states` trong [preview-smoke.mjs](../../packages/ui-system/leptos/preview/e2e/preview-smoke.mjs) (vùng, intent, announce, snackbar theo đồng hồ, 8 recipe, focus dialog, motion giảm/không hoạt động/tab ẩn qua `visibilitychange`, frame có timestamp); CMP `StatePreviewTest`, `DrivenCaptureTest` | Chưa màn app nào dùng; lifecycle Android/iOS chưa có host; TalkBack/VoiceOver, giảm chuyển động của OS thật chưa chạy |
| Shell/điều hướng UIS-04 ([#22](https://github.com/loveoverflowcom/vink/issues/22)) | Preview `#navigation.<họ>/<kịch bản>` (ví dụ `#navigation.scaffold/toggle`, `#navigation.overlay/busy`); CMP thêm `#navigation.adapter/*` (back stack, deep link, chữ chưa lưu) | Generator `ui_navigation` + vectors chung (`navigation::tests` Rust, `VinkNavigationTest` JVM/iOS simulator khi bật native); `navigation` trong [preview-smoke.mjs](../../packages/ui-system/leptos/preview/e2e/preview-smoke.mjs) (thứ tự, link thật, toggle 96→220→96 bằng chuột/Enter/Space giữ trang/chữ/selection, tooltip, một modal, Back đóng lớp phủ trước, Escape trả focus); CMP `NavigationPreviewTest` (deep link round-trip/từ chối link ngoài, Back đóng lớp phủ trước, chữ chưa lưu giữ Back/điều hướng, đổi khung không remount, toggle giữ selection, thứ tự thanh dưới, một modal); app thật trong `adaptiveShell` (`web`) | Android predictive back, iOS swipe-back, lifecycle, TalkBack/VoiceOver chưa chạy; adapter CMP là host chẩn đoán, không phải app native |
| ToggleAction + EndorsementAction (LVE [#73](https://github.com/loveoverflowcom/vink/issues/73), UI System) | Preview `#action.toggle-action/<kịch bản>` (gồm `lifecycle`: gửi, mất phản hồi, Kiểm tra lại, từ chối) và `#composition.endorsement/<kịch bản>`; CMP chọn cùng ID | Generator `ui_toggle` + vectors `toggle-contract-v1` (`toggle::tests` Rust, `VinkToggleTest` JVM; `ui_toggle::tests` từ chối hàng phát lệnh mù/unavailable thành số); `vink-patterns` (`endorsement_copy…`, `EndorsementTest`); `vink-ui-preview` (`endorsement::tests`); `toggles` trong [preview-smoke.mjs](../../packages/ui-system/leptos/preview/e2e/preview-smoke.mjs) (click đúp một lần, không optimistic, mất phản hồi chặn bấm, Kiểm tra lại, Enter/Space, focus 2px, số không khả dụng, lý do chặn, bốn đích độc lập, khách); CMP `PreviewTest`/`CaptureTest` qua catalog ([evidence](ui-system-endorsement-evidence.md)) | Reader Web dùng qua [adapter LVE](#lve-web-adapter-73-wp-04); [Pen Flight R2 A](pen-flight-r2-evidence.md) ở work/root/reply; TalkBack/VoiceOver/screen reader thật, Android/iOS runtime chưa chạy |
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
| Model → trace → core | [Behavior owner](../../services/vink-app/behavior/discussion/README.md): Quint typecheck/scenarios/simulation/mutation/drift; `quint_generated_inputs_replay_against_the_production_creation_seam`, `quint_lifecycle_inputs_replay_against_the_production_decisions` (gồm receipt/retry), `quint_reaction_inputs_replay_against_the_production_decisions` (cùng hai counterexample no-op của bật/tắt và reaction phải bị production từ chối) và `quint_endorsement_inputs_replay_against_production` ([LVE](literary-endorsement-core.md); route ở mục LVE API bên dưới) và `quint_admission_inputs_replay_against_the_production_decisions` (#120, cùng ba counterexample admission phải bị production từ chối) và `quint_outbox_inputs_replay_against_the_production_decisions` (DISC-04, cùng counterexample hoàn tất lease cũ), cùng `quint_operator_review_inputs_replay_through_production_guards` (hậu kiểm operator) chỉ feed command input rồi so outcome/state; lane đòi đủ các replay có tên của owner; danh sách hiện hành ở `xtask/src/discussion.rs` | Một thread, ID nhỏ, bounds/seed hữu hạn. Chưa model checking; không proof PostgreSQL hay UI |
| Domain vòng đời (`domain/discussion/*`) | Unit tests từng module (`quick`): body NFC/emoji/bidi, mode generation, lifecycle 4 bước, visibility mọi trạng thái × người xem, quyền tác giả, bảng `decide_submission`, cursor, reaction desired-state/giới hạn/tally; doctest ranh giới type có lawful twin; [Kani](../../services/vink-app/src/domain/discussion/proofs.rs) (`kani`): kiểm duyệt/gỡ/chế độ với mọi version `i64`, projection không lộ bình luận ẩn, `PageLimit`, `reply_placement`, receipt trước mọi gate, reaction và tally, miền cursor, fence hai yêu cầu của chế độ và reaction (yêu cầu đã xác nhận chặn yêu cầu cũ hơn), admission có version/shadow/hậu kiểm (#120, [quyết định](../architecture/discussion-admission.md)) | Thuần; Kani kiểm một quyết định hoặc một cặp yêu cầu, không chuỗi lệnh dài; không chứng minh transaction hay transport; reaction chưa có store/API/UI |
| Use case `Vink::{submit_comment, remove_own_comment, set_comments_mode, moderate_comment, discussion, replies}` | [discussion_tests.rs](../../services/vink-app/src/application/discussion_tests.rs) với store ghi nhận (`quick`) | Store ghi nhận không chứng minh SQL/khóa; HTTP và grant kiểm duyệt ở các hàng dưới |
| Kiểm duyệt `GET /api/v1/moderation/comments`, `POST …/{id}/decisions`; lệnh `vink-app moderator grant\|revoke\|list`, `vink-app admin` và `/api/v1/moderation/{grants,groups}` | [Kiểm chứng governance](moderation-runtime.md#9-bằng-chứng-và-giới-hạn) (`postgres`), gồm tách quyền `works`/`comments`, cùng phản hồi cho ID ngoài phạm vi và ID vắng mặt, hàng chờ/count/cursor đúng scope, các race về quyền và provenance/rollback. Fixture HTTP provider, consumer Web và parse đối số CLI (`quick`). | Chưa có UI kiểm duyệt bình luận, UI grant/nhóm, comment cases/appeal/hide-restore, cảnh báo backlog. Production trả 401 khi sign-in tắt. |
| `PgStore` discussion + migration | [tests/postgres/discussion.rs](../../services/vink-app/tests/postgres/discussion.rs) (`postgres`): SQL trực tiếp, khóa tường minh, 8 retry đồng thời, audit, neo/context, keyset ties, SQL ↔ domain; provenance admission giữ bằng schema, đình chỉ tại commit (hai thứ tự race) và nâng cấp dữ liệu cũ (#120) | Superuser harness; chưa có lệnh rút bài thật. Số đo EXPLAIN/pgbench của đường đọc ở hàng DISC-02/03 bên dưới |
| Public projection + overlay riêng tư (#120 DISC-02/03, [quyết định](../architecture/discussion-reads.md)): `GET …/discussion/public` (+ replies), `GET …/discussion/me`, migration `20261113000000_own_comment_overlay` | Domain `own_overlay`, cursor chính tắc/có scope/≤56 byte (`quick`, Kani `the_private_overlay_reveals_only_the_writers_own_comments`); `Vink::{public_discussion, public_replies, own_comments}` + negative control kiểu public (`quick`); fixtures `discussion.public.*`, `discussion.me.*` (`quick`); [tests/postgres/discussion_reads.rs](../../services/vink-app/tests/postgres/discussion_reads.rs) (`postgres`): own ngoài trang/dưới root ẩn, A/B/tác giả/moderator, pending→approved một lần, đối soát theo ID, oracle SQL↔domain, public = guest + oracle độc lập, duyệt/gỡ giữa trang, HTTP bounds/no-store/404/401/503, SSR guest, Identity outage, đình chỉ vs gỡ/bật-tắt (hai thứ tự), ghi đồng thời không deadlock, rollback index; [số đo](evidence/disc-02-03-reads/README.md) | Chưa có consumer Web/CMP của hai đường đọc mới (#129); late response/actor switch phía client chưa có; parser chuỗi cursor không có Kani/fuzz; phiên Identity bị thu hồi có cửa sổ một request; số đo trên một máy dev |
| `GET/POST/DELETE/PUT /api/v1/works/{id}/discussion…` ([contract](../../services/vink-app/contracts/http/v1/README.md#discussion-d2t-02b)) | Fixture tests `http::contract::tests::discussion` (provider) và `decodes_discussion_fixtures`/`classifies_discussion_error_fixtures` (`vink-web`) trong `quick`; `the_http_api_keeps_every_discussion_outcome_distinct` qua router trên PostgreSQL (`postgres`) | Resolver test lấy principal từ header; production chưa có auth nên ghi trả 401; UI/browser ở mục Consumer D2T-03; chưa rate limit |

### Consumer D2T-03 (#45)

[LayoutSpec và state matrix](../design/discussion-ui.md) ghi vùng, semantics và dependency gaps.
Các hàng dưới là assertion đã viết, không tự xác nhận kết quả chạy.

| Điểm vào | Assertion có trong code/runner | Giới hạn |
| --- | --- | --- |
| Reader `#feedback-region` | `api::discussion_resource`, `PublicReadsAdapter::discussion` gọi `public_discussion` (không actor); `discussionReadingAndWrites` kiểm SSR không chứa own-pending/rejected sau commit thật | Anonymous SSR; private projection chỉ qua no-store API sau hydration. Không auth production |
| `DiscussionController` | Unit tests D2T-03: double send, new typing/IME, scope đổi, replay key/payload và physical attempt riêng, late ACK/error, Closed, context revoked | Rust consumer production seam; không phải Quint/Kani, OS IME hoặc chứng minh DB |
| Root/reply paging và context | Adapter giữ cursor opaque, hai tầng, neutral anchor/unavailable, ID-keyed rows; unit tests projection/dedupe/cursor và mutation/read epoch fencing | Không endpoint context ngoài trang, ranking/count tổng/reaction/report |
| Gửi/gỡ/author toggle cục bộ | `discussionReadingAndWrites` trong browser smoke: POST commit rồi mất ACK, exact keyed replay, pending receipt, giữ typing, PUT generation, DELETE own | LocalFixture + scratch PostgreSQL; không production auth/moderation E2E. Runtime result phải đọc log |
| Web/CMP patterns | `sharing-open`, `sharing-closed`, `discussion-flat`, `discussion-anchor` cùng catalog IDs; Web render tests và CMP Desktop tests giữ text/selection qua policy | CMP là composition mẫu; chưa native service consumer, OS IME/screen reader/performance |
| Responsive/target revocation | Browser smoke captures light/dark 320/390/834/1024/1440 và 200% ở 320/390; context fixture revoke giữ peers/draft | Capture labels tách real local PostgreSQL khỏi intercepted provider fixture; ảnh cần mở riêng |

## DISC-WEB-PUBLIC-OWN-READS (10/11/2026)

Nền `4891c77`; [DecisionMap/LayoutSpec/scope](discussion-web-reads.md). Coverage dưới đây là
assertion, không tự là kết quả PASS.

| Điểm vào | Owner / assertion | Giới hạn |
| --- | --- | --- |
| Public SSR và GET roots/replies | DTO public strict, `separated_discussion_tests`, paths encoded; SSR resource chỉ serialize public | Không cache/validator mới |
| Nhận xét của bạn | `discussion_reads::OwnState`: own cursor, dedup ID, lookup ≤20/batch, lookup thắng list; unit và `discussionOwnFixtures` | Status/rejected/unlisted trong browser dùng provider fixture có nhãn |
| Session đổi / read hoặc ACK trễ | `ViewState::change_session`, controller ACK typed, private child Owner; `separated_read_regressions`, `discussionSessionRaces` | App + Identity thật với local outbox, CDP đổi cookie thật/giữ response; không production auth |
| Loading/busy và public độc lập | Old request không settle owner mới; own/capability 503 không mất public; 404 work bỏ public/own | Browser 503 riêng dựng và ghi nhãn; reducer kiểm chống hồi sinh body/context |
| Own layout / composer identity | `discussionOwnFixtures`: 320/390/834/1440 × sáng/tối × chữ100/200; node/selection, ảnh | Programmatic input, không OS IME/screen reader/native |

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

Reader `/tac-pham/{id}`: “Thấm” sau văn bản và “Hữu ích” trên từng bình luận gốc/phản hồi
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


Delta chuẩn bị #226 (baseline `a81333c`, review trên `11c6a9c`): [LayoutSpec và dependency](../design/action-surfaces-reader-discussion-226.md).
[reader_actions.rs](../../packages/vink-patterns/leptos/tests/reader_actions.rs) kiểm composition
giữ confirmed value/count qua phase, lý do block và capability matrix root/reply.
`endorsementAsReader` bổ sung DOM identity/focus, count không optimistic khi pending/uncertain và
count không có target con. Đây là coverage baseline; kết quả thực ở
[evidence #226](evidence/action-surfaces-226/README.md). Contract surface #224 đã có trên develop qua #228; reply glyph vẫn thiếu.
Presentation #226 (kể cả AC-10 self-view và AC-11 header) chưa triển khai; không suy PASS UI mới từ regression.

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

Điểm vào tìm toàn thư viện là `GET /api/v1/works`. SSR trang chủ dùng request mặc định. Web đã dùng query/thẻ/cursor trên toàn corpus từ #100; xem [discovery UI](discovery-ui.md).
Lựa chọn, plan, đo đạc và gap: [discovery](discovery-api.md); hợp đồng:
[HTTP v1](../../services/vink-app/contracts/http/v1/README.md#public-discovery-tag-pr-04).

| Điểm vào / scope | Owner / assertions | Coverage limit |
| --- | --- | --- |
| Luật thuần: `q` (giữ chữ gõ, kiểm độ dài, có chữ hoặc số, từ chối NUL), `tag` (slug chuẩn hóa, ≤ 4, 8 giá trị thô), `limit` 1..24, cursor gắn phạm vi, `filter_tags` (chỉ thẻ đang hoạt động là facet), `cut_page` | `domain::discovery`; `domain::discovery::tests` (`quick`): rỗng = vắng, NFC/NFD/Đ, biên ký tự/byte, khử trùng, truth table resolve → ràng buộc, phạm vi theo query/thẻ canonical, layout cursor viết tay, một cách viết hex | Unit; không Kani (Unicode/chuỗi cursor như các lát trước) |
| Use case `Vink::{discover_works, publication}` + port `PublicationStore` | [discovery_tests.rs](../../services/vink-app/src/application/discovery_tests.rs) (`quick`): input sai không chạm store theo thứ tự q → tag → limit → cursor, mặc định = trang chủ 12, cắt trang + cursor sau alias, cursor của tìm kiếm khác → `CursorScope`, không có/ngừng dùng → rỗng có lý do, thẻ card chỉ đang hoạt động theo thứ tự snapshot, catalog thiếu/slug lệch → `Internal`, outage → `Unavailable` | Store giả không chứng minh SQL, snapshot hay plan |
| `GET /api/v1/works?q=&tag=&limit=&cursor=`, `GET …/publication` (thẻ công khai) | `api::{works_query, list_works}`; `http::tests::work_search_validates_before_reading_and_reports_outages` (`quick`: mọi tham số sai `422` đúng field trước database, outage `503 no-store` không ETag); fixture `works.{list,search,search.unmatched}.json`, `publication.get.json` (provider + `decodes_public_fixtures` của web) | Consumer Web query/thẻ/paging ở #100; xem [discovery UI](discovery-ui.md) |
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

Hồi quy #260 ở [discovery-chip-geometry.mjs](../../apps/vink-web/e2e/discovery-chip-geometry.mjs),
được gọi từ `discovery-corpus` và phase tập trung `discovery-chip-geometry`: đo tâm nhãn/rect
trên native Checkbox thật, 0/1/4 selection/max4/deselect, long NFC/NFD, hai theme/text100/200,
320×900 / 390×844 / 834×1112 / 1440×900, keyboard/forced-colors/accessibility tree.
[Evidence](evidence/discovery-chip-260/README.md) tách baseline FAIL và kết quả sau sửa;
coverage source không tự là runtime PASS.

## PUB-RUNTIME-01 — publication persistence và API tác giả (#158, 11/11/2026)

#230 tách [owner-state metadata projection](publication-state-read.md): shared restore invariants,
full history, không SELECT body. `tests/postgres/publication_state.rs` kiểm role không quyền đọc
body, 106 revision/ties/legacy, corruption/command rollback/public read, job digest/cross-work FK và
concurrent snapshot. Command/worker giữ content validation; public read kiểm hash đúng body trả ra.
Coverage này không tự là PASS; kết quả thực chạy ở báo cáo/evidence liên kết.

| Phần | Assertion hiện có | Khoảng trống |
| --- | --- | --- |
| Migration `20261116000000_work_publication_persistence` + [down script](../../services/vink-app/migrations-rollback/20261116000000_work_publication_persistence.down.sql) | [tests/postgres/publication.rs](../../services/vink-app/tests/postgres/publication.rs) (`postgres`): `upgrade_backfills_paired_state_for_old_pointers` chạy migration thật trên con trỏ cũ (generation 1 → 2, `legacy_bootstrap`, work không con trỏ thành `private`), seed lại không tạo lại con trỏ đã thu hồi; `backout_refuses_after_an_author_command_and_drops_unused_schema`; chuỗi rollback newest-first trong `abuse.rs`/`outbox.rs` | Không có dữ liệu production để đo backfill |
| `POST /api/v1/works/{id}/snapshots`, `…/publication/request-public` (`/submit`), `…/publication/set-private` (`/private`), `GET …/publication/state` (`/me`) | Cùng file: 401/404 chủ sở hữu, `no-store`, receipt/replay/`idempotency_key_reused`, generation cũ `409`, ticket `open`, r1 vẫn đọc được khi r2 chờ duyệt, riêng tư → public read/ETag cũ `404` (không `304`), discovery bỏ tác phẩm, khóa `publications` FOR SHARE không deadlock; `concurrent_retries_with_one_key_apply_once_and_replay` (4 retry cùng key: một lần ghi, còn lại replay; adapter trước sửa trả `500`) | Consumer Web: xem [PUB-AUTHOR-WEB-01](publication-author-web.md); không CMP consumer; replay của request không trả lại `ticket_id` (Web đọc state); không worker evaluator, admin, xóa tác phẩm; production vẫn `401` khi sign-in tắt |

## PUB-AUTHOR-WEB-01 — vùng công khai của Bàn viết (13/11/2026)

Owner và quyết định kỹ thuật: [publication-author-web](publication-author-web.md); layout:
[LayoutSpec](../design/publication-author.md). Bảng là map coverage, không khẳng định lane đã PASS.

| Hành vi | Bằng chứng | Giới hạn |
| --- | --- | --- |
| Đọc đủ state tác giả, giá trị lạ thành `Unknown`, fixture hai phía | `contract::publication::tests` (`quick`) giải mã mọi fixture `publication.state.*`/`snapshot.*`/`publication.*` và giữ mọi field; `http::contract::tests::author_publication` (`quick`) so provider với cùng fixture | Fixture là mẫu; không thay `verify postgres` |
| Phân loại lỗi, route mã hóa, deadline | `contract::tests::classifies_every_publication_refusal`, `api::publication_path_tests` (`quick`): ba lệnh + state đều có deadline, work id hiểm hóc không đổi đường dẫn | Deadline 30 s; abort không chứng minh server đã rollback |
| Reducer: lưu → chụp → gửi, đúng phiên bản, ghim generation, key, mất ACK, rút bài thắng, fence phiên | `publication_state::tests` (`quick`, 36 test): chữ gõ thêm khi đang lưu dừng gửi, mỗi bước mất ACK, replay chỉ khi generation đã ghim còn đứng, hai lệnh chồng, đổi scope, đọc lệch thứ tự, giá trị lạ | Thuần; không có DOM/transport |
| Bằng chứng DB cho đối soát | `author_web_recovery_contract_on_real_rows` (`postgres`): snapshot bất biến khi nháp đổi tiếp, replay snapshot sau khi nháp đã sang version khác, `409` nêu đúng field, replay request/private sau khi tab khác đổi không hồi sinh ý định, key dùng lại khác payload `422`, mọi khẳng định đọc lại từ hàng DB | Một tác phẩm seed; không đo song song nhiều tab |
| Luồng thật qua Web | phase `publication-author` (`web`, real Identity, ba principal): đăng nhập bằng mã email, viết/gắn thẻ/gửi, người duyệt có grant kết luận bằng trang, Khám phá/reader đúng revision + thẻ, r1/r2, lưu-khi-gõ-tiếp, xung đột nháp/thẻ ở bước chụp, nhấn đôi, mất ACK ở snapshot/request/private, hai tab, rút bài khi còn chữ chưa lưu (ETag cũ `404`, reader ẩn), phiên hết/đổi tài khoản, IME mô phỏng | Chrome headless, một máy; IME là mô phỏng CDP, không phải OS IME; `503`/`403` đọc state là câu trả lời có nhãn “fabricated” |
| Hình học và ảnh theo trạng thái | `matrix` trong phase trên: 5 trạng thái × (320/390/1024 × sáng/tối × chữ 100%/200%): không cuộn ngang, không nằm ngoài vùng, không cắt/ellipsis; `refreshInvariant`: đọc lại không remount editor, không đổi cuộn/focus | Ảnh ở output `verify web` (`target/verify-web/<run-id>/screenshots`); `NO_APPROVED_REFERENCE` |

## Hậu kiểm do operator yêu cầu

`vink-app post-review request|result|status|pending` → application `post_review` → persistence
`post_review` → outbox/`vink-worker` → canonical discussion/own/moderator/endorsement reads.
Owner, DecisionMap, khóa, retry/deadline, kill switch, backout và giới hạn ở
[discussion-post-review](discussion-post-review.md); assertions: `tests/postgres/post_review.rs`,
`post_review::tests`, `post_review_replay`; model `behavior/discussion/post-review.qnt`.
Đây là map coverage, không khẳng định lane đã PASS; admission activation vẫn chưa có.

## MOD-RUNTIME-01 — kiểm duyệt tác phẩm và trang ở URL cũ (#159, 12/11/2026)

| Hành vi | Bằng chứng | Giới hạn |
| --- | --- | --- |
| Worker áp kết quả dưới fence claim, chế độ tự động và `DecisionVersion` | `stale_claim_or_pause_before_finish_changes_nothing`, `retryable_failures_use_fresh_evaluation_ids_then_go_to_people` (`postgres`); unit cấu hình trong `evaluation_worker.rs` | Chỉ evaluator kịch bản; không đo hai worker song song thật |
| Dừng tự động, gửi khi đang dừng, ca duyệt, mất quyền, nhóm, tự né | `paused_submission_goes_to_the_human_queue`, `stale_note_and_lost_authority_are_persisted_honestly`, `group_grants_recusal_and_membership_removal` (`postgres`) | Web ca tác phẩm và chế độ tự động; UI grant/nhóm chưa có |
| Trang “Bản này đã bị gỡ sau khi kiểm duyệt.” giữ bình luận, sống qua lần gửi lại, nhường bản duyệt kế tiếp | `review_withdrawn_page_survives_a_pending_resubmission` (`postgres`); phase `review-withdrawn` (`web`: SSR `noindex`/`no-store`, hydration, xóa cache tab và thẻ Khám phá, chặn bình luận mới) | M3 thêm `ReviewWithdrawn`, 53/53 bước + NC chống lộ body; model tách biệt HTML/IO |


| Trang ẩn không lộ metadata, giữ bình luận và chặn ghi mới; công khai lại | `hidden_reader_retains_only_discussion_and_republication_restores_body` (`postgres`); phase `review-withdrawn` (`web`) | Chưa có tombstone/xóa bài |
| Phản đối duy nhất mỗi quyết định, admin final, riêng tư đóng hồ sơ, ý định mới thắng approval cũ | Nhóm `work_appeal*`, `private_closes*`, `appeal_on_old_revision*` trong `work_moderation.rs`; tác giả gửi/admin đảo qua UI trong phase `review-withdrawn` | Không có thông báo, comment appeals hoặc điều tra |

Queue work/admin appeals #205: routes `/moderation/works/queue/page`, `/moderation/appeals/page`,
`QueuePanel` độc lập dùng ReadOwner/PageChain + authority fingerprint; regression
`postgres::moderation_queue`, provider/Web fixtures và `moderationQueues` trong scenario `review`.
[Registry](moderation-runtime.md#10-hai-queue-hữu-hạn-205),
[LayoutSpec paging](../design/moderation-queue-pagination.md). Comment queue đã paging;
author appeal/revision history ngoài lát này.

LayoutSpec UI: [moderation-workspace](../design/moderation-workspace.md). Trạng thái chạy và SHA
nằm tại [moderation-runtime](moderation-runtime.md#9-bằng-chứng-và-giới-hạn).

## Read lifecycle Web — model và production replay

[Model owner](../../apps/vink-web/behavior/README.md) giữ giả định, properties, fixture và lệnh
tái lập. `read_lifecycle_replay.rs` gọi primitive production, không dựng reducer thứ hai.

| Hành vi | Assertion hiện có | Giới hạn |
| --- | --- | --- |
| Đổi query/session A→B→A, invalidate, dispose/remount, refresh thay append | `quint_read_lifecycle_inputs_replay_through_production`: 21 bước fence và 31 bước paging; outcome + toàn owner/chain/tickets | Scope do adapter cung cấp; không kiểm auth hoặc xóa private DTO ở view |
| Append lỗi giữ cursor/rows, từ chối duplicate/overlap/cursor cycle; refresh bỏ tail | Cùng replay, `ticketFence`, `pageIntegrity`, `rejectedKeepsChain`, `headReplacesTail`; simulation + drift trong `discussion` | Không SQL live keyset, continuation 240 mục hay HTTP/DOM |
| Oracle phát hiện bỏ guard | Hai mutant Quint: ticket cũ sau remount và overlap page; `quint_read_lifecycle_counterexamples_are_refused_by_production` bắt Rust từ chối | Không exhaustive proof, freshness coordinator hoặc IndexedDB transaction |

Consumer thật vẫn cần tests riêng: `draftPagination`/`authWeb` cho Bàn viết, `moderationQueues`
trong `review` cho quyền/authority fingerprint, `discussion_reads` và discussion browser cho
public/own overlay. Source/test presence là coverage intent; [run 16/11](verification-reconciliation-20261116.md)
chỉ ghi những boundary thực sự chạy ở lượt này.

## Tooling local có phạm vi (#184)

[Workflow guide](scoped-workflows.md) sở hữu cú pháp, dependencies, defaults và recovery.
[Planner](../../xtask/src/plan.rs) và [CLI tests](../../xtask/tests/scoped_cli.rs) kiểm selection,
restart → main, invalid/empty input và đường inspection không gọi executor hoặc lộ credentials.
[Workflow tests](../../xtask/src/workflow.rs) kiểm env runtime không mang fixture/socket/migrator
ngoài lựa chọn; [lifecycle](../../xtask/src/lifecycle.rs) kiểm duplicate lock/reaped child.
`api_composition_has_no_ssr_or_static_routes_and_keeps_private_guard` ở HTTP owner kiểm router
mode; source/test presence không thay bằng chứng PostgreSQL/process/browser thật.

`verify web` giữ full coverage của các bảng trên. `--scenario main|storage|discovery|restart|endorsement|review|auth|password|publication`
chỉ chạy coverage được chọn và dependency khai báo; `restart` kéo `main`. `verify preview` là
alias có plan cho Leptos preview hiện hữu; CMP/native không đổi. Evidence theo lượt nằm trong
[registry triển khai](scoped-workflows-evidence.md), không coi selected PASS là full PASS.

Delta [#213](https://github.com/loveoverflowcom/vink/issues/213): Desk bỏ resume/Recent/1 và
version nhìn thấy trên row, giữ full title/href và CAS facts. Current LayoutSpec:
[desk-server-pagination](../design/desk-server-pagination.md). Thời gian Web dùng instant tuyệt đối
`vi-VN` theo device zone sau hydrate, SSR fallback UTC; Web/CMP patterns dùng Thấm/Bỏ Thấm.
Coverage mới nằm trong `draftDashboard`/`draftR6Hierarchy`, pagination smoke, timestamp unit và
endorsement smoke; kết quả thực ghi tại [evidence #213](desk-compact-tham.md).


### Empty artwork CMP diagnostic #261

`ScreenRecipes.kt` tiêu thụ ba asset canonical của #258 tại catalog `screen.explore` và
`screen.desk`, scenarios `empty`/`no-results` hiện hữu. `EmptyArtworkTest` kiểm alpha pixels,
output bounds, cover rejection và cancellation; `EmptyArtworkPreviewTest` kiểm state exclusion,
focus/selection, semantics, fallback/CTA và reflow; `EmptyArtworkCaptureTest` giữ before/after
320/390/834dp × Light/Dark × text100/200. Android `exactDecorativeAlphaWebpDecodesAndKeepsRecovery`
và iOS `testArtworkEmptyStatesKeepRecoveryAtLargeText` là checks riêng, không suy PASS từ JVM.
[Spec](../design/empty-artwork-cmp-261.md) · [kết quả/gap](evidence/empty-webp-cmp-261/README.md).
