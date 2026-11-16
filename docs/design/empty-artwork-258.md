# Empty artwork Web — R1, #258

Hướng theo [issue #258](https://github.com/loveoverflowcom/vink/issues/258) và ZIP đã pin trong
[asset owner](../../apps/vink-web/assets/empty/README.md); trình bày trước code ngày 16/11/2026.
Baseline `develop@f1b46703ae3329ee7cd11fb06db3f8157a31e5b9`.
Scope UX-01/02, TAG-PR-06. Artwork là concept/design candidate, **NO_APPROVED_REFERENCE**.
Không thay PRD, lifecycle/Quint, fonts/token chung, editor hoặc Desk. #259 tái dùng composition;
#261 là diagnostic CMP riêng, không được suy runtime/native parity từ Web.

```text
Screen #explore — tìm tác phẩm công khai / trang thẻ
  Region #explore-heading — H1 văn học và ngữ cảnh thẻ hiện hữu
  Region #public-search — GET q, tag; input owner hiện hữu
  Region #applied-filters — draft-vs-applied, Apply, bỏ thẻ
  Region #community-stream — aria-label="Tác phẩm khám phá", tabindex=-1
    EmptyState — chỉ nhánh successful empty hợp lệ
      Image — 512×427, alt="", decoding=async
      State sentence — một polite result status
      Recovery actions — href thật; bỏ q/tag luôn reset cursor
    Pagination — next_cursor và link về trang đầu hiện hữu
```

Hierarchy: không heading “Kết quả”, H3, đoạn giải thích hay count 0 lặp lại. Nonempty vẫn có
“Trang này: N tác phẩm”, danh sách và phân trang; không suy N là tổng thư viện.
Image → câu → actions; app background, không nested panel/hero/border/glow. UI font hiện hữu
400, body 1rem/1.6 theo body tokens hiện hữu, semantic `on-surface-variant`. Spacing theo token hiện hành.
Ảnh `min(240px, 100%)`; viewport <760px là `min(192px, 100%)`, height auto + ratio/intrinsic size.
200% chỉ tăng chữ và spacing theo token, không nhân đôi artwork. Cột/controls wrap tự nhiên,
cuộn theo trang và giữ bottom inset của shell; không overlay/cuộn riêng mới.
Một transparent file/theme, explicit theme thắng OS như shell hiện hữu. Forced colors có thể
ẩn decoration nhưng giữ đúng không gian để không nhảy layout; HTML nghĩa/actions luôn tồn tại.

| Fact từ owner hiện hữu | Hiển thị |
| --- | --- |
| Unfiltered head rỗng, không continuation | `NoPublicWorks`, “Chưa có tác phẩm công khai.”, link Bàn viết |
| q/tag hợp lệ, không match | `NoResults`, “Chưa có tác phẩm phù hợp.”, bỏ q/thẻ |
| Cursor page rỗng hoặc có next_cursor | `NoResults`, câu riêng về trang; giữ next/first, không nói corpus rỗng |
| Thẻ not_found/retired | Giải thích header + status/recovery đúng thẻ; không artwork empty |
| Loading / failed initial / invalid input/cursor / removed | Nhánh loading/error/alert hiện hữu; không artwork |
| Retained successful empty đang refresh/thất bại refresh | Giữ empty cùng aria-busy/cảnh báo bản đã tải từ owner; không xóa freshness facts |

Interaction: gõ/chọn thẻ chỉ đổi buffer/draft; submit GET/Apply mới commit URL. Không remount input
khi dữ liệu về; input/IME/request owner giữ nguyên. Submit thành công focus landmark có tên và
focus-visible ở frame sau intent submit, khi scope mới đã mount (có thể còn loading).
Kết quả đến sau không dời focus/caret của query đang gõ tiếp; submit lại cùng query vẫn tới
landmark. Reader return giữ anchor hàng, hàng mất focus landmark. Không focus target ẩn.
Keyboard/native links luôn dùng được trước hydration, khi JS/ảnh bị chặn hoặc decode lỗi.
Không JS error handler, retry loop hoặc chỉ dẫn nghĩa nằm trong ảnh. Không aria-hide nhóm actions.

CHOICE-01 A: composition app-private vì `StatePanel` hiện bắt buộc symbol + heading + explanation;
không đổi defaults của primitive hoặc consumer khác để bỏ stack này.
CHOICE-02 A: ordinary `<img decoding="async">`, chỉ mount trong settled-empty, không lazy/preload/
fetchpriority. Network/decode được kiểm bằng scenario thật; không công bố latency gain khi chưa đo.

So sánh cùng q/tag/state, viewport 320/390/834/1440 × light/dark × text100/200. 100% là mục tiêu
bố cục; 200% là reflow/accessibility. Cần mở ảnh trước/sau, đo art/overflow/type/order, contrast,
kiểm focus/GET riêng. OS IME, named screen reader và thiết bị thật là boundary riêng.
[Evidence và giới hạn](../development/evidence/empty-webp-258/README.md).
