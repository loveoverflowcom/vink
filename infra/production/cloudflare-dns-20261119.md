# Checkpoint DNS và Caddy ngày 19/11/2026

Tư liệu lịch sử của #338. Quy trình hiện hành nằm ở [phát hành frontend production](cloudflare-edge.md). Các bảng, địa chỉ, chỉ dẫn giữ test và kết quả bên dưới mô tả topology cũ.

**Checkpoint 19/11/2026:** Cloudflare đã có zone `vink.forum` gói Free, Full Setup,
với bốn bản ghi DNS only, Full (strict), bypass cache cho apex và `www`, và rule giữ
encoding của API.
Spaceship vẫn là registrar và DNS authoritative, DNSSEC đang bật. Production vẫn đi
thẳng đến Caddy. Chuyển nameserver và bật proxy là hai bước vận hành riêng.

Cloudflare nhận lưu lượng HTTPS trước Caddy. Vink vẫn chạy Leptos SSR, API, Worker,
Identity và PostgreSQL trên VPS theo [runbook production](README.md).
`https://vink.forum` giữ canonical và cùng origin cho SSR, `/api/*` và `/pkg/*`.
`www.vink.forum` giữ redirect của Caddy với path và query. Giữ `test` và `origin-test`
trỏ đến đích hiện tại, giữ nguyên zone `cantos.cc`. Dùng [Full Setup](https://developers.cloudflare.com/dns/zone-setups/)
để dùng gói Free. [Partial Setup](https://developers.cloudflare.com/dns/zone-setups/partial-setup/)
giữ được authoritative DNS tại Spaceship nhưng cần gói Business/Enterprise.
[Cloudflare Pages của Vink test](../cloudflare/README.md) có Worker và chính sách cache,
indexing riêng cho môi trường đó.

## DNS đã chuẩn bị

Đọc lại Advanced DNS của Spaceship và bảng DNS Cloudflare ngay trước thao tác.
Nếu có record mới, đồng bộ cả loại, tên, nội dung và TTL trước khi đổi nameserver.
Lưu bản sao zone riêng làm điểm khôi phục. Bảng là inventory đọc được ngày 19/11/2026,
không bao gồm record do hệ thống quản lý như SOA, NS, DNSKEY hay DS.

| Tên | Loại và đích hiện tại | TTL | Cloudflare khi chuyển nameserver | Sau khi bật proxy |
| --- | --- | ---: | --- | --- |
| `@` | A `42.96.18.245` | 300 | DNS only | Proxied |
| `www` | CNAME `vink.forum.` | 300 | DNS only | Proxied |
| `test` | CNAME `vink-test-frontend.pages.dev.` | 300 | DNS only | DNS only |
| `origin-test` | A `42.96.18.245` | 300 | DNS only | DNS only |

| Delegation | Giá trị quan sát |
| --- | --- |
| Nameserver đang dùng | `launch1.spaceship.net`, `launch2.spaceship.net` |
| Nameserver Cloudflare cấp | `carlane.ns.cloudflare.com`, `drew.ns.cloudflare.com` |
| DS cũ tại parent `.forum` | Key tag 25485, algorithm 13, digest type 2. TTL DS và NS tại cả bốn máy chủ parent là 900 giây tại thời điểm đo. |
| TTL tại zone Spaceship | NS 1.800 giây và DNSKEY 3.600 giây ở cả hai NS Spaceship. TTL ở parent chỉ dùng cho delegation và DS. |

Đọc trực tiếp parent, hai nhà cung cấp và resolver công cộng để tách trạng thái đã ghi
khỏi trạng thái cache. Hỏi lại parent trước khi dùng TTL để lên lịch. DS vắng mặt phải
là câu trả lời `NOERROR` có bằng chứng phủ định, không phải timeout hoặc `SERVFAIL`.

```sh
for parent in ns01.trs-dns.com ns01.trs-dns.net ns10.trs-dns.org ns10.trs-dns.info; do
  dig @"$parent" +norecurse +dnssec +noall +comments +answer +authority vink.forum DS
  dig @"$parent" +norecurse +noall +comments +answer +authority vink.forum NS
done

for ns in launch1.spaceship.net launch2.spaceship.net \
    carlane.ns.cloudflare.com drew.ns.cloudflare.com; do
  dig @"$ns" +norecurse +noall +comments +answer vink.forum NS
  dig @"$ns" +norecurse +dnssec +noall +comments +answer vink.forum DNSKEY
  dig @"$ns" +norecurse +noall +comments +answer vink.forum A
  dig @"$ns" +norecurse +noall +comments +answer www.vink.forum CNAME
  dig @"$ns" +norecurse +noall +comments +answer test.vink.forum CNAME
  dig @"$ns" +norecurse +noall +comments +answer origin-test.vink.forum A
done

for resolver in 1.1.1.1 8.8.8.8; do
  dig @"$resolver" +dnssec +noall +comments +answer vink.forum DS
  dig @"$resolver" +dnssec +noall +comments +answer vink.forum A
done
```

## 1. Chuyển nameserver khi cả bốn bản ghi là DNS only

1. Xác nhận quyền thay đổi delegation và khoảng chuyển tiếp DNSSEC với chủ domain.
   [Chuyển DNSSEC liên tục bằng multi-signer](https://developers.cloudflare.com/dns/dnssec/dnssec-active-migration/)
   cần hai bên phục vụ DNSKEY bên ngoài tại apex. UI Spaceship quan sát được cho phép
   chuẩn bị DS cho custom nameserver nhưng không cung cấp loại bản ghi DNSKEY. Dùng
   [quy trình remove DS trước của Cloudflare](https://developers.cloudflare.com/dns/dnssec/).
2. Tại Spaceship, gỡ DS Spaceship cũ. Ưu tiên giữ zone cũ phát DNSKEY và RRSIG cho
   đến khi cache DS cũ hết hạn. Nếu UI chỉ có nút tắt DNSSEC tự động, xác minh việc
   ký zone cũ sau khi bỏ DS hoặc yêu cầu Spaceship xử lý DS trước. Nếu nhà cung cấp
   gỡ chữ ký và DS cùng lúc, resolver còn cache DS có thể trả `SERVFAIL` trong TTL
   cũ. Quyết định chấp nhận cửa sổ lỗi cần ghi riêng trước khi dùng đường đó.
3. Giữ nameserver Spaceship đến khi **tất cả** máy chủ parent đã bỏ DS cũ. Từ lúc
   xác minh cả bốn parent đã bỏ DS, chờ đủ TTL DS cũ. Kiểm resolver công cộng trả
   `NOERROR` cho apex, `www` và `test`. Tại Spaceship chọn custom nameservers và
   lưu đúng cặp Cloudflare đang cấp. Kiểm parent đều công bố cặp mới, chờ hết TTL
   NS cũ ở cả parent và zone, rồi xác minh lại resolver và hai NS Cloudflare. Giữ
   bốn bản ghi DNS only trong suốt bước này.
4. Đợi zone Cloudflare `Active`. Kiểm production trực tiếp, redirect `www`,
   `test.vink.forum`, `origin-test` và Cantos như baseline. Bật DNSSEC trong zone
   Cloudflare. Đọc DS mới từ Cloudflare và thêm vào Spaceship cho custom nameservers
   sau khi hết TTL DNSKEY cũ và truy vấn DNSKEY/RRSIG thành công ở cả hai NS
   Cloudflare. Kiểm parent thấy
   đúng DS mới và hai resolver công cộng trả `NOERROR` với cờ `ad` cho bản ghi apex.
   Kiểm lại `test` và `www` qua DNS thường.

## 2. Bật proxy sau khi có chứng chỉ edge

1. Đợi Universal SSL `Active` và bao phủ apex cùng `www`. Gói Full có thể
   [phát hành chứng chỉ khi record còn DNS only](https://developers.cloudflare.com/ssl/edge-certificates/universal-ssl/enable-universal-ssl/).
   Kiểm TLS direct cho hai hostname trước khi bật proxy. Giữ
   [Full (strict)](https://developers.cloudflare.com/ssl/origin-configuration/ssl-modes/full-strict/)
   và upstream A đi thẳng đến VPS, giữ Host/SNI gốc.
2. Giữ rule Cache Rules `Vink production bypass cache` ở trạng thái Active. Dùng
   `(http.host in {"vink.forum" "www.vink.forum"})` với Cache eligibility = Bypass cache.
   Giữ Compression Rule `Vink API preserve origin encoding` với Disable Compression
   cho `(http.host eq "vink.forum" and starts_with(http.request.uri.path, "/api/"))`.
   Cloudflare có thể nén và đổi ETag khi phản hồi API lớn hơn. Rule giữ API theo
   encoding của baseline Caddy, với nén asset tiếp tục hoạt động theo cấu hình hiện
   có. [Compression Rules](https://developers.cloudflare.com/rules/compression-rules/settings/)
   Kiểm các rule xung đột, Bot Preference Sync/Managed robots và Always Use HTTPS.
   Giữ Caddy sở hữu redirect và phục vụ đường HTTP-01. Log Caddy ngày 18/11 ghi nhận
   phát hành thành công bằng HTTP-01 cho cả hai hostname khi còn direct. Qua edge,
   giữ `/.well-known/acme-challenge/*` về Caddy trên HTTP; không đặt challenge, cache hay
   rewrite lên đường này. Kiểm sau proxy và theo dõi lần gia hạn thực tế. TLS-ALPN-01
   chỉ đến điểm kết thúc TLS ở Cloudflare. [Caddy ACME](https://caddyserver.com/docs/automatic-https#acme-challenges)
3. Bật proxy cho `www` trước, thử TLS và redirect `301` thật còn apex direct; sau
   đó bật apex. Cloudflare có thể proxy cả chuỗi CNAME khi một target được proxy,
   vì vậy một mình trạng thái DNS only của `www` sẽ không cô lập `www` khi apex đã
   Proxied. [Cloudflare proxy status](https://developers.cloudflare.com/dns/proxy-status/)
4. So baseline direct với edge. Điều kiện PASS gồm chứng chỉ đúng host, `CF-Ray`
   từ traffic thật, không có `52x` hoặc vòng lặp, API `200 → 304`, MIME/status và
   digest tài sản giống direct, SSR giữ canonical/noindex theo route, Chrome hydrate
   và đổi route. Kiểm browser trên desktop và mobile, cùng các host đối chứng.
   Caddy hiện thay XFF bằng IP peer. Khi qua edge, limiter/log có thể gom theo IP
   Cloudflare. Giữ nguyên chính sách tin IP và trạng thái sign-in.

## Smoke và phục hồi

Chạy từ root của repository. Script chỉ gửi GET đọc công khai, kiểm TLS, redirect,
SSR, health, ETag, status 404, noindex và tải đầy đủ JS/CSS/WASM/font. Cần `curl`,
`openssl`, `python3` và khoảng trống tạm cho asset lớn nhất. Đo direct và edge trên
cùng release để đối chiếu SHA-256 của body đã giải nén.

```sh
infra/production/smoke-edge.sh direct 42.96.18.245 > /tmp/vink-direct.log
infra/production/smoke-edge.sh edge > /tmp/vink-edge.log
awk '/^asset /' /tmp/vink-direct.log > /tmp/vink-direct-assets
awk '/^asset /' /tmp/vink-edge.log > /tmp/vink-edge-assets
diff -u /tmp/vink-direct-assets /tmp/vink-edge-assets
```

Có thể thêm hai IP edge theo thứ tự apex, www sau đối số `edge` để dùng
`curl --resolve` với Host/SNI thật. Lặp lại không ghim IP để xác nhận DNS công khai.
Kiểm browser ở `/`, deep link `/ban-viet` và `/toi`, thử tải nguội, điều hướng sau
hydrate và quan sát console. Khi có tác phẩm công khai hợp lệ, kiểm thêm Reader 200.
Kiểm `test.vink.forum` còn `200` với `X-Robots-Tag: noindex` và Cantos còn `200`.

- **Lỗi proxy:** chuyển cả apex và `www` sang DNS only, giữ TLS/rule để phục vụ
  resolver còn giữ IP edge. Kiểm lại sau TTL proxy và xác nhận Caddy với cả hai
  hostname. Giữ `test`, `origin-test`, DS và nameserver Cloudflare.
- **Lỗi nameserver/delegation:** kiểm snapshot Spaceship vẫn chứa đủ record trước khi
  phục hồi delegation. Nếu đã công bố DS Cloudflare, trước tiên gỡ DS và giữ zone
  Cloudflare tiếp tục ký đến khi cả parent bỏ DS và hết TTL cũ. Sau đó đổi lại cặp
  Spaceship, đợi delegation cũ hết TTL rồi phục hồi ký DNSSEC và kiểm chuỗi xác thực.
  Giữ hai zone có record khớp trong giai đoạn chuyển tiếp. Nếu sau một thao tác mất
  phản hồi, đọc trạng thái registrar, parent và nhà cung cấp trước khi ghi tiếp.
- **Lỗi Caddy/release/DB:** phục hồi theo [runbook production](README.md#kiểm-chứng-và-phục-hồi).
  Khoanh vùng theo health direct trước khi reload Caddy, restart App hoặc phục hồi DB.

## Bằng chứng 19/11/2026

| Mốc | Kết quả |
| --- | --- |
| Inventory Cloudflare và Spaceship | Đọc xác thực cả hai dashboard. Cloudflare có Full Setup Free chờ delegation, cùng bốn record DNS only TTL 300. Hai NS Cloudflare trả `aa` và toàn bộ bốn record khớp hai NS Spaceship. Full (strict) và rule Bypass cache đã lưu; Always Use HTTPS đang tắt. |
| Baseline production | `smoke-edge.sh direct 42.96.18.245` PASS cho TLS, redirect, SSR, health, `200 → 304`, `404` và đầy đủ năm asset; WASM giải nén 125.610.054 byte, SHA-256 `34791bbf4c9921a2a9b97c4975c69f9e8af7380369b82c66b21a1f3b9a00847d`. Chrome desktop điều khiển được menu và đi tới Bàn viết với trạng thái khách. Test vẫn noindex và Cantos vẫn trả 200. SHA repo chỉ định danh công cụ kiểm. |
| Delegation và edge công khai | Đang chờ chuyển DNSSEC và nameserver. Chưa kiểm Universal SSL Active, HTTP qua edge, hydration qua edge hay gia hạn chứng chỉ khi đã proxy. |
