# Pen Flight R2 — bằng chứng local #73

10/11/2026 · macOS arm64 · base `1329642` sau #139 · branch `codex/pen-flight-r2`.
[Spec/nguồn và owner](../design/pen-flight-r2.md). Lát riêng CHOICE-07 A; không hoàn tất epic.

## Kết quả

| Kiểm | Kết quả / phạm vi |
| --- | --- |
| `cargo xtask verify quick` | PASS 9/9 nhóm; generator từ chối path/hash hoặc provenance sai; clippy SSR/hydrate/preview, unit/contracts, repository checks |
| `cargo xtask verify web` | PASS; PostgreSQL 17.11 scratch thật + LocalFixture loopback; SSR/hydration, counts riêng work/root/reply, double tap, pending, lost ACK/exact replay, 409, keyboard, 401 và restart |
| `cargo xtask ui-preview leptos verify` | PASS 784 contexts / 1.151 PNG trước delta CSS specimen cuối; glyph trang trí 20px + target48 ở 7 scenarios, reflow 320/390/834/1024/1440, chữ100/200, sáng/tối |
| Capture tập trung sau CSS specimen | PASS pending sáng390/100, uncertain tối320/200, selected tối1440/100, khách sáng390/100; DOM Range kiểm từng từ nhãn không bị ngắt; không overflow/console errors |
| `cargo xtask ui-preview cmp verify` | PASS 94 tests, 656 PNG common/desktop; không bật native targets |

Full Leptos gallery không chạy lại sau thay đổi CSS **chỉ của specimen**; bốn capture tập trung
ở trên kiểm delta đó. Source glyph, registry, pattern và adapter không đổi sau quick/Web/full gallery.
Compiler profiles: `CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0
CARGO_BUILD_JOBS=2`; Chrome 157.0.8103.40, Node 22.15.0, Rust 1.99.1, JDK Temurin24.

**VISUAL FAIL → FIXED:** lượt ảnh tập trung đầu tại320/chữ200 lộ nested padding/indent làm
nhãn bẻ từng ký tự dù kiểm không overflow đã PASS. Đã giới hạn sửa ở chrome specimen Leptos,
bỏ padding lồng/indent tại vùng hẹp, kiểm lại bằng Range từng từ và ảnh cùng ca; “Đánh giá cao”/
“Hữu ích” nay xuống dòng theo từ. Không đổi CSS hoặc behavior app để che lỗi.

Đã mở ảnh app thật sáng1440/tối390 sau ghi xác nhận (work1, ✓), before/after contract fixture
ở320/chữ200/tối và desktop sáng; mở các tile pending, selected, uncertain320/chữ200 sau sửa,
khách; CMP wide/dark cho thấy glyph ở work/root/reply và ✓ riêng. CMP narrow raw capture bị
cắt trước specimen trong gallery dài, nên không dùng ảnh đó để khẳng định visual narrow/native parity.

SVG Drive R2 đã đối chiếu trực tiếp: **352 byte**, path + viewBox nguyên văn,
SHA-256 `085f379202df6d1d8a011d3ae0abcbdad5e38d203aa77462449da224c02249c8`.
Provenance Vink-owned tách khỏi license Material trong registry dùng chung.

## Lưu bằng chứng và giới hạn

Local bundle `/tmp/vink-73-evidence/`: `web/`, `preview/` (ảnh/context liên quan),
`focused/` (trước sửa specimen), `focused-after/` (ảnh focus + manifest), `cmp/` (ảnh + XML),
`checks.json`. Source hashes trước checks: `/tmp/vink-73-source.json`; tất cả13 implementation
files đó giữ nguyên sau kiểm, CSS preview cuối là delta kiểm riêng. Script capture tái dùng CDP và
helper `captureSpecimen` thật tại `/tmp/vink-73-focused.mjs`; không dùng ảnh tạo sinh.

NOT_RUN: thiết bị thật, IME hệ điều hành, named screen reader, Android/iOS build/runtime,
native product consumer và production auth/write readiness. Quint/Kani/SQL race lanes không chạy
lại vì không đổi policy/core/SQL. Không kiểm/chờ GitHub CI, không deploy, không tick/đóng epic.

Log hashes (file ở `/tmp/vink-73-*.log`):

- `quick`: `da3da694229e14de146780b45195214a5b023c0c7b6601996663ca1b6a239808`.
- `web`: `bde64b3765354dc807f5b9192c1f5bb334eac4e693e8ee33fc900f1baff5cca9`.
- `preview`: `245c445708974fd8cf0a131399b0e8db5bd2b049633d20465658c2403c328824`.
- `focused-after`: `64d812dd4bb6849ecb90177cac637debaa15b2c54978f2e25ffb581fb88b0a17`.
- `cmp`: `f8f1caa94328293ef7efc8f2e5a997c947fca7393563ff5429dae0fbd808b29f`.
