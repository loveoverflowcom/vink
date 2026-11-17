# #262 — phần selector độc lập

Bắt đầu từ `develop@e4066df9d7cad602a1822414abb418219c612e18`, macOS arm64,
17/11/2026. Checkout riêng `codex/editor-picker`; không sửa/stash/reset checkout khác.
Các run dưới đây do agent thực chạy, self-review. UI code build bằng Rust 1.99.1, Node 22.15.0,
Leptos CSR gallery, dữ liệu mẫu, không App/Identity/PG. LayoutSpec:
[geometry r1](../../../design/editor-format-picker-262.md).

Scope đã giao: thêm `SelectVariant::Inset16`, gallery fixture và phép kiểm component.
Mặc định shared #135 giữ nguyên. Không có format/canonical-source/title authority,
conversion state machine hoặc storage writer mới. #255/#256/#263/#264 chưa cung cấp
end-to-end capability tại base. Tất cả checklist issue vẫn unchecked; issue phải mở.

## Kiểm thực chạy

Cwd của mọi lệnh repo: checkout `vink` của task-6. Build qua resource budget,
`CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0`.
Browser lane không thuộc launcher (UNSUPPORTED qua budget); chạy tuần tự với build,
static server riêng `127.0.0.1:3026`, Chrome cài trên máy. Root font-size 100/200%,
DPR 1; không phải browser zoom hoặc mobile device thật.

| Check | Kết quả / boundary |
| --- | --- |
| `./tools/local-budget run -- cargo xtask ui-preview leptos build` | PASS, 15 generated token files match; CSR bundle + wasm-bindgen. |
| `node --check packages/ui-system/leptos/preview/e2e/{preview-smoke,select-geometry}.mjs` (từng file) | PASS, cú pháp JS. |
| `node --test packages/ui-system/leptos/preview/e2e/focus-capture.test.mjs packages/ui-system/leptos/preview/e2e/overlay-capture.test.mjs` | PASS, 7/7 helper tests; không browser/App. |
| Scoped preview `--selects-only 1` | PASS, 52 captures: 40 format geometry cases + 12 Standard regression; no console errors. |
| `./tools/local-budget run -- cargo xtask verify quick` | PASS toàn bộ: tokens/assets/oracles, fmt, Clippy SSR/hydrate/CSR, doctests, isolated Reader SSR, workspace unit/contract và repository checks; 305.53s qua local-budget. |
| PostgreSQL / Web App main + editor integration | NOT_RUN: chưa có source/provider/title/Reader contracts để tích hợp; không đổi durable boundary trong lát này. |
| OS popup, native touch, screen reader, OS IME, browser zoom, benchmark | NOT_RUN. CUA native Chrome timeout; không suy từ CDP hoặc ảnh. |

Các lượt phát triển FAIL được giữ đúng nghĩa: build đầu lỗi cú pháp closure, build thứ hai
lỗi lifetime trong children closure, đã sửa trước build PASS. Headless menu probes FAIL ở
Space / popup `:open` / CDP timeout; probe hữu hạn cho thấy pseudo-state transient. Những
probe đó không phải oracle menu OS đáng tin và đã bỏ khỏi smoke. Final suite chỉ claim
hit testing, native type-ahead/Tab/focus và geometry; không đổi component để giả menu PASS.

## Render và review

[contexts.json](contexts.json) ghi Chrome/157.0.8103.78, source SHA base + dirty content,
52 capture records và fingerprint `4369c31061a6043ef3cd2fef84af6e90c3214d956d430c98db6129dd3d34c395`.
[build-assets.json](build-assets.json) ghim bundle/CSS phục vụ. Sau capture chỉ có rustfmt
ngắt dòng `format_error`; [post-capture-delta.json](post-capture-delta.json) xác minh hash
của dòng trước/sau và mọi source file khác vẫn khớp. Không có thay đổi runtime sau browser PASS.

Đã mở đúng bốn ảnh final sau (các ảnh khác captured, chưa review thị giác):

