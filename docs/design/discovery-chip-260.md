# Checkbox-chip Bộ lọc thẻ — LayoutSpec #260 R1

Nguồn: develop `abf5c9768c8fdaf56ddbd189c16d0452aa7cd856`, FilterForm tại
`apps/vink-web/src/pages/discovery.rs`, app recipe tại `apps/vink-web/style/vink.css`.
Phạm vi UX-01/02: sửa căn tâm ở consumer, giữ native Checkbox và ngữ nghĩa discovery.
Không có reference mới được duyệt; ảnh issue là regression baseline lịch sử.
Hai lựa chọn dưới đây là quyết định kỹ thuật trong phạm vi sửa, không đổi quyết định sản phẩm.

```text
Region #tag-filter — dock hoặc dialog hiện hữu
  Column #heading — Bộ lọc thẻ / Khớp tất cả thẻ · Tối đa 4
  Fieldset #form — Dạng bài / grid hai cột
  Fieldset #theme — Chủ đề / grid hai cột
    Checkbox #choice — mark cố định / nhãn ở tâm / slot đối xứng
  Region #apply — Áp dụng, Filled khi draft khác applied / Tonal khi bằng nhau
```

**CHOICE-01 A:** grid ba track `1em minmax(0,1fr) 1em`, gap token `--vk-space-1`;
`::before` giữ dấu chọn, `::after` rỗng giữ cân xứng. Chỉ visibility của mark thay đổi.
Nhãn nằm ở track giữa, text-align center, min-width 0 và wrap hiện hữu. Không đổi
padding/font-weight theo checked. CSS alternative text rỗng giữ mark trang trí ngoài
accessible name; native input/label vẫn sở hữu checked/name/value/disabled/GET.

**CHOICE-02 A:** giữ hai cột và breakpoint dock/dialog hiện hữu: delta nhỏ đáp ứng lỗi
căn tâm; intrinsic-width/wrap sẽ thay nhịp panel ngoài nhu cầu bản sửa. Màn hẹp/tăng
chữ tiếp tục dialog, group collapse theo container hiện hữu, cuộn dọc tới lựa chọn/Apply.
Không giảm target 3rem (48 CSS px ở text100), không thêm vùng chạm mở rộng.

Native pointer hoặc Space đổi draft; Apply commit GET; Enter theo native form;
Escape đóng dialog không áp dụng, giữ draft và trả focus theo controller hiện hữu. Hai theme dùng cùng anatomy,
selected có glyph và màu; forced-colors/reduced-motion giữ cue/focus. Giữ thứ tự đọc.

Invariant: tâm hộp nhãn và tâm text range lệch <=1 CSS px so với visual target; rect target/label và
vị trí hàng đổi <=1 CSS px khi toggle trong cùng layout/scale. Đo tọa độ trong form
để không nhầm scroll do keyboard với layout jump. Text ngắn/dài NFC/NFD không clip,
mark không đè chữ; target >=48 CSS px và không overlap. Chữ200 là root font-size,
không browser zoom hay Dynamic Type. Không đổi domain/API/Quint semantics.

[Kết quả thực thi, ảnh và giới hạn](../development/evidence/discovery-chip-260/README.md).
