# UI-REPAIR-04 — Tôi, tài khoản và giao diện

Ngày 03/11/2026. Base `develop@393d624a02a042a5f123bdcd1fd657970b613a71`, nhánh
`codex/ui-account-appearance`. Phạm vi: phân cấp thông tin, radio và khả năng tiếp
cận overlay của Tôi; đóng góp vào UX-01, giữ ranh giới ACC-01/02 và WR-01/03.
Không hoàn tất UX-04, auth, profile hay S-01/S-03.

## Nguồn, hiện trạng và nguyên nhân

- [Current design](https://drive.google.com/file/d/1cNYQh-iukYP-nP0cphLOGdywROAmtkSr/view)
  quy định thứ tự Khám phá → Bàn viết, Tôi ở chân rail/mobile và không giả danh tính.
- [Bộ ảnh được giao](https://drive.google.com/drive/folders/13qS01MxofBB3TojMMFIe-Yqt_LGNrj9Q):
  đã đọc index, mở ảnh
  [Tôi sáng 1440](https://drive.google.com/file/d/17fw5qLsnaGEPzu0RjBsYAuPZC1yxRGSu/view).
  Metadata bộ này ghi scale chưa biết; chỉ dùng làm ngữ cảnh, không dùng để khẳng
  định before/after cùng build. Hình prototype có tên/ảnh mẫu không cấp auth authority.
- Baseline chạy lại từ source `393d624` trước sửa: `cargo xtask verify web` PASS;
  giữ 20 ảnh/manifest liên quan tại `target/ui-repair-04/before/`. Đã mở overlay
  dark 1440 và Tôi light 320/200%. `/toi` xếp trạng thái và radio trực tiếp dưới
  H1, chưa có mô tả phạm vi preference; overlay dùng cùng nội dung và radio dọc.

HOW: `AdaptiveShell` tạo một `Appearance`; `NavUtility` mở native `dialog` qua
`OverlayLayer`. Trang `Me` và overlay cùng gọi `AccountStatus`/`ThemePicker`.
Radio phát intent tới `Appearance::set_theme`, cập nhật signal, `data-theme` và
storage; bootstrap chọn theme trước CSS, logo/favicon/manifest theo cùng root và
OS. Hệ thống gỡ override; storage lỗi vẫn đổi được trong phiên. Không có owner
danh tính thật để dựng profile. Avatar tài khoản là mark trung tính; initials của
tên mẫu trong unit test và recipe preview không là danh tính người dùng.

WHY được ghi tại [spec shell](README.md#shell-thích-ứng-7) và lịch sử UIS-04
`e105cc9`: Tôi là utility để đổi giao diện khi viết; giữ editor instance, một
overlay và một navigation guard. Không có bằng chứng sản phẩm yêu cầu profile giả
hoặc phân cấp phẳng. Rủi ro chật khi chữ lớn được suy ra từ width cũ 360px và
margin account; baseline chưa chụp cửa sổ thấp nên không gọi đó là lỗi đã quan sát.

Finding `UI-REPAIR-04-KBD-01` (P2, đã tái hiện): trong overlay, focus radio Hệ
thống rồi ArrowRight đổi sang Sáng nhưng dialog bị đóng và focus về trigger.
Native radio phát click bàn phím với tọa độ `(0, 0)`; handler cũ xét cả click
bubble từ con như click ngoài. Bản sửa chỉ kiểm tọa độ khi target chính là
dialog/backdrop. `accountAppearance` kiểm lựa chọn, focus-visible và modal vẫn mở,
nên mất focus sẽ làm browser check FAIL; không làm lỏng oracle theo lỗi.

## LayoutSpec r1/r2 — trình bày trong chat trước patch tương ứng

```text
Screen #me — /toi
  #me-title — H1 Tôi
  #page-account — H2 Tài khoản
    neutral-avatar + trạng thái thật + giới hạn đăng nhập
  .vk-appearance-region
    #page-theme-title — H2 Giao diện
    #page-theme — fieldset / legend Chế độ màu
      Sáng / Tối / Hệ thống — native radio
      #page-theme-help — phạm vi thiết bị và nghĩa Hệ thống

Overlay #profile-menu — cạnh chân rail 220/96px
  #profile-title + #profile-close
  #overlay-account — cùng AccountStatus
  #overlay-theme-title + #overlay-theme — cùng ThemePicker
  #open-me — điều hướng thật qua NavigationGuard

Rail #navigation-account
  #profile-trigger — Tôi, aria-expanded / aria-current
Mobile #bottom-navigation
  Tôi — link /toi, không thêm modal
```

Trước → sau: nội dung phẳng → hai vùng có heading, khoảng cách và nền riêng;
radio nhỏ trong khoảng trống → target nguyên hàng với viền/nền chọn và chấm native.
Các nhãn Sáng/Tối/Hệ thống, preference keys và owner vẫn giữ nguyên. Legend được
cụ thể thành “Chế độ màu” dưới H2 “Giao diện”. Help nói “áp dụng” thay vì cam kết
“đã lưu”, vì trình duyệt có thể từ chối storage.

Trang giới hạn bằng `--vk-measure`; padding vùng co theo viewport và spacing
tokens. Overlay rộng tối đa 26rem, co theo phần còn lại bên cạnh rail, chừa mép
bằng `--vk-space-4`, cao tối đa viewport trừ hai mép. Toàn dialog cuộn; con không
co chiều cao để nhét cửa sổ thấp. Focus đầu vào radio đang chọn; Tab/Shift+Tab
đi tới và cuộn lộ Đóng/Mở trang Tôi, Escape/bấm ngoài trả trigger. Back đóng
overlay trước guard editor; resize mobile trả main. Không thêm state/guard mới.

Hai theme có cùng cây; dùng `surface-container-low`, `outline-variant`,
`primary-container`, `on-primary-container`, `primary`, `focus`, type/radius/space
từ [tokens.json](../../packages/ui-system/shared/tokens.json). Không đổi token
chung, logo, primitive UI system, CMP hay recipe preview. CSS mới chỉ scope vào
composition tài khoản/giao diện do app sở hữu.

R2: ảnh cuộn tại 320px/200% phát hiện khoảng radio/padding làm “Sáng”, “thống”
tách giữa từ dù container không tràn ngang (`UI-REPAIR-04-REFLOW-01`, P2). Giảm
gap/padding ngang hàng radio về `--vk-space-2`, giữ target và cỡ chữ. Browser
assertion dùng DOM Range trên từng từ để kiểm điều này độc lập với CSS literal.

## Kiểm chứng

Owner code: [appearance.rs](../../apps/vink-web/src/appearance.rs),
[me.rs](../../apps/vink-web/src/pages/me.rs),
[shell.rs](../../apps/vink-web/src/shell.rs),
[vink.css](../../apps/vink-web/style/vink.css).
[Browser smoke](../../apps/vink-web/e2e/browser-smoke.mjs) mở rộng `adaptiveShell`,
`accountAppearance`, `shellWithDirtyEditor`; không tạo runner riêng.

Ma trận sau sửa: `/toi` tại 1440/1024/834/390/320 × System/Light/Dark × chữ
100/200%; overlay 1440 expanded, 1024/834 compact cao 900; thêm 1440 expanded và
compact cao 360, mỗi trường hợp đủ ba theme/hai cỡ chữ. Ảnh có thể cuộn ghi thêm
đuôi `-bottom`; manifest ghi viewport, root font, scroll và theme thực. Không gọi
root font 200% là browser zoom hay Dynamic Type.

Đã chạy trên patch triển khai với Rust 1.99.1, Node 26.12.0, Chrome
156.0.8078.79 headless macOS, PostgreSQL 17.11 tại cluster local riêng cổng 55434:

| Check | Kết quả quan sát |
| --- | --- |
| `cargo xtask verify quick` | PASS: token drift, fmt, clippy SSR/WASM/preview, doctest, unit/contract và repository checks |
| `cargo xtask verify web` | PASS: toàn bộ main + after-restart, SSR, editor/save/conflict/leave guard, SIGTERM/restart/export; không console error/exception |
| Contrast chữ thực qua `getComputedStyle` | Tôi/overlay tối thiểu 6,28:1 sáng; 8,23:1 tối, gồm nhãn chọn, trạng thái và help |
| Visual review | Mở 15 contact sheet chứa 81 ảnh Tôi/overlay, gồm đủ ma trận trên và điểm cuối vùng cuộn; heading, dấu tiếng Việt, viền chọn và hai đầu dialog đúng spec |
| Shared Leptos preview runtime | NOT_RUN: không đổi primitive, token, pattern hoặc recipe preview; clippy/unit preview vẫn nằm trong `quick` |

Raw ảnh/`contexts.json` ở `target/verify-web/screenshots/`; contact sheets đã mở
ở `target/ui-repair-04/inspection/`, baseline ở `target/ui-repair-04/before/`.
Ảnh 200% của trang dài cho cuộn chính, không ép Giao diện phải nằm trong viewport
đầu. Browser smoke còn chụp riêng vùng lựa chọn khi cuộn tới ở mobile 200%.
System resolve theo OS sáng trong ma trận; OS tối và first paint trước hydration
được kiểm riêng. Ảnh bổ sung mobile 200% dẫn tới R2 ở trên; bằng chứng trước R2
không xác nhận layout radio cuối. Log và SHA của lần kiểm cuối được dẫn trong PR; ảnh không tự là
bằng chứng keyboard, persist, zoom trình duyệt hay screen reader.

Lịch sử gate được giữ đúng: lần gọi web không cấu hình là FAIL prerequisites,
các bước sau NOT_RUN; lần patch đầu quick/web FAIL build do move một `String`
trong `view!`, đã sửa ownership; web tiếp theo FAIL ở counterexample keyboard
trên, đã sửa handler và chạy lại PASS. Không chuyển các FAIL đó thành NOT_RUN.
Self-review đọc lại diff/caller/CSS scope và mở ảnh không còn finding đáng hành
động trong phạm vi đã thử; không thay save/leave authority hoặc dựng tài khoản.

Giới hạn: một Chrome headless macOS, DPR 1; không có thiết bị mobile thật,
Safari/Firefox, OS IME hay phiên đọc bằng VoiceOver. Các bước editor dùng input
chương trình và PostgreSQL thử riêng; không là bằng chứng production auth.