- [Light, 320px, chữ200, LTR](format-select-light-320-200-ltr.png).
- [Dark, 390px, chữ200, LTR](format-select-dark-390-200-ltr.png).
- [Dark, 390px, chữ200, RTL](format-select-dark-390-200-rtl.png).
- [Light, 1440px, chữ100, LTR](format-select-light-1440-100-ltr.png).

| Chiều | Kết quả / giới hạn |
| --- | --- |
| Functional | PASS callback nhận raw `markdown`, native type-ahead và invalid/disabled ở gallery; App format/conversion NOT_RUN. |
| Geometry | PASS 40 format cases: icon 16px, inset 16px, reserve 52px, gap 20px, tâm dọc; target ≥48px, không horizontal overflow. Min content width 183.40625px; Markdown chữ200 rộng 146.6919708251953px. Negative 12px inset bị oracle từ chối. Standard 12 cases vẫn 18/12/38px. |
| Visual | PASS self-review bốn ảnh trên theo regional LayoutSpec: label/diacritics, focus và chevron không chồng, sample help đọc được. Fidelity NO_APPROVED_REFERENCE; không claim đã xem cả 52 ảnh. |
| Accessibility/Responsive | PASS gallery 5 width × 100/200%, LTR/RTL, native AX combobox có tên/required, contrast text ≥4.5:1 ở hai theme, invalid/disabled/forced-colors. Screen reader, browser zoom và device NOT_RUN. |
| Interaction | PASS native type-ahead, focus/Tab và icon hit-test click-through. Popup OS và native touch/IME NOT_RUN; CUA getApp Chrome timeout. |

Lệnh scoped browser thực chạy sau khi static gallery đã build:

```sh
python3 -m http.server 3026 --bind 127.0.0.1 --directory target/ui-preview/leptos
node packages/ui-system/leptos/preview/e2e/preview-smoke.mjs --base http://127.0.0.1:3026/ --chrome '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome' --out ../262-select-captures-verified --catalog packages/ui-system/shared/preview-catalog.json --selects-only 1
```

Log được giữ: [build PASS](build-pass.log), [browser PASS](browser-pass.log),
[helper 7/7](capture-unit-pass.log), [quick PASS](quick-pass.log); lịch sử FAIL:
[syntax](build-fail-syntax.log), [lifetime](build-fail-lifetime.log),
[Space probe](browser-fail-space.log), [menu/CDP timeout](browser-fail-menu-timeout.log),
[quick fmt](quick-fail-fmt.log). Fmt đã được áp trước quick cuối.

Self-review cuối: không có finding cần sửa còn mở trong phạm vi component. Shared default
và mọi App/domain/PG source không đổi; prop mới chỉ chọn CSS anatomy. Ranh giới NOT_RUN
ở dưới là qualification/prerequisite còn thiếu, không được suy thành completion.

## Acceptance và điều kiện tiếp tục

| AC | Evidence hiện có | Còn thiếu / trạng thái |
| --- | --- | --- |
| AC-01 | Source inventory chỉ có document v1, không sniff dữ liệu. | NOT_RUN create/open Mặc định/Markdown; cần #255 và tích hợp App. |
| AC-02 | Không thêm Markdown mode/title UI giả. | NOT_RUN; cần #256/#263, capability thật của #255/#264. |
| AC-03 | Component opt-in có oracle 16px icon/16px inset/52px reserve, LTR/RTL và ma trận 5 width × 2 scale × 2 theme. | Partial; chưa selector App, menu OS, hàng Back/Lưu và ma trận App. Không tick cả AC. |
| AC-04 | Không có mutation/converter, không làm mất source. | NOT_RUN preview/Apply/Cancel; cần converter fidelity và owner seam. |
| AC-05 | Save/IME/CAS/activation state không đổi. | NOT_RUN conversion guards/lost ACK/idempotency trên server. |
| AC-06 | LayoutSpec + feature map + component contract cập nhật; kiểm browser ở gallery. | Partial; Web integration/theme/keyboard App, OS input và benchmark còn thiếu. |

Không có state transition mới nên không tạo/sửa Quint. Proposal schema/hash vẫn PENDING;
product direction confirmed không thay cho signoff architecture hoặc runtime acceptance.
