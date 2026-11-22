# Phát hành frontend Vink production

`https://vink.forum` là URL chính thức. Pages Advanced Mode phục vụ asset từ image Web đã qua qualification. Worker gửi SSR, deep link, health và API đến `https://origin-prod.vink.forum`. Caddy chuyển tiếp đến App. Identity, session và hai database ở VPS. Mỗi đợt phát hành cần một lệnh riêng.

## Mốc triển khai ngày 22/11/2026

- `vink.forum` và `www.vink.forum` chạy trên Pages `vink-production`, nhánh `main`. Bản
  `69fff7eb-88ce-40e9-a3a5-1dc9207f3bd7` chứa 82 asset byte-for-byte từ image
  `a9d6a4f9fb0a35b1e0e56f843132d4e3fe5dbb20` và bản vá riêng của Worker để giữ strong ETag.
  Bản vá đang chạy cần được áp dụng vào source trước khi đóng gói lại từ `develop`.
- Nameserver là `carlane.ns.cloudflare.com` và `drew.ns.cloudflare.com`. DS `2371/13/2` đã
  được cả bốn máy chủ `.forum` công bố. Cloudflare DNS, Google Public DNS và Quad9 đều xác
  thực bản ghi apex. Caddy trả `403` khi gọi thẳng apex qua IP VPS hoặc khi gọi origin riêng
  thiếu khóa. Origin có HTTPS hợp lệ và vẫn giữ SSR/API trên VPS.
- Receipt thủ công và bản Caddy trước cutover nằm trong
  `/opt/vink-prod/pages-bootstrap/a9d6a4f9fb0a35b1e0e56f843132d4e3fe5dbb20/` trên VPS.
  Chưa có receipt qua controller. URL Preview riêng báo lỗi TLS và alias Pages trả về bị rút
  ngắn so với SHA đầy đủ mà controller đang dùng. Production chưa có tác phẩm công khai để
  chạy kịch bản Reader `200`.
