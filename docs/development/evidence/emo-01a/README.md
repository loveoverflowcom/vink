# Bằng chứng cục bộ EMO-01A

**Nền:** `27927c462244f2a70e36a686a37c8c95690775b6`.
**Source runtime đã kiểm:** `036136550e4dd8873288c5dbe553d607ca2467c0`.
Commit bằng chứng sau đó chỉ bổ sung artifact/tài liệu và guard thư mục output của build;
JS renderer, policy, manifest, model và Kotlin contract giữ cùng byte với source đã kiểm.
Ngày chạy: 03–04/11/2026, UTC+07. Đây là pilot độc lập của #25, không phải nghiệm thu toàn issue.

| Lệnh thực tế | Kết quả / boundary |
| --- | --- |
| `cargo xtask verify quick` | PASS: fmt, clippy SSR/hydrate/preview, doctests, unit/contracts, manifest/hash/drift, repo links/boundaries |
| `cargo xtask verify web` | PASS: PostgreSQL scratch + server thật, SSR/no-JS/hydration, lazy network và playback thật, budget, delayed load/unmount, delayed host fact, reduced motion/offscreen/tab hidden, lỗi module/WASM/JSON/poster, CSP test; editor/save/conflict/leave/export/restart cũ tiếp tục PASS |
| `cargo xtask verify expressions` | PASS: 3 manifest negatives/drift tests, Quint 0.33.1 typecheck/simulation seed 25 × 1000 samples × 40 steps; 33-step trace, reachable phases/outcomes, missing-guard negative control; 8 production Rust tests/replays |
| `cargo kani --manifest-path packages/expressions/kani/Cargo.toml` | PASS: 3 harness, 0 failures, 2/2 covers; bounds/unwind ở [scope](../../expressions.md#registry-emo--một-model-lifecycle) |
| `./gradlew :ui:desktopTest --tests vink.ui.NotoExpressionTest` tại `packages/ui-system/cmp` | PASS JVM conformance; lần đầu chạy task thật, lần xác nhận sau UP-TO-DATE; không là native runtime evidence |
| `node apps/vink-web/expressions/build.mjs` | PASS: runtime npm/hash/version, build tự host, không CDN/worker; có kiểm riêng từ chối output ngoài target và giữ sentinel trong thư mục tạm |
| `node --test packages/expressions/validate.test.mjs` | PASS, cũng được gọi từ `quick`/`expressions` |
| Parse `.github/workflows/ci.yml`, `git diff --check` | PASS; giữ byte SVG upstream qua attribute scope hẹp và kiểm SHA-256 |

Môi trường: Linux x86_64 `7.0.0-42-generic`, Rust `1.99.1`, Node `24.22.2`, Chrome headless
`156.0.8078.79`, PostgreSQL `17.11` trong container thử riêng trên loopback; JDK Temurin
`17.0.20.1`. Kani `0.67.0` dùng compiler bundled `nightly-2025-11-21`. Không dùng CI làm gate,
không đợi CI và không deploy.

Ảnh được chụp từ app thật, mở kiểm trực tiếp; context, SHA-256 và thời gian ở
[captures.json](captures.json). Matrix tự động: 320/390/834/1024/1440 × sáng/tối, chữ 200% ở
320; System đổi theo OS, keyboard/touch qua CDP. Contrast text/control ổn định: sáng
11.14/5.79, tối 7.87/8.93 (đều ≥4.5). Frame được quan sát thật trong browser; screenshot
không chứng minh riêng playback hay accessibility.

- [Desktop sáng](expressions-1440-light.png)
- [System/tối](expressions-system-dark-1440.png)
- [Mobile 390 tối](expressions-390-dark.png)
- [320 tối, chữ 200%, vùng catalog](expressions-320-dark-200-catalog.png)

[Byte build thô](runtime-build.json): engine 58,149 B, WASM 1,226,030 B, controller và subset
riêng. Đây **không phải** cold/warm transfer benchmark, compressed size hoặc bằng chứng backend
nhanh nhất; main app Rust/catalog/glue/CSS vẫn có overhead.

Tự review toàn diff: P0/P1 chưa xử lý = 0. Hai P2 đã sửa và kiểm lại: facts lặp làm mất thông báo
budget; notification đến muộn cho phép callback tải dùng facts cũ. Layout giữ một owner gutter,
ảnh chờ theme ổn định; regression cho cả hai đã qua. Source bytes/licenses và no-secret/no-CDN
boundary được kiểm độc lập với hình chụp.

**NOT_RUN / acceptance giữ mở:** so sánh Software/Worker/WebGL2 và benchmark thiết bị mobile;
heap/GC/worker memory; exhaustive Quint/model checking; Android/iOS runtime, OS IME,
TalkBack/VoiceOver/screen reader và đa browser/device. #45 sở hữu picker/composer/focus/IME sản phẩm;
#43 sở hữu durable sticker/reaction/auth/persistence. Không chạy lại `discussion`/`postgres`
vì không đổi model/contract/SQL thảo luận; registry chỉ thêm liên kết owner EMO. Web lane vẫn
kiểm PostgreSQL thật cho luồng editor hiện có. #25 giữ mở.
