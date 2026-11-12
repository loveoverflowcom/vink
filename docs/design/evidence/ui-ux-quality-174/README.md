# Bằng chứng pilot #174 — dialog/sheet

Kèm theo [báo cáo #174](../../ui-ux-quality-174.md). Nguồn: `develop@e54db0d6f3e1b1caae901cbff31027cb3d49d757`,
ngày 12/11/2026, dữ liệu mẫu tổng hợp. Đây là bằng chứng của một lượt pilot, **không** phải
regression baseline hay style anchor đã duyệt ([sổ mốc](../../visual-anchors.md)).

## `cmp/` — Compose Desktop off-screen (runner)

Từ `cargo xtask ui-preview cmp verify`, test `InteractionCaptureTest`; giữ nguyên tên file của
runner. 360×800, density 1, motion reduce, font Be Vietnam Pro/Literata TTF đã pin; không có cửa
sổ, inset hay bàn phím hệ điều hành. `interaction-contexts.json` là sidecar nguyên bản của cả 8
capture; 5 ảnh dưới đây được lưu, 3 ảnh còn lại đã mở nhưng không lưu.

| File | SHA-256 |
| --- | --- |
| `interaction--default--light--360--text100.png` | `2d392154655ed26de37c742bd6cc8bd7a17cf59d68c1c8de12bdf0448452cb7a` |
| `interaction--default--light--360--text200.png` | `808e7eb4feb397925333cc58da8448dd86d7078fc0833a42dde4e872613b8b85` |
| `interaction--default--dark--360--text200.png` | `d53856b1190f6a5b22587058db44a46a2742eff07404c171881f9cd8bd52ce66` |
| `interaction--sheet--light--360--text100.png` | `f34b42b82359059329199acd9b3614d967fd265e870b0662551bc7dc60ef9d79` |
| `interaction--sheet--light--360--text200.png` | `c3628039c8af4f55414a2efed4f3f90786b069617bab6e8879f90d763bf795d2` |
| `interaction-contexts.json` | `cdf37dadf58bfde750d3132969faba5c1cbd74b8efdcbbf0347d6ca334089846` |

## `web/` — preview Leptos

**Bản cắt từ ảnh runner (ảnh phái sinh).** Ảnh raw `state.dialog--default--open--{light,dark}--390.png`
(390×6000, Chrome headless 157.0.8103.78, DPR 1, chữ 100%) không lưu vào repo; bản cắt giữ nguyên
tỷ lệ 1:1, chỉ lấy dải chứa panel và 60px trên dưới.

| File | Cắt từ (SHA-256 ảnh raw) |
| --- | --- |
| `web-dialog-open-light-390-text100--crop.png` | `99230c815a474103e43370c4de3606a195f1b8908d902659a1092377a024d7a7` |
| `web-dialog-open-dark-390-text100--crop.png` | `ba170b70d2ff9e4e4b2dea5daec30614a19a8faa2b90e32aa771af20d742d5c5` |

**Thăm dò thủ công (không phải output của runner).** `cargo xtask ui-preview leptos run`, mở trong
browser pane của Claude desktop với viewport giả lập (Chrome 156, UA mobile, DPR 2), theme Sáng.
Screenshot là JPEG đã thu nhỏ của viewport, chỉ để minh họa; số đo chính xác nằm ở
[probe-measurements.json](web/probe-measurements.json). Trigger được kích hoạt bằng script nên lượt
này không dùng để kết luận về trả focus.

| File | Trạng thái |
| --- | --- |
| `probe-dialog-default-light-320x700-text200.jpg` | Dialog xác nhận, 320×700, root 32px: mở ra đã cuộn tới hành động (F-01, F-02) |
| `probe-dialog-default-light-390x844-text200.jpg` | Dialog xác nhận, 390×844, root 32px: tiêu đề bị cắt mép trên, “Xóa” dưới mép (F-01) |
| `probe-sheet-light-320x700-text100.jpg` | Sheet, 320×700, root 16px: đạt INV-1…4 |
| `probe-sheet-light-320x700-text200.jpg` | Sheet, 320×700, root 32px: đạt INV-1…4 |
