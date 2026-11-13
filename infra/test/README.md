# Nền test-domain và OCI Vink

**Scope:** code/config foundation cho `test.vink.forum`; không phải production hoặc một môi trường
đã online. Không có VPS, IP/CPU, proxy, authoritative DNS, secrets hay quyền publish/deploy được
xác minh trong lượt này. Không lấy IP Cantos hoặc kiến trúc Mac làm target mặc định.

## Ranh giới và lựa chọn

- DNS: **CHOICE-01 A, MCP Spaceship chính thức** là đường apply chính, khi client và tài khoản
  được cấp quyền. Không viết API adapter cạnh tranh. `xtask infra dns-*` chỉ xử lý snapshot chuẩn
  hóa do operator xuất từ provider, không gọi mạng và không chứng minh provider đã kết nối.
- Image: **CHOICE-02 B**, ba final images (`vink-web`, `vink-worker`, `vink-identity`) dùng chung
  builder. Worker không mang site và không chạy pipeline npm; Identity cũng không cần Web build.
  Các binary vẫn cùng workspace, không thêm service/domain owner. Chưa đo kích thước image.
- Runtime: **CHOICE-03 B, Compose**, kế thừa topology Compose hiện hữu ở services/postgres.
  Đây là manifest ứng viên, chưa chốt runtime VPS. Không cài Docker/khởi động VM Cantos, không
  thêm Quadlet/Kubernetes song song. Mac có Podman nhưng VM `cantos-deploy` đang tắt; không tự
  khởi động hoặc sửa VM của công việc khác.

Các lựa chọn kỹ thuật trên có thể đảo ngược; không nâng D-05 hoặc proposal sản phẩm thành Accepted.
Tham chiếu [App/Identity](../../docs/architecture/identity-postgres-boundaries.md),
[Auth operations](../../docs/development/auth-operations.md),
[PostgreSQL owner](../../services/postgres/README.md),
[lệnh scoped](../../docs/development/scoped-workflows.md).

## Lệnh đã triển khai và nguồn cấu hình

Từ root repository:

```
cargo xtask infra --help
cargo xtask infra check /absolute/path/test.toml
cargo xtask infra render-env /absolute/path/test.toml > /absolute/path/compose.public.env
cargo xtask infra build-images podman linux/arm64
cargo xtask infra dns-plan /absolute/path/test.toml /absolute/path/dns-before.json
cargo xtask infra dns-check /absolute/path/plan.json /absolute/path/dns-fresh.json
cargo xtask infra dns-rollback /absolute/path/plan.json /absolute/path/dns-current.json
```

`infra check`, `render-env` và `dns-*` chỉ đọc file, không network/write/start. `build-images` là
thao tác build explicit; không publish/start/migrate. Engine chỉ `podman|docker`, platform phải
được chọn tường minh. Source phải sạch cả tracked/untracked; SHA lấy từ Git, không do caller tự
gán. Container build tiếp tục dùng `cargo xtask build web|worker|identity --release`, lockfiles và
pipeline site hiện hữu. Context build dùng `git archive HEAD` (không lấy ignored env/cache của checkout). `infra` không đổi nghĩa `dev`, `build`, `verify` local.

Copy [inventory](test.example.toml) **ra ngoài Git**. Mẫu cố ý để trống platform, digest và DNS
endpoint: `check` phải FAIL tới khi có facts thật. `public_origin` có thể đổi sang HTTPS hostname khác mà không đổi digest image; DNS planner vẫn
chỉ quản lý host test.vink.forum. Một nguồn origin/port/UID/image reference ở
inventory, `render-env` chỉ xuất cấu hình public cho Compose. Secrets/runtime env ở thư mục riêng,
không export chúng vào shell build. Precedence: inventory → public env → Compose `environment`;
Compose `environment` khóa profile/Origin/auth-off/socket, ưu tiên hơn file env của service.
Không dùng `--env-file .env` local, `.env.example` hay `compose.local.yaml` trên test host.

`compose.yaml` ghép với `services/postgres/compose.yaml`; project `vink-test`, volume external
`vink-test-postgres`, không publish cổng DB. Không dùng default volume dev/Cantos. DB server ở
network Compose riêng; nếu target có PostgreSQL quản lý sẵn thì cần review adapter/network riêng
trước, không tự trỏ cluster đang dùng hoặc expose DB.

```
podman compose --env-file /etc/vink-test/compose.public.env \
  -f services/postgres/compose.yaml -f infra/test/compose.yaml config
```

`config` có thể in secret từ `env_file`: chỉ chạy cục bộ, không upload output/log; đừng dùng để
chứng minh redaction. Manifest này chưa được runtime Compose kiểm chứng trong lượt này.

### Secrets và UID

Tạo các file env riêng mode0600 ngoài repository, không chép URL mẫu/password local:

