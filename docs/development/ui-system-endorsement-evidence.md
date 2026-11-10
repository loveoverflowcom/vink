# Bằng chứng UI System — ToggleAction và EndorsementAction (LVE #73)

Cập nhật 10/11/2026: CHOICE-07 A đã tích hợp; xem [Pen Flight R2](pen-flight-r2-evidence.md).
Các kết quả/gap dưới đây giữ nguyên theo snapshot của lần kiểm trước.


**Ngày:** 06/11/2026 (UTC+07) · **Issue:** [#73](https://github.com/loveoverflowcom/vink/issues/73)
(WP-04, phần tái sử dụng) · **Nhánh:** `codex/lve-ui-endorsement-system`, nền `develop@23fb5af`.
Kết quả chạy trên máy dev; không phải nghiệm thu thiết kế, thiết bị thật, screen reader thật hay
production. Contract: [components.md](../../packages/ui-system/shared/components.md#toggleaction--hành-động-bậttắt-đã-xác-nhận-lve-73);
quyết định: [literary-endorsement](../architecture/literary-endorsement.md#ui-system--phần-tái-sử-dụng-wp-04-06112026).
Bản ghi này là của lát UI System (#85); adapter app đến sau ở
[bằng chứng Web adapter](literary-endorsement-web-evidence.md).

## Môi trường

| Mục | Giá trị |
| --- | --- |
| Máy | Ubuntu 26.04.1 LTS, Linux 7.0.0-42-generic, x86_64 |
| Rust / wasm-bindgen | 1.99.1 / 0.2.129 |
| Node / Chrome | v24.22.2 / Chrome 156.0.8078.79 headless, DPR 1 |
| JDK / Gradle | OpenJDK 17.0.20.1 / wrapper 8.14.3 (Compose Multiplatform theo `cmp/gradle`) |
| Nguồn | `tokens.json` 0.5.0, catalog 0.6.0, `api-coverage.json` 0.2.0, `fixtures/toggle.json` `toggle-contract-v1` |

## HOW / WHY

- **HOW:** adapter (chưa có) chiếu summary/own-state/reducer lệnh của LVE thành `EndorsementFacts`
  (giá trị đã xác nhận, phase, số đếm của riêng đích, lý do chặn) → `EndorsementAction` chọn copy →
  `ToggleAction` trình bày và phát `on_set(desired)` / `on_recover(Reconcile)`.
- **WHY:** epic yêu cầu phân biệt pending/confirmed, không biến unavailable thành 0, không replay mù
  và không leak trạng thái; đặt các luật đó ở một bảng hữu hạn (36 tổ hợp) dùng chung hai renderer rẻ
  và kiểm hết được, thay vì để mỗi màn tự suy. Không có Quint model: không có race trong view; race/
  order thuộc lõi và adapter.

## Kết quả

| Kiểm | Lệnh | Kết quả | Ghi chú |
| --- | --- | --- | --- |
| Nguồn shared hợp lệ, 15 file sinh ra khớp | `cargo xtask ui-tokens --check` (trong `verify quick`) | PASS | Thêm `toggle_vectors.rs`, `VinkToggleVectors.kt`; 3 cặp contrast mới đạt ≥ 4.5 cả hai theme |
| Generator từ chối vector sai | `cargo test -p xtask ui_toggle` (trong `verify quick`) | PASS | Phản ví dụ: pending vẫn phát lệnh, uncertain không mời Kiểm tra lại, phát lại đúng trạng thái đang hiện, thiếu tổ hợp, unavailable thành `0`, số không khớp |
| Lane quick | `cargo xtask verify quick` | PASS | fmt, clippy server/hydrate/preview `csr` (wasm32), test (`vink-ui` 50, `vink-ui-preview` 9, `vink-patterns` 4, `xtask` 40), repository checks (API admission map, style guard, link). Lần đầu FAIL ở style guard (`font-variant-numeric` không có token) → gỡ declaration, chạy lại PASS |
| Vectors chung | Rust `toggle::tests`, Kotlin `VinkToggleTest` (JVM) | PASS | 36 tổ hợp value × phase × availability và 9 ca số đếm giống nhau ở Rust và JVM |
| Preview Leptos | `cargo xtask ui-preview leptos verify` | PASS | 578 kiểm tra `ok`, 774 ảnh ở `target/ui-preview/leptos/captures/`. Hàm mới `toggles`: click đúp chỉ hỏi một lần, không optimistic, mất phản hồi chặn bấm và mời Kiểm tra lại, đọc lại hiện đúng owner, Enter/Space, focus 2px, số không khả dụng là dấu “–”, lý do chặn, bốn đích độc lập, khách không có đăng nhập giả. Lần đầu FAIL: tràn 3px ở 320px/200% do summary catalog bắt đầu bằng `EndorsementAction` → đổi copy, chạy lại PASS. Đã mở ảnh `composition.endorsement` dark/1024 |
| Preview CMP desktop | `cargo xtask ui-preview cmp verify` | PASS | `VinkToggleTest` 3, `EndorsementTest` 1, `PreviewTest` 10 (catalog = REGISTERED), `CaptureTest` 1 (gồm 8 ảnh entry mới ở `packages/ui-system/cmp/preview/build/captures/`); đã mở ảnh `composition.endorsement` dark/wide |
| Kiểm tay trên preview | Built-in browser, 390px sáng/tối, `uncertain`, 320px/200% | Đã xem | Dời ✓/MicroArc sang slot cuối sau khi thấy slot đầu rỗng làm nhãn thụt 40px |
| Lane web | — | NOT_RUN | Không màn app nào đổi; `/pkg/vink.css` chỉ thêm biến/selector mới. Cần chạy khi adapter app dùng component |
| Lane postgres / discussion / kani | — | NOT_RUN | Không đổi lõi, SQL hay HTTP |
| Native Android/iOS | — | NOT_RUN | Không bật `VINK_UI_NATIVE_TARGETS`; chỉ commonMain + JVM desktop |

## Không chứng minh

- Không màn app nào dùng component; không có adapter LVE ở `vink-web`; production vẫn `401` (D-05).
- Glyph CHOICE-07 chưa chọn; CHOICE-10 sản phẩm chưa chốt (primitive chỉ trình bày hướng A).
- Screen reader thật (NVDA/VoiceOver/TalkBack), IME, Android/iOS runtime và thiết bị thật chưa chạy;
  ngữ nghĩa CMP chỉ kiểm qua compile + preview desktop. Contrast là số học token + màu resolve.
