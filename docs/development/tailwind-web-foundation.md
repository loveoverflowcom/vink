# Nền tảng Tailwind CSS v4 cho Web và Leptos preview (#417 / #418)

## 1. Bốn chiều trạng thái

- **Quyết định sản phẩm:** Định hướng migrate CSS Web sang Tailwind v4 và nâng cấp skills Leptos
  theo yêu cầu chủ sản phẩm tại [Epic #416](https://github.com/loveoverflowcom/vink/issues/416);
  giữ nguyên thiết kế M3 Expressive / Lavender, token SSOT `tokens.json` (`0.8.0`, gồm R4.2 #415),
  Leptos SSR/hydration, quyền/dữ liệu và CMP. Issue `#413` (đổi tùy chọn cỡ chữ sang Mặc định 100% /
  Lớn 150%, bỏ Giao diện khỏi rail) **chưa được tích hợp** tại baseline `126095a` và đối chứng `develop@da1bc260`
  (`apps/vink-web/src/appearance.rs` hiện vẫn giữ `100%` / `200%` với `data-text="200"`); lát này
  không tự nhận phần sản phẩm của `#413`.
- **Quyết định kiến trúc:** Proposal
  [`VDR-WEB-TAILWIND-FOUNDATION@r1`](../../decisions/proposals/VDR-WEB-TAILWIND-FOUNDATION.r1.toml)
  (SHA-256 `67f752768e9607fdb18f77aab7062089b350c78b94e778e594115fdea4e1cf71`) đang ở trạng thái
  **`PENDING`** — chưa có signoff đúng revision của chủ sản phẩm theo `VDR-DECISION-SIGNOFF@r1`.
- **Mức triển khai:** `web` (pipeline build `xtask`, token generator `tailwind-theme.css`, guard
  `ui_qualification.rs`, pilot 4 consumer trên Web và Leptos preview, tài liệu & skills W02).
  Chưa migrate toàn bộ control/page (dành cho W03–W06); không đổi CMP, backend, hay deploy production.
- **Bằng chứng kiểm chứng:** Ghi riêng từng lệnh, môi trường, SHA và ranh giới tại mục 8.

## 2. Quyết định kỹ thuật đã chọn (CHOICE-01..04)

| ID | Lựa chọn | Lý do & đánh đổi | Bằng chứng cục bộ |
| --- | --- | --- | --- |
| `CHOICE-01` | **B — Pin `tailwindcss` + `@tailwindcss/cli` `4.3.5` trong package CSS riêng `packages/ui-system/leptos/tailwind`** | Tận dụng Node 24/npm đã bắt buộc ở `apps/vink-web/editor`, `packages/expressions` và `infra/Containerfile`; cô lập khỏi editor; không dùng `npx latest`; một adapter `xtask/src/tailwind.rs` phục vụ cả Web, preview và OCI build. Lockfile giữ đầy đủ native binaries đa nền tảng (`@tailwindcss/oxide-*`, `lightningcss-*`). Đánh đổi: thêm 1 `package.json`/`package-lock.json` nhỏ (28 packages, `npm ci --ignore-scripts` ~0,8 s) và `build preview` cần Node/npm. | `tailwind::tests::repeated_compile_is_deterministic_and_failure_never_keeps_stale_output`, `tailwind::check`, `cargo xtask build web --release`, `cargo xtask build preview`. |
| `CHOICE-02` | **A — Không bật Tailwind Preflight; định nghĩa `@layer theme, base, legacy, components, utilities;` + `source(none)` và `@source` allowlist** | Giữ nguyên reset hiện hành của Vink, prose Markdown, CodeMirror 6 và native `<select>`/`<input>`/`<dialog>`. Utility trong `@layer utilities` thắng `@layer legacy` mà không cần `!important`, còn `!important` không nằm trong layer (reduced motion, forced colors) giữ ưu tiên cao nhất. Đánh đổi: khi migrate component con trước cha (như `.vk-editor-recovery > .vk-ui-notice` hay `.vk-author-work__status`), phải chuyển override ở cha sang utility cùng lát. | `tailwind::tests::source_none_extracts_leptos_syntaxes_rebuilds_new_utilities_and_ignores_excluded_dirs`, computed-style diff 10 ca preview (0 diff màu/bố cục/hình học). |
| `CHOICE-03` | **A — Sinh `packages/ui-system/leptos/style/tailwind-theme.css` từ `cargo xtask ui-tokens` (`@theme inline`)** | `tokens.json` (`0.8.0`) đã có gần 100 role/recipe (gồm R4.2 `#415`); tự động sinh bằng `@theme inline` tham chiếu trực tiếp `var(--vk-*)` loại bỏ drift thủ công mà không sao chép giá trị bảng màu ra `:root`, giữ nguyên `ThemeScope`, `RoleOverride`, density và reduced motion. 15 file sinh cũ (`tokens.css`, `tokens.rs`, `VinkTokens.kt`…) giữ nguyên byte-for-byte. | `cargo xtask ui-tokens --check`, `ui_tokens::tests::tailwind_theme_references_only_generated_vk_vars_without_literal_palette_copies`, `vink_ui::values::tests::tailwind_theme_maps_every_color_role_via_css_var`. |
| `CHOICE-04` | **B — Xóa namespace mặc định trong `tailwind-theme.css` + mở rộng `xtask/src/ui_qualification.rs` quét nguồn Rust UI và 6 file CSS** | Xóa `--color-*: initial`, `--font-*: initial`, `--text-*: initial`, `--font-weight-*: initial`, `--shadow-*: initial`, `--ease-*: initial` ngăn sinh CSS cho bảng màu/chữ mặc định, nhưng chỉ xóa theme sẽ khiến `bg-red-500` âm thầm mất CSS mà không lỗi build. Guard `ui_qualification.rs` (chạy trong `verify quick`) báo lỗi rõ ngay khi nguồn Rust UI hoặc 6 file CSS (4 stylesheet giữ lại và 2 entry) dùng màu/chữ/bóng/motion literal hay arbitrary bypass, đồng thời cho phép `*-vk-*`, `var(--vk-*)`, geometry structural (`w-[320px]`), `forced-colors:*-[CanvasText]` và `style_exceptions` có kiểm stale. | `ui_qualification::tests::rejects_tailwind_default_palette_type_shadow_and_motion_but_allows_vink_tokens_and_layout`, `rust_utility_guard_flags_literals_in_strings_and_class_directives_while_skipping_tests`. |

## 3. Baseline trước thay đổi (`develop@126095a`) và so sánh sau W01

### Môi trường và tham số đo

- **Base chưa sửa:** `develop@126095a36ae8a7d302e819a8c47c4538f39953d4` (`feat(web): áp R4.2 cho rail, Khám phá và sheet Lọc thẻ (#415)`).
- **Máy & toolchain:** Linux x86_64, `rustc 1.99.1 (58cf8f9c3 2026-10-13)`, `leptos 0.8.22`,
  `wasm-bindgen 0.2.129`, `Node v24.22.3`, `npm 11.19.1`, `tailwindcss` + `@tailwindcss/cli` `4.3.5`,
  `Google Chrome 158.0.8127.36`.
- **Tham số nén (`node:zlib`, cũng được đưa vào `apps/vink-web/e2e/web-perf-baseline.mjs`):**
  `gzip` level `6`, `brotli` quality `6` (`br6`), `brotli` quality `11` (`br11`).

### Kích thước file nguồn CSS (source size — không đồng nhất với artifact phục vụ)

| File nguồn | Trước (`126095a`) raw / br11 | Sau W01 raw / br11 | Ghi chú |
| --- | --- | --- | --- |
| `packages/ui-system/leptos/style/tokens.css` | 18,555 / 2,644 B | 18,555 / 2,644 B | Không đổi byte-for-byte (`08859798bba83715`) |
| `packages/ui-system/leptos/style/tailwind-theme.css` | *(chưa có)* | 12,458 / 1,757 B | Sinh tự động từ `tokens.json`; chỉ tham chiếu `var(--vk-*)` |
| `packages/ui-system/leptos/style/vink-ui.css` | 67,279 / 10,485 B | 64,883 / 10,328 B | Đã xóa rule pilot `InlineNotice`/`Banner`/`StatusBadge`/`ListEnd` |
| `packages/vink-patterns/leptos/patterns.css` | 27,296 / 4,652 B | 27,100 / 4,679 B | Đã xóa rule `.vk-draft-meta`, `.vk-draft-meta > span`, `.vk-author-work__status` |
| `apps/vink-web/style/vink.css` | 124,688 / 19,342 B | 124,321 / 19,360 B | Đã xóa `.vk-reader-replacement-badge`, `.vk-discussion-order`, 2 rule `.vk-editor-recovery` child override |
| `packages/ui-system/leptos/preview/style/preview.css` | 15,481 / 2,746 B | 15,492 / 2,762 B | Đổi 4 font literal sang `var(--vk-type-*)` + xóa `.vkp-offline` |

### Số đo lịch sử trước rebase (`--release` Web / `dev` Preview)

| Artifact | Trước (`126095a`) raw / gzip6 / br6 / br11 | Sau W01 raw / gzip6 / br6 / br11 | Delta br11 |
| --- | --- | --- | --- |
| `target/site/pkg/vink.css` | 237,822 / 42,685 / 38,763 / **33,591 B** | 199,873 / 30,421 / 27,685 / **24,860 B** | **-8,731 B (-26,0%)** (chủ yếu do `--minify` của Tailwind/Lightning CSS ở release) |
| `target/site/pkg/vink_bg.wasm` | 7,982,079 / 2,337,228 / 1,884,052 / **1,633,658 B** | 7,985,457 / 2,338,392 / 1,885,771 / **1,638,019 B** | **+4,361 B (+0,27%)** (chuỗi class utility trong Rust pilot) |
| `target/site/pkg/vink.js` | 97,786 / 14,382 / 13,169 / **11,687 B** | 97,786 / 14,382 / 13,169 / **11,687 B** | 0 B (trùng SHA-256) |
| `target/site/pkg/vink-editor.js` | 370,014 / 126,699 / 119,876 / **107,353 B** | 370,014 / 126,699 / 119,876 / **107,353 B** | 0 B (trùng SHA-256) |
| Preview CSS (`4 file gộp` → `vink-preview.css`, dev) | 128,611 / 24,455 / 22,978 / **20,527 B** | 130,051 / 17,725 / 16,634 / **14,558 B** | **-5,969 B (-29,1%)** (tổng bốn response nén riêng → một response) |

### Thời gian build lặp lịch sử (warm cache, cùng máy Linux)

| Lệnh | Trước (`126095a`) | Sau W01 | Delta |
| --- | --- | --- | --- |
| `cargo xtask build web --release` (warm repeat) | 10,825 ms | 11,202 ms | +377 ms |
| `cargo xtask build preview` (warm repeat) | 9,204 ms | 9,490 ms | +286 ms |
| Riêng Tailwind CLI `web.css --minify` | — | 357 ms | — |
| Riêng Tailwind CLI `preview.css` | — | 288 ms | — |

### SSR HTML và hành trình `perf-baseline` (`--release`)

- Helper đo nén bundle và SSR HTML (`COMPRESSION_PARAMETERS`, `measureCompressedBytes`,
  `measureBundleArtifacts`) đã được bổ sung vào `apps/vink-web/e2e/web-perf-baseline.mjs` và kiểm
  unit test tại `apps/vink-web/e2e/web-perf-baseline.test.mjs` (**PASS 5/5**).
- Lượt chạy `verify web --scenario main --scenario review --scenario perf-baseline --release` đầu
  tiên đã qua toàn bộ SSR boundary, public read lifecycle, contrast/geometry của Desk, Explore,
  Filter sheet và Discussion (`discovery-reader.png`, `discussion-composer-1440-100-light.png`),
  phát hiện `.vk-editor-body .cm-editor.cm-focused` trong `@layer legacy` bị base theme không layer
  của CodeMirror 6 (`style-mod`) ghi đè; rule `.cm-editor` và `.cm-line` đã được chuyển ra phần
  không layer cuối `packages/ui-system/leptos/tailwind/web.css`. Lượt chạy lại full `verify web`
  được dừng theo yêu cầu bàn giao sớm vào Draft PR (`NOT_RUN` trên diff cuối `web.css`; lệnh chạy
  tiếp: `cargo xtask verify web --scenario main --scenario review --scenario perf-baseline --release`).

### Tiêu chí kiểm soát hồi quy trước rollout rộng (W03–W06)

Không đặt đích tùy tiện như “CSS phải 10–30 KB” hay lấy `p95` của 10 mẫu lab làm gate. Trước khi
mở rộng từng lát W03–W06, mỗi PR phải đối chiếu trên cùng máy và profile `--release`:

1. **Tổng chi phí truyền tải (`vink.css` + `vink_bg.wasm` + SSR HTML chính, `br11`):** mọi lát xóa
   CSS cũ phải theo dõi đồng thời cả giảm CSS lẫn tăng chuỗi class trong `vink_bg.wasm` và SSR HTML;
   nếu `vink_bg.wasm` tăng vượt mức giảm `vink.css` cần rà soát lặp chuỗi class dài thay vì gom
   helper/component.
2. **Chi phí build lặp:** bước Tailwind CLI chỉ quét đúng `@source` allowlist (không tăng đột biến
   do quét nhầm `target/` hay `docs/`).
3. **Ổn định render & ngữ nghĩa:** 0 lỗi console/hydration, 0 vi phạm token guard
   `ui_qualification.rs`, giữ đủ E2E hook và ảnh trước-sau trên kịch bản tương ứng.

## 4. Kiểm kê CSS family → consumer → test hooks → chủ sở hữu (W03–W06 / giữ lại)

| File CSS (sau W01) | Họ selector chính | Consumer Rust/JS | Test hooks tiêu biểu (`e2e/`) | Kế hoạch owner / lý do giữ |
| --- | --- | --- | --- | --- |
| `packages/ui-system/leptos/style/tokens.css` (570 dòng, 18.6 KB) | `@font-face`, `:root`, `.vk-scope[data-theme/density/motion]` | Toàn bộ Web & Preview + `values.rs` (`include_str!`) | `preview-smoke` (`getComputedStyle` `--vk-*`), `browser-smoke` | **Giữ lại (sinh tự động):** SSOT runtime cho `--vk-*` và font faces. |
| `packages/ui-system/leptos/style/tailwind-theme.css` (233 dòng, 12.5 KB) | `@theme inline`, `@utility duration-vk-*` | `tailwind/{web,preview}.css`, `values.rs` test | `ui-tokens --check`, `tailwind::tests` | **Giữ lại (sinh tự động):** adapter không sao chép giá trị bảng màu. |
| `packages/ui-system/leptos/style/vink-ui.css` (2,026 dòng, 64.9 KB) | `.vk-ui` root/type/tone/layout (`text`/`layout`), controls (`button`/`icon-button`/`link`/`chip`/`toggle`), inputs (`field`/`input`/`select`/`choice`/`search`/`summary`), media (`icon`/`avatar`/`byline`/`frame`), feedback (`skeleton`/`arc`/`edge`/`panel`/`snackbar`/`dialog`/`region`), navigation (`scaffold`/`rail`/`bottom-nav`/`top-bar`/`tooltip`/`skip-link`/`supporting-details`) | `packages/ui-system/leptos/src/*.rs` | `preview-smoke.mjs` (61 class hooks: `.vk-ui-button`, `.vk-ui-rail`, `.vk-ui-dialog`, `.vk-ui-notice`, …), `browser-smoke.mjs` | **W01 pilot xong:** `InlineNotice`/`Banner` (giữ 2 rule nút con chờ W03), `StatusBadge`, `ListEnd`. **W03:** primitives `text`/`layout`/`action`/`field`/`media`/`feedback`/`navigation`; giữ `@keyframes`, `::backdrop`, `:focus-visible`, `@container` đặc thù khi cần. |
| `packages/vink-patterns/leptos/patterns.css` (370 dòng, 27.1 KB) | `.vk-work-card`, `.vk-draft-row`/`resume`, `.vk-author-work`, `.vk-editor-toolbar`, `.vk-endorsement`, `.vk-tag-*`, `.vk-discussion-*`, `.vk-feedback-*` | `packages/vink-patterns/leptos/src/{lib,tags,discussion}.rs` | `browser-smoke.mjs` (`.vk-draft-row__link`, `.vk-draft-meta`, `.vk-tag-chip`, `.vk-discussion-root`), `preview-smoke.mjs` | **W01 pilot xong:** `.vk-draft-meta` (`DraftMetadata`/`DraftRow`/`AuthorWorkRow`). **W04:** compositions còn lại; giữ `@container` phức tạp nếu utility giảm rõ ràng. |
| `apps/vink-web/style/vink.css` (2,874 dòng, 124.3 KB) | Base reset / `:root[data-text/rail]` / `:focus-visible` / reduced motion; vỏ editor; Markdown prose `.vk-markdown`/`.vk-reader__body`; các màn Explore/Discovery/Filter sheet, Reader/Replacement/Discussion, Desk/Editor/WorkInfo/Publication, Me/Auth/Profile/Handle/Relationships/Appearance, Moderation, Expressions | `apps/vink-web/src/**/*.rs`, `apps/vink-web/editor/` | `browser-smoke.mjs` (177 class hooks), `preview-smoke.mjs:667` (đọc trực tiếp `:focus-visible` trong `vink.css`), `publication-author-smoke.mjs`, `visual-evidence.mjs` | **W01 pilot xong:** `.vk-reader-replacement-badge`, `.vk-discussion-order`, 2 rule `.vk-editor-recovery` child override. **W05–W06:** trang Khám phá, Reader/Thảo luận, Bàn viết/Editor/Công khai, Tôi/Hồ sơ/Kiểm duyệt. **Giữ lại:** `:root` bootstrap, `:focus-visible`, reduced-motion fallback (`style_exceptions`), `.vk-markdown`/`.vk-reader__body` prose. |
| `packages/ui-system/leptos/preview/style/preview.css` (683 dòng, 15.5 KB) | `.vkp-*` preview workbench, sidebar, controls, inspector, swatches, motion demos | `packages/ui-system/leptos/preview/src/*.rs` | `preview-smoke.mjs` | **W01 pilot xong:** `.vkp-offline` + chuyển 4 font literal sang `var(--vk-type-*)`. **W03/W06:** migrate chrome preview hoặc giữ gọn phần demo animation `.vkp-mover`/`.vkp-loop`. |

## 5. Hỗ trợ trình duyệt và khoảng trống cần lưu ý

- **Không có chính sách sàn trình duyệt chính thức** trong `prd/` hoặc `docs/architecture/`.
- **Tính năng CSS Vink đã sử dụng sẵn trước W01:**
  - Media query range `@media (760px <= width < 1100px)` (Safari 16.4+, Chrome 104+, Firefox 63+).
  - `color-mix()` (Safari 16.2+, Chrome 111+, Firefox 113+).
  - `:has()` (Safari 15.4+, Chrome 105+, Firefox 121+).
  - `@container`, `@layer`, `100dvh`.
- **Yêu cầu chính thức của Tailwind CSS v4:** **Safari 16.4+, Chrome 111+, Firefox 128+** (`@property`
  và cascade layers).
- **Khoảng trống cần kiểm:** Firefox 121–127 (cũ hơn Firefox 128 theo [Tailwind](https://tailwindcss.com/docs/compatibility#browser-support)). Lát này giữ nguyên chính sách hỗ trợ
  và ghi rõ khoảng trống thiếu chính sách sàn trình duyệt chính thức để chủ sản phẩm chốt nếu cần hỗ
  trợ Firefox < 128.

## 6. LayoutSpec rút gọn cho pilot W01

Mục tiêu: kiểm chứng đầy đủ pipeline Tailwind v4 (`web.css` + `preview.css`), cú pháp Leptos 0.8
(`static`, `class:`, finite `match`, descendant variant), `@layer` coexistence và token guard mà
**không đổi thiết kế hay hành vi**.

```text
Pilot W01 — giữ nguyên anatomy, geometry, màu, chữ và semantics hiện hữu
  1. vink-ui (packages/ui-system/leptos/src/feedback.rs)
     - InlineNotice & Banner (.vk-ui-notice[.vk-ui-notice--banner])
       Row(wrap, gap-y-vk-2, gap-x-vk-3, py-vk-3, px-vk-4 | banner: px-vk-6, rounded-vk-md | banner: rounded-none)
         - Icon (.vk-ui-notice__icon) + Body (.vk-ui-notice__body: Title + Message) + Action + Dismiss
       * Parent coexistence: 7 <section class="vk-editor-recovery"> thêm [&>.vk-ui-notice]:p-0/bg-transparent/text-inherit
         (và 2 vùng trong .vk-editor thêm [&_.vk-ui-notice__icon]:hidden); xóa 2 rule legacy ở vink.css.
     - StatusBadge (.vk-ui-status): inline-flex, reactive class:text-vk-on-surface / variant + symbol tone
     - ListEnd (.vk-ui-list-end): flex quiet caption
  2. vink-patterns (packages/vink-patterns/leptos/src/lib.rs)
     - DraftMetadata / DraftRow / AuthorWorkRow (.vk-draft-meta; thẻ AuthorWorkRow giữ DOM, flex-wrap, nhãn, số liệu và link của #429)
  3. apps/vink-web (src/pages/mod.rs, src/pages/discussion.rs)
     - .vk-reader-replacement-badge (trang thay thế SSR)
     - .vk-discussion-order ("Cũ đến mới" trên Reader SSR/hydrate)
  4. preview (packages/ui-system/leptos/preview/src/lib.rs)
     - .vkp-offline (header preview)
```

## 7. Kiểm sau rebase lên develop

Rebase giữ #426/#429/#430. Đối chứng `da1bc2602de87034ba88a4cb9a7a4a1bd03542c1` có Bàn viết
mới và ưu tiên cỡ chữ 100/200%. Ghi phép thử stress 200% thành ca riêng. So sánh CSS dưới đây dùng
cùng Mac arm64, Node 22.15.0, zlib gzip 6 và Brotli q6/q11. Hai cột đối chứng tái tạo luồng ghép
Web của xtask từ các blob đã pin. Preview cũ đo từng response; preview mới dùng một stylesheet
Tailwind. Số đo này là payload CSS local, trước HTTP overhead.

| CSS | Baseline `126095a` raw / gzip6 / br6 / br11 (B) | Đối chứng `da1bc260` | W01 sau rebase |
| --- | --- | --- | --- |
| Web | 237.822 / 42.685 / 38.763 / 33.591 | 238.186 / 42.730 / 38.853 / 33.633 | 200.293 / 30.569 / 27.800 / 24.949 |
| Preview | 128.611 / 24.455 / 22.978 / 20.527 | 129.618 / 24.630 / 23.158 / 20.667 | 131.475 / 17.942 / 16.838 / 14.742 |

R4 sau #415 thay đổi CSS Web **+42 B br11** trên cùng cách ghép cũ. W01 sau rebase thay đổi CSS
Web **−8.684 B br11** so với đối chứng, có cả hiệu ứng minify. CLI Web chạy hai lần,
`--input web.css --output <file> --minify`, trả cùng SHA-256
`c9c99ee74083fced523c68c038608ee2c28fafd00eef96f71edb6b4f7b137cd5`.
Preview dùng `--input preview.css --output <file>` cho SHA-256
`0380a51bb97028e02763075ff864539fe4f1ccfed0eb2a2ca11f4506a8b692bb`.
Hai lượt Web lần này mất 0,15 và 0,14 s wall, riêng CLI. Build lặp đầy đủ, HTML SSR, WASM và
hành trình release trên cùng corpus/profile vẫn thiếu. Budget tổng truyền tải ở mục 3 còn mở.

| Chiều UX | Kiểm đã có trên source/CSS sau rebase | Browser trên revision này |
| --- | --- | --- |
| Functional | Giữ vai trò và hook, cấu trúc thẻ #429, CSS Web/preview biên dịch | `NOT_RUN` SSR và hydration |
| Geometry | Giữ kế thừa typography; đưa toàn bộ rule DOM CodeMirror về cùng cascade không layer, theo thứ tự cũ | `NOT_RUN` computed geometry ở app và preview |
| Visual | Số liệu CSS và ảnh pilot lịch sử có provenance riêng | `NOT_RUN` cặp ảnh trước/sau trên đối chứng mới |
| Accessibility / Responsive | Oracle tooltip theo R4.2 kiểm ngoài vùng cuộn, tên hiển thị và không che đích đầu | `NOT_RUN` browser, forced colors, root stress 200% |
| Interaction | Giữ lifecycle của EditorView, thêm oracle focus, Escape, hover và icon recovery | `NOT_RUN` tooltip và editor sau hydration |

## 8. Kiểm chứng và bàn giao

- **#417 AC-02/03/04/06:** Harness nhỏ trên Mac dùng trực tiếp module compiler, token và guard
  của xtask với lock offline và một job. Test compiler gọi Tailwind thật; 35 test đạt, gồm
  generated token drift, nguồn Rust/CSS trong worktree thật, ký tự `'đ'`, production sau module
  test, literal màu/thời lượng, root thiếu hoặc sai entry, lệch manifest/lock khi cache nóng/lạnh
  và output stale khi setup hoặc compile lỗi. Đây là lượt kiểm phạm vi module.
- **#417 AC-05:** 19 test unit đóng gói local đạt. CSS Web đầu ra giữ đường tương đối tới font và
  brand. Runtime /pkg/ và kiểm build container thuộc lượt tích hợp.
- **#418 walkthrough:** Lần theo hướng dẫn từ `desk.rs` (hai ca lưu lỗi/xung đột) và preview entry
  đến Tailwind compiler và CSS đã sinh. Kiểm raw string Rust giữ đủ dấu thoát cho selector cha
  `[&_.vk-ui-notice\_\_icon]:hidden` và selector con `.vk-ui-notice__icon`. Fixture Rust
  của `tailwind::tests::source_none_extracts_leptos_syntaxes_rebuilds_new_utilities_and_ignores_excluded_dirs`
  kiểm class tĩnh, `class:`, tuple, rebuild và canary loại trừ. Phần trạng thái đã hydrate trong
  smoke preview/Web còn thiếu bằng chứng. Canonical guide và source map phản ánh owner hiện hành.
- **#417 AC-01/07 và #418 AC-06:** `PARTIAL`. Giữ Draft để review độc lập trên SHA mới.
  Ưu tiên tooltip preview và editor Web (hình học, nội dung, forced-colors, focus, recovery và SSR)
  ở môi trường được phép có trần tổng RAM 12 GB cho cây tiến trình và scratch service.
  Full quick, ma trận Web rộng và bộ đo release cùng corpus chuyển sang hai lượt cuối.
  Proposal `VDR-WEB-TAILWIND-FOUNDATION@r1` vẫn `PENDING`.

## 9. Đối chiếu sau squash #431

`develop@2712145b08a6a4c7a12c1d57033ecd2a5bd33b26` có cùng tree với
`5e8fbd840ad83f662f17c22df416f47a67e78acf`. Bằng chứng tại mục 7–8 giữ provenance lúc
#431 còn là Draft. `VDR-WEB-TAILWIND-FOUNDATION@r1` vẫn `PENDING`.

Walkthrough tự kiểm của S1 dùng ví dụ `Notice` có thật tại `packages/ui-system/leptos/src/feedback.rs`.
Chèn tạm `class=("sm:gap-x-vk-6", banner)` vào opening tag, parse bằng `rstml 0.13.1` mà
`leptos_macro 0.8.19` đang khóa, rồi chạy adapter `xtask/src/tailwind.rs::compile` cho Web
minify và preview dev. CSS ở cả hai entry sinh media query 40rem và
`column-gap: var(--vk-space-6)`. Build lặp cho cùng bytes, hoàn nguyên fixture trả lại SHA-256
Web `c9c99ee74083fced523c68c038608ee2c28fafd00eef96f71edb6b4f7b137cd5` và preview
`0380a51bb97028e02763075ff864539fe4f1ccfed0eb2a2ca11f4506a8b692bb`.

Harness cô lập chạy `cargo test --offline --manifest-path <harness>/Cargo.toml --target-dir
<harness>/target -- --test-threads=1` với một job. Đạt 37 test, gồm token drift và guard trên
tree thật, fixture stale style exception và vòng compile mới/lặp/hoàn nguyên. RSS cao nhất đo
được là 353.533.952 B trên Mac arm64. Với tuple, source `tachys 0.2.19` nối class khi
điều kiện đúng và `rebuild` chỉ thêm/bỏ token. Phần đó là source inspection. Render, hydration
và layout live của ví dụ trên browser là `NOT_RUN`.

| Issue / tiêu chí | Phạm vi đã đối chiếu | Trạng thái |
| --- | --- | --- |
| #417 AC-02/03/06 | Compiler pin, hai entry thật, đủ source roots, compile lặp và lỗi stale output, guard Rust/CSS và stale exception qua `ui_qualification::check` | `PASS` |
| #417 AC-01/04/05/07 | CSS payload, token/layer và asset contract ở mức source/compile/unit; còn phép đo đầy đủ, theme/runtime, đóng gói artifact release và UX pilot | `PARTIAL` |
| #417 AC-08 | Đối chiếu lại docs và registry giữ đúng owner và trạng thái PENDING; cần tích hợp cập nhật docs | `PARTIAL` |
| #418 AC-04 | Mô tả default/markdown, stable EditorView, ranh giới format extension/convert, CSS giữ lại và test hooks khớp với source đã tích hợp | `PASS` ở phạm vi tài liệu |
| #418 AC-01/03/05 | Các chỉnh lý canonical guide, guard và lệnh verification đã đối chiếu với source thực | Chờ tích hợp |
| #418 AC-02 | Fixture qua `rstml` và Tailwind. Còn kiểm bằng macro `view!` của Leptos với `banner=true` và `banner=false` | `PARTIAL` |
| #418 AC-06 | Walkthrough S1 thay variant và sinh CSS. S2 đã review độc lập docs ở `5614b77f`. Guide cập nhật ví dụ `class:` theo review | Đủ bằng chứng tài liệu, ghi nhận sau khi tích hợp |

Các gate sau đã chuyển sang hai lượt kiểm cuối: tooltip rail, editor SSR/hydration, full quick,
full Web/preview và perf release CSS/HTML/WASM trên cùng máy/browser/profile/corpus. Giữ
`NOT_RUN` khi thiếu lượt chạy đúng ranh giới. Kiểm trực tiếp ảnh trước/sau, forced-colors và
stress 200% trong lượt UX tương ứng. App tại baseline vẫn dùng lựa chọn 100%/200%.
