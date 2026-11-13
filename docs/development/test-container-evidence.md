# Bằng chứng nền test-domain/container

Ngày13/11/2026, Mac-mini-cua-Manh.local (darwin/arm64), worktree riêng từ
`origin/develop@34a9e5d` sau #188. Không thay checkout đang dùng hoặc worktree moderation.

## Phạm vi kiểm chứng

- `cargo test --locked -p xtask`: PASS, 80 unit tests + 1 CLI integration; 1 ignored test cũ
  của owner khác không đổi. Infra có 8 pure tests: inventory/digest/path, đổi origin không đổi
  image, DNS idempotency/case normalization, scoped rollback, drift/authority/scope, target invalid,
  A/CNAME replacement/TTL, unknown fields và không render credential/migrator/auth unlock.
- `cargo clippy --locked -p xtask --all-targets -- -D warnings`: PASS.
- `cargo fmt --all --check`, `git diff --check`: PASS.
- YAML Ruby parser cho Compose và manual workflow: PASS syntax, không thay thế Compose runtime.
- `cargo xtask infra --help`: PASS, đúng command surface.
- `cargo xtask infra check infra/test/test.example.toml`: expected FAIL (unknown platform/digests).
  Đây là kiểm fail-closed, không phải inventory ready.
- Independent source review tìm/sửa5 vấn đề: brand provenance thiếu trong context, ARM runner,
  DNS self-CNAME qua case, snapshot FQDN case, provisioning order/context. Không còn source blocker
  qua review, nhưng chưa có Linux runtime evidence.
- `git ls-remote https://github.com/actions/checkout.git refs/tags/v7.0.1`: pin checkout
  `3d3c42e5aac5ba805825da76410c181273ba90b1` xác minh đúng tag chính thức.

Không đọc/poll/chờ GitHub Actions, checks hoặc workflow logs. Manual workflow không dispatch.

## Prerequisites và trạng thái thật

`podman machine list` cho thấy chỉ có VM `cantos-deploy`, chưa chạy; `podman info` trả connection
refused. Không tự start VM Cantos, cài runtime mới, sửa security settings hoặc tạo Linux target.
Mac chỉ còn khoảng1,7GiB trống sau compile xtask, không phù hợp release build3 images. Image
build/loader smoke/SSR và Linux peer UID/lifecycle là **NOT_RUN**, không gọi cấu hình là đã chạy.

Không có target VPS/IP/CPU/proxy/secrets được giao. Không có Spaceship tool đã kết nối trong
catalog; chưa xác minh account, authoritative NS hoặc đầy đủ records. DNS diff thật/apply/readback
và HTTPS/TLS renew là **NOT_RUN**. MCP/source docs chỉ là phương án, không là bằng chứng auth.

| AC | Evidence | Trạng thái |
| --- | --- | --- |
| AC-01 test config riêng | inventory + Compose, unit fail-closed; không có secret/fixture | PASS code; target NOT_RUN |
| AC-02 OCI source build + SSR/assets | Containerfile dùng xtask, context Git archive + brand provenance | NOT_RUN image build/smoke |
| AC-03 lifecycle/socket UID | manifest3 process; guard/UID contract theo source | NOT_RUN Linux topology/restart/SIGTERM |
| AC-04 DB/role/migrate boundary | runtime env tách, one-shot provision/migrate, DB không host port | PASS config; NOT_RUN DB runtime |
| AC-05 origin runtime | pure test cùng digest đổi origin; không business code đổi | PASS config; NOT_RUN same-image runtime |
| AC-06 GHCR/release/rollback | manual-only workflow, source metadata, remote digest recording, runbook | PASS source; NOT_RUN publish/registry digest |
| AC-07 DNS | pure snapshot plan/drift/idempotency/scoped rollback | PASS pure; NOT_RUN provider apply/readback |
| AC-08 test online | cần endpoint/HTTPS/SSR/assets/health/flows thật | NOT_RUN |

Các tests DNS dùng địa chỉ/hostname tổng hợp, không phải đích test được chọn. Mẫu inventory có
empty fields chủ động, không có digest giả. Không cung cấp release digest khi chưa publish.

## Mức bàn giao và bước tiếp

**Code-ready:** foundation + focused checks như trên. **Image-built:** chưa. **Image-published:**
chưa. **DNS-applied:** chưa. **Test-online:** chưa. Không deploy hoặc đổi production/NS/apex/www.

Khi có target/runtime/disk và quyền phù hợp: chạy exact commands của [runbook](../../infra/test/README.md),
release build3 images rồi Linux smoke với PostgreSQL scratch được cấp quyền, peer đúng/sai UID,
Identity restart và SIGTERM. Chỉ sau đó publish explicit, lấy remote digest thật, kiểm migration
compatibility/rollback, xác minh Spaceship auth+NS+endpoint rồi DNS diff/apply/readback và HTTPS.
Không bỏ peer/auth/profile guard, dùng local outbox làm email thật, hoặc coi noindex là access control.
