# Bật xác thực Vink có kiểm soát (Resend HTTPS)

**Mặc định tắt.** Manifest thường của test và production giữ `VINK_AUTH_SIGN_IN=disabled`.
Overlay `auth-resend.compose.yaml` của từng môi trường là tệp thứ ba, đặt sau base PostgreSQL
và overlay triển khai hiện tại. Chỉ dùng khi đã có build hỗ trợ `resend-api` và đã kiểm chứng kế hoạch
mạng trên Linux/Podman đích. Không tự chạy lệnh vận hành, đổi DNS/firewall hay gửi thư thật.
Chạy qualification trên host hoặc máy ảo riêng có origin HTTPS từ inventory test. Compose test
dùng volume external `vink-test-postgres` và mạng mail có tên cố định, vì vậy hãy xác minh host
thử trước khi tạo dữ liệu. Host này vẫn dùng được sau khi ngừng public test theo
[runbook Cloudflare/VPS](../production/cloudflare-edge.md). Tách secret, sender, DB và tài khoản thử khỏi production.

## Cấu hình chỉ phía máy chủ

Đặt biến **không bí mật** vào env file của Compose (`--env-file`), ví dụ cho test:

```dotenv
VINK_TEST_AUTH_SIGN_IN=disabled
VINK_TEST_RESEND_FROM=no-reply@mail.qualification.example
VINK_TEST_RESEND_SECRETS_DIR=/etc/vink-test/resend
VINK_TEST_AUTH_MAIL_IDENTITY_IP=
```

