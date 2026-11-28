# Thiết kế Vink — Lavender M3 Expressive

Thư mục này giữ **đặc tả thiết kế hiện hành bằng chữ**: LayoutSpec, quy tắc trạng thái, copy và
lựa chọn đã chốt. Nó không giữ ảnh, canvas hay bằng chứng chụp màn hình.

## Nơi lưu từng loại thông tin

| Thông tin | Owner |
| --- | --- |
| Giá trị màu, chữ, shape, spacing, mật độ, focus, motion | [`packages/ui-system/shared/tokens.json`](../../packages/ui-system/shared/tokens.json); `cargo xtask ui-tokens` sinh CSS/Rust/Kotlin, `--check` bắt drift |
| Component, state, navigation contract dùng chung | [UI system](../../packages/ui-system/README.md), [components](../../packages/ui-system/shared/components.md), [states](../../packages/ui-system/shared/states.md), [navigation](../../packages/ui-system/shared/navigation.md) |
| Font tự host, logo, empty artwork | [fonts](../../apps/vink-web/assets/fonts/README.md), [brand](../../apps/vink-web/assets/brand/README.md), [empty](../../apps/vink-web/assets/empty/README.md) |
| Concept, canvas, ảnh tham chiếu, ảnh trước/sau, visual evidence | Google Drive (thư mục thiết kế Vink). Không sao chép vào Git |
| Mốc thị giác đã được duyệt để review | [Sổ mốc thị giác](visual-anchors.md) |
| Rubric review UI, vòng sửa có điểm dừng | [UX quality](../../.agents/skills/vink-ui-design/references/ux-quality.md) |
| Cách viết LayoutSpec | [vink-layout-spec](../../.agents/skills/vink-layout-spec/SKILL.md) |

Thứ tự ưu tiên: chỉ dẫn hiện tại của người dùng > quyết định mới hơn đúng phạm vi > LayoutSpec còn
hiệu lực. Ảnh tham chiếu không ghi đè quyền của server hay ngữ nghĩa lưu/transport. Khi
`tokens.json` và một tài liệu khác nhau, `tokens.json` là hiện trạng. Lavender là palette riêng của
Vink, không phải thương hiệu Cantos. Trạng thái `DESIGN_PROPOSED` và `NO_APPROVED_REFERENCE` trong
từng LayoutSpec không trở thành phê duyệt chỉ vì code đã merge.

## Nguyên tắc nền

