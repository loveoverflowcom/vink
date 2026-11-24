# #487: nén asset `/_v/<sha>/` qua zone Cloudflare

Đo ngày 24/11/2026 (giờ Việt Nam). Base `develop` `82dd51b5`. Production đang chạy release
`e98725569657502e5701afeb3499c8b87ed1d84d` với Worker cũ. Bản sửa **chưa deploy**. Báo cáo gốc là
[điều tra #473](../web-reload-473/README.md) (gộp từ PR #481). Thư mục này giữ phép kiểm riêng trên production (chỉ đọc,
phiên khách), replay lab trước/sau cùng corpus và smoke edge.

**Bản sửa (lựa chọn A).** [Worker](../../../../infra/cloudflare/production/worker.mjs) giữ
`public, max-age=31536000, immutable` cho asset `/_v/<sha>/pkg/`. Với `text/javascript`,
`text/css`, `application/wasm`, `image/svg+xml`, `application/manifest+json`, `application/json`
và `text/plain`, Worker bỏ `no-transform` và thêm `Vary: Accept-Encoding` để zone nén. Font và ảnh
giữ `no-transform`. Worker không tự nén và không gửi header của client sang Pages. ETag vẫn là
SHA-256 của byte inventory, và so `If-None-Match` theo phép so yếu như trước. READ-01, HTML, SSR/API,
cookie và fallback `r26` không đổi. Proposal kiến trúc:
[`VDR-EDGE-ASSET-COMPRESSION@r1`](../../../../decisions/proposals/VDR-EDGE-ASSET-COMPRESSION.r1.toml)
(`PENDING`).

## Kết luận

1. **`no-transform` là nguyên nhân duy nhất.** Trên production, `/_v/<sha>/` và alias `/pkg/` cùng đi
   qua nhánh Worker `inventory/r26`, cùng gọi `env.ASSETS.fetch` không kèm `Accept-Encoding`. Điểm
   khác duy nhất là `Cache-Control`. Alias (`no-cache`) được nén Brotli/gzip theo request, còn
   `/_v/<sha>/` (`…, no-transform`) luôn trả identity. Việc không gửi `Accept-Encoding` tới Pages
   không ảnh hưởng mã hóa trả cho trình duyệt, nên giữ nguyên.
2. **Mức nén của zone tương đương Brotli q4 với cửa sổ 128 KiB**, lệch không quá 1% trên WASM, CSS
   và JS. WASM của release live sẽ còn khoảng 2,2 MB (Brotli q11 cho 1,63 MB).
3. **Replay lab ở 10 Mbps/60 ms:** hydrated của cold giảm từ 7.292 ms xuống 2.488 ms (desktop, CPU
   1×) và từ 7.556 ms xuống 2.741 ms (mobile mô phỏng, CPU 4×). Lần đầu sau phát hành và hard reload
   giảm tương tự. Byte qua mạng giảm từ 8,71 MB xuống 2,64 MB. **Đạt mục tiêu TTI cold khoảng 2–3
   giây** trong cùng điều kiện benchmark.
