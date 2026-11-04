# UI-REVIEW-20 — lịch sử fragment và shell hiện hành

Ngày 04/11/2026. Nền tích hợp cuối
`origin/develop@ae2c89b1af42bc0001af2583f1e1387739d953eb`, nhánh
`codex/review-shell-navigation`. Ban đầu fetch/chuyển develop tại `fab4f952`,
fast-forward rồi tạo worktree riêng; checkout cũ không có thay đổi và được giữ nguyên.
Trong lúc review, PR #68 (READ-02) vào develop; patch được rebase không conflict
và cặp BEFORE/AFTER được dựng, chụp lại trên cùng nền `ae2c89b`.
Snapshot `c6aa486` của prompt chỉ dùng đối chiếu, không là đích reset.

## Kết quả và phạm vi sửa

**R20-04, P2, CONFIRMED:** trên `/toi#noi-dung`, mở Tôi rồi Back làm URL trở về
`/toi` nhưng overlay vẫn mở. `NavigationGuard::popstate` bỏ qua mọi traversal có
cùng path/query trước khi hỏi overlay. Bản sửa hỏi overlay trước, rồi mới bỏ qua
fragment khi không có overlay. Giữ một guard, cùng cơ chế phục hồi URL hiện có.
Back và Forward đóng overlay, giữ URL hiện tại và trả focus về trigger; lần Back
sau vẫn đi qua fragment bình thường. Probe bổ sung ở 1440×360/text200 xác nhận
native history có thể đưa scroll500 về0 sau callback `popstate`, dù URL đã được
giữ. Guard phục hồi vị trí ở animation frame kế tiếp, chỉ khi URL vẫn đúng trang
đã giữ; không đổi `history.scrollRestoration` toàn cục hoặc cuộn một URL mới.
Editor bẩn không mở thêm leave dialog cho
traversal nội trang, không lưu hoặc remount.

Regression mới `shellFragmentHistory` thất bại trên bundle BEFORE ở
`fragment Back closes Tôi`; trên bản guard-only trung gian nó bắt tiếp lỗi scroll,
và đạt đủ trên AFTER cuối. `shellWithDirtyEditor` mở rộng ca
fragment với chữ chưa lưu, tiếp tục kiểm cùng EditorView, selection, title,
version/state và không có POST/PUT ngoài ý muốn.

Không đổi CSS, token, UI primitive, router, API/database, preference keys, thứ tự
đích hoặc các sửa preview/reflow của PR #55. PR #33, #40, #55 đã được đọc; từ
`c6aa486` đến nền mới có CSS recovery và READ-02; không có bản sửa fragment trùng
trong shell/appearance/navigation/primitive. Không kiểm hoặc chờ CI GitHub; chỉ dẫn mới của
người giao việc cho phép merge sau local checks nếu branch protection cho phép,
không tự approve hoặc bypass gate. Không deploy.

## LayoutSpec đã trình bày trước patch

```text
LayoutSpec: VINK-SHELL-20 / UIS-04, scoped R20-04
Screen #shell
  Region #navigation
    NavUtility #profile-trigger — owner shell.rs
  Region #noi-dung — cùng instance route/editor
  Overlay #profile-menu — owner shell.rs + OverlayLayer

BEFORE → AFTER:
  popstate cùng path/query bỏ qua overlay
  → NavigationGuard hỏi overlay trước mọi traversal;
    không có overlay thì fragment nội trang không gọi leave check.
Responsive: giữ rail220 >=1100, rail96 760–1099, bar <760;
  editor compact/hide-bar, preference người dùng vẫn thắng mặc định.
Themes/text200: cùng cây, CSS và token, không thay hierarchy/nhãn.
Focus: đóng overlay trả profile-trigger còn thấy hoặc main;
  giữ scroll trước traversal sau bước native history restoration,
  chỉ phục hồi khi URL chưa đổi bởi intent mới.
Acceptance: cùng route/hash, fixture, viewport/theme/text scale;
  Back/Forward đóng overlay trước, next Back/Forward nội trang tự nhiên;
  scroll khác0 giữ nguyên; editor giữ text/selection/version,
  không POST/PUT hay leave dialog thừa.
```

