# SSR Reader #235 — giữ Async và kiểm thao tác trước hydration

## Kết luận kỹ thuật trong phạm vi này

Giữ **WEB-PERF-02 / CHOICE-01 A**. Không đổi scheduling public read, cache policy, SSR mode,
bootstrap, framework hoặc authority. Đây là lựa chọn kỹ thuật của lát #235, **không** phải
signoff kiến trúc Accepted. Ngoài regression và bằng chứng, review Mac sửa trạng thái sẵn sàng
của hai nút tải lại discussion trước hydration; không tuyên bố giảm latency hoặc đã thử nghiệm
streaming trên proxy.

Base đã fetch: `develop@f1b46703ae3329ee7cd11fb06db3f8157a31e5b9`, chứa PR #251/#234.
PR #212/#202 (`73fdbe0`) cũng có trong lịch sử base. Trước khi tạo worktree mới đã kiểm branch,
worktree, dirty state, PR #235 và worker; không đụng checkout #229 đang có thay đổi.

[Baseline #234](web-perf-baseline.md) đã chạy 10 cold/10 warm **debug** lab. Nó chưa hoàn tất:
WASM debug khoảng 350 MB, chưa có release profile, secondary/race negatives, validator/304,
stale/offline và go/no-go. Số warm discussion 292 ms / Reader 102 ms không đo secondary chặn
SSR. Vì vậy không dùng số đó để chọn kiến trúc B/C hoặc tuyên bố A nhanh hơn.

| Lựa chọn | Đánh giá và điều kiện xem lại |
| --- | --- |
| A — Async | Giữ baseline hiện hành; status, full article và meta quyết trước response. Regression ở đúng route và typed Reader owner. Không loại bỏ read nào chỉ vì có URL gần giống. |
| B — critical SSR + selective streaming | Chưa bật: thiếu release/direct/proxy chunk evidence và gain trên meaningful article. `OutOfOrder` cần JS cho suspended fragment; chỉ đổi route mode không bảo toàn metadata/status. Chỉ xem lại bằng cùng fixture secondary 0/200/1000 ms và matrix visibility/no-JS. |
| C — client defer vùng phụ | Chưa bật: thảo luận công khai hiện có trong SSR/no-JS; bỏ nó sang browser sẽ thêm waterfall và cần fallback tương đương. Không defer toàn văn hoặc serialize own overlay. |

Đã đọc source pinned `leptos_router 0.8.17/src/ssr_mode.rs` và
`leptos_axum 0.8.10/src/lib.rs`: Async thu HTML bất đồng bộ trước response;
OutOfOrder có thể commit shell trước async meta. Không suy API từ phiên bản book khác.
Rollback của lát này là giữ route Async; regression `reader_route_retains_async_as_the_rollback_baseline`
đọc route list thật. Không có optimization runtime cần flag/rollback mới.

Review Mac tìm thấy một thiếu sót đã có ở base: `discussion-refresh` và
`discussion-load-retry` hiện enabled trong SSR dù callback cần hydration. Hai control nay bắt
đầu unavailable với lý do “Cần JavaScript để tải lại nhận xét.”, rồi bật sau effect của chính
component. Trạng thái đầu giống nhau ở SSR/hydration; không xếp hàng click, không đổi quyền ghi,
read coordinator hoặc transport. Regression SSR mới đã FAIL ở source trước sửa; browser kiểm
refresh unavailable khi bootstrap bị chặn/giữ và trở lại enabled sau hydration. Đây là guard
trình bày theo lifecycle, không thay semantic của các model publication/read owner.

## Owner critical và secondary

| Vùng | Owner hiện hành và boundary |
| --- | --- |
| Route /tac-pham/:id, shell/hydration | `apps/vink-web/src/lib.rs`: `SsrMode::Async`, hydration đầy đủ. |
| Critical visibility/body/status/meta | `pages/mod.rs::ReaderScope/ReaderLoaded/PublicationContent/ReplacementView`; `api.rs::reader_resource` → SSR `SharedPublicReads::reader` hoặc browser `PublicKey::Reader`. Hidden/ReviewWithdrawn là control riêng, không publication snapshot. |
| Public SSR adapter | `services/vink-app/src/http/ssr.rs::PublicReadsAdapter`: gọi App in-process, không HTTP roundtrip, không actor riêng trong public discussion/counts. |
| Secondary public discussion | `pages/discussion.rs::Discussion` → `api.rs::discussion_resource`: public page rồi endorsement batch; failure giữ main article và báo chưa tải nhận xét. |
| Own overlay/capability | Browser-only, account/session generation riêng. Các GET public, own, actor và endorsement theo target không được cộng thành duplicate typed `/reader`. |
| Fresh/stale/history/race | READ-02 `public_reads`, `read_cache`, `freshness`, `read_lifecycle`; seed không gia hạn tuổi, ticket/scope fence vẫn là owner. Không thêm cache/coordinator mới. |

[Decision snapshot](evidence/reader-ssr-235/decisions.json) được query tại base thực thi:
READ-01/02 và DISC-WEB r1 giữ `DESIGN_PROPOSED`; O-03/O-17 r2 là `PRODUCT_CONFIRMED`;
VDR-PUBLIC-OFFLINE-AGE r1 không có signoff, vẫn pending. Không bật persistence/offline gate hoặc
chốt max-stale/expiry/clearing UX. PRD và các INV vẫn là oracle về authority/privacy.

## Regression thêm vào owner hiện hữu

Matrix SSR chạy trong một integration-test process riêng. Pinned `leptos_server 0.8.8` dùng
`SuppressResourceLoad` với AtomicBool cấp process trong lúc sinh route list; các test library
song song có thể chặn resource của nhau. Lượt library song song đã treo, trong khi serial PASS.
Matrix cũng được gọi trong lane `quick` hiện hữu, ngoài process của unit library.
Không đổi framework hay giảm assertion; matrix riêng có deadline 30 giây và không dùng DB giả
để thay cho lane Web/PG.

- `services/vink-app/tests/ssr_boundary.rs`: route list thực phải giữ Async; port public
  **tổng hợp** giữ secondary tại 0/200/1000 ms. Response không được trả trước khi nhả secondary;
  sau secondary failure vẫn có full article, title/description/canonical và 200. Matrix control
  Hidden/ReviewWithdrawn/404/503 kiểm status, no-store/noindex của replacement và không có title,
  body/canonical/og:title của publication. Alternate publication method mang marker riêng để phát
  hiện dùng nhầm representation; retry sau secondary failure phải unavailable trước hydration.
  Đây là Leptos/Axum/serialization, không chứng minh quyền SQL.
- `apps/vink-web/e2e/reader-ssr-boundary.mjs`: được gọi trong **main** của browser-smoke hiện có.
  Bootstrap blocked vẫn giữ article/link/GET search và không lộ draft fixture. Bootstrap được
  giữ 200/1000 ms rồi nhả; click trước hydration không gửi/replay mutation, fresh SSR không tạo
  GET `/reader` trùng trong cửa sổ quan sát. Refresh unavailable trước bootstrap và enabled sau
  hydration được assert trực tiếp. Transport là CDP-visible path/method, không body,
  cookie, credential hoặc SQL; bootstrap injection được ghi rõ.
  Harness chờ event `Fetch.requestPaused` thực với deadline trước khi kiểm hold: DOM SSR có
  thể sẵn sàng trước deferred bootstrap import. Không suy DOM ready thành request đã bị giữ.
- Các assertions stale/history/reordered navigation có sẵn ở `cacheFirstPublicReads` (#202),
  canonical redirect ở discovery, và Hidden/ReviewWithdrawn qua App/PG thật ở review được giữ.
  Test hiện có không tự trở thành PASS mới chỉ vì source đã đọc.

Lệnh tái lập:

```sh
cargo test --locked -p vink-app --test ssr_boundary -- --nocapture
node --check apps/vink-web/e2e/reader-ssr-boundary.mjs
node --check apps/vink-web/e2e/browser-smoke.mjs
node --test apps/vink-web/e2e/web-perf-baseline.test.mjs
cargo xtask verify web --scenario main --scenario discovery --scenario review --plan
cargo xtask verify web --scenario main --scenario discovery --scenario review
cargo xtask verify quick
```

Kết quả thực chạy, source identity, raw artifacts và các giới hạn ở
[evidence #235](evidence/reader-ssr-235/README.md). Không đọc/chờ/poll GitHub CI.
