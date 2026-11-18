# Reader Async qua HTTP trực tiếp và Caddy

Base `9fcd75ec03229a27464123eac1d68104e8447b19`. So A → B → A trên cùng route Leptos, view thật và fixtures Published/Hidden/ReviewWithdrawn/Missing/Error trong test binary release `8ed89e25f448ed6ff6967d579d0ef865c5c20fcd`. Test thay kết quả và độ trễ của public-read boundary. Caddy 2.11.7 dùng `flush_interval -1`. Mỗi ô một lần trên loopback, phù hợp xác nhận contract và thứ tự byte. [84 quan sát direct/proxy](chunks.json.gz).

| AC | Kết quả |
| --- | --- |
| 01 | PASS. A giữ critical authority, canonical/meta và HTML dùng được khi tắt JS. C sẽ cần xây fallback riêng cho discussion. |
| 02 | PASS. A và A sau rollback đạt 56/56 tổ hợp status/meta/privacy/fallback. Với B và critical chậm 30 ms, header đã commit 200 cho Missing/Error ở cả hai đường. |
| 03 | PASS. Tắt JavaScript thật, guest vẫn đọc toàn bài, đi theo link và gửi form GET. [Release đã merge](../web-perf-234-236/README.md) kiểm riêng bootstrap bị chặn, WASM-only 200/1000 ms, 503 và click trước hydrate. |
| 04 | PASS. [Guest history](guest-history.json.gz) tại `9cb767c784c444f7640f065ccd58f336f13f68b5` với App/PostgreSQL thật. Direct fresh không GET Reader lặp, history document verify đúng một GET. #234 giữ response Reader/discussion để quan sát article, retry, ABA và control answer sau dispose. |
| 05 | PASS cho quyết định giữ A. Cả 28 ca sau khi trở lại Async đúng contract. B đạt 8/28 theo contract HTML tĩnh. Ở A, secondary chậm 200/1000 ms thật sự chặn article SSR khoảng tương ứng. Với B, trả shell sớm nhưng ca critical chậm để article trong template cần JS, hoặc mất fallback retry. Dừng B. C chưa chạy. |

Browser Chrome 157.0.8103.78/Linux x86_64, PostgreSQL 17.11, Rust 1.99.1, bundle guest release có probe từ #234. Lane authority dùng bundle thông thường của develop. Guest dùng corpus 200 work/1.000 comments, SQL từ #100/#234; bài ngắn canonical. Các kết quả proxy chỉ áp dụng cho cấu hình Caddy đã đo. Browser không có JS và matrix synthetic có ranh giới riêng với kiểm tra App/DB.

Từ gốc repo, với Caddy trong PATH và App guest trên scratch đã seed

```sh
cargo test --release --locked -p vink-app --test ssr_boundary -- --exact reader_ssr_boundary
BIN=$(find target/release/deps -type f -executable -name 'ssr_boundary-*' | head -1)
python3 docs/development/evidence/web-perf-235-canonical/chunks.py "$BIN" "$(command -v caddy)" > chunks.json
node apps/vink-web/e2e/browser-smoke.mjs --base "$SCRATCH_ORIGIN" --chrome "$VINK_CHROME" --state "$ARTIFACTS/state.json" --phase reader-guest-history
cargo xtask verify web --scenario main --scenario discovery --scenario review --release
```

Đã chạy lại browser release của develop tại `9fcd75e`. [Bootstrap, WASM-only và click](reader-ssr-boundary.json.gz) và [lifecycle/withdrawal/ABA](public-read-lifecycle.json.gz) đều PASS.

Lane release hoàn tất cả `main`, `discovery` và `review` trên scratch App/PostgreSQL của develop `9fcd75e`. Ca O-17@r2 chạy pause, admin approve, intervention reject, SSR noindex/no-store, cache purge và comments. [Kết quả từng biên](real-authority.log).

[Digest SQL seed, source canonical và cấu hình artifact](fixtures.json).
