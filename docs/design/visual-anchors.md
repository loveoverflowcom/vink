# Sổ mốc thị giác Vink

Sổ này ghi **mốc nào đã được chủ sản phẩm duyệt** để review giao diện. Định nghĩa ba loại mốc
(concept, style anchor, regression baseline) và quy tắc dùng nằm ở
[Chất lượng UX §2](../../.agents/skills/vink-ui-design/references/ux-quality.md#2-mốc-thị-giác-có-provenance);
trang này chỉ là sổ đăng ký, không lặp lại định nghĩa.

Lập ngày 12/11/2026 cho [#174](https://github.com/loveoverflowcom/vink/issues/174). Tại thời điểm
lập **chưa có style anchor hay regression baseline nào ở trạng thái `approved`**. Review hiện
nay dựa trên LayoutSpec, rubric và các quyết định thiết kế đã ghi; kết luận fidelity ghi
`NO_APPROVED_REFERENCE`.

## Quy tắc ghi sổ

- Một dòng là một mốc cho một phạm vi. `approved` cần lời duyệt của chủ sản phẩm về **đúng
  revision đó** (link hoặc trích nguyên văn, kèm ngày). Agent chỉ thêm dòng `candidate`.
- Trong manifest capture, mốc `approved` ứng với `references[].authority = confirmed`; mốc
  `candidate` hoặc chưa rõ giữ `candidate`/`unknown`.
- Ảnh mốc phải là render thật với font thật đã tải (không phải font fallback), ghi renderer,
  theme, cỡ chữ, constraints và SHA nguồn. Concept tạo sinh không ghi vào bảng style anchor.
- Mỗi lần thay mốc thêm dòng mới và chuyển dòng cũ sang `superseded` kèm lý do và người duyệt;
  không xóa lịch sử. Ảnh sau một vòng sửa không tự thành mốc.
- Phạm vi áp dụng ghi cả phần **không** áp dụng. Anchor của màn khác chỉ hướng dẫn nhịp và
  phân cấp, không là mục tiêu so pixel.

## Style anchor

| ID | Archetype / phạm vi | Nguồn và revision | Renderer, theme, chữ, constraints | Phải giống / được đổi | Trạng thái | Duyệt |
| --- | --- | --- | --- | --- | --- | --- |
| — | — | — | — | — | Chưa có mốc `approved` | — |

### Ứng viên chờ duyệt

Ứng viên là đề xuất của agent; **không** được dùng làm chuẩn nghiệm thu cho tới khi có lời duyệt.

| ID | Archetype / phạm vi | Nguồn và revision | Renderer, theme, chữ, constraints | Lý do đề xuất và giới hạn | Trạng thái |
| --- | --- | --- | --- | --- | --- |
| ANCHOR-CAND-01 | Dialog xác nhận (`state.dialog/default`, panel giữa khung): tiêu đề → nội dung → hàng hành động | Preview Leptos, `develop@e54db0d6f3e1b1caae901cbff31027cb3d49d757`; ảnh trong [báo cáo #174](ui-ux-quality-174.md#ảnh-đã-mở) | Chrome headless, light và dark, chữ 100%, khung 390 CSS px, Be Vietnam Pro/Literata tự host đã tải | Specimen dialog có assertion bàn phím/focus và đã được mở, review ở hai theme. Chỉ đề xuất cho **nhịp và phân cấp** của dialog xác nhận ở chữ 100%; không áp dụng cho sheet, form dài, chữ 200% hay CMP. Ảnh hiện có là bản cắt từ ảnh raw 390×6000 (F-05); trước khi duyệt cần ảnh chụp ở viewport thật | `candidate` |

## Regression baseline

| ID | Scenario, renderer, môi trường | Artifact và hash | Lý do nhận / thay | Trạng thái | Duyệt |
| --- | --- | --- | --- | --- | --- |
| — | — | — | — | Chưa có baseline `approved` | — |

Các ảnh trong `docs/design/evidence/` và các báo cáo UI trước đây là bằng chứng của lượt kiểm
tương ứng; chúng không tự là baseline so sánh. Assertion của runner (`ui-preview … verify`,
`verify web`) vẫn là oracle hồi quy hiện hành cho những gì chúng thực sự kiểm.

## Mốc đã ghi ở nơi khác

Các quyết định thiết kế đã có owner riêng vẫn giữ nguyên hiệu lực và không chuyển về đây: palette
và tương phản ở [baseline thiết kế](README.md), các hướng R5/R6/R14 trong những trang
`docs/design/*` tương ứng, logo ở [brand](brand/final-selected/README.md). Khi một trang như vậy
được chủ sản phẩm duyệt làm mốc review cho một archetype, thêm một dòng trỏ tới nó thay vì chép ảnh.
