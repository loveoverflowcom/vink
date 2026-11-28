# Lượt xem trong Bàn viết — LayoutSpec r3 (#514 · #323)

`DESIGN_PROPOSED · r3 (Work Item Visual Polish R2, 25/11/2026) · fidelity: NO_APPROVED_REFERENCE`.
Nghiệp vụ nằm ở [đặc tả Lượt xem](../architecture/work-views.md) và [PRD §17](../../prd/requirements.md#17-lượt-xem-tác-phẩm-work-views); mức triển khai và bằng chứng nằm ở [registry](../development/work-views.md).
Tài liệu này kế thừa [LayoutSpec R4.3](redesign-r4/layout-spec.md#53-thẻ-tác-phẩm) và [R4.4 Kiểm duyệt](redesign-r4/desk-moderation.md), cập nhật theo quyết định ngày 25/11/2026 của chủ sản phẩm (`D-01`, `D-02`, `UX-01..10`, `OQ-1..4`) và gói thiết kế **Work Views r3** ([thư mục Drive](https://drive.google.com/drive/folders/1QtovsyJfBO-98GJXGb_2ysR4Wa6trd_3), [DESIGN_SPEC](https://drive.google.com/file/d/1Tc7ONm1muiggBp77kOJx7olXYpirM69i/view), [LAYOUT_SPEC](https://drive.google.com/file/d/1L8PujbF-rppu082YqHQJuAOVR5-F7kO4/view); bản r1 và r2 nằm trong thư mục archive tương ứng và không còn là chuẩn triển khai).

## 1. Ý định và ranh giới

- **Người dùng và tác vụ:** tác giả mở `/ban-viet` để tìm và mở bản thảo tiếp tục viết, đồng thời quét nhanh số **Lượt xem** riêng tư cạnh **Thấm** và **Nhận xét** mà không chuyển Bàn viết thành dashboard hay thêm biểu đồ.
- **Điểm nhấn:** nhan đề tác phẩm (Literata 22/500) và thao tác mở bản thảo của hàng. Lượt xem là số liệu phụ đặt cuối cụm số liệu.
- **Quyền riêng tư:** số liệu chỉ đọc từ `GET /api/v1/author/works` (`no-store`) sau khi phiên tác giả được xác nhận; không bao giờ xuất hiện trong HTML SSR, Reader công khai, Khám phá, hồ sơ hay cache công khai.

## 2. Cây vùng (phương án B — giải thích một lần)

```text
Screen #desk — Bàn viết (/ban-viet, phiên tác giả đã xác nhận)
  Column #page — trục 48rem (size.page-column), gutter 16px dưới 760px
    Region #heading — H1 “Bàn viết” · Tìm · Định dạng · Tạo bản thảo
    Row #status (.vk-draft-controls)
      Row #status-left — “Hiển thị N tác phẩm” (#draft-scope) · Nút chữ “Cách tính lượt xem” (#views-explainer-trigger)
      Region #draft-tools — Select Sắp xếp
    List #draft-results — gap 8px (space.card-gap), mỗi WorkId một Card #draft-row-{WorkId}
      Card (.vk-author-work, bg-content, radius.card 20px, hover tonal color-mix 7% primary trên bg-content)
        Link #title (.vk-draft-row__link, padding 16px 20px, gap 8px) — mở /ban-viet/{WorkId} (khi không có draft: khối tĩnh, không tạo link giả)
          Text #title — Literata 22/500, wrap-anywhere (untitled: italic on-surface-variant)
          Row #state (.vk-author-work__status, gap 8px 10px) — tối đa 3 nhãn trạng thái (tonal chọn lọc + icon 18px) · thời gian cập nhật
          Row #metrics (.vk-author-work__metrics, cách #state 12px, gap 4px 20px, borderless, non-interactive)
            [Thấm: pen-flight 22px + số Việt] (chỉ khi current_public)
            [Nhận xét: chat-bubble-outline 20px + số Việt] (chỉ khi current_public)
            [Item #views: visibility-outline 20px + số Việt hoặc “—” + “Tính từ dd/mm/yyyy” hoặc lý do] (Lượt xem luôn đứng cuối)
          Row #context — “Bản công khai: …” (nếu khác tên nháp) / “Không có bản nháp để sửa”
        Region #review-action (.vk-desk-review-action) — “Xem kiểm duyệt” (ButtonVariant::Tonal, icon verified-user, 48px, radius.control-48)
    Region #continuation & ghi chú cuối trang (.vk-desk-privacy)
  Overlay #views-explainer — DialogKind::Sheet (Desktop căn giữa; dưới 760px neo đáy)
```

- **D-01 (đã chốt 25/11):** thứ tự DOM và thị giác cố định là **Thấm → Nhận xét → Lượt xem** cho mọi metric được phép hiển thị.
- **D-02 (đã chốt 25/11):** bỏ hẳn nút/link “Đọc” / “Đọc bản công khai” trong hàng tác phẩm Bàn viết (không nút, icon, menu, vùng giữ chỗ hay tab stop thay thế; bỏ `AuthorWorkPublic.href` và CSS `.vk-author-work__reader`). Giữ link mở bản thảo của hàng/nhan đề và giữ nguyên Reader công khai.

## 3. Quy tắc trình bày r3 và token

Không thêm token màu mới vào `tokens.json`; chỉ thêm một icon chuẩn Material Icons Outlined `visibility-outline` (`src/action/visibility/materialiconsoutlined/24px.svg`, đã kiểm SHA-256 trong `packages/ui-system/shared/icons.json`).

| Thành phần | Quy ước r3 triển khai |
| --- | --- |
| Thẻ hàng | `bg-content`, `radius.card` (20px), khe giữa thẻ 8px; padding 16px trên/dưới, 20px hai bên (`space.4`, `space.4 + space.1`); gap tiêu đề → trạng thái 8px, trạng thái → số liệu 12px; hover tonal `color-mix(in srgb, var(--vk-bg-content), var(--vk-primary) 7%)`; không viền/bóng/gradient |
| Nhãn trạng thái | Icon 18px + chữ `13/500`, `radius.sm` (8px), padding `3px 10px 3px 8px`. Tone chọn lọc: **ok** (“Đã đăng”) `success-container`/`on-success-container`; **attn** (“Bản mới chờ duyệt”, “Bản mới cần người duyệt”) `secondary-container`/`on-secondary-container`; **bad** (“Bản mới không được duyệt”, “Đã ẩn”, “Đã gỡ”, “Đã xóa”) `error-container`/`on-error-container`; **quiet** (“Nháp”, “Chỉ mình tôi”, “Chưa có bản công khai”, “Có bản sửa”, “Chưa rõ”, “Chưa rõ bản mới”) không nền, `on-surface-variant`. Luôn có icon + chữ |
| Số liệu | `type.label` 14/500 + `tabular-nums`, định dạng dấu chấm hàng nghìn kiểu Việt (`1.234`, `12.345`, `1.234.567`), icon `on-surface-variant` (20px cho nhận xét & `visibility-outline`, 22px cho `pen-flight`), gap icon–số 6px, giữa mục 20px |
| Xem kiểm duyệt | `ButtonVariant::Tonal`, 48px, `radius.control-48`, icon `verified-user` (`secondary-container`/`on-secondary-container`), giữ đích và quyền R4.4/#477 |
| Giải thích (`#views-explainer`) | Nút chữ `#views-explainer-trigger` (icon `info`, `primary`, `radius.control-40`, cao ≥ 44px) ở dòng `#status`, mở `DialogKind::Sheet` (CHOICE-UI-01: tái dùng primitive `Dialog` có sẵn với `focus_title`, nền `inert`, Esc/backdrop/Đóng trả focus về trigger). Câu giải thích đọc ngưỡng động từ `vink_work_view_policy::WorkViewDurationPolicy::DEFAULT` (mặc định `5 giây`, không hardcode lần hai) |

## 4. Ánh xạ `views.state`

| `views.state` | `current_public` | Hiển thị trong `#views` | Tên truy cập / ghi chú |
| --- | --- | --- | --- |
| `counted` (`count ≥ 0`) | có | `visibility-outline` + số Việt (`0` thật khi `count = 0`); kèm `Tính từ dd/mm/yyyy` (`Asia/Ho_Chi_Minh`) khi `counted_since != null` | `{n} lượt xem` (thêm ngày khi có `counted_since`) |
| `counted` (`count ≥ 0`) | không (`Chỉ mình tôi`, `Đã ẩn`, `Đã gỡ`) | Số cũ màu `on-surface-variant`; không hiện Thấm/Nhận xét (`OQ-1 = A`, `D-06`) | `{n} lượt xem` (số cũ không tăng) |
| `never_public` | — | Ẩn ô khỏi hàng số liệu (`D-03`), không hiện `0` | Chữ ẩn thị giác trong hàng: “Lượt xem: chưa có số vì tác phẩm chưa từng công khai” |
| `not_started` | — | `visibility-outline` + `—` + caption `Chưa bắt đầu thống kê` | “Lượt xem: chưa bắt đầu thống kê” |
| `unavailable` / trạng thái lạ / thiếu field | — | `visibility-outline` + `—` + caption `Chưa tải được` | “Lượt xem: chưa tải được” |

## 5. Quyết định còn `OPEN` (không tự chốt trong PR này)

- **`DQ-05` (định dạng giờ trên hàng, `OPEN` trong #410).** Bàn viết áp dụng [chính sách hiển thị Web #619](human-time.md) cho `updated_at`. Mốc báo cáo `counted_since` tiếp tục dùng ngày Hồ Chí Minh theo VIEW-09/12.
- **`DQ-10` (màu tertiary cho “Chờ duyệt” — `OPEN`):** tuân thủ r3 dùng `secondary-container`/`on-secondary-container`, không dùng `tertiary`.
- **`D-03..D-08` & `CHOICE-UI-01` (Sheet thay popover cho `UX-07`):** giữ trạng thái `DESIGN_PROPOSED`, chờ chủ sản phẩm xác nhận nếu muốn điều chỉnh.
- **`VDR-WORK-VIEWS@r2`:** giữ trạng thái `PENDING` theo `VDR-DECISION-SIGNOFF@r1` cho tới khi có signoff trích đúng payload/SHA-256.
