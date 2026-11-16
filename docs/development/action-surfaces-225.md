# Action surfaces Bàn viết / Khám phá — #225 r2

Ngày 16/11/2026; base `6e07806aa87145d0e053f4061f47ccaf3830c14a`.
[LayoutSpec r2](../design/action-surfaces-desk-discovery-225.md),
[issue #225](https://github.com/loveoverflowcom/vink/issues/225),
[baseline chuẩn bị #240](evidence/action-surfaces-225/README.md).

## Phạm vi và trạng thái

Implementation `web`: tạo nháp Filled + Add (kể cả query miss), first-use vẫn một CTA trong
empty panel; Refresh Tonal opt-in; launcher Thẻ Tonal + FilterList. Helper “Khớp tất cả thẻ ·
Tối đa 4” luôn hiện và mô tả form; “Trang này: N tác phẩm” là live status của đúng trang, kể cả 0
và trang cuối. Error/stale/refresh và hành vi Apply/GET vẫn do owner hiện hữu quyết định.

CHOICE-01/02/03 chọn A trong scope kỹ thuật của issue; thiết kế vẫn `DESIGN_PROPOSED`,
`NO_APPROVED_REFERENCE`, không đổi quyết định sản phẩm hoặc authority. Không đổi Editor,
Timestamp, row metrics, Pen Flight, API/DB/cache/auth hoặc thêm model Quint.

Registry `shared/icons.json` thêm FilterList tại pin Material Icons
`10ecc14790612ca8fd164e3f4ffe9c16fc50d5b7`, Apache-2.0; SHA-256 SVG upstream
`da5f9ac466b2a296bdbbcee8eef2eaecf992207e83b8304451701e2e13a2bc0f`.
Generator sinh Web/CMP từ một nguồn; không đổi token/primitive/CSS. CMP không có consumer mới.

## Owner và kiểm chứng

| Claim | Owner | Kiểm tra |
| --- | --- | --- |
| Một create target có nền/Add; Refresh có nền, giữ guard | `desk.rs::CreateDraftButton` / `DraftListView` | `draftDashboard`: matrix, pending keys, uncertain/reconcile, no duplicate POST |
| Launcher có icon lọc đúng nghĩa, tên/target giữ nguyên | `discovery.rs::DiscoveryChrome`, icon registry | `discoveryReading`: glyph trang trí, nhãn/target, Escape/focus |
| AND/max4 luôn có trước/khi chọn và khi catalog lỗi | `FilterForm` | 0/1/4 thẻ, thử thẻ thứ 5, bỏ chọn, catalog 503, no-JS GET |
| Count chỉ nói trang, một live owner | `PublicResults` | SSR, trang rỗng và trang cuối 2 mục; không giấu dưới ngưỡng 12 |
| Apply, URL và history giữ nghĩa | owner GET/discovery hiện hữu | unchanged/changed Apply, q/tag/cursor, Back/Forward/reload, reader return |

Lượt kiểm r2 và bảng năm chiều được bổ sung sau khi chạy trên source đã chốt. Chưa dùng
baseline PASS của #240 để nhận delta UI PASS. OS IME/VoiceOver và native runtime: `NOT_RUN`.
