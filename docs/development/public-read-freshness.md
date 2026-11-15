# Đọc công khai cache-first trong tab (READ-02, Web pilot)

**Issue:** [#53](https://github.com/loveoverflowcom/vink/issues/53), cập nhật tính đúng tại issue [#200](https://github.com/loveoverflowcom/vink/issues/200) · **Nền:** `develop`
(có READ-01 [#52](https://github.com/loveoverflowcom/vink/issues/52), Discovery #100, Author publication #158, Moderation & `O-17@r2`)

**Bốn chiều trạng thái (tách biệt độc lập):**
- **Quyết định sản phẩm:** `OPEN` cho chính sách hiển thị dữ liệu cũ (`max_display_age = 15 phút` là giả định kỹ thuật v1, chưa chốt với chủ sản phẩm); `PRODUCT_CONFIRMED` cho `O-17@r2` (trang thay thế khi rút bài sau kiểm duyệt) và `PUB-07` (ẩn tác phẩm).
- **Quyết định kiến trúc:** `DESIGN_PROPOSED` cho `READ-02-CHOICE-01..04` (chỉ áp dụng ở Web pilot; không tự nâng lên `Accepted` khi test xanh). Tương thích với cập nhật CHOICE-01 = A (adapter lỗi có kiểu theo `status + error.code + error.field`) và CHOICE-02 = A (tách `mark_stale` giữ backoff khỏi `mutation_confirmed` wake ngay).
- **Mức triển khai (`implementation level`):** `web` pilot (`apps/vink-web/src/freshness.rs`, `public_reads.rs`, `contract.rs`, `api.rs`, `pages/discovery.rs`, `pages/mod.rs`); CMP/native là `NOT_IMPLEMENTED`.
- **Bằng chứng kiểm chứng (`verification evidence`):** ghi theo từng lần chạy thực tế ở mục [Bằng chứng](#bằng-chứng), tách riêng unit/contract, PostgreSQL và trình duyệt Chrome headless.

Mục tiêu: có snapshot được phép thì hiển thị ngay, làm mới phía sau, không xóa nội dung đang đọc
để hiện loading toàn màn; kiểm tra lại khoảng 2 phút khi màn đang hiển thị. Không thêm cache phía
server, realtime, Service Worker hay lưu bền.

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

View (`PublicResults` trong `pages/discovery.rs`, `PublicationView` trong `pages/mod.rs`) render `Live { data,
refreshing, failed, removed, rejected, key }` và gửi ý định “Thử lại” / “Xem bản mới” / “Thử từ trang đầu” / “Bỏ toàn bộ bộ lọc”.

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
| `404` / rút bài / ẩn bài (`Outcome::Removed`, `invalidate_withdrawn`) | Xóa entry `Publication` ngay và ghi **removal fence** (`Store::removed` + `Coordinator::withdrawn_at`), lọc card khỏi mọi `WorkList` đang giữ hoặc `seed` cũ hơn mốc rút bài, đồng thời báo trang reader kiểm `/api/v1/works/{id}/reader` (`ReviewWithdrawn` / `Hidden` theo `O-17@r2` / `PUB-07`). **Không** là tombstone vĩnh viễn: điều hướng `load()` mới nhận `200` (`Outcome::Fresh`) sẽ gỡ fence khi tác phẩm được công khai lại |
| Bản đổi nội dung (`200` khác) | Khám phá cập nhật tại chỗ (key ổn định: giữ row, focus, scroll). Reader **không** thay chữ đang đọc: hiện “Tác phẩm vừa có bản cập nhật” + “Xem bản mới”; áp dụng thì về đầu nội dung và focus `#noi-dung` |
| `max_display_age = 15 phút` | Snapshot xác nhận quá 15 phút không được render ngay ở **lần mount mới** (sẽ tải với loading thật). Nội dung đang trên màn hình không bị rút vì lỗi tạm |
| `retain_unobserved = 5 phút`, tối đa 16 entry | **Soft cap LRU/retention trên entry không quan sát**: entry không view nào quan sát bị thu gom sau 5 phút; vượt `16` (`Policy::V1.max_entries`) thì bỏ entry không quan sát cũ nhất (entry đang quan sát không bị bỏ). **Chưa có hard byte budget** theo dung lượng payload |
| Hydration | Snapshot SSR được seed với `validated_at = 0` (chỉ tính cận dưới an toàn của điều hướng hiện tại). Trang mở bằng back/forward có `rendered_at = -∞` vì tuổi HTML cache không rõ. `seed` bị chặn nếu đã có snapshot mới hơn, request đang bay, hoặc removal fence tại/sau `rendered_at` |
| Tách `mark_stale` vs `mutation_confirmed` (**CHOICE-02 A**) | **`mark_stale` / `mark_stale_where`** (stale chung) đánh dấu `stale = true` và vô hiệu hóa ticket đang bay nhưng **giữ nguyên `failures`/`retry_at`** để không vượt backoff khi lỗi tạm; đọc `status()`/`changed()` hay lặp `mark_stale` không xóa backoff. **`mutation_confirmed` / `mutation_confirmed_where`** (qua `invalidate_publication` / `invalidate_withdrawn` từ ACK của `pages/publication.rs`, `pages/moderation.rs`, `ReplacementView`) đánh dấu `immediate_retry = true` để các owner **đang quan sát trên tab hiện** kiểm tra lại ngay không cần chờ timer 120 s (gộp single-flight; owner ẩn hoặc không có view quan sát không tự poll). Lưu nháp riêng vẫn **không** invalidate snapshot công khai |

Ngoài phạm vi store: nháp riêng (`no-store`, CAS/409), editor CodeMirror và văn bản chưa lưu,
discussion (`/discussion/public*` và `/discussion/me` dùng `discussion_reads::OwnState` không cache),
endorsements, catalog thẻ, `vink-worker` outbox và màn Bàn viết/kiểm duyệt. Store chỉ ở bộ nhớ tab;
đóng tab/kill process là mất. Khi đổi tài khoản/phiên hoặc auth fail, `session.rs`/`account` gọi
`clear()` (đồng thời giải quyết mọi waiter đang chờ bằng `LoadError::Unavailable` để không treo).
Không IndexedDB/HTTP disk policy riêng/Service Worker.

### Chỗ còn cần quyết định sản phẩm

1. **Hiển thị dữ liệu cũ:** v1 chọn hướng 1 của issue — giữ snapshot công khai đã xem trong phiên
   tab, có trạng thái “chưa làm mới được”, revalidate khi online/hiện lại, và `max_display_age`
   15 phút cho lần mount mới. Giá trị 15 phút là giả định `OPEN` cần chủ sản phẩm chốt.
2. **Rút bài đa thiết bị:** tab khác / thiết bị khác vẫn chỉ nhận ra bản bị gỡ ở lần revalidate/mở
   lại tiếp theo (404 / `reader`), trừ khi chính tab đó thực hiện mutation hoặc mở `/works/{id}`.
3. **Offline full-text** và lưu bền: chưa làm; cần quyết định riêng (account clearing, thiết bị
   dùng chung, migration schema).

## Ranh giới race và an toàn

- **Ticket tăng đơn điệu** cho mỗi request; chỉ ticket hiện hành của entry được commit.
  `mark_stale`, `mutation_confirmed`, `remove`, `clear` làm request đang chạy không thể commit; ticket
  không bao giờ lặp và kết hợp **removal fence** (`withdrawn_at` + `removed`) chặn `seed` cũ hơn mốc
  gỡ bài hồi sinh bản đã rút hoặc card đã gỡ. Không so sánh ETag để suy bản mới.
- **Single-flight & dọn waiter (`WaiterGuard`):** view mount khi chưa có snapshot hiển thị được sẽ chờ
  request đang chạy (`Load::Join`), không tạo request thứ hai. Nếu view unmount/Future bị drop trước
  khi hoàn tất, `WaiterGuard` hủy đúng `WaiterId`; nếu `invalidate`/`withdraw`/`remove`/`clear` xảy ra
  lúc fetch khởi tạo đang chờ, waiter được khởi động lại ngay (khi còn hợp lệ) hoặc giải quyết ngay
  (`NotFound`/`Unavailable`), không bao giờ treo trạng thái bận/chờ vô hạn.
- **Key** = `PublicKey::Works(ExploreQuery)` gồm **`q`, `tags` chuẩn hóa và `cursor`** (Khám phá #100
  đã bỏ việc chỉ lọc local trong 12 card), hoặc `PublicKey::Publication(work_id)` cho trang đọc. Key và
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
- **Soft cap (`16` entry) vs hard byte budget:** READ-02 chỉ giới hạn số entry không quan sát là `16`;
  chưa đo/cưỡng chế ngưỡng byte cứng.
- **Runtime vs model:** **Top 100 (`TOP`)** và ranking/recommendations vẫn chỉ là model đích trong
  `services/vink-app/behavior/target` (`NOT_IMPLEMENTED` trong runtime/UI); **CMP/native** chưa có
  product owner hay network layer (`NOT_IMPLEMENTED`).

| Endpoint / Owner | Phạm vi actor & cache HTTP (READ-01) | Con trỏ / Giới hạn dữ liệu | Cache client (READ-02) & Key | Vòng đời & Hủy / Rút |
| --- | --- | --- | --- | --- |
| `GET /api/v1/works` | Công khai (`no Actor`); `200`/`304`: `ETag` strong + `private, no-cache`; lỗi `422`/`503`: `no-store` | `DiscoveryCursor` v2 (33 B: `published_at + work_id + SearchScope` SHA-256), **không** là sync watermark; `limit` mặc định `12`, tối đa `24`; tối đa `4` tag lọc (`8` ref) | **Có (Web pilot):** `PublicKey::Works(q, canonical_tags, cursor)`, stale 120 s, `max_display_age` 15 m, soft cap 16 | Card của tác phẩm bị `invalidate_withdrawn` được cắt khỏi snapshot & `seed` cũ hơn mốc rút, rồi xác nhận lại với origin; `422 cursor` → `#restart-public`; `422 q/tag/limit/query` → báo lỗi field, dừng retry |
| `GET /api/v1/works/{work_id}/publication` | Công khai (`no Actor`); `200`/`304`: `ETag` strong + `private, no-cache`; `404`/lỗi: `no-store` | Theo `work_id`, chỉ trỏ `revision_id` đang công khai hiện hành | **Có (Web pilot):** `PublicKey::Publication(work_id)`, cùng policy trên | Reader giữ chữ đang đọc và mời “Xem bản mới” khi `200` đổi nội dung; `404` hoặc `set-private`/case/appeal/reader `withdraw` xóa ngay + đặt removal fence; công khai lại mở bằng `200` mới |
| `GET /api/v1/works/{work_id}/reader` | Công khai (`no Actor`); khi công khai dùng `ETag` READ-01 (`private, no-cache`), khi `ReviewWithdrawn`/`Hidden` (`200`) hoặc `404`/`503` trả `no-store` | Read model đơn `ReaderResponse` (`published` / `review_withdrawn` / `hidden`) | **Ngoài READ-02:** gọi trực tiếp `fetch_reader` khi `Publication` báo đã rút/404 để dựng `ReplacementView` (`O-17@r2` / `PUB-07`) | Nếu nhận `ReviewWithdrawn`/`Hidden`, gọi `invalidate_withdrawn(work_id)` và hiển thị notice + bình luận đủ điều kiện (`noindex`, không title/body) |
| `GET /api/v1/tags`, `/tags/suggestions`, `/tags/slugs/{slug}` | Công khai (`no Actor`); `Cache-Control: no-store`, chưa có `ETag` READ-01 | Bounded/top-K catalog: `MAX_LISTED_TAGS = 200` (+ `truncated`), `MAX_SUGGESTIONS = 10` | **Ngoài READ-02:** nạp theo lượt trang/gõ có debounce/hủy từ UI | Đổi/ngừng dùng thẻ phản ánh ngay ở lần gọi kế tiếp |
| `GET /api/v1/works/{work_id}/discussion/public`, `.../roots/{root_id}/replies` | Công khai (`no Actor`); `Cache-Control: no-store`, chưa có `ETag` | `PageCursor` v1 (`created_at ASC, id ASC`, `CursorScope::Thread`), trang mặc định `20`, tối đa `50`, preview `3` reply | **Ngoài READ-02:** không đưa vào `freshness::Store` | `404`/`409` khi tác phẩm gỡ/ẩn hoặc sai phạm vi cursor |
| `GET /api/v1/works/{work_id}/discussion/me` | Có auth (`Actor`, `401` cho khách, `503` khi Identity lỗi); `no-store` | `PageCursor` v1 (`CursorScope::Own`), batch `20` | **Ngoài READ-02:** `discussion_reads::OwnState` (generation-fenced ticket trong RAM, không cache liên phiên) | Tự reset khi `clear` session hoặc đổi tài khoản |
| `GET /api/v1/works/{work_id}/endorsements`, `/endorsement/me` | Tổng hợp công khai (`no Actor`) và trạng thái riêng (`/me`, có auth); `no-store` | Tóm tắt theo work/target | **Ngoài READ-02:** controller riêng của LVE; `409 revision_changed` báo READ-02 kiểm tra lại | Đọc riêng không trộn với cache công khai |
| Top 100 (`TOP`) | — | — | **NOT_IMPLEMENTED** (chỉ có model đích trong `services/vink-app/behavior/target`) | Chưa có API hay UI |

## Bằng chứng

| Kiểm | Lane | Kết quả |
| --- | --- | --- |
| `freshness::tests` (20): 119/120/180 s; đọc không gia hạn; spread có trần; một wake cho mọi entry; bốn trigger đồng thời → một ticket; manual/invalidate không bị snapshot fresh nuốt; `mark_stale` giữ nguyên `failures`/`retry_at` còn `mutation_confirmed` cho 1 lần thử ngay rồi nối lại backoff + `Retry-After`; lỗi giữ last-success; phân loại `422` (`cursor` vs `q`/`tag`/`limit`/`query` vs lỗi/envelope hỏng) dừng retry; 404 xóa và response muộn bị loại; `clear_removal`/`record_fresh`; reset scope; mốc từ lúc bắt đầu request; `max_display_age` chỉ chặn lần mount mới; GC + trần mềm 16 entry; entry không quan sát không bị poll; raw 304 → một lần không điều kiện; chuỗi ngẫu nhiên 300 × 80 bước | `quick` / `unit web` | PASS cục bộ |
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
- Tắt SWR hoàn toàn: trong `api.rs`, `works_resource`/`publication_resource` phía trình duyệt gọi
  lại transport trực tiếp thay cho `public_reads::load`, và `PublicCollection`/`PublicationView`
  dùng `Signal::stored(initial)`; không có migration hay dữ liệu bền nào cần dọn.

## Chưa làm / rủi ro còn lại

- Đã nối caller mutation cho `invalidate_publication` / `invalidate_withdrawn` từ tác giả (`pages/publication.rs`), kiểm duyệt viên/admin (`pages/moderation.rs`), trang đọc khi rút/ẩn bài (`pages/mod.rs`) và `clear()` từ đổi/lỗi tài khoản (`session.rs`/`account`). Rủi ro còn lại là **tab khác hoặc thiết bị khác** chưa nhận push/broadcast liên tab ngay lúc rút bài (vẫn chờ revalidate hoặc lần mở tiếp theo).
- Giá trị giả định `max_display_age = 15 phút` (và chính sách hiển thị stale khi lỗi tạm / offline lâu hơn) vẫn là mục **`OPEN`** chờ chủ sản phẩm chốt; `READ-01-CHOICE-*` và `READ-02-CHOICE-*` vẫn là `DESIGN_PROPOSED`.
- Offline full-text, lưu bền ngoài bộ nhớ tab (IndexedDB/SQLite), Top 100 runtime và CMP/native product network owner đều **`NOT_IMPLEMENTED`** (CMP không có product owner).
- Spike `leptos-fetch` chưa chạy trong Vink; kết luận dựa trên tài liệu + source 0.4.10.
- Kiểm trình duyệt chạy trên Chrome headless cục bộ với đồng hồ `performance.now`, `visibilityState` và fault injection Fetch qua CDP; chưa đo trên thiết bị di động thật, mạng chậm hay suspend/resume thật.
- Chưa có Quint model riêng cho `freshness::Store`/`Coordinator`; bằng chứng hành vi là unit + chuỗi ngẫu nhiên có xác định + test coordinator + E2E.
