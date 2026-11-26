# Vận hành Vink production

`https://vink.forum` dùng Web SSR + API Vink, Worker và Identity qua Unix socket riêng. Caddy
của Cantos giữ cổng HTTPS công khai. App, Worker và Identity dùng PostgreSQL 17 riêng, với ba
owner database (App, Identity, Notifications), mỗi owner có role runtime và migrator riêng. Không publish port backend, Identity hay
PostgreSQL. Sign-in và mail Resend giữ tắt theo [Auth operations](../../docs/development/auth-operations.md).
Từ #536, worker mặc định chạy mock duyệt kịch bản `approve` cho tác phẩm (không phải AI). Opt-out
và kiểm backlog trước deploy theo [vận hành mock](../../docs/development/moderation-runtime.md#31-mock-bootstrap-và-công-tắc-vận-hành-536).
Giữ Vink test và Cantos trên mạng và dữ liệu hiện tại.

Điều kiện kết thúc giai đoạn giữ public test là production qua qualification và hoàn tất inventory retirement. Đóng gói frontend từ OCI, phát hành Pages, đổi origin và rollback theo [runbook Cloudflare edge production](cloudflare-edge.md).
Quy trình giữ domain ở registrar Spaceship và chuyển authoritative DNS sang Cloudflare Free
trước khi bật proxy. Build và phát hành image dùng workflow bên dưới.

Mỗi lượt deploy bắt đầu từ [quy trình Early Production](early-production.md). Chọn FAST, SCHEMA, BOOTSTRAP hoặc MAINTENANCE theo hai revision và dữ liệu đang có.

## Build và xác nhận bản phát hành

Chạy [workflow thủ công](../../.github/workflows/publish-images.yml) với `platform=linux/amd64` cho
VPS đã kiểm CPU. PR thay đổi hạ tầng production có lane `Qualify production OCI` chỉ đọc.
`publish=false` cũng kiểm tra một branch theo yêu cầu mà không đẩy image. `publish=true` chỉ trên
`develop` và chỉ sau khi các image vừa build chạy với PostgreSQL scratch, migration riêng theo
owner, HTTP App, Unix socket Identity, API và SIGTERM. Ghi SHA/run ID, bốn GHCR digest và digest
PostgreSQL từ cấu hình release. Kiểm lại revision label và `amd64` sau khi pull. Build ở runner,
không build trên VPS. Nếu package cần xác thực, dùng đường pull đọc riêng đã được phê duyệt.

## Nền VPS và lịch sử ingress trực tiếp

Các bước DNS/Caddy trực tiếp bên dưới mô tả nền #304. Khi chuyển sang Pages, dùng thứ tự trong [runbook hiện hành](cloudflare-edge.md).

1. Xác minh baseline HTTPS của Cantos và Vink test, tài nguyên, UFW, các bridge/IP và quyền
   deploy. Chỉ dùng plan này nếu hai dải IP đang trống. Deploy từ checkout sạch đã pin vào
   `/opt/vink-prod/releases/<sha>`, với các file trong `services/postgres` và `infra/production`.
   Trỏ `/opt/vink-prod/current` vào release đã chọn. Cấu hình `/etc/vink-prod/release.env` từ
   `release.env.example` với đúng bốn digest. Với release tiếp theo, giữ bản env cũ theo SHA.
   Khi sửa riêng manifest vận hành, ghi cả commit manifest và commit được ghi trên image.
2. Tạo `/etc/vink-prod` mode 0700. Chạy `python3 infra/production/create-runtime-files.py
   /etc/vink-prod/runtime` trên VPS một lần. Script chỉ nhận thư mục mới và tạo mật khẩu riêng,
   không dùng secrets Cantos/test. Giữ các file env ngoài Git, owner root. Bootstrap file có mode
   0444 để UID PostgreSQL bên trong container đọc được qua bind mount duy nhất, parent trên host
   mode 0700. `compose config` có thể in credentials, không lưu hoặc chia sẻ output.
3. Tạo external volume `vink-prod-postgres` sau khi xác minh tên trống. Tạo hai bridge Podman với
   `--internal --disable-dns`, `--interface-name` là `vinkpdb0` và `vinkping0`, `--subnet` là
   `10.89.3.0/24` và `10.89.4.0/24`, tên là `vink-prod-db` và `vink-prod-ingress`.
   Sau xác nhận thay đổi firewall, mở route phía DB từ members bridge `vinkpdb0` tới đúng
   `10.89.3.10/32:5432/tcp` trên cùng bridge. Phía ingress chỉ cho `10.89.4.2/32` (Caddy) đến
   `10.89.4.10/32:3000/tcp` trên `vinkping0`. Không thêm inbound port hay đổi UFW defaults.
4. Đặt `PODMAN_COMPOSE_PROVIDER=/usr/bin/podman-compose` và dùng đường dẫn tuyệt đối cho
   cả hai manifest. Giữ healthcheck PostgreSQL trong base để Podman Compose 1.3 không nối hai
   danh sách `healthcheck.test`. Dùng cùng prefix như trong `vink-prod.service` cho `up -d postgres`.
   Đợi `pg_isready -h 127.0.0.1 -U vink -d postgres` trong container. Dùng `--profile provision`
   cho `run --rm provision` và `run --rm provision --check`, rồi dùng `--profile migration` cho
   `run --rm migrate-app`, `run --rm migrate-identity` và `run --rm migrate-notifications`. Podman Compose 1.3 cần profile tường minh
   cả khi lệnh `run` chỉ đích danh service. Khi chạy từ một script nhận lệnh qua SSH stdin,
   chuyển stdin của các lệnh `compose run` sang `/dev/null` để provider không đọc phần script sau.
   Kiểm các lệnh migrate chạy lại no-op. Chỉ bật `vink-prod.service` sau khi hoàn tất migration.
   systemd tạo thư mục socket mode 0700. Start command chuyển owner sang UID 10001 và chạy `up`
   trong cùng tiến trình để systemd không đặt lại owner giữa hai lệnh. Restart policy của container
   xử lý process crash. Không bật `podman-restart.service` toàn VPS.
5. Kiểm readiness từ App và socket, UID, migrations, role hạn chế và cổng publish trước khi
   chạm edge. Bật backup timer riêng và kiểm thử restore trước khi mở public.
6. Sau xác nhận thay đổi edge, gắn Caddy container đang chạy vào `vink-prod-ingress` với
   `--ip 10.89.4.2`, thêm `vink-prod-ingress.conf` vào drop-in của `cantos-edge.container` để giữ
   khi boot. Kiểm `caddy validate` của toàn bộ candidate config và readiness `10.89.4.10:3000` từ
   Caddy. Import `vink-prod.caddy` rồi reload Caddy tại chỗ. Giữ bản trước thay đổi và đo lại
   Cantos/Vink test. Caddy thay header forwarding bằng IP peer và App tin đúng một IP ở ingress.
7. Dùng Spaceship Advanced DNS, đọc toàn zone và xác nhận lại diff. Sau khi owner xác nhận, thêm
   `@ A <IPv4 VPS>` và `www CNAME vink.forum` nếu hai tên trống. Giữ TTL 300 nếu UI hỗ trợ, lưu
   readback từ hai NS và resolver công cộng. Kiểm HTTPS, chứng chỉ và redirect giữ path/query.

## Bổ sung Notifications vào release ba image đang chạy

1. Lấy bốn digest cùng revision từ workflow đã qua smoke. Cập nhật receiver và `artifacts.py` đã cài trên host để nhận cả receipt cũ và mới. Chuẩn bị env của candidate từ env hiện hành, thêm ba biến Notifications trong [mẫu public](release.env.example). Giữ nguyên cờ auth, mail overlay và credential của App/Identity. Với runtime đã có, chạy `create-runtime-files.py` vào một thư mục riêng mode 0700; lấy riêng hai file `notifications-{runtime,migrator}.env` và hai dòng `VINK_NOTIFICATIONS_{RUNTIME,MIGRATOR}_PASSWORD` tương ứng cho `provision.env`. Khi tiếp tục lượt chuẩn bị dở, dùng lại đúng cặp mật khẩu đã cấp cho DB.
2. Dùng manifest và env candidate trong bước provision và kiểm role ở trên. Tạo thư mục inbox và ingest riêng, mode 0700, UID/GID của runtime. Kiểm unit systemd **và drop-in Resend** tạo/chown cả hai thư mục khi boot và start/stop service `notifications`. Đưa hai biến đường dẫn vào env dùng bởi controller. Không thay pin image của các tiến trình đang chạy.
3. Trước lượt chuyển đầu tiên, đối chiếu migrations của `services/vink-notifications` giữa hai SHA. Nếu giống nhau, chạy `migrate-notifications` của candidate trên DB mới, rồi kiểm chạy lại no-op. Nếu khác, chuẩn bị DB ở ledger của predecessor rồi dùng schema review của cả ba owner. Với BOOTSTRAP + SCHEMA đã review, ghi nhận bộ backup timer của predecessor. Với cửa sổ bảo trì hoặc lượt chuyển topology theo controller cũ, chuẩn bị bộ backup theo điều kiện của [đường phát hành đã chọn](early-production.md). Dùng controller cho các migration còn lại và chuyển release theo [runbook edge](cloudflare-edge.md).
4. Kiểm `/health/ready` qua socket inbox **từ App** và socket ingest **từ worker**. Cả hai phải trả `ready`, `schema=current`, `role=least_privileged`, `recovery=serving`. Kiểm count và inbox bằng phiên test qua HTTPS. Chỉ sau đó bật thu sự kiện bằng `vink-app notifications activate <ghi chú>`. Cutoff lấy tại giao dịch kích hoạt.
5. Khi backout, dùng `vink-app notifications deactivate <ghi chú>`, kiểm nghĩa vụ relay và giữ DB cùng receipts. Khi restore, đóng cổng recovery trước khi cho đọc và chỉ mở sau đối soát/replay theo [runbook Notifications](../../docs/development/notifications.md#73-retention-sự-kiện-muộn-và-phục-hồi). Khi quay về topology ba image, dừng relay và Notifications trước khi đổi manifest, đồng bộ lại unit/drop-in với release cũ.

## Kiểm chứng và phục hồi

- Ghi App/Identity `ready` với `database=ok`, `schema=current`, `role=restricted`, UID peer,
  trạng thái Worker và `_sqlx_migrations` cho từng DB. Kiểm runtime env không có quyền migration.
- Kiểm landing/deep link, `/api/v1/works`, console/hydration trên desktop và mobile, WASM MIME
  `application/wasm` và nén thật, JavaScript/CSS/fonts. Chỉ đánh dấu sign-in gate khi kiểm được
  response disabled. Không dùng tài khoản hoặc nội dung của test cho production.
- Chạy một restart **chỉ Vink production**, kiểm lại health, migration và dữ liệu. Kiểm unit được
  enable và dependency trong systemd. Reboot VPS là bài kiểm riêng cần lịch và đồng ý do dùng chung.
- `vink-prod-backup.timer` chạy hằng ngày từ 02:15 UTC. Script của topology bốn image yêu cầu cả ba custom dump có checksum
  (`vink_notifications`, rồi `vink`, `vink_identity`; dump
  Notification lấy trước để bản restore không bao giờ đi trước App), giữ bảy bộ trên cùng VPS với mode 0700. Không khẳng định offsite hoặc
  snapshot chung giữa các DB. Khi khôi phục, làm theo [runbook theo loại sự cố](../../docs/development/notifications.md#73-retention-sự-kiện-muộn-và-phục-hồi)
  (Notification không bao giờ được phục vụ khi đi trước App; replay lấp khoảng trống).
  Restore rehearsal dùng PostgreSQL 17 riêng và volume tạm, kiểm checksum, replay `pg_restore
  --exit-on-error`, so khớp schema/migrations và số dòng. Vai trò phải tồn tại khi dùng owner/grants
  trong lần recovery thật. Ghi vị trí/checksum, thời gian và giới hạn, không đính kèm dumps vào PR.
- Với release kế tiếp, áp dụng chính sách backup theo [chế độ deploy](early-production.md), kiểm migration tương thích và giữ release/digest cũ trước
  khi đổi `current` và env. Nếu rollback lần đầu, gỡ import production và reload edge sau khi
  validate, hoàn nguyên đúng hai record theo fresh UI, rồi dừng riêng các container ứng dụng nếu cần. Giữ PostgreSQL chạy và `vink-prod.service` active vì backup service khai báo `Requisite=vink-prod.service`. Giữ timer hằng ngày.
  Giữ volume, backup và bằng chứng. Nếu đã thêm bridge/UFW production, chỉ gỡ chính rule/attachment
  production khi đã xác nhận phạm vi. Không xóa bản ghi hoặc hạ tầng của Cantos/Vink test.
- Không rollback schema theo image. Chỉ quay về image trước khi readiness của từng owner đã được
  chứng minh trên bản sao của schema hiện tại. Recovery có khả năng mất writes phải restore vào
  kho riêng, đối soát ba database rồi xin xác nhận thời điểm và dữ liệu trước khi chuyển hướng.

Overlay [Resend HTTPS opt-in](../auth-mail/README.md) là đường chuẩn bị riêng cho native Identity.
Chỉ thêm overlay sau khi đã thử egress và rollback trên môi trường qualification cô lập. Khi chủ dự án mở cờ,
đăng nhập/đăng ký công khai; không chờ tính năng quên mật khẩu O-11 để hiện form.
