# Lavender R3 · chữ thoáng, ít khối · #336

**19/11/2026 · [#336](https://github.com/loveoverflowcom/vink/issues/336) · `DESIGN_PROPOSED`.**
Chủ sản phẩm duyệt hướng thị giác trên canvas ngày 19/11/2026. Đây là mốc thiết kế cho
lượt triển khai; trạng thái kiến trúc và mức triển khai không đổi (`NOT_IMPLEMENTED` cho R3).
Đối chiếu tại `develop@eb7e65e`. Fidelity so với ảnh tham khảo của issue: `NO_APPROVED_REFERENCE`
cho tới khi ảnh chụp từ bản dựng được duyệt.

- Canvas Claude Design (riêng tư, cần chủ sở hữu chia sẻ): <https://claude.ai/artifact/U2EGFmygrgoT4hq5dHH1SP>
- Nguồn từng khung: [canvas/](canvas/) (`.dc.html` dùng runtime `support.js` của Claude Design,
  không chạy độc lập trong repo; đọc như đặc tả markup và style).
- Token đề xuất dạng máy đọc: [tokens-r3.json](tokens-r3.json). Chủ giá trị vẫn là
  [tokens.json](../../../packages/ui-system/shared/tokens.json); áp dụng bằng `cargo xtask ui-tokens`.

## Vì sao chữ đang dày (đọc từ develop)

| # | Hiện trạng | Nguồn |
| --- | --- | --- |
| 1 | Be Vietnam Pro chỉ có 400 và 600; nhãn, tên, nút, legend, handle đều 600. `font-weight: 500` hiện rơi về 400. | `tokens.json` `fonts`, `type.label` |
| 2 | Tiêu đề Literata 600 với tracking −0.025em / −0.015em; dấu tiếng Việt chen nhau. | `type.display`, `type.headline` |
| 3 | Nhịp dòng 1.2–1.3 cho tiêu đề nhiều dòng. | `type.display`, `.vk-author-public h1` |
| 4 | Ô nhập `surface-container-low` trên `surface`: 1.07:1 sáng, 1.10:1 tối. | `field.container` |
| 5 | Thẻ Khám phá, section hồ sơ và fieldset quyền đều có nền hoặc viền. | `apps/vink-web/style/vink.css` `.vk-public-row__card`, `.vk-author-public section`, `.vk-profile-form fieldset` |
| 6 | Handle hồ sơ màu primary, nét 600, giống link. | `.vk-author-public header p:last-child` |
| 7 | `ProfileWork` chỉ có `work_id`, `title`. | `apps/vink-web/src/contract.rs` |
| 8 | Quyền đã khóa vẫn là select có lựa chọn bị tắt. | `apps/vink-web/src/pages/profile/browser.rs` `PolicyField` |

## Thang chữ R3

Mốc ở cỡ chữ mặc định, mobile / desktop. Tracking 0 cho mọi vai trò; chỉ thử âm nhẹ ở
display ≥ 40 px sau khi so với bản 0 bằng font đóng gói.

| Vai trò | Font · weight | Cỡ · nhịp dòng |
| --- | --- | --- |
| Display trang (Khám phá) | Literata 500 | 36 / 44 · 1.15 |
| Tên hồ sơ | Literata 500 (thử 400) | 30 / 40 · 1.25 |
| Nhan đề Reader | Literata 500 | 30 / 36 · 1.25 |
| Nhan đề trong danh sách | Literata 500 | 22–23 / 24 · 1.3 |
| Tiêu đề tác vụ (Chỉnh hồ sơ) | Be Vietnam Pro 600 | 28 · 1.25 |
| Tiêu đề nhóm | Be Vietnam Pro 600 | 18 · 1.4 |
| Trích đoạn | Literata 400, tối đa 3 dòng | 17 · 1.6 |
| Thân bài Reader | Literata 400 (giữ token) | 17–20 · 1.85 |
| Bio, lời bình | Be Vietnam Pro 400 | 16 · 1.65 |
| Tên tác giả, nhãn, nút, nav | Be Vietnam Pro 500 (face mới) | 15, nhãn trường 14 · 1.45 |
| Handle, thời gian, thể loại | Be Vietnam Pro 400 / 500 | 13–14 · 1.5 |

Face mới `be-vietnam-pro-500.woff2` đi theo quy trình trong
[README font](../../../apps/vink-web/assets/fonts/README.md): nguồn google/fonts đã pin,
cùng subset, OFL, `provenance.json`, Desktop TTF. Canvas dùng Google Fonts chỉ để xem trước.

## Bề mặt và tương phản

Đổi một role: `field.container` từ `surface-container-low` sang `surface-container-high`.
Tính từ hex của `tokens.json`:

| Cặp | Sáng | Tối |
| --- | --- | --- |
| Chữ chính / nền | 15.81 | 15.33 |
| Chữ phụ / nền | 6.72 | 10.99 |
| Chữ phụ / ô nhập (container-high) | 5.40 | 7.68 |
| Vòng focus / nền · / ô nhập | 5.78 · 4.64 | 10.83 · 7.57 |
| Nút chính | 6.98 | 8.16 |
| Ô nhập / nền — develop → R3 | 1.07 → 1.25 | 1.10 → 1.43 |

Không nền ô nào đạt 3:1 mà vẫn nhẹ. R3 nhận diện ô bằng nhãn thường trực, ô cao 56 px,
focus `outline` 3px lệch 2px (không dùng box-shadow), và viền hệ thống ở `forced-colors`.

## Quy tắc bề mặt

1. **Đọc và danh sách phẳng.** Thẻ tác phẩm, Bio, lời bình nằm trên nền trang; tách bằng
   khoảng cách 36–40 px. Đường kẻ chỉ dùng cho danh sách chỉ có nhan đề (hồ sơ).
2. **Tonal là chỗ thao tác.** Ô nhập, select, chip đang chọn, nhóm Phản hồi, thanh Lưu, thanh dưới.
3. **Một điểm tím mỗi vùng.** Nút chính và link điều hướng dùng primary; handle, ngày, thể loại dùng chữ phụ.
4. **Bo ở control, không bo vùng đọc.** Ô nhập 12 px, nút và search dạng pill.
5. **Byline là một link.** Avatar + tên + handle mở hồ sơ, tách khỏi link nhan đề. Ngày và thể
   loại lên dòng trên nhan đề (`Tản văn · 18 tháng 11`). Link giữa đoạn văn vẫn gạch chân.
6. **Input không viền, vẫn nhận ra.** Nhãn thường trực, nền container-high, padding 16 px.

## Khung trên canvas

| Khung | File | Ghi chú |
| --- | --- | --- |
| Spec R3 | [Main](canvas/Main.dc.html) | Đối chiếu, thang chữ, tracking/weight, tương phản, quy tắc |
| 05/06 Khám phá sáng/tối | [KhamPha](canvas/KhamPha.dc.html), [KhamPhaToi](canvas/KhamPhaToi.dc.html) | Search + Lọc thẻ như develop; hai “Hải Đường Nhỏ” phân biệt bằng Handle |
| 01/02 Chỉnh hồ sơ sáng/tối | [ChinhHoSo](canvas/ChinhHoSo.dc.html), [ChinhHoSoToi](canvas/ChinhHoSoToi.dc.html) | Thứ tự trường của develop, quyền cạnh giá trị, quyền khóa dạng đọc, Địa chỉ tác giả là hàng điều hướng, “Về Tôi” |
| 04 Hồ sơ mobile tên dài | [HoSo](canvas/HoSo.dc.html), [HoSoToi](canvas/HoSoToi.dc.html) | Người khác xem: Theo dõi; tác phẩm chỉ nhan đề |
| 07 Reader và nhận xét | [Reader](canvas/Reader.dc.html), [ReaderToi](canvas/ReaderToi.dc.html) | Nhóm Phản hồi tonal, “Đang trả lời … · Đến bình luận” |
| 03 Hồ sơ desktop | [HoSoDesktop](canvas/HoSoDesktop.dc.html) | Rail 220 px, chính chủ: Chỉnh hồ sơ; minh họa focus |
| AC-04 320 px / chữ 200% | [Hep320](canvas/Hep320.dc.html) | Minh họa; thanh Lưu vào luồng khi chữ lớn |

## Ngoài phạm vi hoặc cần chủ sản phẩm chốt

- Hàng chip “Tất cả / Tản văn / Thơ” của ảnh tham khảo chưa có ở develop; là đề xuất IA riêng.
- Trích đoạn, thể loại, ngày trên danh sách hồ sơ cần mở rộng `ProfileWork` (#336 loại trừ đổi DTO).
- `SOC-OPEN-01` (công khai lại) và `SOC-OPEN-04` (tải ảnh đại diện) vẫn `OPEN`.
- Khung 320 px/200% không phải bằng chứng AC-04; cần đo trên Chrome theo SHA sau khi triển khai.

## Lưu ý khi triển khai

Đổi `type.*` dùng chung ảnh hưởng mọi consumer (Bàn viết, dialog Auth, preview UI system,
CMP qua Kotlin sinh ra). Liệt kê consumer và chụp lại trước/sau ở 100% và 200%, sáng/tối,
theo [UX quality](../../../.agents/skills/vink-ui-design/references/ux-quality.md).