Production dùng tiền tố `VINK_PROD_` cho cả bốn biến, sender thuộc realm khác (chẳng hạn
`no-reply@mail.vink.forum`), thư mục riêng (chẳng hạn `/etc/vink-prod/resend`) và IP từ
subnet mail production. Các tên miền trên là ví dụ, không suy ra DNS hoặc sender đã xác minh.
Dùng gửi-only key riêng, giới hạn đúng domain gửi trên Resend; giữ quyền chỉnh DNS riêng. Không đặt
key trong env, Git, history shell, runtime backup, image hay output của CI. Đọc
[Resend API keys](https://resend.com/docs/dashboard/api-keys/introduction) và
[Domains](https://resend.com/docs/dashboard/domains/introduction) để đối chiếu quyền và xác minh.

Trong mỗi thư mục key chuyên dụng chỉ có tệp `api-key`, mode `0600` và UID/GID mà tiến trình
Identity thấy là chính nó; thư mục mode `0700` và đủ quyền traverse cho tiến trình. Với Podman
có user namespace, kiểm tra mapping thực tế, không suy từ UID host. Chỉ Identity nhận thư mục
read-only; `create-runtime-files.py` chỉ sinh mật khẩu DB, không sinh hay sao lưu Resend key.
Đổi key bằng cách tạo tệp tạm mode 0600 trong cùng thư mục, chown đúng UID/GID, rồi thay tên
nguyên tử vào `api-key`. Kiểm tra với key mới trước khi thu hồi key cũ. Không in nội dung key,
dùng kiểm tra quyền và tính tồn tại thay cho `cat`.

Không đặt flag trong `app-runtime.env` hoặc mode/sender/key trong `identity-runtime.env`:
`environment` của Compose ưu tiên hơn `env_file`. Xóa khóa cấu hình mail cũ khỏi file
Identity nếu có. Dùng release env rõ đường dẫn và không export biến cùng tên trong shell:
shell có thể ưu tiên hơn `--env-file`. Chỉ App đọc flag; Worker luôn tắt. Bản chưa gắn
overlay không bật được bằng một cờ rơi vào shell.

## Egress: proxy riêng, không cấp Internet trực tiếp cho Identity

1. Sau khi duyệt thay đổi mạng, chọn hai dải IPv4 không trùng route/bridge, mỗi môi trường
   một dải. Tạo external bridge Podman `--internal --subnet <CIDR>` riêng:
   `vink-test-auth-mail` và `vink-prod-auth-mail`. Cần DNS container để Identity phân giải alias
   nội bộ, không dùng `--disable-dns` cho hai bridge này. Kiểm thực tế với phiên bản Netavark/Aardvark
   [đích](https://docs.podman.io/en/stable/markdown/podman-network-create.1.html).
2. Hạ tầng cấp một proxy CONNECT **riêng cho mỗi môi trường**, dùng image đã pin digest và xem xét,
   với hai attachment: bridge mail nội bộ và uplink chỉ của proxy. Trên bridge mail, đăng ký
   alias `vink-resend-egress`, IP listener tách khỏi IP Identity. Không publish port, không join mạng
   DB/edge/Cantos và không mount key, socket, DB hay Docker/Podman socket. Dùng
   [Squid policy mẫu](squid.conf.example) sau khi thay địa chỉ listener và CIDR /32 của chính
   Identity, kiểm syntax bằng `squid -k parse -f <candidate>` trên **image sẽ dùng**. Không chèn
   policy vào proxy dùng chung. Squid chỉ cho nguồn Identity mở tunnel tới hostname đúng
   `api.resend.com:443`; tunnel không giải mã TLS. Identity vẫn xác minh chứng chỉ TLS và
   gửi tới endpoint cố định. Vận hành xác nhận uplink, DNS của proxy và firewall; không
   đổi rule chung hoặc mở inbound để sửa lỗi egress.
3. Không đổi bridge DB/ingress production hoặc default bridge/cổng loopback đã dùng của test.
   Overlay test bổ sung một bridge DB internal theo project, chỉ có PostgreSQL và Identity.
   PostgreSQL giữ attachment cũ cho App/Worker/migrations; Identity không vào default bridge.
   Lên lịch thao tác test vì cập nhật Compose có thể recreate container PostgreSQL để thêm
   attachment. Giữ nguyên external volume và kiểm health PostgreSQL, public read và loopback
   App sau khi thêm. Xác nhận host trong DSN Identity test là `postgres:5432` và alias
   `postgres` phân giải trên bridge DB internal mới; không dùng IP/hostname của default bridge.
4. Kiểm `podman network inspect` và attachment từng container, không log toàn bộ inspect có
   environment. Identity chỉ vào DB/mail internal, proxy đích không publish, PostgreSQL không
   publish, socket vẫn đúng UID. Kiểm từ Identity: CONNECT/TLS không payload tới Resend qua proxy,
   direct egress không thành công và proxy từ chối CONNECT tới destination ngoài allowlist, port
   khác, HTTP thường và nguồn khác Identity. Với các probe qua proxy không dùng Bearer key.
   Không chạy probe 'send mail' để kiểm network.

## Qualification và bật flag

Lấy backup đúng môi trường và pin SHA/digest App/Identity/Worker trước. Review output của
`podman compose ... config` trực tiếp trên host hoặc redirect vào `/dev/null`; không lưu nó
vào CI/log/PR vì có thể chứa credentials đã có. Dùng cùng thứ tự và đường dẫn tuyệt đối cho
Compose base/overlay/overlay auth. Với test, dạng lệnh trên host là:

```sh
podman compose --project-name vink-test --env-file /etc/vink-test/deploy.env \
  -f services/postgres/compose.yaml -f infra/test/compose.yaml \
  -f infra/test/auth-resend.compose.yaml config >/dev/null
```

Dùng network test đã chuẩn bị, khởi động bằng prefix đã render với flag disabled. Migration và
provision vẫn theo tài liệu môi trường. Khởi động riêng Identity bằng overlay trước khi recreate App, để lỗi key/UID không làm gián đoạn
App đang phục vụ. Overlay yêu cầu Identity healthy trước khi Compose khởi động App. Nếu lần
chuẩn bị đầu thất bại và chưa từng cấp phiên thật, bỏ overlay và dùng đúng bản base đã qua kiểm
chứng để phục hồi App; không dùng phương án này sau khi đã có phiên thật. Runtime Identity phải
qua health probe Unix socket và
báo `authority=available`, `mail_transport=resend_api`, database restricted và schema current.
App `/health/ready` vẫn dành cho public read, kiểm riêng `integrations.identity=available`.
Sức khỏe process/queue hoặc HTTPS probe **không** chứng minh Resend đã nhận hoặc mailbox đã nhận.
Trước khi kích hoạt production, đối chiếu cấu hình unit cho cả up/stop và thứ tự khởi động
proxy, network sau reboot. Dùng đường dẫn dưới `/opt/vink-prod/current` cho bộ ba file Compose.
[Orchestrator VPS](../cloudflare/production/origin.sh) giữ overlay khi nhãn `config_files`
của App khớp bộ ba theo đúng thứ tự và release mới có overlay. Unit mặc định chỉ dùng hai file.
Cổng schema của [release](../cloudflare/production/release.py) yêu cầu cả image cũ và mới
đạt readiness trên schema sau migration. Trong rehearsal PostgreSQL 17.11, Identity có mã
như `ac77a0f` trả 200 trước migration `20261120000000` và 503 (`schema=diverged`) sau đó.
Identity Resend trả 200 trên schema mới và bản ghi delivery đang chờ vẫn còn. Vì vậy
`old_on_result` là **FAIL** cho cặp này. Nếu image tiền nhiệm ở VPS có cùng giới hạn,
release bị chặn đến khi có lộ trình chuyển schema tương thích đã được thử trên bản sao
với đúng cặp digest. Cờ Auth chỉ điều khiển cấp phiên, không giải quyết cổng schema.

Chỉ sau khi có chấp thuận, thử thư transport tới hộp thư do chủ dự án kiểm soát, đối chiếu
provider accepted và mailbox delivered riêng; không dùng key/test recipient thật trong CI.
O-11/recovery là phạm vi riêng, không phải điều kiện bắt buộc phải có email hoặc invite để đăng ký. Sau qualification test, chỉ người vận hành có
quyền mới đổi `VINK_TEST_AUTH_SIGN_IN=enabled` và recreate App (restart không nạp env file mới).
Thử từ URL HTTPS: SSR rồi hydration, username/password đăng ký/login, xác minh email, session
cookie Secure/HttpOnly/SameSite, principal/bút danh, Bàn viết, logout/login lại, refresh, hai tab, CSRF và trạng thái
lỗi. Sau khi test đạt, lặp lại từ đầu trên production **với bộ key, sender, DB, proxy khác** và
chỉ bật `VINK_PROD_AUTH_SIGN_IN=enabled` khi đã có quyết định go/no-go riêng.

## Tắt và rollback không phá dữ liệu

Đổi cờ môi trường về `disabled` và recreate App bằng cùng overlay ngay; thử `/toi` guest và
endpoint start challenge trả về disabled, đồng thời kiểm session hợp lệ vẫn được nhận và có
thể logout, public reads và Worker vẫn chạy. Không dừng Identity trước nếu cần xác minh session.
Cờ tắt ngăn cấp phiên/challenge mới qua App, không thu hồi challenge đã gửi hoặc hủy thư đã vào
hàng đợi; chúng tự hết hạn. Nếu sự cố yêu cầu dừng cả thư in-flight, ngắt chỉ proxy mail riêng
và/hoặc thu hồi key theo incident, giữ socket Identity để phiên hiện có được xác minh. Xác minh lại
không còn delivery mới được chấp nhận; không xóa outbox, account, session, volumes hay migrations.
Chỉ quay image cũ sau khi kiểm tương thích schema và khả năng kiểm phiên của authority. Chuyển về
base-only với authority chưa cấu hình có thể làm phiên cũ không xác minh được, không dùng đó
làm emergency stop thay cho flag. Sau khi rollback kiểm lại origin và dữ liệu của chính môi trường vừa thay đổi.

## Kiểm trên máy phát triển

`ruby infra/auth-mail/check-static.rb` đọc các manifest và chính sách mẫu.
`bash infra/auth-mail/check-render.sh` cần Docker Compose và jq, chỉ render fixture không có secrets
thật và không start container. Cả hai không thay cho thử runtime Podman/egress ở môi trường đích.
