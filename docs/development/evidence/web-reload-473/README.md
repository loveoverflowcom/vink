# Điều tra #473: Vink chậm khi tải lại và mở lại

Đo ngày 24/11/2026 (giờ Việt Nam) trên **release đang chạy `e98725569657502e5701afeb3499c8b87ed1d84d`**.
HTML của `https://vink.forum/` trỏ namespace `/_v/e9872556…/pkg/`, mọi asset trả
`X-Vink-Asset-Revision` cùng SHA, và ETag của `vink_bg.wasm` bằng SHA-256 của byte đã tải về
(`95c578a3…`). Source được đọc ở develop `7baae1eac11ac91f46b1b5dfcaa9dac732ee2987`. Release
live cũ hơn develop, nên kết luận về byte và header chỉ áp dụng cho `e9872556`. Hậu kiểm của
PR #400 vẫn là lịch sử; mọi số liệu dưới đây đều đo mới trong ngày.

**Môi trường.** Một máy trạm Linux x86_64 ở Việt Nam (Cloudflare `/cdn-cgi/trace`: `loc=VN`,
`colo=SIN`), mạng thật không throttle, Chrome 158.0.8127.88 `--headless=new` (`--no-sandbox`
vì sandbox của công cụ chặn Chrome). Desktop: 1280×900, DPR 1, CPU 1×. “Mobile” là **mô phỏng**
gồm viewport 390×844, DPR 3 và CPU throttle 4×; không phải điện thoại thật. Phiên khách không có
cookie, route `/`. Catalog production đang trống nên trang chủ là empty state SSR. Reader production
`NOT_RUN`. Mỗi cấu hình chạy 10 mẫu, mỗi mẫu một profile Chrome mới. Mốc thời gian tính bằng ms
từ đầu điều hướng, ghi median (min–max). Không báo p95 vì n = 10.

## Kết luận

1. **Khi HTTP cache còn nguyên, Vink không tải lại asset.** Ở reload thường, mở lại tab, mở lại
   trình duyệt và Back, 100% request tĩnh (WASM, JS, CSS, font, ảnh) lấy từ memory/disk cache.
   Mỗi lượt chỉ còn khoảng 6,5–7,0 KB qua mạng: HTML `no-store`, `/api/v1/auth/session` và
   beacon RUM của Cloudflare. Giả thuyết “lần sau vẫn tải lại bundle” bị loại trừ cho trường hợp
   cùng release.
2. **Với cache còn nguyên, thời gian đến nội dung chủ yếu là round trip của document.** Ở reload
   desktop, TTFB 260 ms và FCP 314 ms, tức nội dung SSR hiện khoảng 55 ms sau byte đầu. Tách theo
   chặng: chỉ riêng edge đã chiếm 82 ms median (58–799 ms), đi tiếp tới origin thêm khoảng 90 ms,
   còn SSR Async trang chủ với hai resource thêm 20–70 ms so với SSR không Async hoặc
   `/health/live`. Phần lớn TTFB là mạng Việt Nam → Cloudflare SIN → origin. SQL/SSR không chi
   phối.
3. **Trên CPU chậm, khi mở lại, phần lớn thời gian nằm giữa lúc có nội dung và lúc tương tác được.** Ở mobile
   mô phỏng, mở lại tab cho FCP 528 ms nhưng hydrated tới 1.254 ms, kèm một long task Σ 604 ms.
   WASM instantiate từ cache mất 199 ms. Hydrate sau WASM mất 321 ms vì renderer mới phải
   lazy-compile 5.410 hàm WebAssembly (trace mở lại trình duyệt: 318–325 ms trên main thread).
   Trong 4 lần mở lại trình duyệt có trace, V8 không lần nào dùng code cache cho module của Vink
   (không có `wasm.Deserialize`).
