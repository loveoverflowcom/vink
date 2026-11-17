# Empty artwork R1 · CMP diagnostic #261

Phạm vi: `screen.explore/{empty,no-results}`, `screen.desk/{empty,no-results}` trong
`:diagnostics`; không có app native sản phẩm. Kế thừa R1 của
[#261](https://github.com/loveoverflowcom/vink/issues/261),
[Discovery #258](empty-artwork-258.md) và [Desk #259](empty-artwork-desk-259.md).
Artwork là **design candidate**; `fidelity: NO_APPROVED_REFERENCE`.

```text
screen (Mẫu, dữ liệu cố định)
  sample-notice
  heading — Khám phá / Bàn viết
  search — một controlled TextFieldValue, cùng call site qua các state/theme
  content
    decorative-artwork — Fit, 512:427, không contentDescription
    status — một câu tiếng Việt
    recovery — VinkButton, callback sample thật
  sample-result — chỉ xác nhận intent cục bộ
```

| Screen/state | Artwork | Câu trạng thái | Action |
| --- | --- | --- | --- |
| Explore/empty | no-public-works | Chưa có tác phẩm công khai. | Bắt đầu viết → sample editor intent |
| Explore/no-results | no-results | Chưa có tác phẩm phù hợp. | Xóa tìm kiếm → clear controlled query |
| Desk/empty | first-draft | Một trang dành riêng cho những điều bạn muốn viết. | Viết bản thảo đầu tiên → sample editor intent |
| Desk/no-results | no-results | Chưa có tác phẩm trong kết quả này. | Xóa tìm kiếm → clear controlled query |

Câu trạng thái trùng Web #258/#259; nhãn action là của mẫu diagnostic và còn khác Web ở
hai ô (đối soát 17/11/2026): Web Explore/empty dùng link “Đến Bàn viết”, Web Desk/no-results
dùng nút “Xem tất cả tác phẩm” (cùng ý xóa tìm kiếm). Consumer native sản phẩm sau này phải
đối chiếu lại copy với Web, không coi bảng này là copy đã chốt.

Loading/error/auth/forbidden/pending/uncertain/refresh/partial/stale không dùng artwork empty.
Không đổi RegionState, permission, lưu/gửi hay navigation sản phẩm.
Scenario cố định không tự biến thành dữ liệu thật sau callback.

Invariant: stage dùng Surface Lavender hiện hữu, không card/border/halo/gradient mới.
Slot 176dp khi content width <400dp, 240dp khi rộng hơn, tự co nếu hẹp hơn slot;
tỷ lệ 512:427 với sai số làm tròn 1px; không crop. Font multiplier 2 không phóng ảnh.
Khoảng cách dùng `VinkDimens.Space3/4`; không thêm token/font/theme.

Interaction: không auto-focus; query/selection/composition giữ owner qua region/theme.
CTA click/Enter/Space dùng VinkButton hiện hữu; clear là hành động có chủ ý, không gửi server.
Ảnh không có tên accessibility; ảnh loading/failed vẫn giữ câu và nút. Không animation mới.
Cuộn và safe drawing/IME inset do DiagnosticHost sở hữu, không thêm inset bên trong.

CHOICE-01 **A**: ba alpha WebP canonical → generated build data → closed enum → worker đơn,
fit decode không upscale, tối đa 512×427 (874.496 byte pixel ARGB8888 theo phép tính).
Đó là trần kích thước output, không phải số đo RSS/heap. Không runtime Drive/network.
Android API28+ dùng ImageDecoder software sRGB; API24–27 dùng BitmapFactory chỉ cho closed set
đã kiểm hash khi build. Skia Desktop/iOS dùng eager codec hiện hữu. Coroutine theo composition,
cancel không công bố completion cũ; background dừng attempt, foreground có thể thử lại.
Không sửa admission opaque hoặc allowlist hai content fixtures của #115/#116.

CHOICE-02 **A**: composition riêng của diagnostics, dùng primitives có sẵn; không mở rộng API
`:ui`/`:patterns`, không sửa source Web. Canonical asset/manifest chỉ có một owner ở
`apps/vink-web/assets/empty`; Gradle sinh bytes vào build output, không check-in bộ ảnh thứ hai.
Giới hạn native và kết quả thật: [evidence](../development/evidence/empty-webp-cmp-261/README.md).
