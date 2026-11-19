# Runbook Vink production: Cloudflare trước Caddy, giữ DNS Spaceship

**Quyết định lần chuẩn bị này: NO-GO cho cutover.** Chỉ có runbook và bằng chứng đọc công
khai; chưa đổi Cloudflare, DNS, VPS hay lưu lượng. PR tài liệu này không cấp quyền triển khai.

## Phạm vi và phương án

Giữ URL chuẩn `https://vink.forum`, cùng origin với trình duyệt cho SSR Leptos, `/api/*` và
`/pkg/*`. Giữ Caddy/HTTPS và VPS hiện tại; không thay image, Caddy, App, Identity, PostgreSQL,
firewall hay chính sách tin IP. `www.vink.forum` vẫn redirect về apex, giữ nguyên path/query.
Không đụng Cantos, `test.vink.forum`, `origin-test.vink.forum`, mail, DNSSEC hay các record
xác minh đang có. Không bật sign-in, SMTP hoặc evaluator.

| Phương án | Quyết định |
| --- | --- |
| Cloudflare reverse proxy **Partial Setup** + Spaceship **ALIAS @** | Ưu tiên có điều kiện. [Cloudflare chỉ hỗ trợ Partial ở Business/Enterprise](https://developers.cloudflare.com/dns/zone-setups/partial-setup/) và apex cần flattening hoặc tương đương. [Spaceship hướng dẫn nhập CNAME với Host `@` để DNS trả ALIAS](https://www.spaceship.com/knowledgebase/dns-alias-record/). Hai tài liệu xác nhận khả năng chung, không xác nhận plan, zone, target, chứng chỉ hoặc DNSSEC của zone này. |
| Giữ Spaceship → VPS | Tiếp tục dùng khi bất kỳ điều kiện GO nào chưa đạt. Không ghim IP anycast dùng chung làm record apex, không trộn A cũ với ALIAS, không chuyển nameserver. |
| Pages tại `www.vink.forum` | Chỉ là phương án riêng cho chủ sản phẩm cân nhắc: [Pages hỗ trợ CNAME của một subdomain trên DNS ngoài, nhưng apex Pages yêu cầu nameserver Cloudflare](https://developers.cloudflare.com/pages/configuration/custom-domains/). Nếu chuyển canonical sang `www`, cần kế hoạch riêng cho redirect, SSR/API/assets cùng origin, cookie/CSRF và SEO. Không đánh đồng Pages với reverse proxy của luồng này. |

[Runbook test](../cloudflare/README.md), [Worker](../cloudflare/worker.mjs) và
[packager](../cloudflare/package.py) chỉ là tham chiếu. Chúng ghi `noindex`, có cache policy
và cách chuyển Host riêng cho test. Không đưa chúng, project Pages test hoặc upstream
`origin-test` vào production. Tham khảo cách tách [ba nhóm phục hồi của Cantos
tại `a1fca178`](https://github.com/loveoverflowcom/cantos/blob/a1fca178/docs/deployment/cloudflare-rollout.md#domain-plan-and-three-recoveries),
không sao chép topology Pages của Cantos.

## Baseline đọc công khai · 19/11/2026, 13:42–13:44 UTC

Nguồn kiểm tra: `develop` tại `eb7e65ee8a42d512343eb657abdd29cdec3d4f89`;
[runbook production](README.md), [Caddy import](vink-prod.caddy) và
[Compose](compose.yaml) của [PR #304](https://github.com/loveoverflowcom/vink/pull/304).
SHA là nguồn đối chiếu tại thời điểm đo, **không** phải SHA image đã xác thực trên VPS.

| Quan sát trực tiếp | Kết quả |
| --- | --- |
| `dig` vào từng authoritative NS và resolver; `dig +dnssec` | Cả hai NS trả cùng apex A; không thấy AAAA, CNAME hay CAA ở apex; DS parent (key tag 25485, alg 13, digest type 2) và DNSKEY hiện diện. Không phải snapshot đầy đủ zone hoặc chứng nhận ALIAS chạy được với DNSSEC. |
| `curl` HTTPS public và `--resolve` đúng Host/SNI tới VPS | Apex trả `200`, peer `42.96.18.245`; TLS verify 0. `openssl s_client` xác minh tên `vink.forum`, SAN đúng tên, CA Let's Encrypt YE2, hạn 16/02/2027 11:31:26 UTC. `www` direct HTTPS cũng xác thực được. Chưa chứng minh gia hạn sau khi proxy. |
| `GET /`, `GET /toi?edge_smoke=1` | `200` HTML SSR. Trang chủ có nội dung “Khám phá”, canonical `https://vink.forum/`, không thấy robots meta hoặc `X-Robots-Tag` noindex. `/toi` và `/ban-viet` có meta `noindex`. HTML khai báo JS/CSS/WASM và hydrate; chưa chạy hydration. Đây là tín hiệu cho crawler, không xác nhận đã được Google lập chỉ mục. |
| `GET /health/live`, `GET /health/ready` | `200` JSON; App báo `live`/`ready`, `database=ok`, `role=restricted`, `schema=current`, `identity=no_authority`. Không gọi đăng nhập, POST hoặc dùng session. |
| `GET /api/v1/works` và `If-None-Match` | `200` JSON, danh sách rỗng, strong ETag, `Cache-Control: private, no-cache`; cùng ETag trả `304`, 0 byte và giữ cache policy. Không có mẫu Reader đã xuất bản để kiểm `200`; không seed dữ liệu. |
| URL sai và indexing | URL Web ngẫu nhiên, Reader với UUID không có và asset JS không có đều `404` (asset lỗi hiện là HTML, không phải asset `200`); API sai `404` JSON. `/robots.txt` và `/sitemap.xml` hiện `404`; không thêm/bật file ở edge trong đợt này. |
| Assets | HEAD JS/CSS/WASM/font `200` với MIME `text/javascript`/`text/css`/`application/wasm`/`font/woff2`. JS GET gzip thật; HEAD CSS/WASM báo gzip. WASM HEAD identity: 125.610.054 byte; probe Range bị origin trả `200` full và curl dừng theo giới hạn tải. Không đánh dấu PASS cho full WASM GET/hash hoặc `206`. |
| Redirect và đối chứng | HTTP apex `308` lên HTTPS. HTTPS `www/toi?edge_smoke=1` `301` về đúng apex, một hop đến `200`. HTTPS trang chủ test `200` qua Cloudflare, có `X-Robots-Tag: noindex, nofollow`; trang chủ Cantos `200`. Không thay đổi hoặc mở phiên ở hai bề mặt này. |

## DNS trước, dự kiến và rollback

Đọc hai NS vào thời điểm thao tác và chụp snapshot **toàn zone** trong Spaceship, kể cả TTL,
NS/SOA, DS/DNSKEY, CAA, mail/MX/SPF/DKIM/DMARC và record xác minh. `dig` công khai không
thay được snapshot. Lưu riêng, không đưa giá trị xác minh, credentials hoặc HAR vào Git.

| Chủ quản / tên / loại | Trước (đã quan sát công khai) | Sau (chỉ khi GO) | Khi rollback về baseline |
| --- | --- | --- | --- |
| Spaceship `@ A` | `42.96.18.245`, TTL 300 | Thay riêng A bằng ALIAS @ trỏ đến **target chính xác Cloudflare cấp cho apex**, TTL 300 nếu được hỗ trợ. Chưa có target được xác minh. | Bỏ ALIAS @, đặt lại `A 42.96.18.245`, TTL 300, theo fresh snapshot. |
| Spaceship `@ AAAA/CNAME` | Không có câu trả lời AAAA/CNAME | Không tạo record AAAA/CNAME DNS thô ở apex. Đọc A/AAAA tổng hợp, và HTTPS RR nếu có; xác minh cả IPv6 nếu ALIAS trả về. | Không để sót route edge; `AAAA`/CNAME trở lại không có như snapshot. |
| Spaceship `www CNAME` | `vink.forum.`, TTL 300 | Không sửa record; **sẽ theo địa chỉ apex mới**. Chỉ GO khi nhà cung cấp xác nhận topology này và probe edge chứng minh TLS/redirect ở `www`. | CNAME tiếp tục theo apex trở về VPS; kiểm HTTPS `www` trực tiếp và qua DNS. |
| Spaceship `test CNAME`, `origin-test A` | `vink-test-frontend.pages.dev.` và `42.96.18.245`, TTL 300 | Không đổi. | Không đổi. |
| Spaceship NS/DNSSEC, mail, xác minh hiện hữu | `launch1.spaceship.net.`, `launch2.spaceship.net.`; DS/DNSKEY có mặt; cần snapshot đầy đủ phần còn lại | Không đổi record hiện hữu. TXT xác minh zone hoặc record DCV mới chỉ được thêm ở pha chuẩn bị với quyền riêng, đúng tên/giá trị từ Cloudflare. | Không đổi các record cũ. Không xóa ngay TXT/DCV đã được duyệt khi resolver còn có thể vào edge hoặc zone còn dùng. |
| Cloudflare ingress cho apex và www | Trạng thái account/zone chưa xác minh | Hai hostname phải định tuyến tới đúng virtual host, upstream public IP VPS đã kiểm; không trỏ lại tên sẽ được proxy. Cần xác minh riêng khả năng hỗ trợ alias chain của `www`. | Giữ cấu hình edge đủ phục vụ thời gian TTL cũ/đích còn cache; không disable zone/edge cert ngay. |

Điểm dễ bỏ sót: `www CNAME vink.forum` không giữ `www` ở đường trực tiếp khi apex qua Cloudflare.
TLS chọn host **trước** redirect. [Cloudflare hướng dẫn mỗi hostname proxied trỏ tới target
CNAME tương ứng](https://developers.cloudflare.com/dns/zone-setups/partial-setup/setup/#3-add-dns-records);
không dựa vào cert apex hoặc một rule redirect để kết luận chuỗi CNAME hiện hữu hoạt động.
Nếu provider không hỗ trợ topology này, **dừng**; mở một quyết định riêng về việc tách
`www` thành A DNS-only tới VPS, hoặc đặt mapping riêng do Cloudflare cấp. Phải có chấp thuận,
chứng chỉ riêng, readback và rollback cụ thể trước khi đổi `www`; không ghép vào diff @ này.

## Điều kiện GO và thứ tự thao tác

1. **Đọc inventory và xin quyền từng pha.** Chủ zone xác nhận account/zone đúng, plan
   Business/Enterprise đang áp dụng, Partial Setup Active và target routing chính thức cho apex;
   đọc quyền, hostname routing, TLS mode, origin, cert/expiry, rule hiện có. Lấy fresh snapshot
   Spaceship và xác nhận có thao tác thay A bằng ALIAS không để apex rỗng hoặc cùng lúc có A cũ.
   Chuẩn bị owner rollback và cửa sổ theo dõi; không dùng thông số đo cũ để ghi đè fresh record.
2. **Tách kiểm TLS edge và origin.** Edge cert phải Active, có SAN đủ cho từng hostname phục vụ,
   xác minh bằng TLS client tới edge với đúng tên trước khi đổi apex. Đọc phương thức DCV/renewal,
   CAA và TTL; không dựa vào Universal SSL tự phát hành sau cutover. Nếu cần DCV trước traffic,
   dùng phương thức được provider xác nhận; [delegated DCV cần Advanced certificate, không
   dùng với Universal](https://developers.cloudflare.com/ssl/edge-certificates/changing-dcv-method/methods/delegated-dcv/).
   TXT xác minh zone khác với DCV chứng chỉ. Nếu cần plan/add-on hoặc record mới, xin quyền riêng;
   nếu cert chưa kiểm được trước khi đổi traffic thì NO-GO.
3. **Chứng minh origin không vòng lặp.** Định tuyến Cloudflare tới địa chỉ public của VPS hiện
   hữu thay vì phân giải lại `vink.forum` hoặc một alias theo apex. HTTPS edge → Caddy dùng
   [Full (strict)](https://developers.cloudflare.com/ssl/origin-configuration/ssl-modes/full-strict/),
   giữ Host và TLS SNI theo tên public tương ứng. Không đổi sang Flexible, Full không strict,
   không dùng `-k`, không cần giả định [Host/SNI override vốn giới hạn theo plan](https://developers.cloudflare.com/rules/origin-rules/#availability).
   Luồng HTTP chỉ redirect lên HTTPS và không forward API/SSR dưới dạng plaintext; Full (strict)
   không tự biến request của client HTTP thành origin HTTPS. Giữ `308`/path/query qua hành vi
   Caddy/edge đã kiểm. Xác minh đường gia hạn chứng chỉ public Caddy sau khi proxy, riêng với
   DCV edge: ví dụ kiểm chứng HTTP-01 qua route challenge được duyệt hoặc đánh giá DNS-01 riêng;
   không giả định [TLS-ALPN-01 xuyên edge](https://caddyserver.com/docs/automatic-https#acme-challenges).
   Đường direct phải vẫn được trình duyệt tin khi rollback, không thay bằng Origin CA-only,
   không áp mTLS/firewall-only làm hỏng lối phục hồi hoặc ảnh hưởng dịch vụ chung.
4. **Giữ pass-through trong đợt đầu.** Không dùng Worker/Pages, không rewrite URL, Host,
   canonical, `Origin`, Cookie/`Set-Cookie` hoặc CSRF. Bypass cache CDN cho cả hai tên
   được proxy trong đợt đầu, kể cả `/pkg/*`, vì tên JS/CSS/WASM có thể lặp giữa các bản;
   [mặc định Cloudflare có thể cache theo extension/status ngay khi origin không đặt header](https://developers.cloudflare.com/cache/concepts/default-cache-behavior/).
   Không bật Cache Everything, TTL override, ignore query string, Rocket Loader hoặc biến đổi
   body/header trên đường production. Giữ nguyên `private, no-cache`, `no-store`, ETag mạnh,
   `304`, `404`, MIME và security headers. Không thêm `noindex` toàn host, thay robots hay
   chặn crawler ngoài chính sách hiện tại; [managed robots có thể tạo file dù origin không có](https://developers.cloudflare.com/bots/additional-configurations/managed-robots-txt/).
   Kiểm tra xung đột của rule có sẵn và tính toàn vẹn của ETag với encoding
   ([Cloudflare ETag](https://developers.cloudflare.com/cache/reference/etag-headers/)).
   Chỉ mở cache asset ở một lần riêng khi có URL gắn nội dung và đã kiểm hai phiên bản SSR/WASM.
5. **Không tin thêm header nguồn.** Caddy đang thay X-Forwarded-For bằng TCP peer; App chỉ tin
   `10.89.4.2/32`. Sau khi proxy, App có thể nhìn IP Cloudflare thay vì IP khách, làm gom bucket
   limiter/log. Giữ behavior hiện tại, ghi nhận ảnh hưởng trước GO; không mở sign-in và không
   thêm trust `CF-Connecting-IP`/XFF tùy tiện khi origin còn public. Nếu cần IP thật cho auth,
   xử lý trong một thay đổi riêng được duyệt, kèm kiểm chứng đường tin cậy.
6. **Rehearse rồi mới đổi đúng @.** Chỉ sau khi được phép set up/test riêng: so sánh direct
   Caddy và candidate edge bằng cùng Host/SNI, chạy ma trận bên dưới, kiểm rollback
   đã có quyền và điểm khôi phục; xem cả public resolver có validation DNSSEC. Nếu cần thử
   ALIAS, dùng zone thử được phép, độc lập với Vink test. Nếu thiếu một cổng chứng minh thì
   dừng. Chỉ khi chủ zone duyệt cutover, thay đúng apex A → ALIAS, đọc lại UI và cả hai NS,
   chờ TTL, kiểm A/AAAA/HTTPS/DS ở resolver công cộng rồi lặp lại smoke bằng DNS thật. Không
   coi `--resolve` là bằng chứng public đã chuyển. Không retry một thay đổi mất ACK trước
   khi đọc trạng thái thật.

## Smoke, tiêu chí dừng và phục hồi

Các lệnh dưới đây là **read-only**; không truyền tài khoản, cookie, code hoặc header xác thực.
IP direct là mốc quan sát, phải kiểm lại trước cửa sổ vận hành.

```sh
for ns in launch1.spaceship.net launch2.spaceship.net; do
  for rr in NS A AAAA CNAME CAA HTTPS; do
    dig "@$ns" +norecurse +noall +comments +answer vink.forum "$rr"
  done
  dig "@$ns" +norecurse +noall +answer www.vink.forum CNAME
  dig "@$ns" +norecurse +noall +answer test.vink.forum CNAME
  dig "@$ns" +norecurse +noall +answer origin-test.vink.forum A
done
dig +dnssec vink.forum DS
dig +dnssec vink.forum DNSKEY
openssl s_client -connect 42.96.18.245:443 -servername vink.forum \
  -verify_hostname vink.forum -verify_return_error </dev/null
curl -fsS --resolve vink.forum:443:42.96.18.245 https://vink.forum/health/ready
curl -sS -D - -o /dev/null --resolve www.vink.forum:443:42.96.18.245 \
  'https://www.vink.forum/toi?edge_smoke=1'
```

Lặp probe HTTP trước qua direct, rồi qua edge được xác nhận (`curl --resolve HOST:PORT:IP`
với IP lấy từ target thật, URL vẫn mang Host thật), cuối cùng qua resolver thường. Không dùng IP
mẫu hoặc tự đoán hostname. Dùng GET `curl -sS -D - -o /dev/null --max-time 15 URL` để xem
status/header, `--head --compressed` cho HEAD asset. Với API `200`, lấy ETag từ chính lần
GET ở đích đó và gửi lại qua `-H "If-None-Match: $etag"`; kiểm `304` không body và cùng ETag.

| Cổng | Kỳ vọng tối thiểu |
| --- | --- |
| DNS/TLS/redirect | Hai NS Spaceship không đổi, DS/DNSKEY giữ nguyên và resolver có DNSSEC giải được; apex và `www` có cert đúng tên, không có `52x`/vòng lặp; HTTP `308`, HTTPS `www` `301`, giữ path/query. Không thay đối chứng test/Cantos hoặc các record mail. |
| SSR/indexing | `/`, `/toi` và một deep link cho ra SSR đúng; canonical public là apex, private giữ `noindex`; không xuất hiện noindex mới trên public hoặc robots/sitemap do edge sinh. Reader `200` chỉ khi có mẫu công khai hợp lệ; vẫn kiểm `404` cho Reader/route sai. |
| API/cache | Ready/live đúng, GET works `200→304`; API sai `404` JSON; không cache HIT cho HTML/API/health/asset, không biến `404` thành fallback `200`, không ghi hoặc dùng phiên production. Cookie/method không ghi nhận là PASS nếu chưa có kiểm thật ở môi trường được duyệt. |
| Assets và browser | So asset status/MIME/encoding và hash **giải nén** của JS/CSS/WASM/font với direct trên cùng release (một GET đầy đủ có giới hạn tải đã duyệt cho WASM); thử tải nguội và refresh trên desktop/mobile, mở deep link và đổi route sau hydrate, kiểm JS console/network. Không kết luận hydrate từ HTML hoặc HEAD. |

**Dừng hoặc rollback** nếu TLS/renewal, DNSSEC, redirect, origin readiness, status API, parity
cache/indexing hoặc hydration hỏng, hoặc bất kỳ dịch vụ ngoài phạm vi thay đổi. Nếu chưa đổi @,
giữ DNS công khai nguyên. Nếu đã đổi, ưu tiên phục hồi **apex DNS** theo bảng bằng quyền đã duyệt:
chụp fresh DNS, bỏ riêng ALIAS, hoàn nguyên A/TTL và trạng thái không có AAAA như snapshot,
đọc hai NS, chờ hết TTL còn lại và xác minh trực tiếp lẫn qua resolver. Giữ edge còn phục vụ
trong thời gian resolver có thể trả địa chỉ cũ; giữ chứng chỉ và Caddy direct. Không đổi
`www`, NS, DS hay record đối chứng để chữa một lỗi @. Ghi giờ, xác nhận cả `www`/apex/health/API/SSR
và các bề mặt đối chứng. DNS rollback không tức thời.

Phục hồi **cấu hình edge** (khi được phép) dùng snapshot đúng phiên bản rule/origin đã chứng minh,
không ảnh hưởng cả account. Phục hồi **Caddy/release/DB** là hoạt động riêng theo
[runbook production](README.md#kiểm-chứng-và-phục-hồi), không nằm trong rollback DNS; không
restart app, reload Caddy hay phục hồi database vì một lỗi edge.

## Bằng chứng và việc đang chặn

| Pha | Kết quả |
| --- | --- |
| Baseline production trực tiếp | PASS cho những request đọc đã liệt kê; Range WASM không cho `206`; full GET/hash, Reader có tác phẩm và hydration NOT_RUN. |
| Kiểm cục bộ | PR chỉ tài liệu; kiểm diff/whitespace và liên kết tương đối. Không chạy `caddy validate`, build OCI hay app suite vì không có config/code thay đổi. |
| Inventory và test edge được duyệt | NOT_RUN: chưa có đường đọc xác thực Cloudflare/Spaceship account cho zone. Chưa biết plan/zone Active, target được cấp, ALIAS thực tế/DNSSEC, route/cert `www`, cert/DCV/renewal edge, gia hạn Caddy sau proxy và Full (strict) tại edge. |
| Cutover, hậu kiểm và rollback thật | NOT_RUN. Giữ routing đang hoạt động tới VPS. Không thay Cloudflare, Spaceship, VPS, auth hay DNS test/Cantos. |
