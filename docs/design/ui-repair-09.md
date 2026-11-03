# UI-REPAIR-09 — Rail riêng và scaffold theo frame

Ngày 03/11/2026. Nhánh `codex/ui-navigation-specimen-fit`, nền `origin/develop`
`2829cea` sau PR #51/UI-REPAIR-08. Phạm vi UX-01 là qualification preview và reflow
nhãn rail; không thay NavLayout policy, route, key preference, dirty guard hay API.
Không triển khai native product navigation.

## Xác nhận ở HEAD trước patch

| Finding | Source và tái hiện | Kết luận |
| --- | --- | --- |
| F09-01 | Chrome 156.0.8078.73/Linux, viewport390, text200: frame304.8, rail220, main82.8 CSS px; đã mở raw và focus tile. Cột trang tách gần từng ký tự | CONFIRMED |
| F09-02 | `Frame::default` chọn Expanded, `navigation.rail` gọi `shell` dựng AdaptiveScaffold với quiet page; container hẹp không đổi class cố định | CONFIRMED |
| Counterpart CMP | `Shell` dùng window cố định, rail specimen dựng cả scaffold; baseline common/Desktop verify PASS, giữ capture off-screen riêng | SOURCE_CONFIRMED; không suy từ CSS px sang dp |
| App thật | Owner `AdaptiveShell` khác recipe; không suy lỗi breakpoint production từ finding preview | Không đổi policy app; lane web kiểm regression |

Nguồn lịch sử: [UI-REPAIR-06/07](ui-repair-06-07.md), PR #48; chúng không là ảnh
baseline của run này. Baseline được chụp lại trên `2829cea`, fixture catalog synthetic.
Regression mới thất bại trên bundle trước sửa ở assertion rail-only không có
`nav-main`/`nav-shell`; không dùng ảnh cũ hay probe HTML độc lập để chứng minh fix.

## LayoutSpec đã trình bày trước patch

```text
#navigation-rail-specimen
  mô tả variant + route/metadata ngoài rail
  rail độc lập: default220, compact96, giữ toggle và chân Tôi
#navigation-scaffold-specimen
  mô tả requested/effective/frame + đơn vị
  fixed: canvas đủ rộng trong vùng cuộn ngang nhận focus
  adaptive / adaptive-editor: đo frame → policy hiện có
  page/input và overlay: giữ instance, state ngoài quyết định layout
#capture
  raw document-capped + focus/tile nguyên scale, dùng helper hiện có
  ghi source fingerprint, font, theme, DPR, frame và mode
```

Các scenario cũ giữ ID/mode, gắn nhãn ép cố định. Canvas Web tối thiểu 760/1100 CSS
px và CMP600/840 dp theo owner WindowClass; không thu nhỏ UI để nhét frame.
`adaptive`/`adaptive-editor` là hai scenario mới, catalog sinh lại bằng
`cargo xtask ui-tokens`. `default` của rail vẫn220. Không remount input để đổi mode.

Baseline xác nhận thêm rail220/text200 có thể tách từ “Khám”. CSS primitive reflow
icon lên trên nhãn khi bề rộng theo rem không đủ, giữ220/cỡ chữ. CMP không áp query
CSS vì padding và constraints dùng dp. Stress `long-labels` ở rail96 vẫn có nhãn
hạn chế rõ và toggle mở220; không gọi đó là PASS usability đầy đủ.

Observer frame có cleanup; hidden/zero-width mount bắt đầu Compact rồi đo lại khi
hiện. Dialog adaptive giữ mở qua resize; đóng khi trigger bị ẩn trả focus main
với preventScroll. Đây là host mẫu, không đổi policy overlay của app production.

## Bằng chứng và kết quả

Kết quả cuối và ảnh đại diện sẽ được ghi trong bundle evidence của PR này.
