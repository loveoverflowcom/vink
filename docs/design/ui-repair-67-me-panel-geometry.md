# #67 — Kiểm lại hình học Tôi và panel trên develop mới

Ngày 07/11/2026. Base `develop@552ab46daefda029d7d6dc5918c63c3bd4f34704`,
đã chứa PR #89 (#88) và PR #91 (#66). Nhánh `codex/me-panel-responsive-polish`.
Phạm vi là AC-11–AC-13 bổ sung của [#67](https://github.com/loveoverflowcom/vink/issues/67).

## Kết quả và nguồn

**Không tái hiện sai lệch hình học còn lại trong các ca được giao.** Giữ nguyên
CSS/component hiện hành; thêm assertion đo bề rộng chữ, gutter, trục nội dung và
ảnh cuối trang vào browser smoke hiện có. Không làm lại R2 hoặc các PR cũ.

Đã đọc [LayoutSpec Expressive Lavender R2](https://drive.google.com/file/d/1ChFchoWsrc9XFtkW5BvP5XUfr6GAAbKy/view)
và mở ba board: [mobile sáng](https://drive.google.com/file/d/1N4jjBTEHLHML-1K0ibEKNiUEY0Suf1xj/view),
[panel tối](https://drive.google.com/file/d/1turL07DaXgMtOPYaNDObFBY7hNt5aUVS/view),
[320/chữ200](https://drive.google.com/file/d/1qRJwdxEvnkLYDjhnPdjO5ekrEr1dWywi/view).
Các board là SVG render thành PNG, **không phải runtime**. Ảnh implementation cũ
c003/c004/c005/c017 và nhận xét trong issue là đầu vào lịch sử; không gán chúng
cho HEAD mới.

Baseline và final chạy cùng production source sạch của base trên. Diff chỉ thêm
browser assertions/metadata và tài liệu; không thay appearance, shell, CSS, tokens
hay fixture để có ảnh đẹp. Baseline Web đã nạp runner trước khi bổ sung phép đo;
final Web chạy runner mới. SHA trong context là base, không phải tuyên bố toàn
checkout final sạch. Bundle giữ patch/hash của inputs để phân biệt hai lần chạy.

## LayoutSpec rút gọn và lựa chọn

`Tôi → account identity → Giao diện → ba native radio → helper`.
Panel: `header/Đóng → account → Giao diện/choices/helper → Mở trang Tôi`.
Kế thừa R2: gutter16 khi hẹp, flex-wrap account, chiều cao theo chữ; nhóm ngang
chỉ khi container đủ 38em; panel một surface và một scroller. Không thu font hoặc
chép tọa độ từ board. Các lớp icon/label/check vẫn có inset riêng có chủ đích.

CHOICE-04: giữ `flex-wrap` hiện có; tại 320/390 chữ200, text tự xuống dưới avatar
và dùng trọn measure. Không cần grid hay threshold mới. CHOICE-05: giữ chủ inset
ở panel; đo cho thấy header/account/appearance/choices/helper cùng trục. Không
cần bỏ padding của child hay gom lại layout đang đúng. CTA cùng trục trái và đủ
chữ; không ép rộng toàn panel chỉ vì board minh họa như vậy.

## Recapture và số đo

Chrome 156 headless, macOS, DPR1; root font 16/32px tương ứng chữ100/200%, không
phải browser zoom hay Dynamic Type. LocalFixture do repo sở hữu, không phải auth
production. Chờ hydration và `document.fonts.ready`; renderer và font thật của
app. Có scrollbar dọc 15px, nên layout viewport ở 320px rộng305px: gutter16 hai
bên cho content273px; không bù scrollbar bằng CSS lệch.

| Ca lịch sử tái hiện | Kết quả current runtime | Số đo CSS px |
| --- | --- | --- |
| c003/c004: `/toi` 390×844, sáng/tối, chữ100 | Khung lồng cũ đã hết; hai theme cùng geometry | account/appearance/helper x16, width343; account text x68, width291 |
| c017: `/toi` 320×900, tối, chữ200 | Account xếp dọc; chữ dùng toàn vùng, không thành cột hẹp | account text x16, width273; label đầy đủ hai dòng; font label28px |
| c005: panel 1440×900, tối, expanded rail | Một surface, chung trục, không khung con | panel x236/w448; content x260/w400; inset24 mỗi bên |
| Panel thấp 834×360, tối, chữ200, expanded rail | Một scroller; Shift+Tab lộ CTA cuối, Tab lộ Đóng, Escape trả trigger | panel x252/w550/h296; content x300/w439; inset48 mỗi bên trong client box, scrollbar15 |

Ở 320/chữ200, “Hệ thống” xuống hai dòng theo từ đầy đủ và chiều cao linh hoạt;
không phải clipping hay ellipsis. Text account xếp dưới avatar; label hai dòng,
thông báo ba dòng. Trục/measure của account và nhóm appearance vẫn đúng. Helper
và footer cuối có ảnh cuộn riêng, tới được phía trên vùng navigation.

Đã mở bảy ảnh baseline cùng ca và tám ảnh final: 390 sáng/tối; 320 tối chữ200
ở đầu, appearance và cuối; panel1440 tối; panel834×360 chữ200 ở đầu/cuối. Không
gọi các ảnh viewport là full-page. Cùng fixture policy; nội dung danh sách phía
sau panel giữa baseline full lane và focused run khác trạng thái do baseline
kiểm editor. Không dùng nền danh sách đó làm bằng chứng before/after sản phẩm.

## Kiểm local và độ nhạy

Cwd: `/Users/manhblue/Documents/Codex/2026-11-07/task-3/vink`.
Lệnh theo [guide](../development/README.md), với debug info/incremental tắt và
tái sử dụng target có sẵn để tiết kiệm đĩa. PostgreSQL17 scratch riêng trên
loopback; npm cache tại `/tmp/vink88-npm` tránh lỗi quyền cache người dùng.

- `cargo xtask verify web` baseline: PASS, gồm DB thật, SSR/hydration,
  theme/first-paint, radio/overlay, editor dirty guard, restart/export/endorsement.
- Phase `appearance` của runner hiện có: PASS, gồm geometry mới và các kiểm
  tương tác/theme cũ. `cargo xtask verify web` trên final assertions: PASS toàn bộ
  các bước, gồm các kiểm editor/dirty guard/restart và ghi PostgreSQL thật.
- `cargo xtask verify quick`: PASS (fmt, clippy, unit/contract, doctests,
  generated drift, repository checks).
- Negative control: copy tạm của chính runner inject `padding-inline:16px` vào
  account text bằng DOM; **EXPECTED FAIL** tại assertion “truthful account text
  uses its whole column without clipping”. Không đổi production CSS.

Matrix hiện có: Tôi 1440/1024/834/390/320 × System/sáng/tối × chữ100/200;
panel 1440/1024/834, expanded/compact, cao900/360 theo runner. Assertion mới so
content box thực, gutter16, account text không bị inset lồng, common axes, và cuối
trang mobile. Giữ Arrow/Space/Tab, selected/focus riêng, forced-colors/reduced-motion,
Escape/outside/Back/Forward, `/toi` không duplicate, storage denied, explicit
ngược OS, System theo OS, first-paint và dirty-editor checks đã có từ các PR cũ.

Các thử setup ban đầu FAIL trước prerequisite/build vì port/role scratch không
khớp và quyền npm cache; đã sửa cấu hình local và chạy lại đạt. Giữ log lần thử;
không gọi các lần đó là PASS. Không xem/chờ GitHub Actions.

## Coverage và bàn giao

Bundle local: `/Users/manhblue/Documents/Codex/2026-11-07/task-3/vink67-evidence/`:
manifest, contexts/bounds, ảnh đã mở, logs, patch/hash, navigation và REPORT.
Binary/log không commit vào repo. Self-review owner/callers/diff không phát hiện
regression có thể hành động trong scope; production source không đổi.

AC-11: recapture đúng base, phân loại các lỗi lịch sử đã sửa. AC-12: geometry
hiện đạt R2 trong scope; không tạo CSS diff không cần thiết. AC-13: ghi phép đo,
ảnh đã mở, kiểm tương tác và giới hạn đúng final content. Checklist lịch sử
AC-01–10 giữ nguyên; không đóng toàn issue từ delta này.

Đây là Web responsive và input chương trình CDP. Mobile/device thật, screen
reader, IME OS, native CMP và auth production không được nghiệm thu bởi bộ này.
Không deploy; không suy editor R1 cross-acceptance hay native acceptance từ ảnh Web.