- **Điều hướng:** Khám phá đứng trước Bàn viết (quyết định 31/10/2026). Rail desktop/tablet:
  Khám phá → Bàn viết, Tài khoản ở chân rail; thanh dưới mobile: Khám phá → Bàn viết → Tài khoản,
  thêm Thông báo khi phiên có quyền. `/` vẫn là trang công khai. Shell hiện hành theo
  [LayoutSpec R4.3 §3](redesign-r4/layout-spec.md#3-shell-rail-và-thanh-dưới) và
  [navigation contract](../../packages/ui-system/shared/navigation.md).
- **Logo:** V/ngòi bút R1 (#117), chỉ ở góc phải Khám phá; mono mask trong forced colors.
- **Theme:** Light mặc định; Dark theo `prefers-color-scheme` khi `<html>` không có `data-theme`;
  `data-theme` ghi đè hệ thống ở cả hai chiều. Thành viên đã xác nhận chỉnh màu, chữ Mặc định 100%
  / Lớn 150% và rail tại `/toi` ([Giao diện R1](me-preferences-r1-209.md)); khách theo hệ thống.
  Bootstrap trước CSS đồng bộ favicon/manifest.
- **Bề mặt:** nhóm nội dung bằng fill và khoảng cách; viền chỉ cho input, focus và đường phân
  cách. Nền trang, rail và thanh dưới dùng chung `bg-page`; thẻ rời `bg-content`, bo 20, cách 8
  (R4.2). Hướng chữ thoáng, ít khối theo [Lavender R3](lavender-r3-336/README.md).
- **Chữ:** UI Be Vietnam Pro 400/500/600; đọc, editor và tiêu đề Literata variable. Văn đọc
  `clamp(1.0625rem, 1rem + 0.25vw, 1.25rem)`, leading 1.85, measure 42rem.
- **Chuyển động:** 160/320 ms; `prefers-reduced-motion` rút chuyển tiếp hữu hạn về 0 và dừng vòng
  lặp tải.
- **Tương phản:** mọi cặp chữ/nền khai báo trong `contrast` của `tokens.json` được generator tính
  lại hai theme và bị từ chối dưới ngưỡng; đây là số học token, không phải chứng nhận WCAG của màn.
- **Chưa kiểm định kỳ:** zoom trình duyệt thật, Safari/Firefox, thiết bị thật, IME hệ điều hành,
  screen reader.

## LayoutSpec hiện hành theo bề mặt

| Bề mặt | Đặc tả |
| --- | --- |
| Bàn viết, Khám phá, rail (đầu mối) | [LayoutSpec R4.3](redesign-r4/layout-spec.md) · [HOW Bàn viết](redesign-r4/how-ban-viet.md) · [HOW Khám phá](redesign-r4/how-kham-pha.md) · [WHY R4.3](redesign-r4/why-r4.md) |
| Bàn viết: danh sách và số liệu | [phân trang server](desk-server-pagination.md) · [trạng thái và số liệu bản công khai](desk-author-feedback.md) · [Lượt xem r3](work-views.md) · [action surfaces #225](action-surfaces-desk-discovery-225.md) · [empty artwork #259](empty-artwork-desk-259.md) · [Select R1.5](select-r15.md) |
| Bàn viết: kiểm duyệt | [LayoutSpec R4.4](redesign-r4/desk-moderation.md) |
| Editor | [Editor R4](editor-r4.md) (bố cục hiện hành) · [lưu hợp nhất #217](ui-editor-unified-save-217.md) · [Markdown Live Preview, Mục lục, đổi định dạng](editor-live-preview-256-257.md) · [selector định dạng #262](editor-format-picker-262.md) · [mặt viết mobile và tiêu đề dài #193](editor-writing-reflow-193.md) · [bộ chọn thẻ](tag-selector.md) · [vùng Công khai](publication-author.md) |
| Khám phá và Reader | [đọc công khai](public-reading.md) · [trang thẻ](tag-discovery.md) · [chip lọc thẻ #260](discovery-chip-260.md) · [quay lại vị trí #236](discovery-return-position-236.md) · [Search R5.1](search-r5.md) · [empty artwork #258](empty-artwork-258.md) |
| Nhận xét | [Thảo luận hai tầng D2T-03](discussion-ui.md) · [brief gửi bình luận](discussion-submission-brief.md) · [action surfaces #226](action-surfaces-reader-discussion-226.md) |
| Thấm và Lưu | [Pen Flight R2](pen-flight-r2.md) · [Lưu bài và tab Đã lưu](saved-works.md) |
| Hồ sơ tác giả và Theo dõi | [Social r2](social-289.md) · [Người theo dõi R5](social-relationship-r5.md) · [Địa chỉ tác giả #480](profile-inline-handle-480.md) |
| Tài khoản và Auth | [Tài khoản và Chỉnh hồ sơ R5](account-profile-r5.md) · [Giao diện R1](me-preferences-r1-209.md) · [đăng nhập và mật khẩu](auth-password-web.md) · [đăng nhập trong Reader](auth-reader-web.md) · [quên mật khẩu (đề xuất)](password-recovery.md) · [xóa tài khoản](account-deletion.md) · [chữ giao diện #585](web-microcopy-585.md) |
| Thông báo | [Thông báo R4](notifications.md) |
| Điều hướng mobile | [NavigationBar R1](bottom-nav-r1-131.md) |
| Trạng thái, phản hồi, thời gian | [trạng thái và phản hồi (UIS-03)](state-and-feedback.md) · [ngày giờ](human-time.md) |
| CMP | [interaction foundation](ui-interaction-foundation.md) · [empty artwork diagnostic #261](empty-artwork-cmp-261.md) |
