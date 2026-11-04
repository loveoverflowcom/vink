# UI-REVIEW-21 — Tôi và tiện ích tài khoản

Ngày 04/11/2026 (UTC+07). Base cuối `develop@4ce0ff18c9602ec2ce0bfe09c59dc5f84eef1193`,
nhánh `codex/review-me-profile`. Hợp đồng: [UI-REPAIR-04 r2](ui-repair-04.md).
Đóng góp UX-01; giữ ranh giới ACC-01/02, WR-01/03. Không có auth/profile production.

## Nguồn và truy vết

[PR #46](https://github.com/loveoverflowcom/vink/pull/46) đã merge tại `0fc8c04`;
patch `09662cd` và `15c9f41` sửa click bàn phím `(0,0)` và ngắt từ 320/200%.
Ba owner `appearance.rs`, `pages/me.rs`, `shell.rs` không đổi từ head #46 tới base
này ở preflight `ae2c89b`. Trong lúc bàn giao, develop nhận [PR #69](https://github.com/loveoverflowcom/vink/pull/69) (`4ce0ff1`): Back/Forward fragment đóng overlay trước, giữ scroll, thêm browser assertions. Đã rebase và giữ nguyên fix đó; lỗi fragment phân loại RESOLVED_BY_NEWER_PR, không patch lại. Appearance/CSS/Me/shell của base mới không đổi nên ảnh baseline `ae2c89b` vẫn áp dụng cho hai finding thị giác. Không có branch/PR `codex/review-me-profile` trước preflight. Checkout chính
sạch, đã chuyển về `develop` và pull fast-forward; triển khai trong worktree riêng.

`AdaptiveShell` cung cấp một `Appearance`; `Me` và dialog dùng chung
`AccountStatus`/`ThemePicker`. Native `RadioGroup` phát intent tới `set_theme`,
cập nhật signal, root `data-theme` và storage khi khả dụng. System gỡ override;
CSS, mark, favicon và manifest giải theo root/OS. `OverlayLayer` giữ một modal;
`NavigationGuard` duy nhất hỏi overlay trước guard editor. Avatar Account trung
tính, local fixture không cấp danh tính. Lý do giữ utility khi viết được ghi tại
[UI-REPAIR-04](ui-repair-04.md) và [contract navigation](../../packages/ui-system/shared/navigation.md).

Các ảnh đã mở, chỉ dùng đối chiếu lịch sử:

- Prototype [desktop profile](https://drive.google.com/file/d/1jZop3AR6qI4W_VQudZVfNaUsYWt8kiGO/view)
  và [Tôi mobile](https://drive.google.com/file/d/15NmtzIjPxrAbsklOc98wy9UcGVZ3NLkp/view).
- Implementation [overlay tối](https://drive.google.com/file/d/1Xxa9ipIHN2oPvWtVEPfo8YF0AomTiEp4/view),
  [Tôi sáng](https://drive.google.com/file/d/17fw5qLsnaGEPzu0RjBsYAuPZC1yxRGSu/view),
  [320/200%](https://drive.google.com/file/d/1GP6mHGdBw223aSRct3tcHB4GOAYqIBfx/view).

## Finding và phân loại

| ID | Phân loại | Kết luận |
| --- | --- | --- |
| UI-REVIEW-21-01 | CONFIRMED · P2 · tin cậy cao | Trước hydration, explicit Sáng/Tối đã điều khiển nền/brand nhưng cả hai radio group vẫn chọn System. Người dùng thấy sai preference, rõ hơn khi WASM tải chậm hoặc lỗi. |
| UI-REVIEW-21-02 | CONFIRMED · P3 · tin cậy cao | Rail mở rộng cao 360px/chữ 200% có cuộn ngang nội bộ: 219px nội dung trong 205px khả dụng sau thanh cuộn dọc. Thanh cuộn ngang chiếm thêm chiều cao của điều hướng. |
| UI-REPAIR-04-KBD-01 | RESOLVED_BY_46 | ArrowRight không đóng dialog; kiểm thêm ArrowLeft và Space bằng native keyboard. |
| UI-REPAIR-04-REFLOW-01 | RESOLVED_BY_46 | DOM Range giữ nguyên từng từ tại 320/200%; không viết lại CSS. |
| Fragment Back/Forward | RESOLVED_BY_NEWER_PR | #69 giữ URL, focus và scroll khi overlay tiêu thụ fragment traversal; giữ nguyên code/assertions khi rebase. |
| UI-REVIEW-21-D01 | DESIGN_DELTA | Tên An Nhiên, initials, profile/tác phẩm đã đăng và control ngang trong prototype không thuộc accepted r2; không triển khai. |

`UI-REVIEW-21-01`: Page/Overlay, vùng `#page-theme`/`#overlay-theme`.
Expected: lựa chọn native và theme bootstrap cùng preference trước/sau hydration.
Actual: lưu `light`/`dark`, OS ngược lại, chặn `/pkg/vink.js`; root đúng explicit
nhưng hai group đều `system`, tái hiện ở 1440/320 × chữ 100/200% (8 counterexample).
Design URL: [accepted spec](https://github.com/loveoverflowcom/vink/blob/develop/docs/design/ui-repair-04.md).
Before: `target/ui-evidence/ui-review-21/before/screenshots/appearance-first-paint-dark-os-light-1440-text100.png`
và cùng tên `light-os-dark`. Root cause xác nhận tại `appearance.rs`:
`Appearance::provide` mặc định System rồi đọc storage trong Effect; bootstrap đầu
trang mới chỉ cập nhật root/brand, chưa cập nhật checked properties của SSR radio.
Fix nhỏ nhất: đồng bộ checked từ root khi parse nhóm; hydrate owner từ cùng root.

`UI-REVIEW-21-02`: vùng brand cạnh Profile trigger, tại 1440/834 × 360 × rail mở
rộng × chữ 200%, cả sáng/tối/System. Expected: rail chỉ cần cuộn dọc để tới các
thao tác; Actual: cuộn ngang 14px. Design URL: accepted spec ở trên (rail 220/96,
không tràn ngang). Before HEAD:
`target/ui-evidence/ui-review-21/before/screenshots/account-overlay-834-360-expanded-light-text200.png`.
Đã mở ảnh; probe geometry sau đó dùng đúng CSS HEAD, cùng viewport/scale/rail,
ghi 219/205 trong `appearance.railScrollWidth`/`railClientWidth`. Root cause:
brand inline-flex không wrap theo phần còn lại sau scrollbar, tại
`apps/vink-web/style/vink.css` (`.vk-brand`, `.vk-rail-brand`). Fix: giới hạn brand
bằng slot và cho wrap trong rail mở rộng; không đổi asset/token/nav policy.

## LayoutSpec bổ sung r1 — xuất trước patch UI

```text
Screen #me — /toi
  Heading #me-title — Tôi
  Region #page-account — AccountStatus trung tính
  Region #page-theme — ThemePicker, H2 Giao diện, legend/help
    RadioGroup — Sáng / Tối / Hệ thống
Overlay #profile-menu — cạnh rail 220/96
  Header #profile-title + #profile-close
  Region #overlay-account — cùng AccountStatus
  Region #overlay-theme — cùng ThemePicker
  Action #open-me — /toi qua NavigationGuard
Trigger #profile-trigger — mở modal hiện có
```

Before → After: nền explicit + radio System trước hydration → radio native chọn
đúng preference từ root bootstrap, `Appearance` hydrate từ cùng giá trị.
Owner: `apps/vink-web/src/appearance.rs`; script chỉ đặt checked properties, không
đổi DOM shape, không phát intent/lưu/điều hướng, không tạo preference state thứ hai.
Không đổi sizing/spacing/copy: trang theo `--vk-measure`, overlay tối đa 26rem,
mép và cuộn theo token r2. 320/chữ 200% giữ wrap/touch target; cùng cấu trúc hai
theme, viền/nền chọn và focus ring hiện hành áp cho radio đúng. System theo OS;
storage bị từ chối vẫn có System ban đầu và lựa chọn trong phiên. Editor, caret,
undo, guard, focus return và scroll dùng owner hiện có.
Acceptance: cùng route/viewport/text/theme/fixture/rail/scroll và chuỗi thao tác;
chặn bundle để đọc first paint riêng, kiểm lại khi hydrate, mở PNG trước/sau.

### Bổ sung r2 — xuất trước patch CSS

Region brand trong `#navigation`: mark và tên vẫn là một link trang chủ. Trước →
sau: một hàng intrinsic gây cuộn ngang khi scrollbar lấy 15px → brand giới hạn
trong slot, tên xuống hàng dưới mark nếu thiếu chỗ. Chỉ container rail >100px;
rail compact giữ mark hiện có, các kích thước 220/96 và khoảng cách token giữ
nguyên. Không đổi màu, nhãn, accessible name, focus, profile trigger hoặc guard.
Viewport hẹp mobile giữ brand riêng. Acceptance: `scrollWidth <= clientWidth`
của rail trong toàn ma trận và hai đầu overlay vẫn tới được; mở ảnh cùng scenario.

## Kiểm chứng và bằng chứng

Runtime: macOS 26.5.2, Rust 1.99.1, Node 26.12.0, PostgreSQL 17.11 thử riêng
trên loopback cổng 55441, Chrome 156.0.8078.80 headless, DPR 1. Fixture
`VINK_DEV_PRINCIPAL` sẵn có của repo; AccountStatus vẫn là chưa có thông tin.

```sh
CARGO_INCREMENTAL=0 VINK_CHROME='/Applications/Google Chrome.app/Contents/MacOS/Google Chrome' \
VINK_TEST_DATABASE_URL=postgres://manh.pd1@127.0.0.1:55441/postgres \
cargo xtask verify quick

CARGO_INCREMENTAL=0 VINK_CHROME='/Applications/Google Chrome.app/Contents/MacOS/Google Chrome' \
VINK_TEST_DATABASE_URL=postgres://manh.pd1@127.0.0.1:55441/postgres \
cargo xtask verify web
```

Các phase `appearance`/`first-paint` của chính `browser-smoke.mjs` cho phép chạy
lại tập trung với `--base`, `--chrome`, `--state` trên server fixture/scratch được
chuẩn bị sẵn. Không thêm runner song song. Text 200% là root CSS font, không phải
browser zoom hoặc Dynamic Type. Main matrix: Tôi 1440×900, 834×900, 390×844,
320×900; thêm trang/overlay 1440×360 và 834×360; overlay đủ rail expanded/compact,
Light/Dark/System, chữ 100/200%; thêm 1024×900 compact. Ảnh đầu/cuối vùng cuộn
tách riêng. First paint kiểm explicit ngược OS và System theo cả OS sáng/tối.

| Thành phần | Quan sát/đường kiểm |
| --- | --- |
| Identity/AccountStatus | Hai vùng dùng cùng component, avatar Account trung tính, H1/H2 rõ, copy Việt thật; không suy danh tính từ fixture. |
| ThemePicker | Fieldset/legend/help, tên radio tách biệt, một checked mỗi nhóm, chọn nguyên hàng; native mũi tên/Space, focus ring và đồng bộ page/overlay. |
| Overlay | Native modal có accessible name; Enter/chuột mở, focus selected, Tab hai chiều, Escape/outside/Đóng, stale close/reopen, Back/Forward và resize trả focus theo contract. |
| Responsive | Root/dialog không tràn ngang, giữ từng từ tiếng Việt tại 320/200%, cuộn lộ Đóng/Mở trang Tôi; oracle bổ sung kiểm cuộn ngang nội bộ rail. |
| Editor | Cùng EditorView/text/selection/undo qua đổi theme nhiều lần, đóng/mở và resize; không PUT/POST khi chỉ đổi utility; mở Tôi khi dirty, pending save hoặc conflict đi qua guard hiện có. |

Lịch sử FAIL được giữ trong `target/ui-evidence/ui-review-21/logs/`: lần đầu
thiếu đường dẫn Chrome; probe Space đầu gửi sai `key` (đã sửa thành ký tự space);
first-paint trước sửa tái hiện 8 lựa chọn sai và 4 chuỗi frame System → explicit;
một lần build hết chỗ đĩa (đã dọn cache build trong worktree này); lần Web trung
gian treo helper khi save pending vì chờ `finished` của loading arc vô hạn (đã
giới hạn việc chờ vào animation hữu hạn). Không gọi các lần đó là PASS.

| Kiểm tra cuối | Kết quả |
| --- | --- |
| `cargo xtask verify quick` | PASS: token drift, fmt, clippy SSR/WASM/preview, doctest, unit/contract, repository checks; `logs/quick-rebased.log` |
| `cargo xtask verify web` | PASS: main + after-restart, SSR/hydration, radio/bàn phím/focus, theme/OS/storage denied, EditorView/chữ/selection/undo, pending/conflict/guard, SIGTERM/restart/export; không console error/exception; `logs/web-rebased.log` |
| UI-REVIEW-21-01 | PASS: 8 counterexample explicit và chuỗi frame nay cùng preference; System vẫn theo OS. Owner hiện tại: `appearance.rs:18,34,123`. |
| UI-REVIEW-21-02 | PASS: rail client/scroll width 205/205 thay cho 205/219 tại short/text200; oracle qua toàn ma trận. Owner: `vink.css:85`. |
| Contrast Tôi/overlay | PASS: tối thiểu 6,28:1 sáng, 8,23:1 tối từ màu resolve, gồm nhãn chọn/help/trạng thái. |
| Self-review | PASS: diff so với `origin/develop@4ce0ff1`, đúng owner, native radio và một state/guard, SSR/hydration cùng lựa chọn, không đổi draft/save/identity API hoặc shared primitives. |

Bundle local (không upload/commit PNG hoặc log):
`target/ui-evidence/ui-review-21/manifest.json`, `reports/REPORT.md`,
`reports/navigation.mmd`, `reports/navigation-adjacency.json`, `sources/`,
`before/source.json` và `after/source.json`. Hai source record pin hash nguồn và
binary/WASM/CSS được dùng, nên không nhầm base SHA với dirty patch đã build.
HEAD commit cuối được ghi tại PR; nguồn production/test trong commit phải khớp
hash `after/source.json` của lần kiểm trên. Feature map cập nhật assertions.

Coverage: 170 tên capture cố định mỗi phía đều có; cộng các điểm cuối cuộn và ảnh
bổ sung thành **220 before + 220 after**. Mở 13 contact sheet mỗi phía, gồm **78
PNG mỗi phía**, và mở original first-paint desktop trước patch. Các PNG còn lại
giữ `inspected=false`, không gọi là visual PASS toàn bộ. File `failure.png` của
probe FAIL được giữ trong raw before, không tính vào 220 ảnh dùng đối chiếu.
Sau rebase, 75/78 ảnh đã kiểm khớp SHA-256 với lần trước; đã mở lại 3 PNG còn khác
trực tiếp (1440 light, 320 light và 390 System ở chữ 100%).
Metadata ghi route, viewport, font, theme yêu cầu/hiệu lực/OS, rail, checked option,
scroll trang/dialog, fixture và build. Browser behavior và screenshot evidence
được ghi riêng; 5 ảnh lịch sử cũng đã mở nhưng không dùng thay HEAD.
ScrollTop nội bộ rail trước sửa chưa được runner ghi; cùng click-path tự cuộn
tới trigger được dùng hai phía. Không suy ra so sánh pixel tuyệt đối của rail.

After cho finding 01:
`target/ui-evidence/ui-review-21/after/screenshots/appearance-first-paint-dark-os-light-1440-text100.png`
(cùng basename với before, thêm biến thể light/320/200% và `-appearance` khi cuộn).
After cho finding 02:
`target/ui-evidence/ui-review-21/after/screenshots/account-overlay-834-360-expanded-light-text200.png`.
Chọn cùng viewport/theme/font/rail/fixture/chuỗi thao tác và điểm cuộn; brand wrap
là khác biệt có chủ ý theo LayoutSpec r2. Những vùng AccountStatus/ThemePicker
khác giữ hierarchy, copy và spacing r2; không có finding mới cần patch trong các
ảnh đã mở và hành vi đã chạy.

Giới hạn: Chrome headless trên một macOS, input chương trình; không có thiết bị
mobile thật, Safari/Firefox, OS IME hoặc phiên dùng VoiceOver/TalkBack. Semantics
DOM/AX không là lượt dùng screen reader. Auth/profile production, tên tài khoản
thật/dài và native product app chưa có, không dựng fixture danh tính để nghiệm thu.
Preview runtime NOT_RUN vì không đổi shared primitive, token, pattern hay recipe;
clippy/unit preview nằm trong Quick. Không đổi query/transaction, lane PostgreSQL
riêng NOT_RUN; Web vẫn dùng PostgreSQL thật trong database thử có thể xóa.