| File | Chỉ chứa |
| --- | --- |
| `app-runtime.env` | `DATABASE_URL` của App runtime; `VINK_TRUSTED_PROXIES` chỉ IP proxy thật nếu cần |
| `worker-runtime.env` | App runtime `DATABASE_URL`; tùy chọn outbox hợp lệ |
| `identity-runtime.env` | `VINK_IDENTITY_DATABASE_URL`; authority chưa cấu hình hoặc authority/mail đã được review |
| `app-migrator.env` | Chỉ `VINK_MIGRATE_DATABASE_URL` |
| `identity-migrator.env` | Chỉ `VINK_IDENTITY_MIGRATE_DATABASE_URL` |
| `provision.env` | Bootstrap `PGPASSWORD` và passwords/roles của owner provisioning, chỉ one-shot |
| `postgres-bootstrap-password` | Bootstrap password độc lập; không phải credential service |

Không đặt migrator, `VINK_TEST_DATABASE_URL`, dev principal, mock scenario hoặc local mail outbox
trong env runtime. PostgreSQL roles được provision bằng owner script với database/role test riêng,
không dùng superuser cho App/Identity; readiness phải báo schema current và role restricted.

Manifest dùng cùng UID non-root cho hai socket peer để ít quyền hơn so với chown/privileged init
container. Trên host, chuẩn bị `socket_dir` do UID đã map sở hữu, mode0700, trước khi start.
Identity tạo socket0660. Parent0700/0750 hợp lệ; 0770/0755 bị source từ chối. App mount read-only
parent vẫn cần quyền connect socket; phải đo UID thực trong namespace hai container. Rootless
Podman mapping không tự đồng nghĩa UID host10001; dùng `podman unshare`/operator provisioning đúng
mapping, không chmod777, bỏ peer guard hoặc chạy privileged để chữa.

SMTP nếu bật sau này cần bind mount password file0600 đúng UID Identity và cấu hình transport TLS.
Compose secret mặc định0444 không đáp ứng kiểm tra source; base manifest **không cấu hình mail**.
Auth sign-in bị khóa `disabled`; muốn opt-in test phải review authority/rate-limit/mail transport,
thêm override explicit và kiểm lại, không sửa default global hoặc dùng fake email như mail thật.

## OCI build, smoke, publish và release

Build không cần DB/DNS/SMTP/SSH. `.dockerignore` allowlist source; final image chỉ binary/site,
CA certificates, curl và Debian runtime. Không Rust/Node/source/secrets trong final image.
Toolchain1.99.1 và wasm-bindgen0.2.129 theo repo; khi nâng lockfile phải cập nhật pin rồi build lại.
Kiểm linux libs bằng `ldd`, non-root, site đầy đủ, config-fail exit2, health và SIGTERM thật trước
khi đánh dấu image-built/qualified. Base tags phải resolve và lưu digest ở lần build thật; không
coi version tag hoặc local image ID là registry digest. Multiarch chỉ build platform được giao.

[Manual workflow](../../.github/workflows/publish-images.yml) chỉ `workflow_dispatch` trên develop,
không trigger push/PR, không credentials ở build/smoke; `GITHUB_TOKEN packages:write` chỉ dùng bước
push. Không có Spaceship/SSH secret. Publish thật chưa được thực hiện/dispatch. Operator chọn
platform đã xác minh. Workflow ghi remote RepoDigests cùng source SHA/platform vào summary TOML;
copy `web_image` sang `app_image` của inventory. Pull credential trên VPS chỉ `read:packages`,
không tái dùng token publisher. Không dùng `latest` để rollback.

Trước publish phải smoke image với scratch PostgreSQL/roles đúng quyền và image topology, không
chỉ loader smoke trong workflow. Lần này chưa có container daemon nên smoke đó NOT_RUN. Workflow
loader smoke chỉ chứng minh loader/config và asset presence, không chứng minh Auth/SSR chạy thật.

## DNS plan và apply sau khi có quyền

