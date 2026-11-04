# Đọc công khai cache-first trong tab (READ-02, Web pilot)

**Issue:** [#53](https://github.com/loveoverflowcom/vink/issues/53) · **Nền:** `develop@fab4f95`
(đã có READ-01 [#52](https://github.com/loveoverflowcom/vink/issues/52)) · **Trạng thái:** Web pilot
cho đúng hai public read (`/api/v1/works`, `/api/v1/works/{id}/publication`); đã kiểm bằng unit
(đồng hồ giả) và Chrome headless cục bộ. CMP/native: **NOT_IMPLEMENTED** — chưa có product app
hay network owner. Chính sách hiển thị dữ liệu cũ dưới đây là **đề xuất v1**, chưa phải quyết định
sản phẩm đã chốt.

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
            Store<PublicKey, Snapshot> thuần ─ freshness.rs (policy, ticket, GC; không I/O)
            1 timer · listener visibilitychange/focus/online/pageshow · waiter single-flight
                                                   ▼
          api::http::public  fetch cache: no-cache (+ deadline 30 s)
            → HTTP cache trình duyệt gửi If-None-Match (ETag READ-01) → 304 được merge thành 200
```

View (`PublicCollection`, `PublicationView` trong `pages/mod.rs`) chỉ render `Live { data,
refreshing, failed, removed }` và gửi ý định “Thử lại” / “Xem bản mới”.

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

Hằng số ở `freshness::Policy::V1`. Mọi mốc thời gian dùng `performance.now()` (monotonic theo tab).

| Tham số / sự kiện | Hành vi |
| --- | --- |
| `stale_after = 120 s` | Ngưỡng kiểm tra lại, **không phải TTL**: không xóa gì. `validated_at` là thời điểm **bắt đầu** request được origin xác nhận (cận dưới, an toàn); đọc cache không gia hạn nó |
| Timer | Một timer cho cả tab, hẹn đến mốc sớm nhất của các key đang được view quan sát: `validated_at + 120 s + spread`, spread ngẫu nhiên `0–15 s` chọn lại sau mỗi lần xác nhận. Không timer theo card |
| Mount, `focus`, hiện lại (`visibilitychange`), `online`, `pageshow` | Kiểm tra nếu đã quá 120 s (không cộng spread). Gộp với request đang chạy (single-flight); bốn tín hiệu đồng thời → một request. `online` chỉ là gợi ý: lỗi vẫn backoff |
| Ẩn tab | Hủy timer, bỏ qua mọi tín hiệu trừ thao tác thủ công; hiện lại thì kiểm tra nếu stale |
| “Thử lại” (thủ công) | Bỏ qua ngưỡng 120 s và backoff, vẫn single-flight và vẫn có validator |
| Lỗi tạm (mạng, timeout 30 s, 5xx, phản hồi lạ) | Giữ last-success trên màn hình với thông báo riêng; backoff 5 → 10 → 20 → 40 → 80 → 120 s (trần), jitter 80–100 %, tôn trọng `Retry-After` dạng giây (trần 120 s). Không retry mutation |
| `404` (removal có thẩm quyền) | Xóa entry ngay, reader thay chữ bằng trang “Không tìm thấy”; card của tác phẩm đó bị bỏ khỏi snapshot danh sách cho tới khi list được origin xác nhận lại. Không dùng stale-if-error cho trường hợp này |
| Bản đổi nội dung (200 khác) | Khám phá cập nhật tại chỗ (key ổn định: giữ row, focus, scroll). Reader **không** thay chữ đang đọc: hiện “Tác phẩm vừa có bản cập nhật” + “Xem bản mới”; áp dụng thì về đầu nội dung và focus `#noi-dung` |
| `max_display_age = 15 phút` | Snapshot xác nhận quá 15 phút không được render ngay ở **lần mount mới** (sẽ tải với loading thật). Nội dung đang trên màn hình không bị rút vì lỗi tạm |
| `retain_unobserved = 5 phút`, tối đa 16 entry | Entry không view nào quan sát bị thu gom sau 5 phút; vượt 16 thì bỏ entry không quan sát cũ nhất. Entry đang quan sát không bị bỏ |
| Hydration | Snapshot SSR được seed với `validated_at = 0` (thời điểm bắt đầu điều hướng, sớm hơn lần đọc của server). Trang mở bằng back/forward có thể là HTML từ HTTP cache với tuổi không biết, nên coi là stale và kiểm tra ngay. Seed không ghi đè dữ liệu mới hơn hay request đang chạy. Kiểm tra đầu tiên chạy sau hydration, không làm lệch HTML server |
| Mutation có ACK | `Store::invalidate` đã có (giữ hiển thị, chặn completion cũ, kiểm tra ngay) nhưng **chưa có caller**: Vink chưa có lệnh publish/withdraw hay mutation công khai. Lưu nháp không invalidate public snapshot |

Ngoài phạm vi store: nháp riêng (`no-store`, CAS/409), editor CodeMirror và văn bản chưa lưu,
discussion (`no-store`). Store chỉ ở bộ nhớ tab; đóng tab/kill process là mất. Không
IndexedDB/HTTP disk policy riêng/Service Worker.

### Chỗ còn cần quyết định sản phẩm

1. **Hiển thị dữ liệu cũ:** v1 chọn hướng 1 của issue — giữ snapshot công khai đã xem trong phiên
   tab, có trạng thái “chưa làm mới được”, revalidate khi online/hiện lại, và `max_display_age`
   15 phút cho lần mount mới. Giá trị 15 phút là giả định cần chủ sản phẩm chốt.
2. **Rút bài (PUB-03):** polling không thu hồi được bản đã nằm ở thiết bị khác; v1 chỉ xóa khi
   chính tab nhận 404. Chưa có lệnh rút bài sản phẩm, nên chưa có invalidation chủ động.
3. **Offline full-text** và lưu bền: chưa làm; cần quyết định riêng (account clearing, thiết bị
   dùng chung, migration schema).

## Ranh giới race và an toàn

- **Ticket tăng đơn điệu** cho mỗi request; chỉ ticket hiện hành của entry được commit.
  `invalidate`, `remove`, `clear` (đổi scope) làm request đang chạy không thể commit; ticket không
  bao giờ lặp nên response muộn không “hồi sinh” entry đã xóa. Không so sánh ETag để suy bản mới.
- **Single-flight:** view mount khi chưa có snapshot hiển thị được sẽ chờ request đang chạy (waiter),
  không tạo request thứ hai. Request chạy trong task tách khỏi view, có deadline 30 s, nên luôn kết
  thúc và không treo entry khi view unmount.
- **Key** = endpoint + `work_id`; không có token/cookie/nội dung bản thảo trong key hay log. Tìm
  trong 12 card vẫn lọc local, không tạo query server.
- **SSR:** store chỉ tồn tại trong bundle trình duyệt (`cfg(feature = "hydrate")`); server không có
  cache dùng chung giữa request/người dùng. HTML không-JS, status 404/503 và metadata giữ nguyên.
- Listener lifecycle cài một lần cho tab; view chỉ `observe`/`release` khi mount/unmount, nên điều
  hướng lặp lại không cộng dồn listener hay timer.

## Bằng chứng

| Kiểm | Lane | Kết quả |
| --- | --- | --- |
| `freshness::tests` (16): 119/120/180 s; đọc không gia hạn; spread có trần; một wake cho mọi entry; bốn trigger đồng thời → một ticket; manual/invalidate không bị snapshot fresh nuốt; backoff không dồn + `Retry-After`; lỗi giữ last-success; 404 xóa và response muộn bị loại; reset scope; mốc từ lúc bắt đầu request; `max_display_age` chỉ chặn lần mount mới; GC + trần entry; entry không quan sát không bị poll; raw 304 → một lần không điều kiện; chuỗi ngẫu nhiên 300 × 80 bước giữ single-flight và generation | `quick` | PASS cục bộ |
| `cacheFirstPublicReads` (Chrome headless + CDP, đồng hồ `performance.now` và `visibilityState` do test điều khiển) | `web` | Xem dưới |

`cacheFirstPublicReads` kiểm: hydration không gửi request list; lần ghé client đầu có loading và
một request; quay lại trong 120 s render ngay, **không** request, **không** loading; 119 s + focus
không request; 121 s + focus/online/visibilitychange/pageshow cùng lúc → **một** request có
`If-None-Match`, wire 304, giữ nguyên phần tử row và focus; tab ẩn không request khi focus/online,
hiện lại → đúng một request; 503 giữ danh sách + thông báo riêng, “Thử lại” phục hồi không
remount; reader nhận bản khác → thông báo, chữ cũ giữ nguyên, áp dụng khi bấm; 404 → xóa chữ;
về Khám phá thấy card bị bỏ trong lúc list đang revalidate, list của origin trả card về.

Đồng hồ giả chỉ dịch `performance.now()`; timer thật 120 s không được chờ trong smoke — đường
timer được kiểm ở unit (`next_wake`/`due`). Ẩn tab được giả lập bằng cách ghi đè
`document.visibilityState`, không phải chuyển tab hay khóa máy thật.

### Số đo

**Môi trường:** Apple M4, macOS 26.5.2, Chrome headless, server debug của `verify web` trên
loopback, dataset seed (1 tác phẩm công khai). Một lần chạy, không lặp thống kê.

| Đại lượng | Trước (`develop@fab4f95`) | Sau |
| --- | --- | --- |
| Thời gian từ click tới row khi quay lại Khám phá trong 120 s | Luôn tải lại qua mạng + loading | 6–10 ms, **0 request**, không loading (lần ghé đầu qua mạng: 7–19 ms trên loopback) |
| Request list mỗi lần quay lại Khám phá trong 120 s | 1 | 0 |
| Request khi hiện lại/focus/online/pageshow cùng lúc lúc stale | — (không làm mới) | 1, có `If-None-Match`, wire 304 |
| Request khi tab ẩn | 0 | 0 (timer bị hủy) |
| Request định kỳ khi màn hiển thị | 0 | ≤ 1 mỗi `120–135 s` cho mỗi key đang hiển thị (từ policy; timer thật không được chờ trong smoke) |
| `vink_web.wasm` `wasm-release` trước wasm-bindgen | 12 800 367 B (gzip -9: 2 398 576) | 13 342 443 B (+4,2 %; gzip -9: 2 494 225, +4,0 %) |

Số mili-giây trên loopback chỉ cho thấy đường cache không chờ mạng, không phải latency thực tế.
Không đo bộ nhớ heap; trần là 16 entry × kích thước một publication. Truy vấn phía server cho một
304 không đổi so với READ-01 (vẫn một câu SQL).

## CMP và native

**NOT_IMPLEMENTED / NOT_RUN.** `packages/ui-system/cmp` và các diagnostic host không có network
owner; không thêm Ktor hay scheduler vào UI/pattern packages (phải DTO-free). Khi có product app,
hợp đồng hành vi cần giữ (conformance, không phải thư viện dùng chung):

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

- Chưa có caller cho `invalidate` (không có mutation công khai); chưa có account scope (không có
  auth), nên reset scope chỉ được kiểm ở unit.
- Spike `leptos-fetch` chưa chạy trong Vink; kết luận dựa trên tài liệu + source 0.4.10.
- Chỉ Chrome headless; chưa đo trên thiết bị di động thật, mạng chậm hay suspend/resume thật.
- Chưa có Quint model riêng cho store; bằng chứng là unit + chuỗi ngẫu nhiên có xác định.
