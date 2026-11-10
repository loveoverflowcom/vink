# Select Web R1.5 — bằng chứng local #135

Base `84e635f` (develop sau #140), branch `codex/desktop-select-chevron`.
[LayoutSpec và owners](../design/select-r15.md). Ngày 10/11/2026 UTC,
Mac, Rust 1.99.1, Node 22.15.0, Chrome 157.0.8103.40, PostgreSQL 17.11
UTF8 riêng trên loopback. App dùng LocalFixture với dữ liệu mẫu; gallery không
có service. Không có thay đổi ở Editor/TagSelector hoặc CMP.

## Nguyên nhân và phép đo

Chọn CHOICE-01 A: app và gallery cùng lệch recipe shared Select. Ở 100%, inset
12px/gap 8px và tâm dọc vốn đúng, nhưng icon 24px thay vì 18px. Các spacing rem
đã tăng thành 24px/16px ở chữ200 và ăn vùng nhãn. Sửa shared CSS theo HTML R1.5,
không thêm layout override Bàn viết. Typography dùng label tokens 0.875rem/600;
font vẫn tăng theo root text scale, icon/inset/gap giữ cố định.

| Cùng app fixture, 320px/chữ200 | Trước | Sau |
| --- | --- | --- |
| Icon / inset / gap (CSS px) | 24 / 24 / 16 | 18 / 12 / 8 |
| Lệch tâm dọc | 0 | 0 |
| Vùng nhãn | 145px | 191px |
| Chữ “Mới cập nhật” (canvas) | 203.65px, bị cắt | 181.78px, đủ chỗ |

`selectChevron` ở app và gallery dùng chung helper đo DOM bounds + canvas font
resolve cho option native (không có DOM Range cho text của select). Matrix:
320/390/1440 × light/dark × chữ100/200. App thử cả “Mới cập nhật” và “Tên A–Z”;
gallery thử “Văn xuôi”, disabled option “Kịch”, native disabled/invalid + error
association; forced-colors giữ icon và focus. Native keyboard là type-ahead qua
CDP: t/m ở app, v/k ở gallery; tách prefix qua buffer 1 giây của Chromium.
Arrow navigation trong popup OS không được chứng minh bởi lớp này.

Khi mở ảnh forced-colors, chevron còn nhạt dù phép đo visibility đã qua. Delta
cuối đặt `ButtonText` cho enabled/invalid và `GrayText` cho disabled. Sau delta
chỉ đổi màu này, chạy lại toàn matrix Select app và gallery tập trung: PASS;
assert màu trùng system color và mở lại ảnh forced-colors. Hai lane web/gallery đầy đủ bên
dưới chạy trước delta màu; không gán các lượt đó thành kiểm lại toàn app sau delta.
Canvas dùng font longhands để không rơi về 10px mặc định khi shorthand rỗng.


## Kiểm tra

- PASS — regression tập trung app `--phase select-chevron` (12 contexts + forced-colors).
- PASS — cùng regression gallery (12 contexts + native states), chạy trực tiếp trên bundle.
- PASS — `cargo xtask verify quick` (lint, doctests, unit/contract và repo guards).
- PASS — `cargo xtask verify web` (build, SSR/privacy, hydration, editor/save/conflict, restart, discovery, endorsement và matrix Select).
- PASS — `target/debug/xtask ui-preview leptos verify` (cùng lane của `cargo xtask`, tái dùng binary đã build): 796 capture contexts, reflow, native fields và không lỗi console.
- PASS — JS syntax, diff whitespace, relative links và tự review phạm vi/owner; không còn finding trong scope.

Ảnh app baseline và sau sửa ở 320/chữ200 sáng, cùng focus/state đã mở; gallery baseline/after
viewport đúng specimen cũng đã mở. Baseline gallery dùng CSS nguyên byte từ
`84e635f`; thêm bộ cùng option “Văn xuôi” để so đúng state. Bộ app trước/sau cùng một nháp mẫu; thêm replay CSS base nguyên byte để giữ
focus bàn phím giống sau sửa. Bộ web cuối có fixture riêng và được giữ riêng,
không dùng nó làm before/after của nháp mẫu. Evidence local có PNG, logs và
manifest hash tại `/Users/manhblue/Documents/Codex/2026-11-10/task-6/evidence/vink-135`.
Không coi ảnh
prototype Drive là production, không gọi toàn bộ capture gallery là đã review.

Lượt đầu FAIL do thiếu biến database/Chrome, sau đó cluster SQL_ASCII thiếu
collation ICU, rồi hết đĩa khi giữ đồng thời các graph build. Đã dùng PostgreSQL
UTF8 riêng, chỉ rõ Chrome Mac và dọn cache build do lượt này tạo. Native arrow
probe không đổi selection trên Chrome Mac qua CDP; dùng type-ahead thực đã đo.
Không chuyển các lượt FAIL thành PASS; kết quả cuối ghi riêng bên trên.

## Giới hạn

Chưa chạy Safari/Firefox, zoom thật (text200 là root font CSS), bàn phím/IME hệ
điều hành, VoiceOver/screen reader, thiết bị/native runtime. `verify postgres`,
Quint, Kani và CMP NOT_RUN vì không đổi domain/store/native. Không kiểm/chờ GitHub CI;
không deploy. Issue vẫn mở cho nghiệm thu còn lại.
