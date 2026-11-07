# #66 — Hình học hàng bản thảo dài

Ngày 07/11/2026. Base `develop@5ca574c41419323d628e43aca9c2e2780fafacb0`,
đã chứa PR #89 của #88. Nhánh `codex/desk-responsive-row-geometry`.
Phạm vi: CAP-66-01/02 và AC-11–AC-13 của [issue #66](https://github.com/loveoverflowcom/vink/issues/66).

## Nguồn và tái hiện

Đã đọc issue/comments, source mới và [LayoutSpec R2, phần R1 còn hiệu lực](https://drive.google.com/file/d/1ChFchoWsrc9XFtkW5BvP5XUfr6GAAbKy/view).
Đã mở [R1 Desk board](https://drive.google.com/file/d/1WWXBr89aNk1Rp5TLGLsiotKgBsBZ_KMl/view)
và [ảnh CAP-66-01 lịch sử](https://drive.google.com/file/d/18x7xLIxIldQEav1LY1Iu-LVmeW9LfcMb/view).
Board là thiết kế; ảnh lịch sử không được dùng làm current-runtime.

Before thực chạy trên base trên, production source sạch; dirty input chỉ bổ sung
phase/geometry vào browser runner. After chạy cùng base với diff production ở
`packages/vink-patterns/leptos/patterns.css` và `apps/vink-web/style/vink.css`,
cùng regression mới trong `apps/vink-web/e2e/browser-smoke.mjs`.
SHA trong contexts là base; không phải tuyên bố checkout after sạch.

Fixture giữ nguyên `draftDashboard`: 100 summary có nhãn `[Fixture]`, title đầu
NFD dài, cùng work ID/version/timestamp; không đổi seed cho ảnh đẹp. Chrome 156
headless trên macOS, DPR 1, viewport cao 900 CSS px, root text 100/200%, sáng/tối.
Chờ hydrate, font và animation hữu hạn theo runner hiện có. Đo content box,
không bù scrollbar 15px bằng margin lệch.

## LayoutSpec và lựa chọn

`heading → resume → list heading → search/sort/reload → scope/count → recent rows`.
Mỗi row tiếp tục là **một link**: title đầy đủ → metadata → affordance mở.
Chọn **CHOICE-04 hướng giữ toàn tên/reflow**: ở inner measure ≤30rem, ẩn mark
trang trí, text dùng trọn content width, “Mở bản thảo” cùng arrow xuống hàng dưới.
Ngưỡng rem theo cỡ chữ và container thật, dùng cả trong gallery. Không thêm clamp,
disclosure hoặc nested interactive. Bản rộng giữ anatomy ba cột.

Bỏ riêng padding ngang của `ul.vk-draft-rows`; `DraftRow` giữ chủ inset ngang.
Padding dọc, type tokens, semantics, full accessible name, DTO và thứ tự không đổi.
Ở mobile, text row và text resume cùng trục; outer heading/resume/tools/list vẫn
cùng trục. Đánh đổi: tên cực dài ở 200% vẫn cao, đọc bằng cuộn tự nhiên.

## Số đo cùng ca trước–sau

Đơn vị CSS px; số dòng đo bằng text Range. Font title 22px ở 100%, 44px ở 200%
trong cả hai lần.

| Ca | Rộng title trước → sau | Dòng trước → sau | Cao row trước → sau |
| --- | --- | --- | --- |
| 390×900, sáng, 100% | 219 → 311 | 10 → 7 | 359,53125 → 276,953125 |
| 320×900, tối, 200% | 145 → 177 | 34 → 26 | 2248,34375 → 1814,84375 |
| 834×900, sáng, 100% | 427,578125 → 443,578125 | 5 → 5 | 191,765625 → 191,765625 |
| 1440×900, sáng, 100% | 792,578125 → 808,578125 | 3 → 3 | 134,578125 → 134,578125 |

Ở 390/100: title rộng hơn 42%, row thấp hơn 23%; meta một dòng đầy đủ.
Outer heading/resume/tools/list x=16; title row chuyển x=84→32, bằng title resume.
Search x=16/w343, sort x=16/w251,375, reload x=279,375/w79,625; sort và reload
cùng đáy. Ở 1440/100: outer x=310,5, search/sort/reload cùng đáy 113,546875.
Ở 320/200: outer x=32, search/sort/reload xếp dọc, không ép cùng hàng.

CAP-66-02: current R1 tonal fields và các trục controls đã đúng; không sửa lại
Khám phá hay thêm wrapper/padding. Explore giữ measure riêng main/sidebar;
regression native GET `q`, no-JS Enter/clear và reader-return vẫn chạy.

## Bằng chứng và kiểm local

Local bundle: `/Users/manhblue/Documents/Codex/2026-11-07/task-2/vink66-evidence/`.
`manifest.json` pin hashes của inputs/ảnh đã mở, contexts, geometry và logs.
Binary/log giữ local, không commit vào repo. Các cặp PNG đã mở:

- `desk-list-390-100-light`, `desk-list-1440-100-light`, `desk-list-834-100-light`.
- `desk-row-start-320-200-dark`, `desk-row-end-320-200-dark`: cuộn riêng đầu/cuối
  của cùng row, metadata/action đặt trong viewport thật.
- `explore-390-100-light`, `explore-1440-100-light`.

Cwd cho các lane: `/Users/manhblue/Documents/Codex/2026-11-07/task/vink`.
Dùng `CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0`
để tái sử dụng target gọn. PostgreSQL 17 scratch riêng trên loopback; không dùng DB
production. Lệnh chính: `cargo xtask verify web`, `cargo xtask verify quick`,
`cargo xtask ui-preview leptos verify` theo [guide](../development/README.md).
Kết quả cuối (logs trong manifest; không dùng GitHub CI làm gate):

| Kiểm | Kết quả |
| --- | --- |
| `cargo xtask verify web`, before và after | PASS, gồm PostgreSQL scratch, Chrome, restart/export/endorsement |
| `cargo xtask verify quick` | PASS toàn bộ các bước |
| `cargo xtask ui-preview leptos verify` | PASS, 774 captures; không phải 774 ảnh đã mở |
| Negative control trên CSS trước sửa | EXPECTED FAIL đúng assertion full inner measure 390/100 |
| Diff/relative links sau khi thêm báo cáo | PASS |

Self-review diff/callers: không phát hiện regression có thể hành động trong scope.
Đã mở thêm specimen gallery `screen.desk/long`, sáng, 834/100 trên build mới;
đây là composition mẫu, không phải màn product hay native evidence.

Regression mới kiểm title Range không clip/thu gọn, full NFD trong AX link name,
metadata và href, một target, inner measure đầy đủ khi hẹp, affordance đọc được,
trục controls ở 1440/1024/834/390/320 × hai theme × text100/200. Mở cùng **nháp
PostgreSQL thật** bằng Enter rồi CDP touch, quay lại giữ title/version. Giữ các
ca resume disclosure, search/sort, create pending/uncertain/reconciliation,
editor save/conflict/leave, no-JS GET và reader-return của lane Web.

Negative control: assertion mới chạy trên CSS base **FAIL đúng** ở
`light 390px / 100%: recent title uses the whole inner measure...`; after phải PASS.
Build thử đầu ở checkout khác FAIL do hết đĩa; các thử setup DB/output thư mục
đã thất bại trước khi cấu hình scratch riêng đúng. Giữ lỗi và prerequisite trong
bundle; không gọi chúng là PASS. Full baseline Web sau sửa môi trường PASS.

## Giới hạn và checklist

AC-11: replay đúng version/dirty/fixture, tách controls đã sửa với residual row.
AC-12: đo được cải thiện, giữ full title/AX/navigation/touch/keyboard, không giảm
font và không thay cơ chế resume. AC-13: một chủ inset ngang, axes/reflow có số
đo, ảnh cùng ca đã mở và checks đúng owner.

Đây là Web responsive và touch/keyboard mô phỏng CDP. Không suy native CMP,
thiết bị mobile, IME OS, screen reader hoặc auth production đã nghiệm thu.
Các AC lịch sử ghép Web/CMP/native chỉ tick khi có đủ evidence tương ứng;
không đóng toàn #66 chỉ từ delta này. Không deploy.