Nguồn chính thức đã đọc 13/11/2026:
[Spaceship MCP](https://www.spaceship.com/knowledgebase/spaceship-mcp/),
[Spaceship API/schema](https://docs.spaceship.dev/),
[GHCR](https://docs.github.com/en/packages/working-with-a-github-packages-registry/working-with-the-container-registry),
[publish](https://docs.github.com/en/actions/tutorials/publish-packages/publish-docker-images).
MCP được công bố tại `https://mcp.spaceship.com/mcp`; client hiện không có Spaceship tool đã kết
nối. Không lấy dashboard URL làm bằng chứng auth. API không phải đường triển khai thứ hai.

Sau khi owner xác thực vào đúng account: đọc domain và authoritative NS thật; nếu không do
Spaceship phục vụ thì dừng, không đổi NS. Đọc **mọi trang records**, giữ export gốc + provider ID.
Chuẩn hóa snapshot JSON dạng sau (giá trị thật từ provider, không dùng ví dụ này để apply):

```
{"domain":"vink.forum","authoritative_provider":"spaceship","nameservers":["VERIFIED_NS"],
 "complete":true,"records":[{"name":"@","type":"TXT","value":"original value","ttl":300}]}
```

Tên là relative, `test` chỉ A/CNAME. Snapshot không phải API wire schema. Planner giữ nguyên
records khác; chặn nếu `test` có TXT/MX/AAAA hoặc FQDN chưa normalize. CNAME chuyển từ A là thay
record trong scope, không đụng apex/www/mail. TTL phải nằm trong bound planner và provider thật.
Không hỗ trợ apply AAAA trước khi IPv6/HTTPS/routing có evidence.

1. `dns-plan` → lưu plan JSON gồm before/after type/name/value/TTL; hiển thị diff và xác minh
   endpoint thực, không localhost/private/example. Unit tests là dữ liệu tổng hợp, không endpoint.
2. Đọc fresh snapshot ngay trước apply; `dns-check` chặn drift scoped hoặc NS. Unrelated changes
   được giữ nguyên. Nếu before==after, **zero writes**.
3. Gọi MCP chỉ để remove/create/update đúng records trong plan, không force toàn zone. Ghi từng
   outcome; nếu mất ACK/partial apply, readback rồi lập plan mới, không retry mù.
4. Readback provider + tra A/CNAME có thẩm quyền và public resolver, sau đó mới gọi DNS-applied.
5. Rollback: `dns-rollback` yêu cầu fresh scope còn bằng after; trả plan đảo chỉ scope này. Nếu
   có drift phải dừng. Không khôi phục cả zone snapshot hoặc xóa thay đổi của người khác.

`dns-check` là kiểm pure snapshot, không khóa transaction provider. MCP apply phải fresh-read và
kiểm lại từng kết quả; nếu provider không có CAS, khoảng race còn tồn tại, không hứa atomicity.
Đích test hiện chưa có nên chưa tạo diff thật, chưa apply hay verified DNS.

## Runbook test và rollback

Không SSH/deploy/restart trong lượt này. Trước vận hành, xác nhận CPU, runtime+version, socket/UID
mapping, proxy đang dùng, port3300 trống, disk, secret management và quyền target. Không dựng proxy
thứ hai tranh80/443 của Cantos.

1. Preflight inventory/check + validate secret ownership, no sensitive logs/context.
2. Tạo volume test external explicit. Dùng đúng compose pair và public env, `up -d postgres`;
   chờ `exec -T postgres pg_isready -U vink -d postgres` thành công. Chỉ server nhận kết nối, chưa
   phải schema ready. Chạy `run --rm provision` rồi `run --rm provision --check`: service one-shot
   dùng [provision.sh](../../services/postgres/provision.sh) mounted read-only trong network Compose,
   `PGHOST=postgres`, bootstrap config riêng ở `provision.env`. Không mở DB host port. Paths trong
   Compose resolve từ file đầu `services/postgres/compose.yaml`, nên mount `./` là owner postgres,
   không phải toàn repo. Không dùng volume/provision.env local.
3. Chạy one-shot `run --rm migrate-app`, rồi
   `run --rm migrate-identity`. Kiểm lần hai no-op, schema thiếu và role sai bị từ chối. Runtime
   không nhận file migrator. Dừng khi schema incompatible, không tự down migration.
4. `up -d identity app worker`; xác minh readiness App/Identity, peer caller đúng/sai UID,
   restart Identity và stale socket guard, App phục hồi. SIGTERM worker/Identity và App; grace20s
   lớn hơn drain10s của outbox. Không dùng health Identity để thay test caller App.
5. Proxy hiện có tạo vhost test riêng, upstream loopback host_port từ inventory, Host/origin đúng,
   forwarded IP chỉ từ proxy thật. TLS/renew độc lập image; `X-Robots-Tag: noindex, nofollow` trên
   test (không phải access control). Không cache session/private/no-store, không CORS* hoặc cookie
   Domain=.vink.forum. Review config diff không thay route Cantos, kiểm config trước reload có quyền.
6. HTTPS/cert/renew, health, SSR no-JS, hydration, fonts/brand/editor/expressions, Origin/CSRF/cookie
   và luồng đã chọn phải kiểm thật. Nếu Auth chưa opt-in thì luồng viết là NOT_RUN, không fake PASS.
7. Chỉ báo test-online khi những boundary trên đạt. Ghi source SHA, remote digest, platform và
   migration versions của hai owners. Giữ release cũ theo digest cùng backup/schema compatibility.

Rollback image không rollback schema. Binary cũ có thể readiness `diverged`; chỉ rollback digest
khi compatibility đã kiểm, nếu chưa thì giữ hệ thống dừng và phục hồi theo plan operator. Không
`down -v`, xóa volume, tự down migration, watchtower hoặc auto-pull timer.

## Acceptance và evidence

Xem [báo cáo lượt chạy](../../docs/development/test-container-evidence.md). Code-ready,
image-built, image-published, DNS-applied và test-online là năm mức khác nhau. Không có quyền
hạ tầng không cản PR code độc lập, nhưng tuyệt đối không đổi NOT_RUN thành PASS.