Contract: [shared navigation](../../packages/ui-system/shared/navigation.md),
[shell #7](README.md#shell-thích-ứng-7). Trace: `lib.rs → AdaptiveShell → NavLayout
→ NavigationRail/CSS`; preference `appearance.rs::BOOTSTRAP → CSS → hydration`;
traversal `NavigationGuard → close_profile → OverlayLayer`.

## Findings và Design authority

Các ảnh Design là **CANDIDATE** cho so sánh thị giác. Contract UIS-04 và shell #7
là **CONFIRMED** cho hành vi đã có. Hai ảnh implementation cũ trước PR #55 chỉ là
lịch sử, không phải BEFORE của lần này. Không pixel-diff tablet 834/light/Explore
với tablet 1024/dark/Me/text200.

| ID | Screen/region | Design URL | Before URL | After URL | Expected / Actual | Severity | Confidence | Source owner | Evidence type | Fix / Intentional difference / Gap |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| R20-04 | Tôi/overlay/history | [contract](../../packages/ui-system/shared/navigation.md) | Chưa upload; local `before-fragment-after-back` | Chưa upload; local `after-fragment-after-back` | Back đóng overlay trước / BEFORE còn mở và mất hash; AFTER đóng, giữ hash/focus và scroll khác0 | P2 | Cao | `apps/vink-web/src/navigation.rs` | Browser interaction + ảnh đã mở + regression âm/dương, scroll500 | FIXED |
| R20-03 | Rail trước hydration | [desktop][design-desktop], [tablet][design-tablet] | Chưa upload; local `before-ssr-tablet`, `before-ssr-desktop-saved-compact` | Chưa upload; local `after-ssr-tablet` | CSS rail96 / SSR icon `menu-open`, `aria-expanded=true`, tên Thu gọn; sau hydration đúng compact | P3 | Cao cho probe giữ hydration; chưa xác nhận nháy cold load bình thường | `shell.rs`, `appearance.rs`, shared `NavigationRail` | Hydration module bị giữ qua CDP; DOM + ảnh đã mở | OPEN GAP; không sửa trong patch history |
| R20-02 | Tooltip/heading | [desktop][design-desktop] | Chưa upload; local `before-tooltip-1440-light` | Chưa upload; local `after-tooltip-1440-light` | Tooltip ngoài rail, Escape ẩn; bbox chạm H1 42.375×0.789px, không thấy che glyph H1; có che đầu dòng mô tả | Chưa gán | Cao cho geometry, chưa đủ cho defect H1 | shared `navigation.rs`/`vink-ui.css` | 1440/light/rail96, ảnh đã mở + DOM | Không xác nhận lỗi H1; giữ tooltip transient theo contract |
| R20-01 | TopBar/search | [desktop][design-desktop], [tablet][design-tablet], [mobile][design-mobile] | Chưa upload; local `before-explore-*` | Chưa upload; local `after-explore-*` | Design có global search/CTA; contract hiện hành top bar mobile, tìm trên danh sách đã tải | — | Cao | `shell.rs`, `pages/mod.rs`, shared navigation | Source + ảnh Design/HEAD đã mở | INTENTIONAL DIFFERENCE; không tạo search/CTA giả |
| R20-200 | Rail/reflow | [ảnh cũ][history-tablet] | Ảnh lịch sử khác revision | Chưa upload; local `after-me-1024x900-dark-text200` | Giữ nguyên từ tiếng Việt trong rail96, cuộn được | — | Cao trong Chrome đã kiểm | shared `vink-ui.css`, PR #55 | Source + ảnh đã mở + lane web | RESOLVED/không sửa lại |

R20-03 còn được giữ rõ: server không biết viewport hay local preference, trong khi
CSS đã giải đúng bề rộng. Sửa mismatch này cần một thay đổi progressive enhancement
có kiểm chứng ở owner bootstrap/primitive; thay riêng giá trị Expanded bằng Compact
sẽ chỉ chuyển lỗi sang desktop mở rộng. Patch này không đổi cơ chế đó hoặc nhận
SSR đã đạt. Probe có chủ ý giữ JS chỉ chứng minh trạng thái trước hydration;
không chứng minh độ dài hay tần suất nháy người dùng thực sự gặp.

## Bằng chứng local và verification

Bundle cuối: `target/ui-evidence/2026-11-04T150659+0700_review-shell-navigation_ae2c89b1/`.
Run đầu trên `fab4f952` được giữ riêng tại
`target/ui-evidence/2026-11-04T144940+0700_review-shell-navigation_fab4f952/`.
`manifest.json` giữ hash, build fingerprint, capture/inspection status và mapping;
`reports/REPORT.md`, `reports/navigation.mmd`, adjacency JSON và ảnh nằm cùng run.
Screenshot names ở bảng là capture IDs, file raw nằm dưới `screenshots/`;
ảnh chuẩn hóa và đường dẫn chính xác do manifest giữ. URL upload là null vì chưa
có yêu cầu upload Drive riêng cho run này; không dùng link cũ làm BEFORE/AFTER.
Không commit binary screenshots hoặc runtime logs.

App thật Leptos SSR/hydrate trên macOS arm64, Chrome headless, DPR1, CSS px,
PostgreSQL 17.11 scratch riêng. Fixture local tổng hợp, không phải auth thật;
sample work/draft `33333333-3333-4333-8333-333333333333`. Cùng database seed được
giữ cho BEFORE/AFTER, lane web dùng database scratch khác. Chờ hydration (trừ probe
giữ module), `fonts.ready` và hai animation frames trước chụp. Text200 là tăng root
font, không phải browser zoom. Build dùng `CARGO_INCREMENTAL=0`,
`CARGO_PROFILE_DEV_DEBUG=0`, `CARGO_PROFILE_TEST_DEBUG=0` để giảm dung lượng;
không đổi source/pin hoặc assertion cho việc này.

Ma trận DOM gồm 5 route × 10 viewport × 2 theme × text100/200 = 200 ca mỗi phía:
`/`, `/ban-viet`, `/toi`, reader/draft hợp lệ; 1440×900, 1100×900, 1099×900,
1024×900, 834×900, 760×900, 759×900, 390×844, 320×900, 1440×360.
Kiểm rail/bar/TopBar, `aria-expanded` sau hydration, không tràn ngang;
không gọi 200 ca này là kiểm mọi interaction ở mọi tổ hợp. Lane web bổ sung
system theme/storage denied/reload, active route/href/modifier click, skip link,
tooltip/Escape, overlay focus/resize, contrast, editor/save/conflict/restart.

| Kiểm | Kết quả |
| --- | --- |
| `cargo xtask verify quick` BEFORE | PASS (run cuối); fmt/clippy SSR/WASM/preview, unit/contract, repository checks |
| `cargo xtask verify web` BEFORE | PASS trên PostgreSQL/Chrome thật |
| Regression fragment trên bundle BEFORE | FAIL đúng lỗi, timeout chờ overlay đóng |
| Regression scroll trên bundle guard-only trung gian | FAIL đúng lỗi scroll500→0 sau native history restoration |
| `cargo xtask verify quick` AFTER | PASS |
| `cargo xtask verify web` AFTER | PASS; main + after-restart, gồm fragment Back/Forward và dirty editor |
| Regression fragment trên bundle AFTER | PASS, cùng assertion đã FAIL ở BEFORE |
| Matrix BEFORE/AFTER | PASS 200/200 mỗi phía; 32 ảnh BEFORE + 32 AFTER và 1 ảnh scroll bổ sung, 21/65 ảnh đã mở |
| `ui-preview leptos/cmp verify` | NOT_RUN: không đổi primitive/shared semantics/CMP |
| CI GitHub | NOT_RUN theo yêu cầu |

Giữ lịch sử FAIL môi trường: quick đầu bị sandbox chặn ba test Unix socket; lần
tiếp theo và web bị hết đĩa; web từng thiếu `VINK_CHROME` trong PATH và cache npm
toàn cục lỗi EACCES. Chạy lại với quyền local phù hợp, đường dẫn Chrome tường minh,
profile gọn và cache npm riêng. Các FAIL không bị đổi nhãn hoặc bỏ assertion.

Giới hạn: R20-03 còn mở; tooltip không được nhận là tránh mọi nội dung ở mọi
scroll/text scale. Chưa kiểm thiết bị thật, browser zoom, Safari/Firefox,
OS IME, screen reader, native Back/lifecycle hoặc safe-area khác0. Reduced motion
chỉ được giả lập trong Chrome bởi lane web; không suy từ ảnh tĩnh.

[design-desktop]: https://drive.google.com/file/d/1yC6XQhvw8wQWrwR_IElwZBzFuWN8iyBO/view
[design-tablet]: https://drive.google.com/file/d/16LOHZdsd-P_oNc9VlzuaH2Im9joK9t8o/view
[design-mobile]: https://drive.google.com/file/d/1kEjk2Sb3lV1IdUhS_joP_BJmahKTL8eR/view
[history-tablet]: https://drive.google.com/file/d/17ygozo_kfEqE_N0ELjDGOLF7w4juTxbr/view
