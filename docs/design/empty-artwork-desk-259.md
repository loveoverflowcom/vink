# Bàn viết — empty artwork R1, #259

LayoutSpec kế thừa [R1 #258](empty-artwork-258.md) và issue #259, trên
`develop@997bb2f5662a6417eebf9bc2d1d3891f925b48a0`. Artwork vẫn là design candidate,
**NO_APPROVED_REFERENCE**. Chỉ đổi empty slot trong Desk index; Editor, API, read/create
lifecycle, font/token và composition dùng chung giữ owner hiện hành.

```text
Screen #desk — Bàn viết riêng
  Region #heading — H1; create hiện hữu khi không first-use
  Region #drafts — Tác phẩm của bạn
    Region .vk-draft-controls — một search/input owner, scope status, sort, refresh
    Region [data-testid=draft-empty|draft-no-results]
      EmptyState — composition #258, không panel/heading/icon phụ
        Image — FirstDraft hoặc NoResults, decorative
        Sentence — một câu ngắn, UI body font
        Action — CreateDraftButton hoặc Xem tất cả tác phẩm hiện hữu
    Region #draft-results / #draft-continuation — không đổi rows/paging/recovery
  Region .vk-desk-privacy — giữ nguyên thông điệp riêng tư
```

First-use cần successful unfiltered head rỗng, part 1, không continuation và không load error.
Filtered miss hoặc head rỗng còn continuation không khẳng định corpus rỗng. NoResults dùng
“Chưa có tác phẩm trong kết quả này.”; first-use giữ “Một trang dành riêng cho những điều bạn
muốn viết.”. Một create target, nhãn/Pending/Uncertain/Reconciled và GET reconciliation theo
owner cũ; không POST từ mount. Loading/error/auth không thành fresh first-use; retained empty
đang refresh còn aria-busy/stale qualifier, lỗi giữ warning hiện hữu.

Sizing/surface đúng `.vk-empty-state*`: ảnh 192 CSS px dưới 760px, 240px ở desktop,
intrinsic 512×427; không nhân đôi theo text200. Body tokens hiện hữu, không fixed hero height,
overlay hay scroll riêng. Responsive giữ CTA và privacy copy cuộn tới được trên bottom nav.
Một status của `#draft-scope`, composition không thêm announcement; alt rỗng. Ảnh lỗi vẫn có
sentence và action HTML. Một file/theme, chỉ request ảnh đúng state; không asset copy/pipeline mới.

Interaction: giữ node search, buffer/applied query, caret, simulated composition, sort/session
fences và clear focus. Không chuyển owner state hay dựng lại cả subtree. CHOICE-01 A: đổi hai
empty branches hiện có vì đủ ngắn; continuation guard là presentation fact, không đổi PageChain.
Quint/domain **NOT_APPLICABLE**: không đổi transition/admission/persistence contract.

Compare: cùng private-summary fixture ở 320/390/834/1440 × light/dark × text100/200;
100% xét phân cấp, 200% xét reflow/accessibility. Mở capture trước/sau, kiểm recovery/privacy
sau cuộn riêng; không gọi root font-size multiplier là browser zoom hoặc OS IME thật.
Evidence theo [báo cáo #259](../development/evidence/empty-webp-259/README.md).
