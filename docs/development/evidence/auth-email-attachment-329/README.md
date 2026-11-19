# Gắn email xác minh lần đầu · #329

- Base: `4cac8373dddd4169d4aca7bbe86ad74d46941438` (sau #330 và #332). Source Chrome đã chạy:
  [237b5ed](https://github.com/loveoverflowcom/vink/commit/237b5ed644ef34c65c859d226fe0d626c833dcfb), tree `505090f17c44c9f3a73031e147b1cf669df30bc6`.
- Mac arm64, PostgreSQL 17.11 scratch trên loopback, App và Identity binary thật, Identity RPC
  qua Unix socket, outbox cục bộ; Chrome 157.0.8103.79 headless/CDP. Không kiểm môi trường thật.
- [Registry runtime](../../auth-web.md#gắn-email-xác-minh-lần-đầu-329) ghi lựa chọn mã, scope,
  quyền, dữ liệu và cách rollback. `decision validate` / `refs` đạt; snapshot
  `43865e626ee7ce88e8f453140666e10d3b5f3ecf5fe6c52d7a44b6887300fb22`.

| Lệnh | Kết quả |
| --- | --- |
| `cargo test --locked -p vink-identity` dưới `tools/local-budget` | 41 unit + 2 CLI + 52 PostgreSQL PASS, có 4 test attach đi qua router Identity và mail worker thật. Log SHA-256: `662982f7ee4c7c466ea3bd9caf04586ecae0c948ddb6ef140e28acda058b74d2`. |
| `cargo test --locked -p vink-app --lib email_attachment` dưới `tools/local-budget` | 2 PASS. Fixture RPC kiểm hợp đồng, Origin, phiên/standing, cookie, sai context, không cấp cookie phiên mới và rate limit. |
| `cargo test --locked -p vink-web --features ssr only_matching_durable_binding_confirms_attempt` dưới `tools/local-budget` | 1 PASS, projection binding: đúng địa chỉ/phiên, mismatch, missing hoặc unavailable. |
| `cargo xtask verify web --scenario password` | PASS, gồm #329 với cookie, mã email và DB thật. Log SHA-256: `2544be24fbe67a373dc3c11464386fc79b2416349e5de03d7f8f1037bcb0c456`. |
| `cargo fmt --all --check`, `git diff --check`, `node --check apps/vink-web/e2e/auth-password-smoke.mjs` | PASS. |

## Bằng chứng theo acceptance

- **AC-01 (gắn lần đầu):** Web bắt xác nhận mật khẩu hiện tại. Mã đúng ghi binding thực vào cùng
  account, không tạo account, session hoặc password version mới. Sau đó sign-in bằng email trả
  cùng subject (Identity integration); Chrome kiểm cùng principal, cookie phiên, bút danh, nháp đã
  lưu và không hiện nút thêm lần hai. PostgreSQL kiểm cạnh tranh hai account cùng địa chỉ; người
  thua không có binding và thư không được enqueue khi email đã có chủ.
- **AC-03 (phần attach):** PostgreSQL kiểm thiếu email, sai mã, sai context/purpose, gửi lại đóng
  mã cũ, session khác/revoked/disabled, password version cũ, hết hạn và rollback insert/consume
  khi proof hết hạn trong lúc chờ unique index. Rollback migration giữ email đã xác minh. Chrome
  giữ rồi bỏ ACK **204 thật**; không hiện xác minh lạc quan, đúng một POST và một GET binding,
  không replay. Focus kiểm lúc mở, Hủy qua bàn phím và đối soát; form không tràn ngang ở 320px
  với chữ gốc 200% (không phải browser zoom).
- **AC-02 và phần reset của AC-03: NOT_IMPLEMENTED.** O-11@r2 còn PENDING (email khi đăng ký
  và phiên sau reset), O-11@r1 OPEN. Không bật production, đổi email đã gắn, đổi login/đăng ký,
  deploy hoặc kiểm GitHub CI.

Lượt rà soát cuối cần chạy `cargo xtask verify quick`, `cargo xtask verify postgres` đầy đủ
và Web đầy đủ, tối thiểu gồm cả `auth` và `password` trên develop đã tích hợp. Không suy từ
ca Chrome này ra Safari/Firefox, IME/password manager/screen reader thật, HTTPS/SMTP production.