4. **Reload và mở lại không đổi:** vẫn 100% asset từ cache, khoảng 5,3 KB qua mạng, hydrated chênh
   trong phạm vi nhiễu. Long task trên CPU chậm gần như không đổi. Riêng mở lại trình duyệt trên
   mobile, long task Σ tăng từ 570 lên 636 ms (min 557 → 611) trong khi hydrated chỉ đổi 893 → 903
   ms. Chưa loại trừ chi phí đọc bản nén từ disk cache; cần trace trong
   [#490](https://github.com/loveoverflowcom/vink/issues/490).
5. **Tham chiếu B (tiền nén q11):** nhanh hơn A thêm khoảng 0,4 s (2.099 / 2.281 ms, 2,05 MB). Mức
   này không cần để đạt mục tiêu, nhưng phải đổi image/packager, inventory và smoke của controller.
   B được ghi vào [#491](https://github.com/loveoverflowcom/vink/issues/491) cùng hash theo từng
   file.

## Kiểm riêng `no-transform` và `Accept-Encoding` trên production

[`encoding-probe.mjs`](encoding-probe.mjs) dùng `curl` HTTP/2, phiên khách, release `e9872556`
([encoding-before.json](encoding-before.json)). Byte giải nén luôn có SHA-256 bằng ETag. Với mỗi
biến thể, `If-None-Match` mạnh và `W/` đều nhận 304.

| File | Đường | Cache-Control | `br, gzip, zstd` | `gzip` | `identity` |
| --- | --- | --- | ---: | ---: | ---: |
| `vink_bg.wasm` | `/_v/<sha>/` | `public, max-age=31536000, immutable, no-transform` | identity 7.940.123 | identity 7.940.123 | 7.940.123 |
| `vink_bg.wasm` (bootstrap) | `/pkg/` | `no-cache` | **br 2.164.334** | **gzip 2.275.205** | 7.872.530 |
| `vink.css` | `/_v/<sha>/` | như trên | identity 227.087 | identity 227.087 | 227.087 |
| `vink.css` (bootstrap) | `/pkg/` | `no-cache` | **br 41.223** | **gzip 39.491** | 223.959 |
| `vink.js` | `/_v/<sha>/` | như trên | identity 97.848 | identity 97.848 | 97.848 |
| `vink.js` (bootstrap) | `/pkg/` | `no-cache` | **br 14.012** | **gzip 13.791** | 94.194 |
| `literata-variable.woff2` | cả hai | như trên / `no-cache` | identity 256.620 | identity 256.620 | 256.620 |

Khi nén, edge thêm `Vary: accept-encoding` và đổi ETag thành `W/"<sha256>"`. Với `identity`, ETag
vẫn mạnh và không có `Vary`. Edge chọn `br` dù request có `zstd`. Font không bao giờ được nén kể cả
khi không có `no-transform`, nên giữ `no-transform` cho font và ảnh chỉ để byte không đổi.

## Hiệu chỉnh mức nén của zone

Byte Brotli của Node `zlib` trên đúng ba file bootstrap mà Cloudflare đã nén ở bảng trên:

| File | Cloudflare | q3 (cửa sổ 4 MiB) | q4 (4 MiB) | **q4, cửa sổ 128 KiB** | q11 |
| --- | ---: | ---: | ---: | ---: | ---: |
| `vink_bg.wasm` | 2.164.334 | 2.146.216 | 2.048.363 | **2.181.167** (+0,8%) | 1.614.571 |
| `vink.css` | 41.223 | 43.467 | 41.156 | **41.218** (−0,0%) | 31.380 |
| `vink.js` | 14.012 | 14.535 | 14.075 | **14.076** (+0,5%) | 11.450 |

Replay dùng Brotli q4, `lgwin` 17 (128 KiB), và gzip mức 9 cho client không nhận `br`. Trên WASM
live, mức này cho 2.200.091 byte. Cloudflare nén theo luồng và có thể đổi theo PoP hoặc thời điểm.

## Replay lab trước và sau

[`edge-replay.mjs`](edge-replay.mjs) dựng lại đúng corpus của #473: HTML, câu trả lời phiên khách
và 17 asset của một cold load live `e9872556`, lấy bằng `mirror.mjs` của #473. Script chạy **module
Worker production không sửa** trong workerd (Miniflare 5.20261019) với inventory dựng từ byte đó.
Phía trước là một stand-in cho zone: nén Brotli/gzip một response 200 chưa có `Content-Encoding` khi
`Content-Type` nén được và `Cache-Control` không có `no-transform`, thêm `Vary` và đổi ETag sang
`W/`, đúng như hành vi đo ở trên. Byte nén được tính trước mẫu đầu, nên thời gian nén phía server
không nằm trong số đo.

Chrome 158 headless, mạng mô phỏng 10 Mbps xuống / 2 Mbps lên / 60 ms (như đối chứng #473).
Desktop 1280×900 với CPU 1×. Mobile mô phỏng 390×844 DPR 3 với CPU 4×. Mỗi mẫu một profile mới,
n = 5, median (min–max) tính bằng ms. Các kiểu truy cập:

- **cold:** HTTP cache trống.
- **reload:** reload thường.
- **mở lại tab:** renderer mới, cache memory + disk.
- **mở lại trình duyệt:** chỉ còn disk cache.
- **offline reload:** mạng tắt, kiểm document có tải được không.
- **sau phát hành:** HTML đổi sang namespace `/_v/<sha mới>/`, byte giữ nguyên, cache còn bản cũ.
  Đây là lần đầu người dùng quay lại sau một release.
- **hard reload:** bỏ qua cache.

“before” là Worker ở `82dd51b5`, “after” là Worker của nhánh này.

Desktop, CPU 1× ([before](replay-desktop-before.json.gz), [after](replay-desktop-after.json.gz),
[ref q11](replay-desktop-ref-q11.json.gz)):

| Kiểu | FCP before → after | hydrated before | hydrated after | byte qua mạng before → after | ref q11 hydrated · byte |
| --- | ---: | ---: | ---: | ---: | ---: |
| cold | 872 → 408 | 7.292 (7.276–7.329) | **2.488** (2.467–2.491) | 8,71 MB → 2,64 MB | 2.099 · 2,05 MB |
| reload | 120 → 112 | 193 (189–204) | 197 (185–206) | 5,3 KB → 5,3 KB | 190 |
| mở lại tab | 160 → 156 | 294 (276–309) | 298 (292–348) | 5,3 KB → 5,3 KB | 283 |
| mở lại trình duyệt | 176 → 172 | 321 (309–338) | 317 (309–329) | 5,3 KB → 5,3 KB | 315 |
| sau phát hành | 816 → 408 | 7.332 (7.242–7.357) | **2.451** (2.427–2.486) | 8,71 MB → 2,64 MB | 2.025 · 2,05 MB |
| hard reload | 820 → 392 | 7.212 (7.186–7.259) | **2.427** (2.384–2.441) | 8,71 MB → 2,64 MB | 1.970 · 2,05 MB |

Mobile mô phỏng, CPU 4× ([before](replay-mobile-before.json.gz), [after](replay-mobile-after.json.gz),
[ref q11](replay-mobile-ref-q11.json.gz)):

| Kiểu | FCP before → after | hydrated before | hydrated after | long task Σ before → after | ref q11 hydrated |
| --- | ---: | ---: | ---: | ---: | ---: |
| cold | 936 → 436 | 7.556 (7.540–7.579) | **2.741** (2.723–2.759) | 488 → 483 | 2.281 |
| reload | 200 → 200 | 545 (527–644) | 545 (513–588) | 276 → 272 | 532 |
| mở lại tab | 308 → 304 | 948 (852–981) | 879 (830–989) | 604 → 610 | 852 |
| mở lại trình duyệt | 284 → 308 | 893 (848–921) | 903 (872–1.033) | 570 → 636 | 879 |
| sau phát hành | 904 → 432 | 7.546 (7.531–7.648) | **2.691** (2.682–2.762) | 487 → 473 | 2.261 |
| hard reload | 860 → 436 | 7.338 (7.329–7.349) | **2.535** (2.474–2.602) | 200 → 195 | 2.060 |

WASM qua mạng: 7,94 MB identity → 2,20 MB `br`. Byte của các kiểu cold/sau phát hành/hard reload
và WASM instantiate (gồm thời gian tải) nằm trong output của [`summarize.mjs`](summarize.mjs). TTFB
cố định khoảng 63 ms do loopback cộng 60 ms latency mô phỏng. Đối chiếu với đối chứng của #473 trên
cùng byte: identity 7.322 ms ở đó so với 7.292 ms ở đây, q11 2.081 so với 2.099 ms (CPU 1×), q11
CPU 4× 2.330 so với 2.281 ms. Hai phép đo nhất quán.

## Không regression

| Hành vi | Bằng chứng | Kết quả |
| --- | --- | --- |
| Hydrate và WASM | Mọi mẫu của 6 lượt replay đặt `data-hydrated`, `instantiateStreaming` thành công, không có exception hay `console.error` | 30/30 load có hydrate mỗi lượt (6 kiểu × 5 mẫu) |
| Cache khi cùng release | reload, mở lại tab, mở lại trình duyệt: 100% asset từ cache, 5,3 KB qua mạng ở cả before và after | Không đổi |
| Offline | Không có Service Worker và HTML `no-store`: offline reload ra trang lỗi Chrome ở cả hai nhánh | Không đổi (5/5) |
| Header, 304, HEAD, alias, font/ảnh | [`worker.test.mjs`](../../../../infra/cloudflare/production/worker.test.mjs): test mới FAIL trên Worker cũ, PASS trên Worker mới | PASS 5/5 |
| HTML, SSR/API, cookie, READ-01, fallback `r26`, 404/405/502 | Các test sẵn có trong `worker.test.mjs`, không sửa | PASS |
| Packager, controller, bridge | `python3 -m unittest test_release` trong `infra/cloudflare/production` | PASS 19/19 |
| Phiên đăng nhập | Nhánh động của Worker không đổi; chưa đo với phiên thật | `NOT_RUN` |

## Smoke edge

[`smoke-edge.sh`](../../../../infra/production/smoke-edge.sh) `edge --release <sha>` giờ kiểm thêm ba
biến thể của `vink.js`, `vink.css`, `vink_bg.wasm` và hai font. Mỗi biến thể kiểm
`Content-Encoding`, `Cache-Control`, `Vary`, ETag bằng SHA-256 của byte giải nén và 304 với ETag vừa
nhận. Nếu `curl` không hỗ trợ Brotli, biến thể `br` chỉ được kiểm header.

- **Trên live (Worker cũ):** `FAIL … vink.js unexpected Cache-Control for identity`, đúng như kỳ vọng
  trước khi deploy ([smoke-live-before.txt](smoke-live-before.txt)). Các bước trước đó PASS.
- **Kiểm logic trên edge thật:** chạy cùng script, chỉ đổi đường dẫn sang alias `/pkg/` (được nén
  sẵn). `Cache-Control` kỳ vọng thành `no-cache`, và chỉ đòi `Vary` cho biến thể nén vì Worker cũ
  chưa gửi `Vary` ([diff](smoke-alias-logic.diff)). Kết quả PASS, exit 0
  ([smoke-alias-logic.txt](smoke-alias-logic.txt)).

## Rollback

Phát hành lại tree trước bằng cùng entry point theo
[runbook](../../../../infra/production/cloudflare-edge.md#cutover-và-khôi-phục). Worker cũ trả lại
`no-transform` và identity. Không cần dọn cache: URL bất biến không đổi, byte sau giải nén giống
nhau, ETag giữ cùng giá trị SHA-256. Nếu chỉ cần tắt nén khẩn cấp ở zone, Compression Rules của
Cloudflare cũng đưa asset về identity mà không đổi Worker. Smoke mới khi đó sẽ FAIL ở bước nén.

## Sau khi deploy (`NOT_RUN`)

Chỉ chạy sau khi bản sửa được nghiệm thu và phát hành theo runbook:

```sh
infra/production/smoke-edge.sh edge --release <sha>
node docs/development/evidence/web-compress-487/encoding-probe.mjs --out encoding-after.json
node docs/development/evidence/web-reload-473/reload-matrix.mjs --url https://vink.forum/ --samples 10 --device mobile --marker data-hydrated --spa-href /ban-viet --out vink-mobile-after.json
```

Kỳ vọng: `/_v/<sha>/pkg/vink_bg.wasm` trả `content-encoding: br`, khoảng 2,2 MB, ETag `W/`. Cold và
hard reload qua mạng khoảng 2,6 MB thay vì 8,72 MB. Reload và mở lại vẫn 100% từ cache.

## Cách chạy lại

Từ gốc repo, cần Node 22+, Chrome và `npm ci --prefix infra/cloudflare --ignore-scripts`.
`mirror.mjs` và ledger cold nằm trong [thư mục #473](../web-reload-473/).

```sh
E=docs/development/evidence/web-compress-487
node $E/encoding-probe.mjs --origin https://vink.forum --out encoding-before.json
node docs/development/evidence/web-reload-473/mirror.mjs --origin https://vink.forum \
  --ledger docs/development/evidence/web-reload-473/vink-desktop.json.gz --out "$MIRROR"
git show 82dd51b5:infra/cloudflare/production/worker.mjs > /tmp/worker-before.mjs
for device in desktop mobile; do
  node $E/edge-replay.mjs --mirror "$MIRROR" --worker /tmp/worker-before.mjs --device $device --samples 5 --out replay-$device-before.json
  node $E/edge-replay.mjs --mirror "$MIRROR" --device $device --samples 5 --out replay-$device-after.json
  node $E/edge-replay.mjs --mirror "$MIRROR" --device $device --samples 5 --zone-quality 11 --zone-lgwin 22 --out replay-$device-ref-q11.json
done
node $E/summarize.mjs encoding-before.json replay-*.json
```

Output chỉ chứa đường dẫn đã làm sạch (`<A>`/`<B>` thay SHA), thời gian, byte, status và một số
header kỹ thuật, không có cookie, body hay storage. [SHA256SUMS](SHA256SUMS) ghim mọi file trong thư
mục.

## Giới hạn

- Replay là lab trên loopback HTTP/1.1, mạng do Chrome mô phỏng, một máy Linux. Stand-in mô phỏng
  hành vi của zone, không phải Cloudflare. Header `Vary` của stand-in lặp `Accept-Encoding,
  accept-encoding`, việc này không ảnh hưởng cache của trình duyệt.
- Mobile là CPU throttle 4× của máy desktop, không phải điện thoại thật. Chưa đo mạng di động thật.
- Trạng thái sau deploy trên production `NOT_RUN` vì chưa deploy.
- Trang chủ đang là empty state. Phiên đăng nhập, Bàn viết và Reader có dữ liệu chưa đo.

## AC → bằng chứng → gap

| AC (#487) | Bằng chứng | Còn thiếu |
| --- | --- | --- |
| 01 Nén đúng | Probe production cho nhánh có `no-cache`; replay: WASM `br` 2,20 MB, byte giải nén = ETag; smoke logic PASS | Xác nhận `/_v/<sha>/` trên live sau deploy |
| 02 Header và cache | `worker.test.mjs` (Cache-Control, Vary, ETag mạnh/`W/`/`*`, HEAD, alias, font/ảnh); probe: 304 với ETag mạnh và `W/` | Smoke `--release` trên live sau deploy |
| 03 Không regression | Bảng ở trên; HTML/SSR/API không đổi | Phiên đăng nhập, thiết bị thật |
| 04 Đo lại | Replay before/after cold, reload, mở lại tab/trình duyệt, sau phát hành, hard reload, 10 Mbps, CPU 1× và 4× | `reload-matrix.mjs` trên live sau deploy |
| 05 Vận hành | Runbook, smoke, rollback; test Worker và controller; `verify quick` (xem PR) | — |
| 06 Phát hành | Chưa deploy | Deploy sau nghiệm thu, chạy mục “Sau khi deploy” |
