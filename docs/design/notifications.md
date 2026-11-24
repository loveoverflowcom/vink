# LayoutSpec · Thông báo R4

**Trạng thái** `DESIGN_PROPOSED`, `NO_APPROVED_REFERENCE`. Specimen dùng hướng Final.
[Issue #485](https://github.com/loveoverflowcom/vink/issues/485) và
[bộ thiết kế R4](https://drive.google.com/drive/folders/1gLJYaMmg35WYpKJ0iXGp-p8JPpUs1Qhn)
là nguồn phạm vi và bố cục. Khi ảnh lệch với mô tả, dùng `DESIGN_SPEC.md` trong bộ R4.
Theo [quy ước LayoutSpec](../../.agents/skills/vink-layout-spec/references/layout-spec.md).

## 0. Phạm vi và dữ liệu

| Lớp | Phạm vi |
| --- | --- |
| Trải nghiệm đích | Thành viên mở chuông, đọc lướt theo nhóm ngày và đi tới đúng nội dung hoặc quyết định. Khách có thể xem danh sách người Thấm theo quyền đọc tác phẩm. |
| Specimen | `composition.notifications/default` và `composition.notifications/badges` trong preview Leptos. Dữ liệu mẫu từ Ví dụ 2–4 của [hợp đồng Gateway v1](../../services/vink-app/contracts/http/v1/notifications.md) và [fixture dòng nhận xét](../../services/vink-app/contracts/http/v1/fixtures/notifications-gateway.json). Dòng Ưng minh họa theo #365. Chuông trong preview mở dialog mẫu để trình diễn trạng thái và focus. |
| Điều kiện runtime | Phiên hợp lệ, Gateway, socket inbox và worker relay. Shell, điều hướng, popover neo rail, polling và thao tác Seen/Read thuộc lát Web. Quyền mở quyết định và danh sách Thấm do owner dữ liệu kiểm tra theo hợp đồng. |

## 1. Hướng thiết kế và quyết định mở

Final giữ danh sách segmented, cookie/burst và chuông đổi glyph từ hướng Expressive. Nền trang
và rail dùng `bg-page` / `bg-nav`, popover dùng `surface`. Badge số dùng `badge` / `on-badge`
ánh xạ sang `error` / `on-error`. Dấu từ chối và chip “Cần xử lý” vẫn dùng mận.

[Đề xuất VDR-NOTIFICATIONS-DESIGN@r1](../../decisions/proposals/VDR-NOTIFICATIONS-DESIGN.r1.toml)
ghi hai thay đổi so với 20/11 là badge đỏ và chuông dưới Bàn viết. Trạng thái `PENDING` chờ lời
chủ sản phẩm xác nhận đúng revision và SHA-256 theo
[VDR-DECISION-SIGNOFF@r1](../../decisions/signoffs/VDR-DECISION-SIGNOFF.r1.toml).

| Handoff | Đề xuất đang mô tả |
| --- | --- |
| Q1 · Q2 · Q3 | Final, câu “Tác phẩm của bạn…”, H1 Literata. |
| Q4 | Dòng Ưng không có trích đoạn theo hợp đồng #365. |
| Q5 · Q6 | Popover modal và chuông có trạng thái hiện hành ở trang hộp thư desktop. |
| Q7 · Q8 | Chỉ ẩn nút khi không thể phản đối. Đích rail cao 56 CSS px. |
| Q9 | #477 đã cung cấp [đích Bàn viết cho quyết định tác phẩm](../development/notifications.md#hợp-đồng-đích-web-cho-quyết-định-tác-phẩm-477). |
| Q10 · Q11 | Badge đỏ và chuông dưới Bàn viết chờ signoff. |

## 2. Shell: rail, chuông, thanh dưới

```text
Screen #shell — ≥ 760: rail + main; < 760: main + bottom bar
  Row #app
    Region #rail (NavigationRail 220 | 96; nền bg-nav = bg-page)
      Action #nav-toggle
      Link #nav-explore (Khám phá) · Link #nav-desk (Bàn viết)
      NavUtility #bell          — mới, ngay dưới Bàn viết; icon notifications-none/notifications, badge
      Region #account (chân rail)
        Link #nav-me (Tôi)      — như hiện hành
    Region #noi-dung (main; không remount khi đổi khung)
    Overlay #notif-popover      — chỉ ≥ 760 và chữ < 150%
  Region #bottom-bar (< 760): đúng 3 đích Khám phá · Bàn viết · Tôi (không có chuông)
Hierarchy: chuông ngay dưới Bàn viết, cùng chữ, độ đậm, chiều cao (56) và kiểu outlined/filled với các đích rail; badge là điểm nhấn duy nhất của rail
Sizing: rail 220/96 (size.rail-*), đích 56 bo 16 (radius.control-56); badge 18 / vòng 10
Responsive: < 760 chuông chuyển lên header trang (§3); 760–1099 rail 96 nhãn dưới icon, badge ở góc icon;
  ≥ 1100 rail 220 badge cuối hàng. Chữ ≥ 150%: chuông là link /thong-bao, không popover
Themes: nền rail = bg-page; đang mở popover hoặc đang ở /thong-bao = primary-container (như đích đang chọn)
States: none · fresh n · 99+ · stale · 503 (vòng) — xem comp-badge
Interaction: Enter/Space → mở (≥760) | điều hướng (mobile, chữ lớn); focus ring 3 px
Accessibility: name "Thông báo, n mục mới" (+ ", có thể chưa cập nhật" | "chưa kiểm tra được mục mới");
  aria-haspopup=dialog, aria-expanded, aria-controls; aria-current=page ở /thong-bao; không aria-live
Invariants: chuông không xuất hiện cho khách, Reader, editor, trang con ở mobile; vùng chạm ≥ 44 × 44 CSS px;
  badge không bao giờ là "0" khi 503/stale
Compare: artboards d-x-light/dark, d-c-light/dark, comp-rail, comp-bell, comp-badge
```

## 3. Popover thông báo

```text
Overlay #notif-popover — dialog modal (OverlayLayer), neo #bell (đỉnh thẳng đỉnh chuông), nền surface, bo 32
  Column
    Row #pop-head — padding 16/8/8/22
      Heading #pt "Thông báo" (Literata 30; tabindex=-1; focus khi mở)
      Button #read-all "Đọc tất cả" (tonal; chỉ khi có dòng chưa đọc)
      IconButton #pop-close "Đóng thông báo" (48)
    Region #pop-body — cuộn riêng, đệm 6/8/8
      InlineNotice #fresh-note (stale | offline)
      Button #new-signal "Có cập nhật mới" (sticky, filled)
      Region #groups — NotificationGroup × (Hôm nay | 7 ngày qua | Cũ hơn)
        List #list — NotificationRow × n
      Region #state — Skeleton | StatePanel(empty) | StatePanel(error + Thử lại) 
      Region #load-more — Xem thêm | Chưa tải thêm được + Thử lại
    Link #pop-more "Mở trang Thông báo →" (khi còn next_cursor)
Hierarchy: tiêu đề → dòng chưa đọc → "Có cập nhật mới" (nếu có, điểm nhấn duy nhất) → đọc/đã đọc
Sizing: rộng 400 (tối đa 100% − 24); cao tối đa viewport − 24 − đáy neo; chỉ #pop-body cuộn
Responsive: không dùng < 760 hoặc chữ ≥ 150% (chuyển sang §4)
Scroll: #pop-body cuộn; header + footer cố định; vòng focus không bị cắt nhờ đệm ≥ 6 px
Overlays: anchor = #bell; dismiss = Esc / Đóng / bấm ngoài (không scrim làm tối); focus vào #pt,
  Tab xoay trong popover, đóng → trả focus #bell; resize < 760 → đóng, focus #noi-dung
Themes: surface + hàng đọc surface-container-low + chưa đọc primary-container (sáng/tối cùng cấu trúc)
States: tải, trống, 503, stale, offline, new, tải thêm lỗi, đang đánh dấu — comp-states-light/dark
Interaction: mở → tải trang đầu → Seen(snapshot) → badge 0, dòng vẫn chưa đọc; bấm dòng → Read rồi tới đích;
  scale .92 → 1 (320 ms, ease.spatial); reduced-motion: hiện ngay
Accessibility: role=dialog aria-modal aria-labelledby=#pt; danh sách ul/li, mỗi dòng một link
Invariants: một modal tại một lúc; popover không nằm trong cây AX khi đóng; Seen không đánh dấu Read
Compare: d-x-light/dark, d-c-light, d-c-dark(stale), comp-states-*, comp-inter, cmp-pop
```

## 4. Trang `/thong-bao`

```text
Screen #thong-bao — mặc định < 760 và deep link; desktop dùng cùng nội dung trong #noi-dung
  Column #np (≤ 640; gutter 24 | 16 hẹp)
    Row #np-top — IconButton #back (mobile/có lịch sử) · khoảng trống · Button #read-all
    Row #np-head — Heading h1 "Thông báo" (Literata 30) + Count "{n} chưa đọc" (neutral)
    Region #np-body — giống #pop-body: note · new-signal · groups · state · load-more
Hierarchy: h1 → #read-all (hàng trên) → dòng chưa đọc
Sizing: nhãn ngày chỉ khi > 1 nhóm; nhóm cách nhau 20; hàng đầu cao ≥ 48
Responsive: dòng chuyển "xếp chồng" khi bề rộng danh sách / hệ số chữ < ≈ 215 px (320 px ≥ 150%);
  "Đọc tất cả" xuống dòng được; h1 xuống dòng; thanh dưới giữ 3 ô, nhãn 2 dòng
Scroll: cuộn trang chính; thanh dưới cố định; không có cuộn riêng
Overlays: không có
States: như §3 + lỗi tải thêm ngay dưới danh sách
Interaction: vào trang → Seen; Back → trang trước, focus về #bell (hoặc #noi-dung)
Accessibility: một h1; landmark main = #noi-dung; không đích nào sáng ở mobile
Invariants: không cuộn ngang ở 320 CSS px, chữ 200%; tối đa 3 dòng nhan đề; dấu tiếng Việt không bị cắt
Compare: d-page-*, m-inbox-*, m-states, cmp-stress, m-inbox-200
```

## 5. Header mobile có chuông

```text
Region #page-head (Khám phá | Bàn viết | Tôi; < 760)
  Heading #h1
  Row #acts — (Khám phá: Tìm (không nền) · Lọc thẻ · logo) | (Bàn viết: Tìm (không nền)) | (Tôi: không)
  IconButton #bell-m — link /thong-bao, 48, không nền, icon 30, ở cuối hàng đầu
Responsive: đủ chỗ → một hàng [h1 · acts · bell]; hẹp hoặc chữ lớn → hàng 1 [h1 · bell], hàng 2 [acts]
Invariants: bell luôn là phần tử cuối hàng đầu và cùng vị trí ở cả ba trang; không chuông ở Reader/editor/trang con
Compare: m-heads, m-heads-dark, m-heads-wrap, m-nobell
```

## 6. `NotificationRow`

```text
Component #notification-row — li > a[href] (một hành động)
  Grid #row — areas "lead | text | dot"; xếp chồng "lead dot / text text" khi hẹp
    Lead #lead — Avatar | cụm Avatar (AvatarGroup ≥ 2) + Glyph cookie | ShapeMark (hệ thống)
    Column #text
      Text #s1 câu (tên 600)
      Text #s2 nhan đề (Literata, ≤ 2 dòng; 3 ở ≥ 150%)
      Text #excerpt "…" (chỉ root/reply approved; ≤ 2 dòng)
      Row #meta — thời gian [· "Bản {ngày}"] · Chip "Cần xử lý"
    Dot #dot — chưa đọc
Segmented: khe 3; đầu/cuối nhóm bo 28, giữa bo 8; không viền; không card lồng
Invariants: dấu tiếng Việt không bị cắt (line-height ≥ 1.35, clamp không để overflow hidden cắt đỉnh dấu);
  chưa đọc = nền + chấm + tiền tố ẩn + tên đậm (không chỉ màu)
Compare: comp-row, comp-lead, comp-anatomy
```

## 7. Màn quyết định, danh sách Thấm, không khả dụng

```text
Screen #decision-work — vùng Kết quả kiểm duyệt của Bàn viết (#477)
  Column (≤ 768; panel bg-content bo 20, không card lồng)
  Link #back "← Kết quả kiểm duyệt"
  Panel #panel
    Row #outcome — Chip(Bị từ chối | Đã được duyệt) + Chip "Cần xử lý" (action_required)
    Heading h1 nhan đề (Literata 30) · Text #sub "Bản gửi ngày … · quyết định lúc …"
    Region #reason "Lý do" (surface-container, bo 20)
    Row #actions — Button "Sửa trong Bàn viết" (filled) · Button "Gửi phản đối" (tonal, chỉ can_appeal)
    Text #hint
Screen #decision-comment — như trên, không #actions, không chữ nhận xét
Overlay/Screen #tham-list — Dialog 480 (desktop) | trang (mobile): Heading + tổng · List · Xem thêm
Screen #gone — StatePanel: icon shape + h1 "Nội dung này không còn khả dụng." + CTA Về Khám phá
Invariants: không tên/ID người duyệt; không chữ nhận xét đã gỡ; "Đã được duyệt" ≠ "Đang công khai";
  trạng thái lỗi danh sách Thấm không bao giờ giống trống
Compare: r-dw-light/mob/var, r-dc-desk/mob, r-tham-d/states/mob, r-gone-d/m
```

## 8. Owner Leptos

| Vùng | Owner |
| --- | --- |
| Chuông và badge | `NavUtility` (slot badge và glyph active), `NotificationBadge` và `ShapeMark` thuộc `vink-ui`. Caller kết hợp `BadgeState::trigger_name` với tên chuông. |
| Dòng và nhóm | `NotificationRow` và `NotificationGroup` thuộc `vink-patterns`. Caller chọn glyph, avatar, ngày, excerpt được phép và đích. |
| Preview | `vink-ui-preview`, hai kịch bản ở §0. Mẫu dùng kiểu chữ có sẵn, cỡ glyph 34 và icon 16. |
| Tích hợp | Shell, overlay neo rail, trang hộp thư, màn quyết định và người Thấm nằm trong lát Web. |

## 9. Tên truy cập và trạng thái

- Chuông lúc ẩn badge đọc “Thông báo”. Fresh dương đọc “Thông báo, {n} mục mới” hoặc
  “Thông báo, hơn 99 mục mới”. Stale thêm “có thể chưa cập nhật”, bỏ số khi count cũ bằng 0.
  Unknown đọc “Thông báo, chưa kiểm tra được mục mới”. Badge trang trí, không là live region.
- Dòng đọc theo thứ tự “Chưa đọc.” (nếu có), câu, nhan đề, trích đoạn trong ngoặc kép nếu có,
  bản gửi, thời gian và “Cần xử lý” nếu có. Trích đoạn dành cho nhận xét gốc hoặc phản hồi
  đã duyệt mà Gateway còn cho phép đọc.
- Lỗi tải trang hoặc tải thêm có lối thử lại và giữ phân biệt với danh sách trống. Khi quyền
  thay đổi, màn đích dùng trang không khả dụng của owner.
- Mục tiêu nghiệm thu specimen gồm 320/390/834/1024/1440 CSS px, sáng/tối, cỡ chữ
  100/150/200%, không tràn ngang, dấu tiếng Việt nguyên vẹn, vùng chạm ≥ 44 CSS px,
  focus thấy được và reduced motion giữ đủ tín hiệu. Ảnh và số đo specimen chưa chạy.