4. **Lần đầu, và lần đầu sau mỗi phát hành, phải tải 8,72 MB không nén.** `vink_bg.wasm`
   (7.940.123 byte), `vink.css` (227.087 byte) và `vink.js` (97.848 byte) được trả **không có
   `Content-Encoding`** dù request chấp nhận `br, gzip, zstd`. Hai điểm trong Worker khớp với
   triệu chứng: Worker [đặt `no-transform`](../../../../infra/cloudflare/production/worker.mjs)
   cho `/_v/<sha>/` và gọi `env.ASSETS.fetch` không kèm `Accept-Encoding`. HTML cùng zone (không
   có `no-transform`) vẫn được trả Brotli. **Kiểm riêng sau điều tra
   ([#487](https://github.com/loveoverflowcom/vink/issues/487)):** alias `/pkg/` đi cùng nhánh
   Worker `inventory/r26` nhưng có `no-cache` thay cho `no-transform`, và được Cloudflare nén
   Brotli/gzip (WASM bootstrap 7.872.530 → 2.164.348 byte br), còn `/_v/<sha>/` luôn trả identity.
   `no-transform` là nguyên nhân. Việc không gửi `Accept-Encoding` tới Pages không ảnh hưởng.
   Namespace đổi theo SHA nên mỗi phát hành làm mọi URL asset mới, kể cả font và ảnh không đổi. Brotli q11 sẽ giảm ba file này còn 1,63 MB / 31,7 KB / 11,7 KB. Ở phép đối
   chứng cục bộ dùng đúng byte live, mạng mô phỏng 10 Mbps, hydrated giảm từ 7.322 ms xuống
   2.081 ms. FCP cũng giảm từ 924 ms xuống 396 ms, vì preload WASM chiếm băng thông của CSS
   render-blocking.
5. **So với Cantos trong cùng điều kiện, Vink hiện nội dung sớm hơn ở mọi kiểu truy cập có cache.**
   Thời điểm tương tác ngang hoặc sớm hơn trên desktop, muộn hơn 150–190 ms khi mở lại trên mobile
   mô phỏng, và chậm hơn rõ ở cold và hard reload. Cantos là CSR, revalidate asset mỗi lượt
   (`max-age=0, must-revalidate`) nhưng gửi WASM ở dạng Brotli (1,27 MB qua mạng). Bảng ở dưới.
6. **BFCache không bao giờ được dùng**: `MainResourceHasCacheControlNoStore` và
   `JsNetworkRequestReceivedCacheControlNoStoreResource`. Back từ trang khác vì vậy là một
   document mới cộng một `GET /api/v1/works`, đúng thiết kế xác minh history của READ-02. Cantos
   cũng bị chặn với cùng lý do.

Môi trường này **không tái hiện** cảm nhận “lần sau vẫn chậm” ở trường hợp cùng release và cache
còn nguyên. Ba cơ chế đã đo được có thể giải thích trải nghiệm đó: (a) mỗi lần phát hành đưa người
dùng về lại cold load 8,72 MB không nén; (b) trên máy chậm, có 0,27–0,60 s main thread bị chiếm sau
khi nội dung đã hiện; (c) đuôi mạng tới edge thỉnh thoảng lên hàng trăm ms đến vài giây cho mỗi
document `no-store` và mỗi API. Phiên đăng nhập, Bàn viết và thiết bị thật chưa đo (xem
[Giới hạn](#giới-hạn-và-phần-chưa-đo)).

## Kiểu truy cập

| Nhãn | Thao tác (CDP) | HTTP cache | Ghi chú |
| --- | --- | --- | --- |
| cold | Profile mới, `Page.navigate` | Trống | Lần đầu tiên |
| reload thường | `Page.reload` (`ignoreCache: false`) cùng tab | Còn | Navigation `reload` |
| mở lại tab | Đóng tab, mở tab mới cùng tiến trình trình duyệt, navigate | Còn (memory + disk) | Renderer mới |
| mở lại trình duyệt | `Browser.close`, khởi động lại Chrome cùng profile, navigate | Chỉ disk | Kết nối và tiến trình mới |
| Back từ trang khác | Navigate sang `about:blank` rồi Back | Còn | `back_forward`, BFCache bị từ chối |
| hard reload | `Page.reload` (`ignoreCache: true`) | Bỏ qua | Đối chứng “lần đầu sau phát hành”: mọi URL `/_v/<sha>/` đều mới |
| SPA | Bấm liên kết `/ban-viet` rồi `history.back()` trong cùng document | — | Cantos: liên kết tác phẩm `#work/:uuid` |

TTFB lấy từ Navigation Timing `responseStart`. Hydrated là lúc `<html data-hydrated>` xuất hiện
(set từ một Effect sau mount). Mốc này **không** chứng minh mọi dữ liệu hay mọi tương tác đã sẵn
sàng. WASM instantiate đo quanh `WebAssembly.instantiateStreaming` (đọc body, compile,
instantiate). “Byte qua mạng” là tổng `encodedDataLength` của CDP. Mốc “list” của Cantos là lần
đầu `.theatre-public-work-link` xuất hiện. Probe gồm wrapper WebAssembly và poll
`Runtime.evaluate` 100 ms một lần, nên mọi con số có kèm chi phí này. Lượt có trace chạy riêng và
không được tính vào bảng.

## Vink trên production

Desktop, CPU 1×, n = 10 ([vink-desktop.json.gz](vink-desktop.json.gz)):

| Kiểu | TTFB | FCP | hydrated | WASM instantiate | long task Σ | byte qua mạng | static từ cache |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| cold (profile mới) | 469 (244–693) | 1082 (788–1568) | 2330 (1694–4667) | 663 (367–2787) | 117 (96–132) | 8.72 MB | 0% |
| reload thường | 260 (172–682) | 314 (232–736) | 391 (307–819) | 68 (58–99) | 0 (0–52) | 6.5 KB | 100% |
| mở lại tab | 215 (154–336) | 300 (236–432) | 463 (402–593) | 57 (47–66) | 136 (112–175) | 6.5 KB | 100% |
| mở lại trình duyệt | 460 (232–1200) | 566 (348–1288) | 736 (459–1460) | 56 (47–61) | 131 (113–155) | 6.5 KB | 100% |
| Back từ trang khác | 201 (159–736) | 248 (216–796) | 321 (278–876) | 64 (41–74) | 0 (0–0) | 7.0 KB | 100% |
| hard reload (≈ sau phát hành) | 234 (182–514) | 704 (600–1656) | 1605 (1504–6861) | 584 (389–4909) | 0 (0–0) | 8.72 MB | 0% |

Mobile mô phỏng, CPU 4×, n = 10 ([vink-mobile.json.gz](vink-mobile.json.gz)):

| Kiểu | TTFB | FCP | hydrated | WASM instantiate | long task Σ | byte qua mạng | static từ cache |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| cold (profile mới) | 493 (299–2141) | 1132 (896–4784) | 2656 (2206–12619) | 672 (344–7044) | 503 (402–628) | 8.72 MB | 0% |
| reload thường | 202 (170–277) | 350 (316–428) | 718 (667–811) | 183 (171–209) | 268 (256–302) | 6.5 KB | 100% |
| mở lại tab | 300 (190–456) | 528 (424–720) | 1254 (1099–1543) | 199 (176–220) | 604 (584–649) | 6.5 KB | 100% |
| mở lại trình duyệt | 511 (375–1363) | 742 (608–1616) | 1477 (1256–2266) | 187 (171–218) | 567 (527–727) | 6.5 KB | 100% |
| Back từ trang khác | 243 (192–550) | 372 (324–676) | 725 (618–1059) | 158 (148–178) | 289 (266–317) | 7.0 KB | 100% |
| hard reload (≈ sau phát hành) | 256 (185–416) | 906 (700–1260) | 2370 (1703–4370) | 870 (600–2714) | 182 (166–226) | 8.72 MB | 0% |

SPA `/ban-viet`: đổi URL sau 30 (28–37) ms và DOM yên sau 23 (22–31) ms ở
desktop, 101 (91–148) ms và 77 (71–111) ms ở mobile mô phỏng, không có request.
Quay lại `/` trong document gọi một `GET /api/v1/tags`. Sau hydrate, request
`/api/v1/auth/session` chờ 153 (113–351) ms ở reload desktop. Request này không chặn
nội dung SSR.

## So sánh với Cantos

`https://cantos.cc/` (CSR Leptos/Trunk, Firebase Hosting sau Cloudflare SIN) được đo bằng cùng script,
máy, mạng và cấu hình Chrome, chạy trước Vink khoảng 15–30 phút, không xen kẽ từng mẫu. Số liệu ghi
median ms, n = 10 ([desktop](cantos-desktop.json.gz), [mobile](cantos-mobile.json.gz)).

| Kiểu | Vink FCP / hydrated (desktop) | Cantos FCP / list (desktop) | Vink FCP / hydrated (mobile mô phỏng) | Cantos FCP / list (mobile mô phỏng) | byte qua mạng Vink / Cantos |
| --- | ---: | ---: | ---: | ---: | ---: |
| cold (profile mới) | 1082 / 2330 | 1154 / 1273 | 1132 / 2656 | 1266 / 1406 | 8.72 MB / 1.43 MB |
| reload thường | 314 / 391 | 662 / 714 | 350 / 718 | 724 / 780 | 6.5 KB / 11.3 KB |
| mở lại tab | 300 / 463 | 578 / 636 | 528 / 1254 | 934 / 1064 | 6.5 KB / 10.9 KB |
| mở lại trình duyệt | 566 / 736 | 1268 / 1372 | 742 / 1477 | 1190 / 1318 | 6.5 KB / 10.9 KB |
| Back từ trang khác | 248 / 321 | 322 / 380 | 372 / 725 | 424 / 482 | 7.0 KB / 4.5 KB |
| hard reload (≈ sau phát hành) | 704 / 1605 | 940 / 973 | 906 / 2370 | 1140 / 1198 | 8.72 MB / 1.41 MB |

Vink có nội dung sớm hơn nhờ SSR. Trên desktop, ở mọi kiểu có cache, Vink hydrate xong trước khi
Cantos hiện danh sách. Ở mobile mô phỏng, reload vẫn sớm hơn, nhưng khi mở lại tab hoặc trình duyệt
thì Vink hydrated muộn hơn danh sách Cantos 150–190 ms vì phải lazy-compile WASM. Cantos thắng rõ khi
phải tải asset (cold, hard reload), chênh 0,6–1,3 s. **VOTHub `NOT_RUN`:** URL production lấy từ runbook vận hành của chủ sở
hữu không phân giải DNS ngày 24/11 (cả resolver hệ thống lẫn DoH), nên không có số so sánh và không
đoán domain khác.

## Phân rã TTFB theo chặng

[`edge-decompose.mjs`](edge-decompose.mjs) chạy `curl` HTTP/2 dùng lại một kết nối, đo thời gian từ
gửi request đến byte đầu, bỏ request mở kết nối mới. Ba lượt 15/20/20 vòng xen kẽ mọi chặng được gộp
lại ([1](edge-1.json), [2](edge-2.json), [3](edge-3.json)), đơn vị ms:

| Chặng | n | median | IQR | min–max | median từng lượt |
| --- | ---: | ---: | ---: | ---: | --- |
| Chỉ edge Cloudflare (`/cdn-cgi/trace`) | 52 | 82 | 66–144 | 58–799 | 96 / 68 / 91 |
| Worker + Pages, asset SVG chưa cache | 55 | 146 | 96–258 | 74–1204 | 139 / 88 / 192 |
| Worker + origin + App, không DB (`/health/live`) | 55 | 173 | 133–289 | 109–1243 | 181 / 130 / 199 |
| `GET /api/v1/auth/session` (khách) | 55 | 183 | 142–247 | 106–1045 | 179 / 191 / 179 |
| `GET /api/v1/tags` | 55 | 168 | 126–209 | 109–1492 | 148 / 167 / 181 |
| `GET /api/v1/works` | 55 | 187 | 141–251 | 114–1362 | 175 / 155 / 214 |
| SSR không Async (`/ban-viet`) | 55 | 220 | 188–297 | 109–776 | 216 / 191 / 258 |
| SSR không Async (`/toi`) | 55 | 223 | 167–388 | 110–1219 | 223 / 175 / 258 |
| SSR Async trang chủ (`/`, hai resource) | 55 | 243 | 190–390 | 137–1013 | 286 / 221 / 262 |

Sàn (min) cho thấy cấu trúc: edge 58 ms → origin 109 ms → SSR Async trang chủ 137 ms. Median và đuôi
của mọi chặng biến động theo mạng; ngay cả chặng chỉ có edge cũng có mẫu 799 ms. Worker → Pages cho
asset chưa cache cộng thêm khoảng 60 ms median. Khi cache còn nguyên thì chặng này không xuất hiện.

## Byte và mã hóa của release live

[`assets.json`](assets.json) đo bằng cùng tham số với `measureCompressedBytes` của harness #234:

| File | Byte qua mạng hiện tại | gzip -6 | Brotli q6 | Brotli q11 |
| --- | ---: | ---: | ---: | ---: |
| `vink_bg.wasm` | 7.940.123 (identity) | 2.321.708 | 1.872.321 | 1.625.497 |
| `vink.css` | 227.087 (identity) | 40.168 | 36.532 | 31.700 |
| `vink.js` | 97.848 (identity) | 14.397 | 13.180 | 11.672 |

Code section của WASM chiếm 7.182.421 byte, data section 645.099 byte. Không có name section hay
DWARF. Pipeline release hiện chỉ chạy `cargo --profile wasm-release` (`opt-level = "z"`, LTO,
một codegen unit) rồi `wasm-bindgen --remove-name-section`, không chạy `wasm-opt`
([xtask/src/build.rs](../../../../xtask/src/build.rs); image production gọi cùng lệnh trong
[infra/Containerfile](../../../../infra/Containerfile)). Header live của mọi file `/_v/` đều là
`public, max-age=31536000, immutable, no-transform`, `cf-cache-status: DYNAMIC` và không có
`Content-Encoding`.

## Đối chứng cục bộ: cùng byte, identity hay Brotli

[`compression-control.mjs`](compression-control.mjs) phục vụ đúng HTML, câu trả lời phiên khách và
17 asset mà một cold load live đã tải ([`mirror.mjs`](mirror.mjs); chỉ nonce trong HTML khác
nhau), trên loopback. Script nén Brotli xong trước mẫu đầu tiên rồi mới đo. Mỗi mẫu xóa cache, mạng
mô phỏng qua CDP, n = 5 mỗi hàng ([desktop](control-desktop.json), [mobile](control-mobile.json)):

| Mạng mô phỏng | CPU | Mã hóa asset | FCP | WASM sẵn sàng | hydrated | byte qua mạng |
| --- | ---: | --- | ---: | ---: | ---: | ---: |
| 10 Mbps / 60 ms | 1× | identity (như live) | 924 (916–932) | 7238 (7196–7295) | 7322 (7275–7383) | 8.72 MB |
| 10 Mbps / 60 ms | 1× | Brotli q5 | 424 (420–432) | 2223 (2184–2275) | 2299 (2255–2354) | 2.34 MB |
| 10 Mbps / 60 ms | 1× | Brotli q11 | 396 (392–396) | 2015 (1966–2039) | 2081 (2049–2110) | 2.06 MB |
| 50 Mbps / 20 ms | 1× | identity (như live) | 280 (264–292) | 1766 (1701–1865) | 1854 (1785–1953) | 8.72 MB |
| 50 Mbps / 20 ms | 1× | Brotli q5 | 240 (220–260) | 622 (591–633) | 702 (670–710) | 2.34 MB |
| 50 Mbps / 20 ms | 1× | Brotli q11 | 224 (220–228) | 574 (564–595) | 663 (634–692) | 2.06 MB |
| 10 Mbps / 60 ms | 4× | identity (như live) | 992 (980–1044) | 7278 (7270–7299) | 7581 (7578–7622) | 8.72 MB |
| 10 Mbps / 60 ms | 4× | Brotli q11 | 416 (412–436) | 2040 (2015–2104) | 2330 (2287–2395) | 2.06 MB |

Đối chứng chỉ thay đúng một biến (mã hóa của asset), nên nó chứng minh nén là yếu tố chi phối của
cold và “lần đầu sau phát hành”. Nó không chứng minh mức nén Cloudflare sẽ dùng (q5 chỉ là ước lượng
cho nén động), cũng không đại diện cho mạng di động thật.

## Main thread và WebAssembly

Trace V8/Blink nằm trong [vink-desktop-trace.json.gz](vink-desktop-trace.json.gz),
[vink-mobile-trace.json.gz](vink-mobile-trace.json.gz) và
[cantos-desktop-trace.json.gz](cantos-desktop-trace.json.gz), mỗi file 2 mẫu cho reload và mở lại
trình duyệt. Chỉ lưu tổng theo thread/category/tên event.

- Reload cùng tab dùng lại native module trong renderer (`wasm.GetNativeModuleFromCache`), nên
  không lazy-compile và hydrate chỉ còn 11–17 ms desktop, 45–65 ms mobile mô phỏng.
- Renderer mới (mở lại tab hoặc trình duyệt) phải validate streaming (103–116 ms desktop,
  179–201 ms mobile, chạy trên worker) và lazy-compile 5.410 hàm trong lúc hydrate (83–98 ms
  desktop, 318–325 ms mobile, chạy trên main thread).
- Có `wasm.SerializeModule` trong reload trước đó của cùng profile, nhưng cả 4 lần mở lại trình
  duyệt đều không có `wasm.Deserialize`. Trace Cantos có một lần `Deserialize`. Nguyên nhân vẫn
  `UNKNOWN`. Kích thước module là một giả thuyết, chưa kiểm.

## Giả thuyết và trạng thái

| Giả thuyết | Trạng thái | Bằng chứng |
| --- | --- | --- |
| Reload hoặc mở lại tải lại bundle cùng release | **Loại trừ** | 100% static từ cache, khoảng 6,5 KB qua mạng; `immutable` có hiệu lực |
| Asset không nén (cold, sau phát hành) | **Xác nhận** | Header live; đối chứng 10 Mbps: 7.322 → 2.081 ms; hard reload 1.605 ms so với Cantos 973 ms |
| Round trip document chi phối reload khi cache còn nguyên | **Xác nhận** (desktop) | TTFB chiếm khoảng 66% thời gian tới hydrated ở reload desktop (260/391); phân rã theo chặng |
| SQL/SSR chậm | **Loại trừ là yếu tố chính** | SSR Async chỉ thêm 20–70 ms median so với `/health/live` và SSR không Async; chưa probe critical/secondary/query |
| WASM compile và hydrate chiếm main thread | **Xác nhận** trên mobile mô phỏng | Long task 0,27–0,60 s; trace lazy-compile 5.410 hàm; chưa đo thiết bị thật |
| Phiên khách chặn nội dung | **Loại trừ** | Nội dung SSR có trước request session; request này bắt đầu sau hydrate và chờ khoảng 150 ms |
| Waterfall đăng nhập trên Bàn viết | **Thiếu bằng chứng runtime** | Source: [`observe`](../../../../apps/vink-web/src/account/browser.rs) gọi `session` rồi `session/account`, sau đó [Bàn viết](../../../../apps/vink-web/src/pages/desk.rs) mới tải danh sách khi có `session.scope`: ba round trip nối tiếp sau hydrate, mỗi chặng API khoảng 170–190 ms median từ máy này |
| Fetch trùng sau fresh hydrate (READ-01/02) | **Loại trừ** cho trang chủ | Reload và mở lại không có `/api/v1/works` hay `/api/v1/tags`; Back có đúng một `/api/v1/works` theo thiết kế |
| BFCache | **Xác nhận không dùng** | Lý do Chrome nêu ở trên; đổi `no-store` cho HTML là quyết định privacy/freshness |
| Regression do Tailwind (#423) | **Không quy kết** | CSS 227 KB identity góp vào cold/sau phát hành, còn 31,7 KB nếu nén; chưa so trước/sau |
| Cache bị đẩy ra trên điện thoại | **UNKNOWN** | Chưa đo trên thiết bị; entry 7,9 MB identity lớn gấp khoảng 4,9 lần entry Brotli q11 |

## Lựa chọn `CHOICE-01`

Đã chọn **A**: Navigation/Resource Timing, CDP Network và trace có sẵn trong Chrome trên production,
cộng một phép đối chứng cục bộ không động tới code sản phẩm. Không cần B: A đã quy được từng khoảng
chờ về owner (mạng/edge, Worker → origin, cache tĩnh, compile/hydrate WASM, phiên). Khoảng chưa
tách là critical/secondary/query trong SSR. Nó không đáng một probe, vì SSR trang chủ chỉ cao hơn
`/health/live` vài chục ms. Không sửa harness `perf-baseline` hay code sản phẩm. Script đo nằm
trong thư mục evidence này, giống #234/#235.

## Việc sửa đề xuất, theo thứ tự tác động đã đo

1. **Nén asset `/_v/<sha>/`** (cold và lần đầu sau mỗi phát hành), triển khai ở
   [#487](https://github.com/loveoverflowcom/vink/issues/487). Owner:
   `infra/cloudflare/production/worker.mjs`, runbook `infra/production/cloudflare-edge.md`, và
   bundle của `xtask` nếu chọn tiền nén. Hai hướng khả thi:
   - *Bỏ `no-transform` chỉ cho asset immutable* để zone tự nén. Thay đổi nhỏ nhất. ETag có thể
     thành weak; Worker đã bỏ `W/` khi so `If-None-Match`, nhưng vẫn phải kiểm 304 thật. Tỷ lệ
     nén do Cloudflare quyết định (đối chứng q5: 2,34 MB toàn cold).
   - *Tiền nén Brotli q11 (và gzip) lúc build*: Worker chọn biến thể theo `Accept-Encoding`, trả
     `Content-Encoding` và `Vary: Accept-Encoding`, ETag riêng cho mỗi biến thể, giữ
     `no-transform`. Byte nhỏ nhất và xác định (2,06 MB toàn cold), nhưng đổi inventory, smoke và
     số file trên Pages.
   - Ràng buộc: không gửi cookie hay `Authorization` tới Pages, giữ `X-Vink-Asset-Revision`,
     không áp Cache Everything cho HTML.
   - Kiểm xác nhận: `curl -sS -D - -o /dev/null -H 'Accept-Encoding: br' https://vink.forum/_v/<sha>/pkg/vink_bg.wasm`
     phải có `content-encoding: br` và khoảng 1,6–1,9 MB. Sau đó chạy lại `reload-matrix.mjs` với
     cold và hard reload (kỳ vọng khoảng 2,1–2,4 MB và hydrated giảm), reload và mở lại không đổi,
     `node --test infra/cloudflare/production/worker.test.mjs`, cùng smoke ETag/304 qua
     `identity`/`gzip`/`br` theo runbook.
   - Rủi ro: nén hai lần, ETag/304 sai, Pages trả HTML thay asset. Rollback bằng Worker trước đó.
2. **Giảm việc main thread sau khi nội dung đã hiện, trên máy chậm**
   ([#490](https://github.com/loveoverflowcom/vink/issues/490)). Trước hết đo trên Android
   thật. Hướng: thử `wasm-opt`, tìm crate chiếm code section, tách phần không cần cho trang đầu.
   Kiểm bằng long task Σ và hydrated của mở lại tab/trình duyệt trên mobile; không đổi SSR/no-JS.
3. **Đo phiên đăng nhập trên Bàn viết trước khi sửa**
   ([#492](https://github.com/loveoverflowcom/vink/issues/492)). Cần một phiên production do chủ sở hữu cho
   phép, hoặc App/PostgreSQL scratch có latency injection. Nếu ba round trip nối tiếp được xác nhận,
   các hướng là gộp state và account vào một response, hoặc để SSR cung cấp standing đã kiểm. Phải
   giữ session fencing, `no-store` và không trộn principal.
4. **Không đề xuất** cache HTML ở edge hay bỏ `no-store` để bật BFCache nếu chưa có quyết định
   privacy/withdrawal riêng. Streaming SSR đã dừng ở #235.
5. Sau mục 1: hash theo từng file sẽ giữ cache cho font/ảnh không đổi giữa các phát hành (khoảng
   0,4 MB). WASM hầu như đổi ở mỗi build
   ([#491](https://github.com/loveoverflowcom/vink/issues/491)).

## AC → bằng chứng → gap

| AC | Bằng chứng | Còn thiếu |
| --- | --- | --- |
| 01 | Live `e9872556` (HTML, `X-Vink-Asset-Revision`, ETag = SHA-256), source `7baae1e`, máy/mạng/Chrome, route `/`, catalog trống, phiên khách | Reader có dữ liệu thật, phiên đăng nhập |
| 02 | Cold, reload, mở lại tab, mở lại trình duyệt, SPA, Back (`back_forward`, BFCache có nhãn), hard reload; n = 10, median/min–max; FCP và hydrated | Thiết bị di động thật |
| 03 | Phân rã HTML/origin (edge-*.json, Navigation Timing), asset/mã hóa (header, `assets.json`), WASM/main thread (probe, trace), session khách | Waterfall Bàn viết khi đăng nhập |
| 04 | Đối chứng identity/Brotli cùng byte; reload so với hard reload; SSR so với `/health/live`; Cantos cùng điều kiện | Mạng di động thật. Mức nén thật của Cloudflare đo sau ở #487 (tương đương Brotli q4) |
| 05 | Cantos: URL, route, máy, mạng, Chrome và script giống nhau | VOTHub `NOT_RUN` (DNS); Cantos không xen kẽ từng mẫu với Vink |
| 06 | Thứ tự sửa, owner, hướng, ràng buộc, kiểm xác nhận, rủi ro/rollback ở trên | Không triển khai; mục 2–3 cần đo trước |

## Cách chạy lại

Từ gốc repo, với Chrome trong `PATH` hoặc `--chrome`:

```sh
E=docs/development/evidence/web-reload-473
node $E/reload-matrix.mjs --url https://vink.forum/ --samples 10 --device desktop --marker data-hydrated --spa-href /ban-viet --out vink-desktop.json
node $E/reload-matrix.mjs --url https://vink.forum/ --samples 10 --device mobile --marker data-hydrated --spa-href /ban-viet --out vink-mobile.json
node $E/reload-matrix.mjs --url https://cantos.cc/ --samples 10 --device desktop --spa-selector .theatre-public-work-link --ready-selector .theatre-public-work-link --out cantos-desktop.json
node $E/reload-matrix.mjs --url https://vink.forum/ --samples 2 --device mobile --marker data-hydrated --spa-href /ban-viet --trace true --out vink-mobile-trace.json
node $E/edge-decompose.mjs --origin https://vink.forum --sha <live-sha> --rounds 20 --out edge.json
node $E/assets.mjs vink_bg.wasm vink.js vink.css
node $E/mirror.mjs --origin https://vink.forum --ledger vink-desktop.json --out "$MIRROR"
node $E/compression-control.mjs --mirror "$MIRROR" --samples 5 --modes 0,5,11 --cpu 1 --conditions '[{"name":"sim-10mbps-60ms","downMbps":10,"upMbps":2,"latencyMs":60}]' --out control.json
node $E/summarize.mjs vink-desktop.json control.json
```

Mọi lệnh production chỉ đọc trang công khai như một khách. Output chỉ chứa đường dẫn đã làm sạch
(chỉ giữ tên tham số, UUID thay bằng `:uuid`), thời gian, byte, status và một số header kỹ thuật,
không có cookie, giá trị header khác, body hay storage. [SHA256SUMS](SHA256SUMS) ghim mọi file
trong thư mục.

## Giới hạn và phần chưa đo

- Một máy, một mạng, một khung giờ. Đuôi phân phối phụ thuộc đường mạng quốc tế và không phải RUM.
- Mobile là CPU throttle 4× của máy desktop, không phải điện thoại. Chưa đo cache eviction, nhiệt
  hay mạng di động thật.
- Trang chủ đang là empty state. Reader production và phiên đăng nhập `NOT_RUN`; không tạo nội
  dung hay tài khoản trên production để lấp gap.
- Probe và poll CDP làm tăng thời gian. Hydrated chỉ là marker, không đại diện cho dữ liệu riêng
  đã sẵn sàng.
- `GITHUB_CI`: `NOT_REQUIRED`. Không chạy `cargo xtask verify` vì không đổi code sản phẩm hay
  harness.