- App và Identity hiện mở [đăng nhập bằng mật khẩu và mã email](../../docs/development/auth-operations.md#31-production-username-mật-khẩu-và-mã-email).
  Xem [bàn giao cấu hình production](production-handoff.md) để đọc lại DNS/Pages/VPS, lấy khóa
  phát hành, khôi phục khóa Resend và giữ overlay qua lần đổi image.

## Khóa gọi workflow thủ công

`VINK_RELEASE_DISPATCH_TOKEN` là GitHub Actions secret ở repo `loveoverflowcom/vink` và là
secret cùng tên trong Google Cloud Secret Manager của project `vink-d0846`. Secret Manager
đã xác minh nội dung phiên bản đang dùng. Dùng tài khoản vận hành được cấp quyền và truyền
giá trị qua stdin hoặc cơ chế nạp secret của môi trường chạy. Khi kiểm kê, đọc tên và trạng thái
phiên bản. `publish-images.yml` dùng `GITHUB_TOKEN` riêng để đẩy GHCR sau qualification.

Lần dispatch [45017318350](https://github.com/loveoverflowcom/vink/actions/runs/45017318350) của
`e60593c0798ebce582ba050749b004be00f76920` đã build image. Qualification PostgreSQL dừng vì
fixture thiếu mật khẩu khi provision role `vink_notifications_migrator`. Cần sửa fixture và
qualify lại trước khi có receipt image mới.

## Nguồn và kích thước

| Thành phần | Trước thay đổi | Cách phát hành |
| --- | --- | --- |
| Vink #207 | Pages test gắn noindex toàn site, WASM lớn quay về origin, inventory GCS ghi auth disabled | Project production riêng, inventory từ OCI, asset cùng origin, bản preview chỉ phục vụ asset công khai |
| #304, #338 | Ba image GHCR, Caddy và checkpoint DNS riêng | Giữ workflow build/qualify, PostgreSQL và runtime. Containerfile chung ở `infra/Containerfile` |
| Cantos | Checkout tham khảo `e70714c530e1cd9075f9e8dd778df8cbe72efc1c` tách build, upload và receipt. Remote develop đọc được `a1fca178f9a284c8177d60601d856fc64dae1953`; chưa đối chiếu nội dung tại SHA remote | Vink giữ SSR/Router và origin riêng. Merge vào develop chỉ cung cấp source cho release |
| #357, #358 | Resend transport và overlay theo môi trường ở hai PR riêng | Packager chỉ đọc asset công khai. Origin giữ env hiện tại và tập Compose đang chạy. Thử Auth trên loopback với database riêng, password hoặc mail sink theo lane. Chủ dự án kiểm thư thật |

Trên Mac arm64, base `5efa25e43565fbc9da2cd8f10a5ce7e740aae07b` (#356), đúng profile `wasm-release` và wasm-bindgen đã pin cho 143.625.523 byte. Section debug `name` chiếm 136.631.833 byte. Thêm `--remove-name-section` cho cùng input cho 6.993.685 byte. Hai bundle qualification với namespace A/B sau thay đổi mỗi bản có 82 file, tổng 10.849.181 byte, hydration WASM 6.998.878 byte. Các số này là kích thước file trước nén HTTP. Mỗi release thật đo lại từ digest image Linux. Mốc 125.610.054 byte thuộc release production lịch sử.

Pages và Workers Static Assets giới hạn 25 MiB/file. Đường build tối ưu thực tế đủ nhỏ để chọn Pages; R2 riêng tư theo release sẽ thêm binding, credential, storage và vận hành. Nếu file mới vượt trần, packager dừng trước upload. [Pages limits](https://developers.cloudflare.com/pages/platform/limits/) · [Workers limits](https://developers.cloudflare.com/workers/platform/limits/) · [Pages Advanced Mode](https://developers.cloudflare.com/pages/functions/advanced-mode/) · [R2](https://developers.cloudflare.com/r2/).

## Chuẩn bị target

1. Kiểm registrar, zone/account, toàn bộ provider export, NS và DNSSEC/DS tại parent, SSL, cache/transform rules, Pages custom domains/routes và các caller VPS. Lưu export, TTL và giá trị rollback ở kho vận hành riêng. HEAD/DNS công khai ngày 20/11 từ Mac cho NS `launch1/2.spaceship.net`, DS 25485/13/2, apex 200, www 301 về apex, test 200 có CF-Ray/noindex và origin-test 200/noindex. Readback này chưa xác nhận quyền quản trị hay image digest đang chạy. [Checkpoint #338 và cách kiểm DS/TTL](cloudflare-dns-20261119.md).
2. Chuẩn bị Pages **Direct Upload** project `vink-production`, nhánh production `main`, subdomain `vink-production.pages.dev` trong đúng account. Compatibility date ít nhất `2026-10-01`, fail closed ở cả hai môi trường. Preview branch `r-<sha>` cần truy cập được các byte công khai qua HTTPS. URL provider trả 404/noindex cho HTML/API. Đặt secret `ORIGIN_KEY` **chỉ** ở deployment config production. Chọn một giá trị ngẫu nhiên từ ít nhất 32 byte, đặt cùng giá trị vào environment `VINK_ORIGIN_KEY` của Caddy qua secret store. Không ghi token/giá trị binding vào manifest hoặc lệnh shell lưu history.
3. Origin DNS-only có chứng chỉ tin cậy cho chính `origin-prod.vink.forum`, renew bằng HTTP-01/DNS-01 phù hợp. Import [vhost dành cho Worker](origin-prod.caddy.example) vào bản candidate của Caddy dùng chung. Validate toàn bộ, reload theo lịch, kiểm yêu cầu thiếu secret trả 403 và TLS hostname hợp lệ. Worker lấy IP từ `CF-Connecting-IP` do Cloudflare cấp, thay các header forwarding từ client. Caddy chỉ chuyển IP qua hop đã xác thực, App chỉ tin peer `10.89.4.2/32`. Tắt Pseudo IPv4 overwrite, kiểm Managed Transforms và IPv6 ở zone thật. Sau cutover bỏ ingress trực tiếp đến App ở vhost apex/www cũ (deny, giữ challenge/renewal cần thiết). Giữ đường khôi phục Caddy private đã kiểm.
4. Cố định repo sạch ở `/opt/vink-prod/releases/<sha>` cho current và candidate. Trên **một** VPS/controller chạy rootful Linux Podman, cài Python 3.11+, Podman Compose 1.3, GNU coreutils, curl, OpenSSL, Node 22+, npm, Chrome và tài khoản không đặc quyền `vink-release-check` có quyền đọc source. Cài `npm ci --prefix infra/cloudflare --ignore-scripts --no-audit --no-fund` từ lock của release. Chuẩn bị `/var/lib/vink-production-release` root:root, mode 0700, đủ chỗ cho archive giữ lại và một bộ staging. Sao lưu các manifest, byte asset và receipt. Token Pages Edit giới hạn account Vink và auth GHCR read thuộc quy trình vận hành. Chỉ controller có quyền cập nhật project/runtime trong cửa sổ phát hành.
5. Chạy backup/restore rehearsal độc lập của cả App và Identity; chọn cặp dump đủ checksum trong 24 giờ. `VINK_VERIFIED_BACKUP` trỏ đến thư mục timestamp đã kiểm trong `/var/backups/vink-prod`. Mặc định adapter so file migration cả hai owner. Khi đổi schema, thử cả binary cũ và mới trên bản sao sau migration. Thêm `--schema-review /secure/rehearsal.json` vào cùng input ở plan/apply/resume. File có `from`, `to` bằng SHA thật, `action` là `expand` (chạy migrator App và Identity) hoặc `keep` (rollback image, giữ schema đã áp dụng). Hai object `vink-app`, `vink-identity` đều có `old_on_result` và `new_on_result` bằng `PASS`, `rehearsal` trỏ bằng chứng kiểm đọc/ghi của cả hai bản. Controller đối chiếu chiều thay đổi và hash migration cũ, lưu hash báo cáo trong receipt. Dị biệt lịch sử hoặc migration phá tương thích cần đợt triển khai riêng. Không hạ schema theo image hoặc restore đè writes mới.

## Từ build đến receipt

Chạy [Publish OCI images manually](../../.github/workflows/publish-images.yml) từ `develop` tại revision đã duyệt với platform VPS. Workflow build từ git archive sạch, qualify image cùng PostgreSQL scratch; `publish=true` push ba digest rồi in JSON. Lưu nguyên JSON làm `release.json`. `publish=false` chỉ qualify. Với `website=false` (mặc định), workflow kết thúc ở GHCR. Để thực hiện toàn luồng từ SHA, dispatch với `publish=true, website=true`, previous_origin, previous_edge và reader UUID đã quan sát. Job website dùng GitHub Environment `vink-production`, khóa concurrency, host key pin và receipt từ chính job publish. SSH forced command nhận JSON hữu hạn qua stdin rồi gọi plan → apply; không tự chạy khi merge.

Chuẩn bị receiver một lần. Khi chưa có tree, receiver clone riêng develop từ URL Vink cố định, kiểm HEAD đúng SHA đã qualify rồi cài Wrangler từ lock và rename vào releases. Nếu develop đã chạy trước, dừng và qualify lại. Các tree đã giữ dùng cho rollback. Host cần GitHub read nếu repo private. Cài [receive.py](../cloudflare/production/receive.py) và artifacts.py cùng thư mục root-owned. User SSH riêng `vink-release` có key với `restrict,command="sudo -n /usr/local/sbin/vink-release-receive"`; sudoers chỉ cho wrapper cố định không có argv. Wrapper root-owned tải env từ file mode 0600 ngoài repo và exec Python receiver ở đường dẫn tuyệt đối. Chỉ root được ghi tree, phụ thuộc và schema-reviews. Environment cần var `VINK_RELEASE_SSH_HOST` và secrets `VINK_RELEASE_KNOWN_HOSTS`, `VINK_RELEASE_SSH_KEY`; token Pages, GHCR read và secret origin nằm trên host/binding. Nếu khác migration, đặt báo cáo đã duyệt trong `/opt/vink-prod/schema-reviews/<old>-<new>.json`. Cấu hình protection của Environment theo quyền vận hành. Mất SSH không dispatch lại; đọc journal và dùng resume ở controller.

Trong checkout candidate trên controller, nhập `CLOUDFLARE_ACCOUNT_ID`, `CLOUDFLARE_API_TOKEN`, `VINK_VERIFIED_BACKUP`, `VINK_RELEASE_CHROME` từ cấu hình vận hành. `OLD_SHA` là revision của current tree và ba container, `OLD_EDGE` là ID deployment canonical quan sát ở Pages (`none` nếu project mới), `READER` là UUID của một tác phẩm công khai thật.

```sh
r=infra/cloudflare/production/release.py
set -- /secure/release.json --account "$CLOUDFLARE_ACCOUNT_ID" \
  --previous-origin "$OLD_SHA" --previous-edge "$OLD_EDGE" --reader "$READER"
python3 "$r" plan "$@"
python3 "$r" apply "$@"
python3 "$r" status "$@"
```

`plan` kiểm API read-only, predecessor, backup/schema, platform, repo và revision label của cả ba digest. Nó pull image, tạo container export chưa start, lấy riêng `/opt/vink/site/pkg`, lưu archive cục bộ và tính SHA-256/MIME/byte của từng file. Mọi tệp phải thuộc inventory public của build; sai target, symlink, thiếu artifact, khác bytes cùng SHA hay vượt trần đều dừng. Nó không publish hoặc thay runtime. Tách credential khỏi bước build.

`apply` giữ host lock qua toàn bộ đợt đổi. Trước tiên xác minh từng byte ở alias preview riêng cho release. Sau đó publish bundle gồm cả release trước lên `main` và đọc lại deployment ID, commit và marker. Sau khi kiểm từng asset của candidate qua URL HTTPS theo deployment ID, controller mới thay ba image, giữ nguyên PostgreSQL, auth overlay, env và volume. Từng thay thế file/link là riêng lẻ; nhật ký ghi pha trước mutation. HTTP smoke kiểm HTML có SHA của release, ETag/304, canonical/index và lỗi; Chrome khách đọc trang chủ, Reader, Bàn viết, Tài khoản và reload/navigation. Receipt ghi riêng origin revision sau khi đối chiếu ID container/image, ba digest, asset digest, worker digest và Pages deployment ID.

HTML/JS/CSS/WASM và lazy imports giữ namespace `/_v/<sha>/pkg/`. Cả A và B tồn tại khi activate B hoặc khi quay lại A. Nếu request đến PoP còn Worker cũ, nó chỉ lấy byte ở alias Pages của SHA đã stage, không gửi cookie. Alias `/pkg/` tương thích cũ trỏ bộ asset bootstrap với revalidation. Khi lần đầu thay HTML **chưa có namespace**, kiểm tab của release cũ và chờ cửa sổ chuyển đổi trước khi tháo origin cũ. Giữ archive và alias còn caller; khi gần giới hạn file/worker, dừng, đo và duyệt chính sách retention trước release tiếp. [Preview aliases](https://developers.cloudflare.com/pages/configuration/preview-deployments/).

SSR/API dùng fetch bypass cache. Phản hồi riêng tư, có session hoặc có Set-Cookie dùng
`no-store`. Chỉ các route READ-01 có `private, no-cache` từ App mới giữ revalidation.
Worker đang triển khai thêm `no-transform` vào các phản hồi này để Pages giữ strong ETag khi
trình duyệt chấp nhận gzip hoặc Brotli. Giữ rule nén của zone theo cấu hình API. Range chưa
được bật ở asset adapter. GET trả toàn bộ file và HEAD không có body. Bypass cache cho SSR/API,
không áp Cache Everything, Always Online hoặc HTML fallback. Kiểm cookie, 304, TTL và header
tại domain thật. Chrome/workerd ở loopback chỉ kiểm hợp đồng cục bộ.

Khi origin trả `404` cho `/robots.txt`, Cloudflare Free có thể trả `200` với thông báo Content Signals chỉ chứa dòng chú thích. Smoke qua edge chấp nhận thông báo này nếu không có chỉ dẫn crawler. [Content Signals Policy](https://developers.cloudflare.com/bots/additional-configurations/managed-robots-txt/#content-signals-policy).

Rule ở zone giữ redirect `308` cho HTTP apex, giữ đường dẫn và query. Phản hồi HTTPS apex giữ HSTS `max-age=31536000`, `X-Content-Type-Options: nosniff` và `Referrer-Policy: strict-origin-when-cross-origin` như cấu hình Caddy trực tiếp trước cutover.

## Cutover và khôi phục

- Trước đổi delegation, export cả hai zone, đối chiếu từng record kể cả MX, SPF, DKIM (từng selector), DMARC, CAA, TXT xác minh và mọi dịch vụ ngoài Web. Query MX hoặc TXT ở apex không đủ để kết luận không có mail. Hạ TTL có thời gian hiệu lực. Gỡ DS cũ tại registrar, đợi parent và resolver cho thấy DS vắng mặt sau TTL; chỉ đổi NS khi hai zone trả dữ liệu tương đương. Xác minh delegation và TLS; sau khi ổn định mới bật signing mới và thêm DS mới. Nếu quay NS, gỡ DS mới và đợi cache trước khi phục hồi NS rồi signing/DS cũ. Giữ registrar Spaceship. [Quy trình DNS chi tiết](cloudflare-dns-20261119.md).
- Đưa direct origin và versioned HTML vào trạng thái ready. Kiểm preview asset, private origin, bản sao session/DB, Caddy và chứng chỉ. Đính apex/www bằng **Pages custom domains** ở đúng zone/account, xác minh certificate, route và origin. Đổi record theo diff đã duyệt, không để CNAME trỏ tới project chưa claim. Smoke qua HTTPS riêng cho apex/www. Khi bootstrap chưa chuyển DNS thì smoke `edge` sẽ dừng ở `origin_ready`; sau cutover chạy `resume` để hoàn tất. [Pages custom domains](https://developers.cloudflare.com/pages/configuration/custom-domains/).
- Nếu lỗi hoặc mất ACK, chạy `status`, đối chiếu Pages canonical ID/marker, alias và bytes với archive, `readlink -f /opt/vink-prod/current`, image ID/revision từng container, readiness, release.env và backup đã giữ. `resume` với **cùng input** nhận lại kết quả đã thực hiện rồi chạy phần còn lại. Với upload chưa đến provider, deployment khác marker hoặc origin chỉ đổi một phần, controller dừng. Đối chiếu request/deployment tại provider và dữ liệu VPS. Chỉ sau khi chứng minh thao tác chưa thực hiện hoặc đã khôi phục **cả hai** predecessor, lưu nhật ký cũ vào hồ sơ sự cố rồi mở lượt mới. Không tự retry khi kết quả chưa rõ.
- Rollback ứng dụng bằng cùng entry point từ tree A với receipt image A và predecessor B thực tế. Vẫn đóng gói A+B. Không rollback riêng deployment Pages cũ thiếu asset B. Nếu lỗi ở chính edge bootstrap, khôi phục vhost canonical trước đó, validate/reload, kiểm trực tiếp bằng `--resolve`, rồi phục hồi record apex/www theo snapshot mới đọc. Backup và restore khi có writes mới cần đối soát trên database riêng.

## Ngừng public test sau qualification

1. Lập bảng JSON riêng với `public_origin`, `production_receipt` và `resources`. Mỗi dòng gồm `kind, id, owner, callers, action, evidence`. Danh sách loại bắt buộc thuộc [retirement.py](../cloudflare/production/retirement.py). Inventory cả deployment hash, preview alias và `pages.dev`, DNS record kể cả origin-test, Caddy vhost, systemd/container/network, volume/DB/backup, GCS bucket, config-reader, credentials, CI, build và Auth qualification. Với tài nguyên đã vắng, lưu readback và dùng `gone`. Đối chiếu access log/caller trong cửa sổ vận hành; xóa dependency từng caller trước khi ghi danh sách rỗng.
2. Sau receipt production đạt và xác nhận cutover, deploy [Worker retirement](../cloudflare/production/test-retired.mjs) dưới tên `_worker.js` trong thư mục Pages trống dành riêng cho project test cũ để trả **410 Gone cho mọi method** ở test host, không forward body hoặc redirect path sang dữ liệu production. Chặn truy cập công khai project test (cả hostname provider, preview và deployment bất biến có app cũ) bằng Access hoặc vô hiệu endpoint theo inventory. Một deployment 410 mới không tự khóa các URL deployment trước. Kiểm GET/HEAD và một POST rỗng vô hại tới host đã nghỉ; URL provider phải trả 410/403/404 hoặc Access challenge, không còn SSR/API của test.
3. Gỡ record DNS test trong khi Pages vẫn giữ claim, đợi TTL/resolver hết CNAME, sau đó detach custom domain/routes. Với origin-test, kiểm hết caller, chặn vhost và kiểm truy cập trực tiếp rồi tháo DNS/vhost. Kiểm không còn direct App qua IP/Host. Retire chỉ systemd/service/network và quyền đọc config dành riêng cho test sau khi xác định owner. Dùng stop/disable; giữ volume, database, backup và bucket. Tách quyền token nếu dùng chung, giữ Containerfile và test Compose để chạy scratch. Qualification #358 dùng host riêng tư và proxy/sink cô lập; test gửi mail thật thuộc chủ dự án.
4. Chạy `python3 infra/cloudflare/production/retirement.py /secure/test-retirement.json`, lưu kết quả cùng readback HTTP, DNS, Pages và xác nhận các dịch vụ còn lại. Validator kiểm inventory khai báo; chỉ quan sát live mới nghiệm thu AC-10. Nếu phải phục hồi, giữ dữ liệu tách riêng, phục hồi service/vhost từ cấu hình cũ vào private network, kiểm lại quyền và backup, claim project/domain trước khi khôi phục DNS đã snapshot. Mọi bước cutover, retirement và phục hồi này do operator thực thi trong cửa sổ được phê duyệt.
