# Đọc công khai cache-first và lifecycle Web (READ-02)

**Issue:** [#53](https://github.com/loveoverflowcom/vink/issues/53), cập nhật tính đúng tại [#200](https://github.com/loveoverflowcom/vink/issues/200) và lifecycle [#202](https://github.com/loveoverflowcom/vink/issues/202) · **Nền:** `develop`
(có READ-01 [#52](https://github.com/loveoverflowcom/vink/issues/52), Discovery #100, Author publication #158, Moderation & `O-17@r2`)

**Bốn chiều trạng thái (tách biệt độc lập):**
- **Quyết định sản phẩm:** `OPEN` cho chính sách hiển thị dữ liệu cũ (`max_display_age = 15 phút` là giả định kỹ thuật v1, chưa chốt với chủ sản phẩm); `PRODUCT_CONFIRMED` cho `O-17@r2` (trang thay thế khi rút bài sau kiểm duyệt) và `PUB-07` (ẩn tác phẩm).
- **Quyết định kiến trúc:** `DESIGN_PROPOSED` cho `READ-02-CHOICE-01..04` (chỉ áp dụng ở Web pilot; không tự nâng lên `Accepted` khi test xanh). Tương thích với cập nhật CHOICE-01 = A (adapter lỗi có kiểu theo `status + error.code + error.field`) và CHOICE-02 = A (tách `mark_stale` giữ backoff khỏi `mutation_confirmed` wake ngay).
- **Mức triển khai (`implementation level`):** `web` pilot (`apps/vink-web/src/freshness.rs`, `public_reads.rs`, `contract.rs`, `api.rs`, `pages/discovery.rs`, `pages/mod.rs`); CMP/native là `NOT_IMPLEMENTED`.
- **Bằng chứng kiểm chứng (`verification evidence`):** ghi theo từng lần chạy thực tế ở mục [Bằng chứng](#bằng-chứng), tách riêng unit/contract, PostgreSQL và trình duyệt Chrome headless.

Mục tiêu: có snapshot được phép thì hiển thị ngay, làm mới phía sau, không xóa nội dung đang đọc
để hiện loading toàn màn; kiểm tra lại khoảng 2 phút khi màn đang hiển thị. Không thêm cache phía
server, realtime hay Service Worker. Adapter lưu bền #203 nằm dưới gate tắt; xem [IndexedDB](#indexeddb-203).

## Đối soát owner và evidence #233

Ngày 16/11/2026, nguồn `develop@69f71f0cb61228dbe5ceea775b3c34c237705922`, phục vụ
[Epic #186](https://github.com/loveoverflowcom/vink/issues/186) và
[#233](https://github.com/loveoverflowcom/vink/issues/233). Đây là **source/registry audit**;
không gọi các kết quả browser cũ ở dưới là PASS của HEAD này. Epic giữ roadmap/dependencies;
tài liệu này giữ hiện trạng READ-02, không thay PRD hoặc kho quyết định.

Đối soát lại khi rebase lên `develop@a81333c71faf6033014a00ec26c9f7b90109d1d9` (PR #232) và merge
`develop@6e07806` (PR #228, PR #240): `apps/vink-web/src/`, `services/vink-app/src/`, `decisions/`
và `xtask/src/cli.rs` không đổi so với pin trên (#228 chỉ sửa UI system/previews; #240 thêm
assertions E2E Desk/Discovery và tài liệu thiết kế/bằng chứng).
Chạy lại `decision validate`/`refs`: cùng 118 revisions và snapshot digest; 79 declared
references (thêm `USER-09@r1` trong report #229), không có problems.
[#229 đã có corpus và report](long-document-budget.md); dùng lại source/fixtures đó cho #234,
không dựng corpus cạnh tranh. Số đo ở report có SHA/profile riêng, không là benchmark tự chạy
của #233 hoặc baseline release cho toàn hành trình.

Review PR #241 trên `develop@313175370d29a03dd0096ece3e6a5933a74ddbea`:
#244 đã đổi presentation Bàn viết/Khám phá, còn #245 thêm LayoutSpec và regression cho #226.
Các owner `read_lifecycle.rs`, `public_reads.rs`, `freshness.rs`, `read_cache.rs`,
`read_events.rs`, `public_persistence.rs`, `services/vink-app/src/`, `decisions/` và
`xtask/src/cli.rs` không đổi từ `6e07806`; không mở rộng kết luận đó ra toàn bộ Web source
hoặc test App. Hiện trạng và evidence UI #225 nằm ở [registry riêng](action-surfaces-225.md);
#226 vẫn [chưa triển khai presentation mới](../design/action-surfaces-reader-discussion-226.md).
Kiểm tra của lượt review docs-only này: `decision validate` và `decision refs` PASS với cùng snapshot
118 revisions/79 refs ở dưới; `git diff --check` và relative file links của hai tài liệu
thay đổi PASS. Không chạy lại model, PostgreSQL, browser hoặc CMP cho delta docs-only này.

Follow-up tích hợp #244: [CI của `d31fa40`](https://github.com/loveoverflowcom/vink/actions/runs/43836039923)
phát hiện test SSR vẫn chờ nhãn số tác phẩm trước #225. Đã tái hiện FAIL bằng PostgreSQL 17
cục bộ rồi cập nhật assertion theo LayoutSpec “Trang này: N tác phẩm”, giữ nguyên các assertion
không lộ nháp/snapshot riêng. `cargo test --locked -p vink-app --test postgres
server_html_has_public_content_and_never_private_drafts -- --exact` PASS 1/1 sau sửa
(197 filtered out); đây là kiểm PostgreSQL/SSR có phạm vi, không full lane hoặc browser.

### Luồng và lifetime hiện hành

| Representation / state | Owner và đường đi thật | Lifetime / boundary |
| --- | --- | --- |
| Public SSR | [`Resource` trong api.rs](../../apps/vink-web/src/api.rs) → [`SharedPublicReads`](../../apps/vink-web/src/server.rs) → [`PublicReadsAdapter`](../../services/vink-app/src/http/ssr.rs) → App use case/PostgreSQL | Mỗi SSR request; chỉ public DTO qua hydration, không chuyển service/credential hoặc dùng tab cache trên server |
| Works / typed Reader | [`PublicKey`, `Coordinator`](../../apps/vink-web/src/public_reads.rs) → [`freshness::Store`](../../apps/vink-web/src/freshness.rs) → transport HTTP; `Reader` luôn gọi `/reader` | Tab; flight/validation age/removal authority ở Store; `WaiterGuard` và keyed route cleanup giải phóng waiter; snapshot mới không bị seed cũ ghi đè |
| Snapshot retention | [`SnapshotCache`](../../apps/vink-web/src/read_cache.rs): `InMemoryCache` (ngân sách trọng số `READ_CACHE_BUDGET` ở `public_reads.rs`: trần `2 MiB` tổng / `1 MiB` mỗi entry proxy) hoặc `NoOpCache`; policy/GC ở `freshness::Store` | Optional retention khác active DTO/flight. Store giữ `max_entries = 16`, `retain_unobserved = 5 phút`; entry đang quan sát giữ active trong `Entry::data` (không đúp bản trong cache), entry vượt trần vẫn đọc/render đủ nhưng không retain; proxy UTF-8 + cấu trúc khác RSS/GPU; eviction không mang authority |
| Timer / lifecycle hints | `public_reads::browser` giữ một scheduler; [`read_events`](../../apps/vink-web/src/read_events.rs) giữ bộ listeners chung và subscription của view | Public timer dừng khi tab ẩn; mounted no-store owners dùng `WakePolicy`, không timer riêng theo card; cleanup gỡ subscription của view, tab listener tồn tại theo tab |
| Private reads / discussion | [`ReadOwner`, `PageChain`](../../apps/vink-web/src/read_lifecycle.rs); public `ViewState` và scoped `OwnState` theo [discussion owner](discussion-web-reads.md) | Browser private facts có session scope/generation, không SSR/public cache. Public/own/capability là reads có nghĩa riêng, không tính tất cả là duplicate |
| Device snapshot | [`public_persistence`](../../apps/vink-web/src/public_persistence.rs) → [`indexeddb.js`](../../apps/vink-web/public-cache/indexeddb.js) dưới coordinator hiện hữu | Adapter đã có, `ENABLED=false`; 32 rows/4 MiB logical UTF-8, envelope ≤512 KiB, queue 8 pending + 1 executing. Không là giới hạn process RAM/GPU; không cold offline launch |
| Chữ/thẻ chưa ACK | [`EditorSession`/`DraftSession`](../../apps/vink-web/src/desk_state.rs), [`TagSession`](../../apps/vink-web/src/tag_state.rs), CodeMirror [`BodySource`](../../apps/vink-web/src/editor.rs) | Feature editor + session scope; lưu thủ công, text/tags có ticket/version riêng; [save slot #217](../design/ui-editor-unified-save-217.md) đã merge, không có durable local recovery |
| Comment command / composer | [`DiscussionController`](../../apps/vink-web/src/discussion_state.rs), `Submission`, typed ACK | Snapshot/key và edit generation bất biến khi gửi; Uncertain dùng exact-key/payload reconcile cùng scope; ACK không xóa chữ mới; collapse khác dispose, không bảo đảm giữ qua rời route/restart |
| Vị trí Khám phá | `remember_public_position` / `restore_public_position` trong [`pages/mod.rs`](../../apps/vink-web/src/pages/mod.rs), URL `ExploreContext` | [#236](../design/discovery-return-position-236.md): đồng bộ row anchor/query/cursor qua router, chờ scope settle và giữ focus người dùng đã chọn; fallback vùng kết quả nếu thiếu hàng. Không có offset snapshot hay retained public view |

### Phân loại delta của Epic

| Loại | Kết luận và owner tiếp theo |
| --- | --- |
| Đã có | #200/#202: typed lỗi, backoff/invalidation, Reader shared flight/control, ReadOwner/PageChain; #184: selected runner/plan; #203: adapter lưu bền dưới gate tắt; #217: save projection |
| Lỗi tài liệu | Inventory `/publication`, recipe rollback trước #202, câu IndexedDB chưa có và consumer moderation chưa migrate được sửa trong #233. Không suy ra runtime defect từ prose cũ |
| Trùng nếu làm lại | Reader cache framework, save-state mới, generic runner hoặc document corpus mới. Reuse #202, #217, #184 và [#229](https://github.com/loveoverflowcom/vink/issues/229) |
| Thiếu / chưa chứng minh | Hard byte budget RAM; baseline journey release/noise; relative-offset/retained-view qualification; physical IME và screen-reader. Thiếu phép đo không chứng minh leak hoặc jank |
| Giả thuyết | Streaming, keep-alive, containment, prefetch có lợi: cần baseline cùng fixture/config và stop condition trong #234–#239; giữ nguyên có evidence là kết quả hợp lệ |
| Product còn mở | Mount age 15 phút, durable clearing/logout và UX hết hạn. Không thay policy hoặc bật gate trong PR này |

### Query quyết định tại base

Đã chạy `cargo xtask decision query ID` cho từng ID trong bảng, `decision validate` và
`decision refs`. Snapshot SHA-256:
`cef5662c19d3db1219bab14d197b5413f203aa816c99eaa11e8f2c31688091c2`.
Validate ghi 118 revisions; refs ghi 79 declared references (tại `69f71f0` trước khi rebase
PR #232 là 78; từ #232 tới `develop@6e07806` là 79), không có problems.
Đây là kiểm cấu trúc và pin; semantic review/signoff là việc riêng.

| IDs | Current / head | Trạng thái / nguồn |
| --- | --- | --- |
| READ-01-CHOICE-01..04, READ-02-CHOICE-01..04, DISC-WEB-CHOICE-01..03 | r1 / r1 | `DESIGN_PROPOSED`, [development import](../../decisions/decision-dev-20261112.toml); `source` của từng record giữ commit/path/line nhập |
| O-17, O-03 | r2 / r2 | `PRODUCT_CONFIRMED`, proposal + signoff tương ứng trong [kho TOML](../../decisions/README.md) |
| USER-02, USER-09 | r1 / r1 | `PRODUCT_CONFIRMED`, [DecisionDelta import](../../decisions/decision-delta-20261110.toml) |
| D-04, D-05 | r1 / r1 | `DESIGN_PROPOSED`, `PROVISIONAL` tương ứng, [D-log import](../../decisions/decision-log-20261111.toml) |
| VDR-PUBLIC-OFFLINE-AGE | không current / r1 | `pending_only`, proposal `PENDING`, digest `72bb815fc75137819c252cc7cc632a17503389d7bbd0e9fbcf866f3c4900d2aa`; chưa signoff |

Lát docs-only #233 không tạo proposal mới; recommendations trong Epic là
câu hỏi kỹ thuật có điều kiện. Thay policy/contract phải qua owner và revision đúng phạm vi.

### Rule → seam → phép kiểm

| Rule / nguồn thẩm quyền | Seam và assertion hiện hữu | Lệnh để kiểm ở HEAD thực thi / giới hạn |
| --- | --- | --- |
| INV-01/07/08, PUB-05/07, O-17: public visibility và không hồi sinh nội dung | App `reader_work`/HTTP `get_reader`; `public_reads::tests::reader_ticket_orders_cache_refreshes_and_records_request_start`, `reader_ticket_survives_republication_and_private_pointer_invalidation` | `cargo xtask verify unit web --filter public_reads`; Web `main`/`review`/`publication` cho transport/provider thật; unit không chứng minh SQL hoặc DOM |
| READ-02 policy: seed, age, backoff, single-flight | `freshness::tests`, `public_reads::tests`, `cacheFirstPublicReads` trong [browser smoke](../../apps/vink-web/e2e/browser-smoke.mjs) | Unit web filters `freshness`, `public_reads`; Web `main`. Logical reads, transport attempts và raw/wire304 phải đếm riêng |
| INV-05/08, COM-07/08: private scope và command authority | `read_lifecycle::tests`, `discussion_reads::tests`, `discussion_state`; `discussionSessionRaces` trong [auth smoke](../../apps/vink-web/e2e/auth-smoke.mjs), `discussionReadingAndWrites` trong [browser smoke](../../apps/vink-web/e2e/browser-smoke.mjs) | Unit + Web `auth`/`main`; durable receipt/current authority cần PostgreSQL. Mô hình D2T không chứng minh browser/session I/O |
| WR-01/02/05: ACK đúng text và tags | `desk_state::tests::late_acknowledgement_does_not_mark_newer_text_saved`, DraftSession/TagSession, `project_save_slot` | Unit web + Web `main`/`publication`; programmatic composition không là OS IME |
| UX-01/02: main HTML, GET, context | `publicReading`, `cacheFirstPublicReads`, discovery corpus; Async routes trong `lib.rs` | Web `main`/`discovery`/`review`; no-JS/status/privacy khác performance trace hoặc screen-reader |

Các command/scenario trên được đối chiếu với [scoped workflows](scoped-workflows.md) và
`xtask/src/cli.rs`; dùng `--plan` để xem đúng phụ thuộc. **NOT_RUN trong audit docs-only #233:** unit/browser/
PostgreSQL/benchmark/model lanes, vì chỉ sửa tài liệu. Historical PASS bên dưới giữ nguyên phạm vi.
**No semantic/model impact trong #233:** không sửa state/guard/permission/race/recovery.
Canonical M1/M3/M4 ở `services/vink-app/behavior/publication` và D2T ở `behavior/discussion`
giữ nguyên. Sau audit này, #243 bổ sung [model/replay primitive Web](../../apps/vink-web/behavior/README.md)
cho `ReadOwner/PageChain`; model đó không phủ `freshness::Store/Coordinator`, save reducer hay
I/O của Desk. Evidence #243 nằm ở [lượt đối soát riêng](verification-reconciliation-20261116.md);
không dùng nó làm mới kết quả browser/PG của #233.

## HOW

```text
SSR (mỗi request)  Resource → SharedPublicReads in-process      không dùng store; không rò giữa request
Hydrate            Resource lấy dữ liệu hydration ─┐
Điều hướng client  Resource → public_reads::load ──┤
                                                   ▼
          public_reads (owner của tab, apps/vink-web/src/public_reads.rs)
            Coordinator + Store<PublicKey, Snapshot> ─ freshness.rs (policy, ticket, removal fence, GC; không I/O)
            1 timer · listener visibilitychange/focus/online/pageshow · waiter single-flight + WaiterGuard
                                                   ▼
          api::http::public  fetch cache: no-cache (+ deadline 30 s) + classify_public_read_bytes (status + code + field)
            → HTTP cache trình duyệt gửi If-None-Match (ETag READ-01) → 304 được merge thành 200
```

View (`PublicResults` trong `pages/discovery.rs`, `ReaderLoaded` trong `pages/mod.rs`) render `Live { data,
cached_unverified, refreshing, failed, removed, rejected, key }` và gửi ý định “Thử lại” / “Xem bản mới” / “Thử từ trang đầu” / “Bỏ toàn bộ bộ lọc”.

## Quyết định chọn owner và thư viện

Đọc tài liệu và source ở execution HEAD (Leptos 0.8.22, toolchain 1.99.1). Không thêm thư viện
mới; chỉ thêm `futures-channel 0.3.34` (đã nằm trong dependency graph qua Leptos) và ba feature
`web-sys` (`Performance`, `VisibilityState`, `RequestCache`).

| Ứng viên | Kết luận | Lý do |
| --- | --- | --- |
| **Baseline `Resource` mở rộng tối thiểu + owner nhỏ trong app** | **Chọn** | Giữ nguyên SSR/hydration hiện có (Resource vẫn serialize payload); phần mới là một state machine thuần có test đồng hồ giả và một adapter mỏng. Mọi semantics issue đòi (giữ last-success khi lỗi, generation guard, max display age, xóa khi 404, một timer dừng khi ẩn) đều nằm trong code của Vink, đọc và kiểm được. |
| `leptos-fetch 0.4.10` (MIT, MSRV 1.88, Leptos ≥0.8.13) | Chưa chọn; **chưa chạy spike trong Vink** | Đọc source 0.4.10: không có xử lý visibility/focus; mốc stale dùng đồng hồ tường `chrono::Utc::now()`, không monotonic; không có tùy chọn riêng cho `Result::Err` (stale-if-error) hay xóa khi 404; mặc định `stale_time = never`, `gc_time = 5 phút`, interval phải tự cấu hình. Kéo thêm `chrono`, `chrono-humanize`, `codee`, `gloo-timers`, `parking_lot`, `task-local`. Vink vẫn phải viết wrapper cho các điểm trên, nên lợi ích chưa vượt chi phí. Mở lại khi có nhiều query hơn hai endpoint này hoặc cần pagination/subscription của nó. |
| `leptos-use-document-visibility 0.20` / `-window-focus` | Không dùng | `vink-ui` đã có `document_visibility()`; owner cần listener một lần cho cả tab (không theo component), `web_sys` trực tiếp đủ và không thêm feature SSR. |
| Ktor `HttpCache` | Không áp dụng | Chưa có product app native. Xem [CMP](#cmp-và-native). |

### Ai giữ validator: phương án A

| Phương án | Kết luận |
| --- | --- |
| **A — Query cache của app + HTTP cache trình duyệt** | **Chọn.** Trình duyệt giữ byte + ETag và tự gửi `If-None-Match`; app không đọc/giữ ETag. Request dùng `cache: "no-cache"`: nếu sau này server đổi header sang có `max-age`, trình duyệt vẫn phải hỏi lại origin trước khi dùng byte đã lưu, nên app không bao giờ ghi `validated_at` cho một bản trình duyệt tự trả từ cache. Không gắn timestamp lên URL; refresh thủ công cũng gửi validator. |
| B — App tự quản validator | Không chọn: phải tự parse 304, đồng bộ data + tag, và tạo owner thứ hai cạnh HTTP cache. Chỉ mở khi A thiếu semantics. |
| C — GET 200 không điều kiện | Đường lui: đổi `RequestCache::NoCache` thành `Reload` trong `api::http::public` (hoặc rút ETag phía server theo [READ-01](public-conditional-reads.md#rút-lại-theo-endpoint)); UI không đổi. |

App không tự gửi validator, nên raw `304` không thể đến tay app. Nếu vẫn đến (proxy lạ, thay đổi
sau này): **không** parse JSON rỗng; gửi đúng **một** request `cache: "reload"` không điều kiện;
`304` lần nữa → lỗi tạm thời (`freshness::public_status`).

## Chính sách v1 (đề xuất, có thể đo/chỉnh)

<!-- vdr: active READ-02-CHOICE-01@r1 -->
<!-- vdr: active READ-02-CHOICE-02@r1 -->
<!-- vdr: active READ-02-CHOICE-03@r1 -->
<!-- vdr: active READ-02-CHOICE-04@r1 -->

Hằng số ở `freshness::Policy::V1`. Mọi mốc thời gian dùng `performance.now()` (monotonic theo tab).

| Tham số / sự kiện | Hành vi |
| --- | --- |
| `stale_after = 120 s` | Ngưỡng kiểm tra lại, **không phải TTL**: không xóa gì. `validated_at` là thời điểm **bắt đầu** request được origin xác nhận (cận dưới, an toàn); đọc cache không gia hạn nó |
| Timer | Một timer cho cả tab, hẹn đến mốc sớm nhất của các key đang được view quan sát: `validated_at + 120 s + spread`, spread ngẫu nhiên `0–15 s` chọn lại sau mỗi lần xác nhận. Không timer theo card |
| Mount, `focus`, hiện lại (`visibilitychange`), `online`, `pageshow` | Kiểm tra nếu đã quá 120 s (không cộng spread). Gộp với request đang chạy (single-flight); bốn tín hiệu đồng thời → một request. `online` chỉ là gợi ý: lỗi vẫn backoff |
| Ẩn tab | Hủy timer, bỏ qua mọi tín hiệu trừ thao tác thủ công; hiện lại thì kiểm tra nếu stale |
| “Thử lại” (thủ công) | `Trigger::Manual` bỏ qua ngưỡng 120 s và backoff, vẫn single-flight và vẫn có validator (`rejected` dừng kiểm tra tự động trừ khi mở điều hướng mới hoặc thử lại có chủ đích) |
| Lỗi `4xx` / từ chối có kiểu (**CHOICE-01 A**) | Adapter có kiểu phân loại theo `status + error.code + error.field` trên hợp đồng hiện có (**không** parse `error.message`; giới hạn envelope 64 KiB). `422 validation_failed cursor` → `RestartCursor` (dừng retry cursor cũ, hiện `#restart-public` về trang 1); `422 validation_failed` theo `q`/`tag`/`limit`/`query` → `InvalidInput` (báo đúng lỗi nhập liệu + link bỏ lọc, không kẹt vòng lặp reset cursor); envelope hỏng/code lạ/`400`/`414` → `Fallback` (dừng retry tự động với thông báo hữu hạn) |
| Lỗi tạm (`429`, mạng, timeout 30 s, `5xx`, raw `304` lần hai) | Giữ last-success trên màn hình với thông báo riêng; backoff 5 → 10 → 20 → 40 → 80 → 120 s (trần), jitter 80–100 %, tôn trọng `Retry-After` dạng giây (trần 120 s). Không retry mutation |
| `404` / rút bài / ẩn bài (`Outcome::Removed`, `invalidate_withdrawn`) | Rút snapshot Published của key `Reader` ngay và ghi **removal fence** (`Store::removed` + `Coordinator::withdrawn`), lọc card khỏi mọi `WorkList` đang giữ hoặc `seed` cũ hơn mốc rút bài, nhận typed control trực tiếp từ `/api/v1/works/{id}/reader` (`ReviewWithdrawn` / `Hidden` theo `O-17@r2` / `PUB-07`). **Không** là tombstone vĩnh viễn: điều hướng `load()` mới nhận `200` (`Outcome::Fresh`) sẽ gỡ fence khi tác phẩm được công khai lại |
| Bản đổi nội dung (`200` khác) | Khám phá cập nhật tại chỗ (key ổn định: giữ row, focus, scroll). Reader **không** thay chữ đang đọc: hiện “Tác phẩm vừa có bản cập nhật” + “Xem bản mới”; áp dụng thì về đầu nội dung và focus `#noi-dung` |
| `max_display_age = 15 phút` | Snapshot xác nhận quá 15 phút không được render ngay ở **lần mount mới** (sẽ tải với loading thật). Nội dung đang trên màn hình không bị rút vì lỗi tạm |
| `retain_unobserved = 5 phút`, `max_entries = 16`, `READ_CACHE_BUDGET = 2 MiB / 1 MiB` proxy | **Retention theo trọng số trong [`InMemoryCache`](../../apps/vink-web/src/read_cache.rs) + trần `16` entry / `5 phút` không quan sát (`#237`, `WEB-PERF-07 CHOICE-01 = B`)**: `snapshot_weight_bytes` tính proxy UTF-8 + container cấu trúc không cấp phát heap. Sau #255, document giữ một chuỗi source: đếm byte nguyên văn (NFD/CRLF/dòng trống), không cộng allocation cho từng dòng. Entry `> 1 MiB` đọc/hiển thị bình thường khi mở nhưng bỏ qua lưu sau `release`; tổng vượt `2 MiB` thì evict LRU đơn điệu theo thứ tự `put`. Entry đang quan sát không giữ bản đúp trong cache và không bị eviction làm mất chữ |
| Hydration | Snapshot SSR được seed với `validated_at = 0` (chỉ tính cận dưới an toàn của điều hướng hiện tại). Trang mở bằng back/forward có `rendered_at = -∞` vì tuổi HTML cache không rõ. `seed` bị chặn nếu đã có snapshot mới hơn, request đang bay, hoặc removal fence tại/sau `rendered_at` |
| Tách `mark_stale` vs `mutation_confirmed` (**CHOICE-02 A**) | **`mark_stale` / `mark_stale_where`** (stale chung) đánh dấu `stale = true` và vô hiệu hóa ticket đang bay nhưng **giữ nguyên `failures`/`retry_at`** để không vượt backoff khi lỗi tạm; đọc `status()`/`changed()` hay lặp `mark_stale` không xóa backoff. **`mutation_confirmed` / `mutation_confirmed_where`** (qua `invalidate_publication` / `invalidate_withdrawn` từ ACK của `pages/publication.rs` và đường đọc `/reader`) đánh dấu `immediate_retry = true` để các owner **đang quan sát trên tab hiện** kiểm tra lại ngay không cần chờ timer 120 s (gộp single-flight; owner ẩn hoặc không có view quan sát không tự poll). Lưu nháp riêng vẫn **không** invalidate snapshot công khai |

Ngoài phạm vi store: nháp riêng (`no-store`, CAS/409), editor CodeMirror và văn bản chưa lưu,
discussion (`/discussion/public*` dùng `ViewState`, `/discussion/me` dùng `discussion_reads::OwnState`, không cache),
endorsements, catalog thẻ, `vink-worker` outbox và màn Bàn viết/kiểm duyệt. Luồng sản phẩm đang bật vẫn chỉ ở bộ nhớ tab;
đóng tab/kill process là mất. #203 có seam `clear_device_cache()` cho durable clear và dọn RAM,
nhưng chưa có UI hoặc caller reset cache công khai khi đổi tài khoản/phiên. Cache này không chứa
dữ liệu riêng theo tài khoản. Adapter IndexedDB nằm dưới gate tắt; chưa có HTTP disk policy
riêng/Service Worker. Không xem một seam clear là policy logout đã được chốt.

### Chỗ còn cần quyết định sản phẩm

1. **Hiển thị dữ liệu cũ:** v1 chọn hướng 1 của issue — giữ snapshot công khai đã xem trong phiên
   tab, có trạng thái “chưa làm mới được”, revalidate khi online/hiện lại, và `max_display_age`
   15 phút cho lần mount mới. Giá trị 15 phút là giả định `OPEN` cần chủ sản phẩm chốt.
2. **Rút bài đa thiết bị:** tab khác / thiết bị khác vẫn chỉ nhận ra bản bị gỡ ở lần revalidate/mở
   lại tiếp theo (404 / `reader`), trừ khi chính tab đó thực hiện mutation hoặc mở `/works/{id}`.
3. **Offline full-text:** chưa bật. Adapter lưu bền, migration và kiểm thử thuộc [#203](#indexeddb-203);
   proposal 24 giờ đã ghi, còn policy account clearing/thiết bị dùng chung và UX hết hạn trước rollout.

## Ranh giới race và an toàn

- **Ticket tăng đơn điệu** cho mỗi request; chỉ ticket hiện hành của entry được commit.
  `mark_stale`, `mutation_confirmed`, `remove`, `clear` làm request đang chạy không thể commit; ticket
  không bao giờ lặp và kết hợp **removal fence** (`withdrawn` + `removed`) chặn `seed` cũ hơn mốc
  gỡ bài hồi sinh bản đã rút hoặc card đã gỡ. Không so sánh ETag để suy bản mới.
- **Đọc `/reader`:** một key/flight cho Resource và kiểm nền; cả cache và kết quả trả về Resource
  phải qua ticket hiện hành. Invalidation không xóa removal fence. Phản hồi cũ (kể cả `404` hoặc
  trang ẩn) không ghi đè phản hồi mới, kể cả khi đồng hồ có cùng giá trị. `validated_at` lấy lúc
  bắt đầu request; cleanup của keyed route owner hủy waiter và fence flight không còn observer.
- **Single-flight & dọn waiter (`WaiterGuard`):** view mount khi chưa có snapshot hiển thị được sẽ chờ
  request đang chạy (`Load::Join`), không tạo request thứ hai. Nếu view unmount/Future bị drop trước
  khi hoàn tất, `WaiterGuard` hủy đúng `WaiterId`; nếu `invalidate`/`withdraw`/`remove`/`clear` xảy ra
  lúc fetch khởi tạo đang chờ, waiter được khởi động lại ngay (khi còn hợp lệ) hoặc giải quyết ngay
  (`NotFound`/`Unavailable`), không bao giờ treo trạng thái bận/chờ vô hạn.
- **Key** = `PublicKey::Works(ExploreQuery)` gồm **`q`, `tags` chuẩn hóa và `cursor`** (Khám phá #100
  đã bỏ việc chỉ lọc local trong 12 card), hoặc `PublicKey::Reader(work_id)` cho trang đọc. Key và
  đọc công khai của READ-02 **không chứa actor/token/cookie** (công khai hoàn toàn không gọi Identity);
  mọi dữ liệu theo tài khoản như `/discussion/me` dùng vé generation tách biệt ngoài READ-02.
- **SSR:** store chỉ tồn tại trong bundle trình duyệt (`cfg(feature = "hydrate")`); server không có
  cache dùng chung giữa request/người dùng. HTML không-JS, status 404/422/503 và metadata giữ nguyên.
- Listener lifecycle cài một lần cho tab; view chỉ `observe`/`release` khi mount/unmount, nên điều
  hướng lặp lại không cộng dồn listener hay timer.

## Kiểm kê bề mặt đọc công khai, con trỏ, cache và vòng đời

Phân biệt rạch ròi bốn khái niệm dễ nhầm lẫn:
- **Con trỏ phân trang (`DiscoveryCursor` / `PageCursor`) vs sync watermark:** mọi cursor trong Vink là
  keyset cursor gắn chặt với một truy vấn/bộ lọc/ngữ cảnh bất biến (chống trượt trang/đổi phạm vi bằng
  `422 cursor`), **không** phải sync watermark đa thiết bị để đồng bộ delta.
- **Thư mục hữu hạn / top-K (`MAX_PAGE_SIZE = 24`, `MAX_LISTED_TAGS = 200`, `MAX_SUGGESTIONS = 10`) vs
  full-library mirror:** trình duyệt không tải toàn bộ thư viện hay đồng bộ bảng tìm kiếm offline.
- **Soft cap (`16` entry) vs ngân sách byte RAM (#237) vs disk quota (#203):** READ-02 giới hạn đồng thời `max_entries = 16` (`retain_unobserved = 5 phút`) ở `Store` và trọng số logic `READ_CACHE_BUDGET` (`max_total_bytes = 2 MiB`, `max_entry_bytes = 1 MiB` proxy UTF-8 + cấu trúc) ở `InMemoryCache`. Ngưỡng này tách biệt hoàn toàn với disk quota IndexedDB `4 MiB / 512 KiB` (`ENABLED=false`) và **không** bảo đảm trần RSS cấp tiến trình OS hoặc bộ nhớ GPU trình duyệt.
- **Runtime vs model:** **Top 100 (`TOP`)** và ranking/recommendations vẫn chỉ là model đích trong
  `services/vink-app/behavior/target` (`NOT_IMPLEMENTED` trong runtime/UI); **CMP/native** chưa có
  product owner hay network layer (`NOT_IMPLEMENTED`).

| Endpoint / Owner | Phạm vi actor & cache HTTP (READ-01) | Con trỏ / Giới hạn dữ liệu | Cache client (READ-02) & Key | Vòng đời & Hủy / Rút |
| --- | --- | --- | --- | --- |
| `GET /api/v1/works` | Công khai (`no Actor`); `200`/`304`: `ETag` strong + `private, no-cache`; lỗi `422`/`503`: `no-store` | `DiscoveryCursor` v2 (33 B: `published_at + work_id + SearchScope` SHA-256), **không** là sync watermark; `limit` mặc định `12`, tối đa `24`; tối đa `4` tag lọc (`8` ref) | **Có (Web pilot):** `PublicKey::Works(q, canonical_tags, cursor)`, stale 120 s, `max_display_age` 15 m, soft cap 16 | Card của tác phẩm bị `invalidate_withdrawn` được cắt khỏi snapshot & `seed` cũ hơn mốc rút, rồi xác nhận lại với origin; `422 cursor` → `#restart-public`; `422 q/tag/limit/query` → báo lỗi field, dừng retry |
| `GET /api/v1/works/{work_id}/publication` | Công khai (`no Actor`); `200`/`304`: `ETag` strong + `private, no-cache`; `404`/lỗi: `no-store` | Theo `work_id`, chỉ trỏ `revision_id` đang công khai hiện hành | API còn tồn tại; **không là endpoint của `PublicKey::Reader`** sau #202. `publication_resource` còn định nghĩa nhưng không có page caller tại lượt đối soát #233 | Không trả Hidden/ReviewWithdrawn. Không đồng nhất wire `Publication` với `ReaderResponse`; không dùng API này thay typed control read |
| `GET /api/v1/works/{work_id}/reader` | Công khai (`no Actor`); khi công khai dùng `ETag` READ-01 (`private, no-cache`), khi `ReviewWithdrawn`/`Hidden` (`200`) hoặc `404`/`503` trả `no-store` | Read model đơn `ReaderResponse` (`published` / `review_withdrawn` / `hidden`) | Điều hướng và kiểm trang thay thế dùng ticket `PublicKey::Reader(work_id)`; kết quả công khai cập nhật cache READ-02, trạng thái ẩn/gỡ không cache | Ticket hiện hành mới được trả vào UI hoặc thu hồi snapshot; `ReviewWithdrawn`/`Hidden` hiện notice + bình luận đủ điều kiện (`noindex`, không title/body) |
| `GET /api/v1/tags`, `/tags/suggestions`, `/tags/slugs/{slug}` | Công khai (`no Actor`); `Cache-Control: no-store`, chưa có `ETag` READ-01 | Bounded/top-K catalog: `MAX_LISTED_TAGS = 200` (+ `truncated`), `MAX_SUGGESTIONS = 10` | **Ngoài READ-02:** nạp theo lượt trang/gõ có debounce/hủy từ UI | Đổi/ngừng dùng thẻ phản ánh ngay ở lần gọi kế tiếp |
| `GET /api/v1/works/{work_id}/discussion/public`, `.../roots/{root_id}/replies` | Công khai (`no Actor`); `Cache-Control: no-store`, chưa có `ETag` | `PageCursor` v1 (`created_at ASC, id ASC`, `CursorScope::Thread`), trang mặc định `20`, tối đa `50`, preview `3` reply | **Ngoài READ-02:** không đưa vào `freshness::Store` | `404`/`409` khi tác phẩm gỡ/ẩn hoặc sai phạm vi cursor |
| `GET /api/v1/works/{work_id}/discussion/me` | Có auth (`Actor`, `401` cho khách, `503` khi Identity lỗi); `no-store` | `PageCursor` v1 (`CursorScope::Own`), batch `20` | **Ngoài READ-02:** `discussion_reads::OwnState` (generation-fenced ticket trong RAM, không cache liên phiên) | Tự reset khi `clear` session hoặc đổi tài khoản |
| `GET /api/v1/works/{work_id}/endorsements`, `/endorsements/me` (`PUT .../endorsement/me`) | Tổng hợp công khai (`no Actor`) và trạng thái riêng (`/endorsements/me`, có auth); `no-store` | Tóm tắt theo work/target | **Ngoài READ-02:** controller riêng của LVE; `409 revision_changed` báo READ-02 kiểm tra lại | Đọc riêng không trộn với cache công khai |
| Top 100 (`TOP`) | — | — | **NOT_IMPLEMENTED** (chỉ có model đích trong `services/vink-app/behavior/target`) | Chưa có API hay UI |

## Bằng chứng

### Phiên triển khai `921ac1c` (lịch sử)

Kết quả dưới đây do phiên triển khai trước review ghi nhận; kết quả bản sửa nằm ở mục kế tiếp.

| Kiểm | Lane | Kết quả |
| --- | --- | --- |
| `freshness::tests` (20): 119/120/180 s; đọc không gia hạn; spread có trần; một wake cho mọi entry; bốn trigger đồng thời → một ticket; manual/invalidate không bị snapshot fresh nuốt; `mark_stale` giữ nguyên `failures`/`retry_at` còn `mutation_confirmed` cho 1 lần thử ngay rồi nối lại backoff + `Retry-After`; lỗi giữ last-success; phân loại `422` (`cursor` vs `q`/`tag`/`limit`/`query` vs lỗi/envelope hỏng) dừng retry; 404 xóa và response muộn bị loại; `clear_removal`/`record_fresh` của bản trước review; reset scope; mốc từ lúc bắt đầu request; `max_display_age` chỉ chặn lần mount mới; GC + trần mềm 16 entry; entry không quan sát không bị poll; raw 304 → một lần không điều kiện; chuỗi ngẫu nhiên 300 × 80 bước | `quick` / `unit web` | PASS cục bộ |
| `contract` (`verify unit web --filter contract`, 20 bài, gồm `contract::tests::classifies_public_read_error_matrix_without_parsing_messages`) và `public_reads::tests` (3): ma trận `status + error.code + error.field` (không parse `error.message`, chặn >64 KiB); mutation ACK chỉ đánh thức owner đang hiển thị & gộp signal; rút bài xóa card ở mọi `Works` snapshot + rào nhân quả `begin_external_read` trên đồng hồ đóng băng và không chặn công khai lại; waiter khởi đầu luôn kết thúc khi invalidate/withdraw/remove/clear/cancel (ánh xạ SSR `422` kiểm end-to-end trong `verify web`) | `unit web` / `web` | PASS cục bộ |
| `cacheFirstPublicReads` (Chrome headless + CDP, đồng hồ `performance.now`, `visibilityState` và Fetch qua CDP) + toàn bộ `cargo xtask verify web` (8 phase) | `web` | PASS cục bộ |

`cacheFirstPublicReads` kiểm: hydration không gửi request list; lần ghé client đầu có loading và
một request; quay lại trong 120 s render ngay, **không** request, **không** loading; 119 s + focus
không request; 121 s + focus/online/visibilitychange/pageshow cùng lúc → **một** request có
`If-None-Match`, wire 304, giữ nguyên phần tử row và focus; tab ẩn không request khi focus/online,
hiện lại → đúng một request; `503` + `Retry-After` giữ danh sách và chặn `focus`/`online`/hiện lại
trước `retry_at`, phục hồi sau hạn không remount row; raw `304` thử lại tối đa 1 lần không điều
kiện và dừng ngay khi dính raw `304` kép; `422 cursor` dừng retry và phục hồi qua `#restart-public`;
`422 q` báo lỗi nhập liệu + `#clear-invalid-filters` không rơi vào vòng lặp reset cursor; `422`
hỏng dừng hữu hạn; reader nhận bản khác → thông báo, chữ cũ giữ nguyên, áp dụng khi bấm; `404` →
xóa chữ và bỏ card khỏi snapshot danh sách ngay, sau khi công khai lại vẫn đọc được `200` mới mà
không kẹt tombstone.

Đồng hồ giả chỉ dịch `performance.now()`; timer thật 120 s không được chờ trong smoke — đường
timer được kiểm ở unit (`next_wake`/`due`). Ẩn tab được giả lập bằng cách ghi đè
`document.visibilityState`, không phải chuyển tab hay khóa máy thật.

### Review và sửa tiếp trên `develop@36a7e47` (15/11/2026)

Review `921ac1c` và bản rebase `46c7e24` tái hiện lỗi xóa fence quá sớm: request `/reader` trước
lúc rút bài có thể ghi lại revision cũ sau invalidation của pointer riêng tư hoặc sau công khai lại.
Bản sửa dùng ticket chung, chặn cả kết quả Resource bị supersede và ghi tuổi từ lúc bắt đầu đọc.
Ba regression test tại `public_reads::tests` kiểm đảo thứ tự, clock đóng băng, invalidation,
request bị hủy cùng waiter và phản hồi `404` cũ. Mã Rust đã chốt tại `b8ddc87`, fixture trình duyệt
đã chốt tại `eab358f`; cập nhật sau đó trong tài liệu này không đổi source kiểm thử.

- `cargo xtask verify web --scenario main --scenario review --scenario publication`: phần `main`
  **PASS** (cache, SSR/hydration, `422`, raw `304`, focus/DOM, editor); toàn lệnh **FAIL** ở setup
  grant của `review`, nên `publication` **NOT_RUN** trong lệnh này. Runner nền chỉ seed principal
  reviewer trong `seed_discovery_fixture`, không phải khi chọn `review` riêng.
- `cargo xtask verify web --scenario discovery --scenario review --scenario publication`:
  **PASS** cả ba scenario. Fixture discovery bổ sung reviewer còn thiếu; không đổi runner hay
  bỏ assertion. Đây là tổng hợp bốn scenario đã kiểm, **không phải full lane 8 scenario**.
- Môi trường: macOS, Chrome headless, PostgreSQL tạm ở loopback `127.0.0.1:55439`, Identity thật
  với local outbox trong scenario publication. Dùng `CARGO_PROFILE_DEV_DEBUG=0`,
  `CARGO_PROFILE_TEST_DEBUG=0`, `CARGO_INCREMENTAL=0` để vừa dung lượng đĩa và cache npm tạm
  riêng; hai lỗi môi trường trước đó (quyền cache npm, hết đĩa) đã được xử lý trước các kết quả này.
- `CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0 cargo xtask verify quick`
  tại `558f63f` (chỉ thêm assertion cho kết quả test so với `eab358f`): **PASS** fmt, ba clippy
  server/browser/preview, 53 doctest và 747 unit/contract (gồm 210 Web, 80 xtask; 1 ignored).
  Toàn lệnh **FAIL** tại repository checks: `docs/development/test-vps-deployment-guide.md`
  có URL PostgreSQL mang password không thuộc danh sách placeholder. Blob tài liệu
  `4d8c6428e5add34b0580b17b541bf25f4dfe0455` và scanner `xtask/src/checks.rs`
  `35d9eb7e989710ccb0e0335a45282595223bd40e` giống hệt `origin/develop@36a7e47`;
  không phải hồi quy do PR này. Các lần chỉnh sau chỉ sửa tài liệu và đã kiểm diff/liên kết.

Review UI là **self-review**, không duyệt một visual baseline mới (`NO_APPROVED_REFERENCE`):

| Chiều | Kết quả và giới hạn |
| --- | --- |
| Functional | **PASS** các scenario và hồi quy cache nêu trên |
| Geometry | **PASS** assertion reflow của `main`, gồm recovery cursor ở 320px/chữ 200% |
| Visual | **PASS** trong hai ảnh đã mở `explore-refresh-failed.png`, `reader-update-available.png` (1280px, sáng); các trạng thái `422` chưa mở ảnh riêng (**NOT_RUN**) |
| Accessibility/Responsive | **PASS** kiểm keyboard/focus và ma trận responsive của `main`; screen reader và OS IME thật **NOT_RUN** |
| Interaction | **PASS** giữ row/focus khi `304`, retry/backoff, ẩn bài và phục hồi; thiết bị native **NOT_RUN** |

Bằng chứng PASS ở mục trước thuộc phiên triển khai `921ac1c`, không thay cho bản sửa này.

### Số đo

**Môi trường tham chiếu READ-02 pilot trước đó:** Apple M4, macOS 26.5.2, Chrome headless, server
debug của `verify web` trên loopback, dataset seed (1 tác phẩm công khai). Một lần chạy, không lặp
thống kê (không đo lại riêng bộ số microbenchmark và kích thước `wasm-release` ở lần sửa `#200`).

| Đại lượng | Trước (`develop@fab4f95`) | Sau (READ-02 pilot) |
| --- | --- | --- |
| Thời gian từ click tới row khi quay lại Khám phá trong 120 s | Luôn tải lại qua mạng + loading | 6–10 ms, **0 request**, không loading (lần ghé đầu qua mạng: 7–19 ms trên loopback) |
| Request list mỗi lần quay lại Khám phá trong 120 s | 1 | 0 |
| Request khi hiện lại/focus/online/pageshow cùng lúc lúc stale | — (không làm mới) | 1, có `If-None-Match`, wire 304 |
| Request khi tab ẩn | 0 | 0 (timer bị hủy) |
| Request định kỳ khi màn hiển thị | 0 | ≤ 1 mỗi `120–135 s` cho mỗi key đang hiển thị (từ policy; timer thật không được chờ trong smoke) |
| `vink_web.wasm` `wasm-release` trước wasm-bindgen | 12 800 367 B (gzip -9: 2 398 576) | 13 342 443 B (+4,2 %; gzip -9: 2 494 225, +4,0 %) |

Số mili-giây trên loopback chỉ cho thấy đường cache không chờ mạng, không phải latency thực tế.
Không đo bộ nhớ heap; trần mềm là 16 entry × kích thước một publication (cộng entry đang có waiter
hoặc quan sát). Truy vấn phía server cho một 304 không đổi so với READ-01 (vẫn một câu SQL).

### Ngân sách byte RAM trong READ-02 (#237, WEB-PERF-07 / CHOICE-01)

**Đối soát canonical, 18/11/2026.** Resource đang chờ mount và view đang quan sát có lifetime riêng. Hết cả hai owner, Store mới đưa snapshot vào admission. Rút bài chỉ sửa danh sách chứa card và giữ thứ tự eviction của các Reader khác. Gate thiết bị tắt bỏ qua bước dựng envelope. Với source hợp lệ 2 MiB, B giao đủ dữ liệu cho view, rồi refetch khi mở lại. [Review, số đo Chrome A/B/C và raw](evidence/web-perf-237/canonical-20261118/README.md).

Bản sửa từ base `origin/develop@afb4020` ([bằng chứng đầy đủ](evidence/web-perf-237/README.md)) đo và chọn **`WEB-PERF-07 CHOICE-01 = B`** trên 21 case corpus `#229` + baseline `#234` ở build `release`/`wasm-release`.

1. **Product decision:** giữ nguyên `OPEN` cho chính sách hiển thị cũ `max_display_age = 15 phút`, `24 giờ` offline và UX dọn cache; `O-17@r2` và `O-03@r2` giữ `PRODUCT_CONFIRMED`.
2. **Architecture decision:** `READ-01/02`, `DISC-WEB` và `WEB-PERF-07 CHOICE-01 = B` ở trạng thái **`DESIGN_PROPOSED`** (bài test xanh và PR không tự nâng lên `Accepted`). Không signoff `VDR-*`, không bật `public_persistence::ENABLED = false`, không đổi 15 phút/24 giờ.
3. **Implementation level:** **`web`** (`apps/vink-web/src/{read_cache,freshness,public_reads}.rs`): `InMemoryCache::with_budget(READ_CACHE_BUDGET, snapshot_weight_bytes)` với `max_total_bytes = 2 MiB` (`2_097_152 B`), `max_entry_bytes = 1 MiB` (`1_048_576 B`), eviction theo thứ tự `put` (entry được đưa vào cache sớm nhất rời trước); `Store::observe` rút bản sao khỏi cache khi đang quan sát, `Store::seed` bỏ clone thừa/xóa cache bị thay, `Store::edit`/`edit_all` trả bản sửa không quan sát về lại `SnapshotCache`, hủy waiter cuối fence ticket cũ trước khi nhận payload. `park_unobserved` là seam riêng; không suy test hủy waiter đã chạy nhánh `Applied::Fresh`. Rollback về `RetentionBudget::UNBOUNDED` ở đối số budget; `NoOpCache` cần đổi cả generic type của `Owner.coord` và initializer, rồi kiểm trên diff rollback.
4. **Verification evidence lịch sử của tác giả:** xem [bằng chứng #237](evidence/web-perf-237/README.md) và [review độc lập/các đính chính](evidence/web-perf-237/review.md). Các kết quả tác giả khai báo trên cây trước commit: `verify quick` (`PASS`), `vink-web --lib --features ssr` (`240 passed; 0 failed; 1 ignored`), `verify unit web --filter public_reads` (`14 passed; 0 failed; 1 ignored`), đối chứng âm bỏ removal fence (`FAIL` có chủ đích ở [negative-control.log](evidence/web-perf-237/negative-control.log) rồi `PASS` `1/1`), `decision validate/refs` (`PASS`, `0` problems), và `verify web --scenario main --scenario discovery --scenario review --scenario perf-baseline --scenario return-position --release` (`PASS` cả 5 scenario trên Chrome/PG scratch, ngoài `local-budget` vì browser `UNSUPPORTED`). Corpus/probe A/B/C (`--release`) chạy trước bước inject/khôi phục đối chứng âm trên cùng logic Option B; `storage`, `restart`, `endorsement`, `auth`, `password`, `publication`, `verify discussion/kani/postgres/expressions` và CMP là `NOT_RUN`.
   - **Tách biến & giới hạn:** `snapshot_weight_bytes` chỉ tính proxy UTF-8 + container cấu trúc của snapshot không quan sát; không phải trần RSS OS hay bộ nhớ GPU. `INV-01/07/08`: không đổi rule hay mô hình M1/M3/M4 hoặc D2T; `apps/vink-web/behavior/read-lifecycle.qnt` chỉ phủ `ReadOwner`/`PageChain` nên ranh giới `Store`/`Coordinator` được kiểm bằng production unit conformance, race matrix và đối chứng âm bỏ fence.

## CMP và native

**NOT_IMPLEMENTED / NOT_RUN.** Không có product app hay product owner cho CMP; `packages/ui-system/cmp`
và các diagnostic host không có network owner; không thêm Ktor hay scheduler vào UI/pattern packages
(phải DTO-free). Khi có product app, hợp đồng hành vi cần giữ (conformance, không phải thư viện
dùng chung):

- Một owner/query key, một owner validator (Ktor `HttpCache` ứng với phương án A).
- `stale_after` ≠ retention ≠ max display age; đọc không gia hạn; mốc từ lúc bắt đầu request.
- Lỗi tạm giữ last-success với trạng thái riêng; 404 xóa ngay; không resurrect sau
  invalidate/reset scope; một request/key; nền/ẩn không polling.
- Các case trong `freshness::tests` là vector tham chiếu cho test native; diagnostic host compile
  được không phải bằng chứng HTTP/lifecycle native.

Chưa chọn giữa Kotlin repository + StateFlow + Ktor (A của issue) và Rust core qua FFI (B).

## Rút lại

- Về GET 200 thường mà vẫn giữ SWR: `RequestCache::Reload` trong `api::http::public`.
- Giảm retention sau #202: dùng seam `NoOpCache` và giữ `Coordinator`/`freshness::Store`,
  typed `/reader`, ticket/removal fence cùng lifecycle. Đây là phương án rollback cần kiểm
  conformance và browser trên diff thực tế, không phải công tắc runtime đang có trên UI.
- Không quay lại recipe trước #202 gọi transport trực tiếp từ `publication_resource` rồi
  bỏ owner: cách đó bỏ cả fence đã bảo vệ Resource. Muốn tắt SWR phải review một thay đổi riêng
  ở `works_resource`/`reader_resource`, `PublicResults`/`ReaderLoaded` và kiểm lại quyền đọc.
  #203 đang tắt; nếu rollout sau này đã ghi dữ liệu, rollback còn phải xử lý policy durable clear.

## Chưa làm / rủi ro còn lại

- Đã nối `invalidate_publication` / `invalidate_withdrawn` từ tác giả (`pages/publication.rs`) và áp dụng thu hồi trên đường đọc `/reader` (`public_reads::Coordinator::finish` khi `api::reader_resource` nhận `Control`/`404` cho `ReaderLoaded` trong `pages/mod.rs`). Chưa có caller invalidation từ `pages/moderation.rs` hoặc caller `clear()` từ `session.rs`/`account`. Rủi ro còn lại là **tab khác hoặc thiết bị khác** chưa nhận push/broadcast liên tab ngay lúc rút bài (vẫn chờ revalidate hoặc lần mở tiếp theo).
- Giá trị giả định `max_display_age = 15 phút` (và chính sách hiển thị stale khi lỗi tạm / offline lâu hơn) vẫn là mục **`OPEN`** chờ chủ sản phẩm chốt; `READ-01-CHOICE-*` và `READ-02-CHOICE-*` vẫn là `DESIGN_PROPOSED`.
- IndexedDB đã có adapter và kiểm thử dưới gate **tắt** ([#203](#indexeddb-203)); offline
  full-text qua restart/App Shell/Service Worker chưa bật, thuộc #204. SQLite, Top 100 runtime
  và CMP/native product network owner vẫn **`NOT_IMPLEMENTED`**.
- Spike `leptos-fetch` chưa chạy trong Vink; kết luận dựa trên tài liệu + source 0.4.10.
- Kiểm trình duyệt chạy trên Chrome headless cục bộ với đồng hồ `performance.now`, `visibilityState` và fault injection Fetch qua CDP; chưa đo trên thiết bị di động thật, mạng chậm hay suspend/resume thật.
- Chưa có Quint model riêng cho `freshness::Store`/`Coordinator`; bằng chứng hành vi là unit + chuỗi ngẫu nhiên có xác định + test coordinator + E2E.

## Lifecycle Web #202

Proposal kỹ thuật: [VDR-WEB-READ-LIFECYCLE@r1](../../decisions/proposals/VDR-WEB-READ-LIFECYCLE.r1.toml), **PENDING**; không thay active READ-02 đã nhập. [Model canonical và replay](../../apps/vink-web/behavior/README.md) chỉ kiểm `ReadOwner/PageChain`, không kiểm freshness/storage/authority của adapter.

Lát #202 trên `develop@ebbde221` (dependency #200/#210). Giữ trạng thái sản phẩm và
kiến trúc ở đầu tài liệu; không chốt retention/offline mới. Mức triển khai là `web`, không có
native, Service Worker hay replay mutation trong #202. IndexedDB được bổ sung sau đó ở [#203](#indexeddb-203), dưới gate tắt.

### Contract và lựa chọn kỹ thuật

- **#202 CHOICE-01 A:** `read_lifecycle::{ReadOwner, ReadTicket, PageChain}` là primitive thuần;
  owner giữ DTO, cách gọi transport và authority. `RequestId` duy nhất trong tiến trình/tab,
  scope + generation + input cursor nằm trong ticket. Đổi scope, dispose và supersede append
  làm ticket cũ vô hiệu, kể cả A → B → A hoặc đổi phiên rồi quay về cùng principal.
- **#202 CHOICE-02 A:** key `PublicKey::Reader(work_id)` đọc `/reader` cả lúc mở và refresh.
  Snapshot Published đã được origin xác nhận được giữ trong cửa sổ v1, mở lại không cần chờ
  `/reader` thành công. `CachedUnverified` chỉ là provenance của lần reuse; không cấp quyền.
  Hidden/ReviewWithdrawn là live control fact trong owner, không đưa vào snapshot retention;
  404 là absence, không dùng thay hai control state. Cả ba rút body/title/tags và card ngay;
  ticket/seed cũ không hồi sinh. Publication mới chỉ vào qua response authoritative mới.
- **#202 CHOICE-03 A:** head refresh thành công thay toàn bộ chain, bỏ tail và continuation
  cũ. Trong lúc chờ/lỗi giữ data và cursor đã chấp nhận. Append chỉ cập nhật cursor sau khi
  scope/ticket/input cursor và page hợp lệ; ID overlap/duplicate/cursor cycle bị từ chối,
  không dedup để che thiếu trang. Refresh đang bay gộp signal; refresh thay append đang bay.
- `read_cache::SnapshotCache` chỉ `get/put/remove/clear`; InMemory giữ snapshot không còn
  observer, NoOp bỏ retention. `freshness::Store` giữ active data, validation age, backoff,
  rejection, removal fence và flight bên ngoài cache. Cache lỗi/miss không tạo authority.
- `read_events` có một bộ listener visibility/focus/online/pageshow; subscription riêng chỉ
  tồn tại theo owner đang mount. Public giữ một timer sớm nhất; no-store Desk/discussion chỉ
  nhận lifecycle hints, không thêm timer theo component. `WakePolicy` giữ deadline/backoff;
  reconnect không đọc từ cursor cuối và không replay POST/PUT/DELETE.

### Conformance theo owner

Bảng consumer được đối soát lại tại `69f71f0` trong #233; evidence của #202 ở mục riêng bên dưới
giữ nguyên SHA và phạm vi lần chạy, không bao phủ tự động các consumer merge sau đó.

| Owner thật | Initial / refresh / append / restart | Fence và dữ liệu | Phạm vi tiếp theo |
| --- | --- | --- | --- |
| Discovery (`api::works_resource`, `DiscoveryScope`, `public_reads`) | SSR seed hoặc cache/network; refresh cùng URL page; next/restart bằng link GET có query | Key gồm representation + normalized q/tags/cursor; scope view keyed; Resource waiter hủy theo owner; Store ticket + typed 422 | Không chuyển infinite scroll; limit cố định hiện hành nằm trong endpoint |
| Reader (`ReaderScope`, `ReaderLoaded`) | SSR in-process; browser cache-first `/reader`; refresh giữ bản đang đọc, apply rõ bằng nút | Keyed route owner; một validator cho Memory/NoOp; control no-store thay body; Published đổi chỉ báo bản mới | Không mở rộng cửa sổ 15 phút hoặc giữ full-text qua restart |
| Public discussion (`ViewState`) | SSR public; lifecycle/manual refresh head; roots/replies continuation; refresh bỏ tail | `ReadOwner<Scope>`/`PageChain`, public epoch và mutation guard; stable composers, public DTO | Không cache discussion |
| Own discussion (`OwnState`) | Browser only; head rồi lookup bounded; cursor riêng | Session scope + ticket; page accept trước cursor; lookup thắng listing; 401/403/404 bỏ private facts | Không authority từ public cache |
| Desk list (`DeskIndex`) | Browser only, scoped server query; reconnect head giữ last success, append/đổi phần có giới hạn | `ReadOwner` scope session/query/sort/limit, `DraftPages` dùng `PageChain`; WakePolicy; route disposal; editor độc lập | [#201](desk-pagination.md) sở hữu paging; sau #221/#222 Web đọc `/author/works`, bỏ resume/Recent/1; [owner hiện hành](desk-author-feedback.md) |
| Moderation (`pages/moderation.rs`) | Hai `QueuePanel` work/appeals đã dùng ReadOwner/PageChain: head/append/restart, refresh supersede append và lifecycle wake | Session/dispose fence, authority fingerprint đổi xóa chain; 401/403/404 xóa private facts; detail/command có guard riêng | [#205/#223](moderation-runtime.md#10-hai-queue-hữu-hạn-205) đã triển khai; case detail/author history không được suy là đã paging |

### Seam cho #201 / #205

Scope phía private chứa representation, query/filter/sort/limit và `Session::scope()`; không
chứa cookie/token. Scope đổi gọi `change_scope`, xóa private list/chain của scope cũ ngay.
Owner dispose gọi `dispose`; cancellation transport chỉ best effort. Ví dụ adapter Bàn viết:

```rust,ignore
let ticket = reads.begin(Intent::Append, chain.next_cursor.clone())?;
let page = api::search_drafts(query, sort, limit, ticket.cursor.as_deref()).await?;
// Kiểm DTO đúng endpoint/scope trước accept_page. Không advance cursor từ lỗi transport.
if reads.accept_page(&ticket, &mut chain,
    page.drafts.iter().map(|d| d.work_id.clone()), page.next_cursor.clone()) {
    // Append: extend rows. Initial/Refresh/Restart: replace rows và bỏ tail cũ.
    if ticket.intent == Intent::Append {
        rows.extend(page.drafts);
    } else {
        rows = page.drafts;
    }
}
reads.finish(&ticket); // chỉ settle đúng request; fail/cancel giữ chain và rows
```

`PageChain::generation` tăng khi head được chấp nhận; `next_cursor` chỉ là cursor của chain
đã chấp nhận. `Intent::Restart` là head request giữ query/filter. UI continuation của #201/#205
phải báo tail đã được tải lại khi refresh head, không nối snapshot cũ với head mới.

### LayoutSpec delta

Giữ Quiet Lavender/shared controls và trang URL của Khám phá. First load có loading thật;
refresh giữ list/article, status riêng; lỗi append cạnh continuation và giữ cursor; restart giữ
filter. Reader cache có nhãn “Đang đọc bản đã tải trước đó.”, lỗi verify có “Thử lại”; chỉ
“Xem bản mới” thay chữ và focus đầu bài. Control Hidden/ReviewWithdrawn giữ replacement hiện có.
Touch/pointer/Enter/Space dùng nút/link native; không gesture/ESC handler mới. Signal nền không
đặt focus, remount editor/composer hoặc ghi đè composition. Reduced motion giữ token hiện hành.
Reference là source regression baseline, `NO_APPROVED_REFERENCE` cho fidelity mới; kiểm riêng
390/1440 × chữ100/200 (root font-size, không browser zoom), keyboard/composition mô phỏng.

### Evidence local và nghiệm thu #202 — 15/11/2026

Worktree `/private/tmp/vink-issue202`, base `ebbde221583360b3b059b00aca696ad68b0595f8`.
Mã Rust/UI tại `1855526ab2d8448c20c3f61285586e4cd206e1a4`; các commit sau chỉ điều chỉnh
fixture browser và ghi evidence. Browser discovery/review tại `e53d146`, auth/publication
tại `63bc070`, main cuối tại `86eb145` + diff evidence docs. Không đọc, chờ hay gọi lại GitHub CI.

Môi trường: macOS, Chrome headless/CDP, Rust 1.99.1, wasm-bindgen 0.2.129, Node 22.15.0;
PostgreSQL loopback `127.0.0.1:55439`, App và Identity scratch do runner sở hữu. Các lệnh cargo
dùng `CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0`. Browser còn
dùng `VINK_CHROME` trỏ Chrome cục bộ, `VINK_TEST_DATABASE_URL` trỏ cluster trên và
`npm_config_cache=/private/tmp/vink-issue202-npm-cache`. Không dừng cluster được chia sẻ.

| Command / đầu vào | Kết quả và boundary |
| --- | --- |
| `cargo test -p vink-web --lib --features ssr` | **PASS**, 215 unit/contract; gồm cùng suite Memory/NoOp, freshness, page chain và private/session guards. Log `target/issue202/unit-web-final.log`; mã Rust trùng `1855526`. |
| `cargo xtask verify unit app --filter review_withdrawn` | **PASS**, chọn đúng 2 test domain/application, không DB. Log `unit-app-final.log`. |
| `rustc --edition=2024 --test apps/vink-web/src/read_lifecycle.rs` rồi chạy binary | **PASS**, 2 test. Mutant riêng trong `target/issue202` thay allocator bằng `RequestId(1)` **FAIL đúng dự kiến** (exit 101, ABA/remount); source thật không bị sửa. Logs `lifecycle-final.log`, `negative-control-final.log`. |
| `cargo xtask verify web --scenario main --scenario discovery --scenario review --scenario publication --scenario auth` tại `e53d146` | **PASS các bước main, discovery, review**, gồm HTTP/SSR và PostgreSQL thật; toàn command **FAIL** ở auth vì assertion cũ đòi B chờ response A, publication **NOT_RUN** trong lần này. Log `web-final-5.log`, capture `target/verify-web/1794736668693159000-67229`. |
| `cargo xtask verify web --scenario auth --scenario publication` tại `63bc070` | **PASS cả command** với Identity/App/PostgreSQL thật và local mail outbox. Auth giữ response A, chứng minh B hoàn tất trước khi nhả A; giữ riêng own/public intent để kiểm cả hai response muộn. Log `web-auth-publication-2.log`, capture `target/verify-web/1794737310681252000-80178`. |
| `cargo xtask verify web --scenario main` tại `86eb145` + evidence docs | **PASS cả command**, thêm đo anchor đoạn văn ở scroll 265px/top 80,375px: giữ vị trí nhìn, article node và selection sau refresh. Log `web-main-repeat.log`, capture `target/verify-web/1794738228853075000-97295`. |
| `cargo xtask verify quick` trên diff cuối (`63bc070` + evidence docs; trong lúc chạy chỉ thêm assertion browser ở `368a3ac`) | **PASS** tokens, manifest, fmt, cả 3 clippy, doctests, unit/contract; toàn command **FAIL** ở đúng 1 repository check có sẵn: password placeholder trong `test-vps-deployment-guide.md`. Log `quick-final.log`. |

Tổng hợp trên bao phủ **5 scenario được chọn**, không phải full Web lane; `restart`, `endorsement`,
`password` **NOT_SELECTED**. Không đổi SQL/transaction; kiểm provider PostgreSQL thuộc các scenario
trên, không gọi đó là full `verify postgres`. Các log nằm trong `target/issue202/`.

Baseline failure của quick được đối chiếu với `origin/develop@ebbde221`: blob tài liệu
`4d8c6428e5add34b0580b17b541bf25f4dfe0455` và scanner `xtask/src/checks.rs`
`35d9eb7e989710ccb0e0335a45282595223bd40e` giống hệt; không sửa file ngoài phạm vi để làm xanh lane.

Các lần chạy sửa soạn còn lưu: scoped `verify unit web` không bật `ssr` vấp 4 test render SSR
(đã dùng lệnh có feature và workspace quick); một lần Web bị resource lock khi unit lane còn
giữ artifact; các lần browser trước vấp fixture cộng đồng hồ vượt retention và selector sai.
Đã sửa bước chuẩn bị, không tăng retention hay bỏ assertion thu hồi. Auth cũ giả định đọc tuần
tự và một nút khởi động đồng thời own/public; test mới giữ nguyên payload thật và kiểm response
đảo thứ tự qua các intent thực tế. Không dùng những lần FAIL làm bằng chứng PASS toàn lane.
Lần đầu chuẩn bị anchor chưa chờ layout đủ để cuộn; bản cuối chờ composer và đo vị trí nhìn
thực tế. `web-anchor-final-2.log` đã qua anchor nhưng **FAIL** ở assertion caret editor khi thu
viewport mobile; chạy lại **nguyên source và command**, không sửa/bỏ assertion, được **PASS**
trong `web-main-repeat.log`. Đây là một failure không tái hiện, chưa xác định nguyên nhân;
không claim đã sửa một lỗi editor.

| Acceptance | Source / bằng chứng / kết luận |
| --- | --- |
| AC-01 | `ReadOwner`/`RequestId`, keyed `ReaderScope`, waiter cleanup; Memory/NoOp + negative control + main ABA + auth session/late response: **PASS**. |
| AC-02 | `PageChain`, roots/own/replies; duplicate/cycle/cursor rejection trước commit, head thay tail; unit và main fail/retry cùng cursor + restart giữ draft: **PASS**. |
| AC-03 | `read_events`, `WakePolicy`, coordinator; unit burst/timer/backoff, main public/Desk burst một GET, không mutation, hidden pause: **PASS** trên fake clock/CDP. |
| AC-04 | Typed Reader cache entry; main held `/reader`, 503 và browser offline vẫn mở snapshot; unit empty/expired mount không dùng snapshot: **PASS** trong policy v1. Không offline sau restart. |
| AC-05 | Một validator cho Published/control/404; unit late/seed + main Hidden/404/republish + review/publication thật kiểm replacement, metadata, no-store, comments và pointer: **PASS**. |
| AC-06 | `memory_and_noop_share_authority_error_scope_and_single_flight_contract` chạy cùng contract, cache-hit khác biệt được kiểm riêng; **PASS**. |
| AC-07 | Main/discovery SSR, GET search, link context, 304; review noindex/no-store; auth/publication không SSR private facts; seed/request fencing: **PASS**. |
| AC-08 | Main giữ article node/selection/scroll, composer node/focus/chữ Việt khi reconnect; Desk search giữ focus; auth/publication giữ chữ cùng người và xóa dữ liệu khi đổi người: **PASS Chrome/CDP**. OS IME thật **NOT_RUN**. |
| AC-09 | Conformance map và seam ở trên, ghi rõ #201/#205; **PASS** kiểm source/diff. IndexedDB/SW và CMP/native chưa triển khai trong lát này. |

Self-review source/contract không còn finding cần sửa trong phạm vi #202; không phải review độc lập.
Không đổi product signoff, `DESIGN_PROPOSED`, D-05 `Provisional` hoặc policy v1 còn `OPEN`.

| Chiều UX | Kết quả và giới hạn |
| --- | --- |
| Functional | **PASS** 5 scenario được tổng hợp, gồm race, error, control và private isolation. |
| Geometry | **PASS** assertion 390/1440 × chữ100/200 của Reader cache; chain restart 390/200 và các ma trận main hiện hữu. |
| Visual | **PASS** self-review các ảnh đã mở ở capture main: `reader-cached-unverified`, đủ 4 `reader-cache-{390,1440}-{100,200}`, `discussion-chain-restarted` và `discussion-chain-restarted-390-200` (sáng). State mới ở theme tối chưa mở riêng: **NOT_RUN**; không claim approved fidelity. |
| Accessibility/Responsive | **PASS** keyboard/focus và reflow trong main; screen reader, mobile thật và OS IME thật **NOT_RUN**. |
| Interaction | **PASS** explicit apply bản mới, reconnect không đổi selection/composition, failed append không advance cursor, restart giữ bản nháp; CDP không chứng minh suspend/resume thiết bị thật. |


## IndexedDB #203

Proposal kiến trúc [VDR-PUBLIC-SNAPSHOT-STORAGE@r1](../../decisions/proposals/VDR-PUBLIC-SNAPSHOT-STORAGE.r1.toml) **PENDING** ghi lại envelope/epoch/quota; độc lập với proposal sản phẩm 24 giờ và không bật gate.

**Phạm vi giao:** [#203](https://github.com/loveoverflowcom/vink/issues/203), một PR vào `develop`,
base `f22815ec387bc02792a3cac44899152ff9f1f031` (đã có #202/#201). Không đổi API server.

- **Sản phẩm:** chủ sản phẩm chọn **24 giờ** và “hết hạn sẽ có tín hiệu riêng để xử lý sau” trong
  chat ngày 15/11/2026; nội dung ghi trong
  [VDR-PUBLIC-OFFLINE-AGE@r1](../../decisions/proposals/VDR-PUBLIC-OFFLINE-AGE.r1.toml),
  `PENDING`, SHA `72bb815fc75137819c252cc7cc632a17503389d7bbd0e9fbcf866f3c4900d2aa`.
  Chưa có exact-revision signoff; không sửa record READ-02 đã nhập. POLICY-02 logout/account switch
  còn **OPEN**; chủ sản phẩm yêu cầu giữ rollout tắt. Cách xử lý UX khi đang đọc mà hết hạn chưa chốt.
- **Kiến trúc:** CHOICE-01 A và CHOICE-02 A là lựa chọn kỹ thuật trong #203, `DESIGN_PROPOSED`.
  Query tại base giữ READ-02-CHOICE-01..04@r1 và READ-01-CHOICE-02@r1 `DESIGN_PROPOSED`,
  O-17@r2 `PRODUCT_CONFIRMED`; registry snapshot
  `f2f2e49cf0c079e71729722841b8e2f50b5d714dc8a87f209700dd47c0e4c1a8` trước proposal mới.
- **Triển khai:** `web` adapter và seam owner đã có; `public_persistence::ENABLED = false` và
  display policy `None`. App bình thường **không mở/ghi/restore IndexedDB**. Memory pilot hiện hành
  giữ semantics v1; 24 giờ chưa thay ngưỡng 15 phút của memory pilot. CMP/native và cold offline
  launch vẫn `NOT_IMPLEMENTED`; PR 4 không thay PR 5.
- **Bằng chứng:** unit owner/envelope và Chrome IndexedDB được tách ở bảng AC dưới đây; không có
  claim production rollout hoặc offline UI E2E khi gate còn tắt.

### Chủ sở hữu và giao thức

[`public_persistence.rs`](../../apps/vink-web/src/public_persistence.rs) chỉ xây `Envelope` từ
`Snapshot::Works(WorkList)` hoặc `Snapshot::Publication(Publication)` đã được origin chấp nhận.
`Control(Hidden/ReviewWithdrawn)`, SSR seed, private DTO, auth/capability, discussion, nháp, own,
pending/rejected và moderation không có đường persist. Field envelope đóng, nested DTO được kiểm
roundtrip; JS kiểm strict allowlist đệ quy trước write và khi đọc. Record tự khai provenance không
chứng thực authority: mọi restore chỉ có thể là `CachedUnverified`.

[`indexeddb.js`](../../apps/vink-web/public-cache/indexeddb.js) chứa I/O và bounded queue; không fetch,
không quyết retry, không quản ETag. HTTP cache vẫn là owner validator duy nhất. API async
`capture → read/restore/write/current` và `barrier(work | null)` độc lập với seam đồng bộ
`SnapshotCache`; không giả `put()` là lưu bền. `restore` trả `hit`, `expired`, `miss`, `unavailable`;
`expired` không kèm body/cursor. Kiểm lease phân biệt `true` (còn hiệu lực), `false` (barrier/known versionchange),
và `null` (storage không khả dụng): lỗi storage không rút RAM hoặc bỏ response online; restore
không có xác nhận lease thì bị bỏ. Đây là tín hiệu cho consumer UX tương lai, chưa có màn xử lý hết hạn.

`Store::begin_restore/restore` dùng RequestId và hàng rào cùng owner hiện hữu: SSR seed, network
completion, scope invalidation, remove/clear/release thắng restore cũ. Restore không đổi flight,
retry/backoff hoặc gia hạn ngày xác nhận. Barrier storage làm rỗng RAM, giữ số observer và chặn SSR
seed trước barrier; lifecycle resume/next load kiểm metadata để nhận ra broadcast bị mất. Không có
cam kết rút chữ tức thì ở một tab đang bị suspend hoặc mất broadcast trước khi nó chạy lại/check.

### CHOICE-01 A — envelope đầy đủ cho từng query/page

Một row chứa schema `1`, origin chính xác, policy `public-snapshot-v1`, PublicKey (query, tags,
input cursor hoặc work ID), provenance `origin_response`, wall-clock `validated_at_ms`, và payload
đầy đủ (bao gồm `next_cursor`, filters, tags, revision/document). Không entity store hay reference
rời; xóa/corruption/eviction loại cả trang, không phục hồi cursor thiếu data. Thẻ/body giữ Unicode.
Đổi lại, card và publication có thể lặp giữa trang; hard budget đo trên JSON UTF-8 kể cả wrapper
row, không dùng kích thước engine IndexedDB như một số đo có thể bảo đảm.

### CHOICE-02 A — epoch trong transaction

Database riêng `vink-public-snapshots`, IDB version `2`, stores `meta` + `snapshots`.
Metadata chứa random `incarnation`, `epoch`, `sequence`, high-water wall time. `capture` cấp lease
**trước** request/restore. Mọi operation kiểm lease trong cùng readwrite transaction với dữ liệu:

- `clear` tăng epoch và xóa toàn public snapshots. `purge(work)` cũng tăng epoch, xóa publication
  và **toàn trang** có card của work; giữ các row không liên quan. Epoch toàn cache vô hiệu mọi
  writer/restore trước barrier, tránh ledger tombstone tăng vô hạn. Đánh đổi: request của work
  không liên quan cũng phải lấy lease mới trước lần đọc origin tiếp theo.
- Sequence ngăn response cũ ghi đè row đã được request mới ghi; queue không coalesce ngược một
  ticket mới thành cũ. Database bị browser xóa rồi tạo lại nhận incarnation mới, nên epoch 0
  không khớp ticket epoch 0 trước eviction. Metadata hỏng không được tự reset thành epoch 0.
- Chỉ `transaction.oncomplete` ACK bền (yêu cầu `durability: strict`); abort sau request success,
  quota, exception, timeout không ACK. Engine/browser không hỗ trợ option transaction sẽ fallback.
  Không có chứng minh chống mất điện ở phần cứng.
- BroadcastChannel chỉ là tín hiệu để owner đọc durable metadata. Hai tab không nghe broadcast
  vẫn bị transaction fence chặn write/restore cũ ở boundary tiếp theo. Lỗi durable clear không
  được báo thành công; UI clear tương lai phải xử lý `false`, không hứa đã xóa qua restart.

Quy tắc transaction/upgrade được đối chiếu với
[IndexedDB specification](https://w3c.github.io/IndexedDB/#transaction-lifecycle).

### Giới hạn, tuổi, lỗi và migration

| Trường hợp | Hành vi |
| --- | --- |
| Budget | Tối đa **32 row / 4 MiB logical UTF-8**, mỗi envelope ≤ **512 KiB**; loại row có thời điểm xác nhận cũ nhất, tie-break key. Đây là retention, không phải quyền hiển thị |
| Queue | Tối đa **8 pending + 1 executing**, coalesce cùng key, trần nội dung khoảng 4.5 MiB; overflow trả false, không chặn mạng/editor |
| Tuổi | Thời điểm tường lấy trước origin request; không ghi `performance.now`. Cache read không đổi `validated_at_ms`; tuổi ≥ 24 giờ trả expired khi còn record rồi bỏ toàn row. GC đã bỏ trước đó thì miss |
| Đồng hồ | Timestamp không rõ/future/ngoài safe integer bị từ chối; rollback dưới high-water durable tăng epoch, xóa row và trả miss/failure. Không thể chứng minh tuổi thật nếu clock bị chỉnh mà chưa có high-water hoặc origin xác nhận; restore vẫn unverified và rollout tắt |
| Đang observe | Eviction storage không rút active RAM; freshness owner vẫn quyết lifecycle. Chỉ authoritative removal/clear rút dữ liệu. UX expiry của nội dung đang mở là việc còn lại |
| Open denied/blocked/error | Deadline 1.5 s, không treo; instance ngừng thử open sau failure. Request open tới muộn bị close hoặc abort upgrade, không reset vòng lặp |
| Abort/quota/corruption | Không ACK, RAM/network vẫn dùng owner hiện hữu. Corrupt/không đầy đủ/unknown envelope schema bị bỏ toàn row. Không log payload |
| Version 1 → 2 | Discard migration nguyên tử chỉ stores public `meta`/`snapshots` trong database riêng. Giữ store ngoài hai tên này; không xóa localStorage, CacheStorage, editor hoặc auth storage |
| Upgrade/versionchange | Connection cũ đóng và instance disabled; app schema cũ gặp DB version mới hơn fallback, không downgrade/reset. Upgrade abort giữ schema cũ; blocked open không tự delete database |
| NoOp/Memory | Cùng transition suite, nhưng **không bao giờ ACK durable write**. Runtime RAM là Store hiện hành; adapter lỗi không chuyển quyền/retry sang storage |

### AC và bằng chứng #203

Bộ browser [`public-storage-smoke.mjs`](../../apps/vink-web/e2e/public-storage-smoke.mjs) chạy cùng
harness và app scratch của `cargo xtask verify web --scenario storage`; cũng nằm trong `main`.
Không chạy hai lần khi chọn cả `main` và `storage`. Fixture toàn bộ tổng hợp, không thu bản thảo.

| AC | Owner/assertion | Kết quả / giới hạn |
| --- | --- | --- |
| AC-01 | Envelope Rust + strict JS; reject nested token/private/control, origin/key mismatch; hydration bình thường không mở IDB | **PASS** unit/contract + Chrome `storage`; chỉ hai public representations, production gate tắt |
| AC-02 | Whole-page transaction; abort thật sau request success giữ data/cursor trước đó và không ACK | **PASS** unit decode + transaction IndexedDB thật; ACK chỉ sau `oncomplete` |
| AC-03 | Chrome quota override thật; injection `IDBFactory.open` ném SecurityError; corruption, blocked/versionchange/eviction | **PASS** Chrome `storage`; denial là API fault injection trong Chrome, không phải cấu hình policy OS; lỗi lease trả unavailable, không thu hồi online snapshot như một barrier |
| AC-04 | Hai tab không BroadcastChannel, A frozen → B purge/clear → A active; old write/restore lease bị từ chối | **PASS** unit owner + Chrome `storage`; chỉ boundary chạy lại, không hứa immediate revocation offline |
| AC-05 | Rust restore vs SSR/network/removal/clear, wall age/rollback; kill Chrome và mở lại cùng profile | **PASS** unit owner/age + Chrome `storage`; giữ nguyên envelope và `validated_at_ms` qua process restart |
| AC-06 | Corpus 40 public envelopes; logical bytes/rows; queue overflow/coalesce; whole-page deletion | **PASS** Chrome `storage`: corpus nhỏ còn **32 row**; corpus lớn còn **18 row / 4.059.846 byte logical UTF-8**, dưới 4 MiB; queue tối đa 8 pending + 1 executing |
| AC-07 | V1→V2 discard; abort upgrade rollback; blocked old connection; V3 làm old app đóng và fallback; sentinel namespace khác giữ nguyên | **PASS** Chrome `storage`; không downgrade/reset DB version mới hoặc xóa storage riêng |
| AC-08 | Proposal 24h/tín hiệu expired đã ghi; POLICY-02 OPEN; ENABLED false, display policy None | **OPEN rollout**; chưa signoff revision, chưa UX expiry/logout |
| AC-09 | Shared JS transitions Memory/NoOp/IndexedDB; Rust owner suite Memory/NoOp + browser IDB + build hydrate | **PASS** tại boundary adapter/owner; chưa bật composition persistent offline UI, chưa SW/cold launch |

Không đổi visual/interaction UI; không thêm LayoutSpec hoặc CTA. Bằng chứng main là regression
UI hiện hữu; không coi ảnh source baseline là approved reference. Không chạy GitHub CI.

### Lần chạy cuối #203 — 15/11/2026

Source triển khai chốt tại `4e68bd861b8b2a1bc090b3d6bf975597a1c227e7`, sau commit adapter
`92d6dcbcb64e813d38ebc64c8e11ff3b1ffb6000`; cập nhật evidence sau đó chỉ sửa tài liệu.
CWD `/Users/manhblue/projects/loveoverflow/vink`. Môi trường macOS, Rust 1.99.1,
wasm-bindgen 0.2.129, Node 22.15.0, Chrome 157.0.8103.78, PostgreSQL 17.11 Homebrew.
Mọi lệnh cargo dùng `CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0`.
Lệnh Web còn dùng `VINK_CHROME='/Applications/Google Chrome.app/Contents/MacOS/Google Chrome'`
và `VINK_TEST_DATABASE_URL='postgres://vink@127.0.0.1:55441/postgres'`, cluster local tạm của
phiên này; runner sở hữu App scratch. Fixture IDB hoàn toàn tổng hợp.

| Command / source | Kết quả và boundary |
| --- | --- |
| `cargo xtask verify quick` tại `4e68bd8` | **PASS toàn lane**: fmt, clippy server/browser/preview, doctest, unit/contract (223 Web tests), repository checks. Log `target/issue203/quick-final.log`. |
| `cargo xtask verify web --scenario storage` tại `4e68bd8` | **PASS toàn command được chọn**, gồm bundle hydrate, app thật, transaction/quota/2-tab/migration/kill-reopen Chrome và gate tắt. Log `target/issue203/storage-final.log`; artifacts `target/verify-web/1794751985141800000-49566`. Không phải full Web lane. |
| `cargo xtask verify web --scenario main` tại `92d6dcb` | **FAIL** assertion `reduced mobile viewport keeps the active caret visible without fixed controls`. Đã mở `failure.png`; chưa xác định nguyên nhân, không claim lỗi có sẵn hoặc đã sửa. Log `target/issue203/main-first.log`; artifacts `target/verify-web/1794751243577030000-35249`. |
| `cargo xtask verify web --scenario main` tại `4e68bd8` | **FAIL** assertion `forced-colors retains selected check and keyboard focus; reduced-motion reduces transitions below 1ms`. Không console errors; chưa xác định nguyên nhân. Log `target/issue203/main-final.log`; artifacts `target/verify-web/1794751830918526000-45421`. Các bước sau **NOT_RUN**. |

Cả hai lần `main` dừng trước bộ storage ở cuối; kết quả storage đến từ command chọn riêng ở trên.
Không đổi/bỏ assertion UI để làm xanh. Không sửa SQL/API; full `postgres`, full Web, OS IME thật
và trình duyệt khác **NOT_RUN**. GitHub CI không đọc/chờ/poll/rerun. Trước bật rollout cần
exact-revision signoff, chốt POLICY-02 và UX expired, sau đó kiểm composition persistent offline
UI thực tế; issue #204 còn sở hữu app shell/cold launch. Lỗi `main` cần điều tra ở các assertion
được nêu, không coi PASS của adapter thay cho regression UI.

| Chiều UX | Kết quả và giới hạn #203 |
| --- | --- |
| Functional | **PASS** quick + storage tại boundary adapter; toàn scenario `main` **FAIL**, persistent offline UI **NOT_RUN** vì gate tắt. |
| Geometry | **FAIL** assertion caret khi giảm viewport ở lần `main` đầu; chưa xác định nguyên nhân. |
| Visual | Không đổi visual; ảnh `failure.png` lần đầu đã mở để chẩn đoán, không phải approved reference; fidelity mới **NOT_RUN**. |
| Accessibility/Responsive | **FAIL** assertion gộp forced-colors/focus/reduced-motion ở lần `main` cuối; chưa tách nguyên nhân thành phần. Screen reader/OS IME **NOT_RUN**. |
| Interaction | **PASS** hai tab và restart Chrome ở adapter; composition offline UI và suspend/resume thiết bị thật **NOT_RUN**. |


### Đồng bộ regression `main` — PR #215, 15/11/2026

Bản `5b418c48c8586c4b622684b6b6799283e0e0f21f` chỉ sửa harness, không đổi CSS,
CodeMirror runtime, Rust, proposal hoặc rollout. Hai assertion trước đọc computed style/caret
ngay sau CDP media/keyboard hoặc resize, trong khi style/layout và selection scroll có thể còn
được lên lịch. Nay chờ **đúng toàn bộ predicate cũ, tối đa 2 giây**, rồi kiểm lại sau fonts/layout
và hai animation frame. Ca caret còn yêu cầu focus ở `.cm-content` và selection tồn tại.
Không gọi scroll/focus để ép kết quả, không nới viewport bounds hoặc bỏ điều kiện accessibility.

Lần chẩn đoán trên `19e68a5` + hai dòng đo bổ sung **PASS toàn command main**; log
`target/issue203/main-diagnostic-215.log`, artifacts `target/verify-web/1794752834358309000-64033`.
Số đo forced-colors: checked/focus-visible đúng, outline solid 2px, opacity 1,
transition `1e-05s`; caret mobile top 295,203125/bottom 320,203125 trong viewport 420px.
Hai lỗi cũ không tái hiện trong lần này; đây là bằng chứng lỗi nhạy thời điểm quan sát,
không chứng minh một hồi quy CSS/runtime hoặc xác định chính xác thành phần sai ở lần cũ.
Giữ nguyên hai kết quả FAIL lịch sử ở trên.

Lần `main` tại `5b418c4` **FAIL** sau timeout 2 giây ở caret mobile, dù forced-colors đã
**PASS**. Log `target/issue203/main-wait-only-215.log`; artifacts
`target/verify-web/1794753195584782000-69461`. Đã mở ảnh failure: viewport nằm dưới editor,
không console errors. Chờ predicate sau phím chưa đủ; không tăng timeout để làm xanh.
Bản `5bf229f` bổ sung chờ fonts/layout của viewport mobile trước khi gửi phím document-end/line-start,
tránh gửi input trong lúc resize/reflow còn đang cập nhật vị trí cuộn. Giữ kiểm sau phím.

`cargo xtask verify web --scenario main` tại **`5bf229f5cdb8af59c701ecff3055a6c6b46d1fc5`**
**PASS toàn command được chọn**, gồm forced-colors/focus/reduced-motion, caret mobile,
SSR/hydration/editor/save/conflict/leave guard và bộ IndexedDB ở cuối; không console errors.
Môi trường và biến cargo/Chrome/DB như lần #203 ở trên. Log
`target/issue203/main-reflow-215.log`; artifacts `target/verify-web/1794753567501336000-80206`.
Trong lúc chạy chỉ cập nhật tài liệu, không sửa source kiểm thử. `node --check` cho harness,
`git diff --check` và kiểm liên kết local tài liệu **PASS**. Không chạy lại quick vì Rust/runtime
không đổi; evidence quick tại `4e68bd8` vẫn áp dụng cho phần ứng dụng đó.

Self-review đã mở hai ảnh cuối `appearance-forced-colors-reduced-motion.png` (1440×900)
và `editor-short-mobile-390.png` (390×420, tối), không nâng thành approved visual reference.

| Chiều UX | Kết quả bản sửa kiểm thử #215 |
| --- | --- |
| Functional | **PASS** command main, gồm IndexedDB; gate production vẫn tắt. |
| Geometry | **PASS** caret trong viewport mobile và toolbar static sau reflow, cùng ma trận main. |
| Visual | **PASS** kiểm tra hai ảnh đã mở ở trên, giữ giao diện hiện hữu; fidelity mới **NOT_RUN**. |
| Accessibility/Responsive | **PASS** forced-colors, focus, reduced-motion và ma trận 100/200% của main; screen reader/OS IME **NOT_RUN**. |
| Interaction | **PASS** keyboard, selection, leave guard, save/conflict và storage restart trong Chrome/CDP; thiết bị mobile thật **NOT_RUN**. |

Đây là PASS của scenario main, không phải toàn Web lane hay bảo đảm không còn flake trên mọi
browser. Giữ AC-08 OPEN, rollout tắt, POLICY-02/UX expired chưa chốt. Không kiểm GitHub CI,
không merge PR.
