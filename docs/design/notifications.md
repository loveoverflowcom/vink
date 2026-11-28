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
| Điều kiện runtime | Phiên hợp lệ, Gateway, socket inbox và worker relay. Shell, điều hướng, popover neo rail, polling và thao tác Seen/Read của #486 được ghi tại [registry](../development/notifications.md#11-web-r4-486). Quyền mở quyết định và danh sách Thấm do owner dữ liệu kiểm tra theo hợp đồng. |

### Trạng thái và điều hướng Web (#603)

| Tình huống | Nội dung và hành động |
| --- | --- |
| Lỗi đọc đầu trang hoặc quyết định | Glyph Error, heading của vùng, nút tonal “Thử lại”. Trang căn giữa khoảng trống, popover dùng biến thể compact trong vùng cuộn. |
| Lỗi khi đã có snapshot | Notice ở cạnh danh sách. Owner giữ dữ liệu, scope và snapshot của mutation khi thử lại. |
| Không có kết quả | Artwork hoặc mark của EmptyState và câu trạng thái đúng phạm vi đọc. |
| Không tìm thấy | H1 Literata Title, mô tả, một link tonal có la bàn Explore trước nhãn “Về Khám phá”. Route sở hữu href, status và robots. |
| Quay về | Back theo history dùng ArrowBack và fallback hiện có. Up là BackLink native tới parent. Close và Escape đóng overlay và trả focus. |

Hộp thư của khách dùng chung NotFoundContent (#575, #603, #608). #621 kế thừa EmptyState và bộ lọc đã áp dụng.
#624 kế thừa ReadFailure khi xử lý loading và DiscoveryFailure riêng.
Ở 320 × 540 với chữ 200%, kiểm thứ tự Tab và hit-test CTA sau khi cuộn.

## 1. Hướng thiết kế và quyết định mở

Final giữ danh sách segmented, cookie/burst và chuông đổi glyph từ hướng Expressive. Nền trang
và rail dùng `bg-page` / `bg-nav`, popover dùng `surface`. Badge số dùng `badge` / `on-badge`
ánh xạ sang `error` / `on-error`. Dấu từ chối và chip “Cần xử lý” vẫn dùng mận.

[Đề xuất VDR-NOTIFICATIONS-DESIGN@r1](../../decisions/proposals/VDR-NOTIFICATIONS-DESIGN.r1.toml)
ghi hai thay đổi so với 20/11 là badge đỏ và chuông dưới Bàn viết. Trạng thái `PENDING` chờ lời
chủ sản phẩm xác nhận đúng revision và SHA-256 theo
[VDR-DECISION-SIGNOFF@r1](../../decisions/signoffs/VDR-DECISION-SIGNOFF.r1.toml).
[#608](https://github.com/loveoverflowcom/vink/issues/608) chốt riêng vị trí Thông báo trên mobile.
[Delta TOML](../../decisions/proposals/VDR-NOTIFICATIONS-MOBILE-NAV.r1.toml) ghi lời chốt và giữ
`PENDING` cho đến khi có signoff đúng revision/hash. Các phần còn lại của đề xuất R4 giữ trạng thái đã ghi.

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
  Region #bottom-navigation (< 760)
    Link Khám phá · Link Bàn viết · Link Thông báo (khi đủ quyền) · Link Tài khoản/Đăng nhập
Hierarchy: bốn ô bằng nhau cho phiên có notification_scope; ba ô công khai cho phiên chưa đủ quyền.
  Mobile giữ pill sau glyph, badge góc trên và nhãn nguyên vẹn ở dưới; Tài khoản dùng avatar hiện có.
  Badge số trên mobile giới hạn chữ ở 16 CSS px để 99+ chừa tâm glyph ở 320 px, chữ 200%.
Sizing: vùng chạm ≥ 44 × 44 CSS px; thanh dưới chừa safe area, chiều cao đo theo nội dung.
Responsive: 760–1099 rail 96 nhãn dưới icon và badge ở góc icon; ≥ 1100 theo preference, rail 220
  đặt badge cuối hàng. Chữ ≥ 150% trên rail dùng link /thong-bao.
Themes: nền rail = bg-page; rail ở popover mở hoặc /thong-bao dùng primary-container.
States: fresh/stale dương hiện cùng kiểu số (tối đa 99+); fresh/stale bằng 0 hoặc unavailable ẩn badge.
Interaction: mobile dùng link native (tap, click hoặc Enter); rail giữ Enter/Space cho popover và
  fallback link. Khi resize popover xuống mobile, đóng dialog và focus #noi-dung.
Accessibility: tên theo §9; aria-current=page ở /thong-bao. Chỉ nút rail mở dialog có
  aria-haspopup=dialog, aria-expanded và aria-controls.
Invariants: không đưa dữ liệu của phiên cũ sang phiên mới. Dùng cùng tập đích ở mọi nơi thanh dưới
  hiện, kể cả Reader. Editor tiếp tục ẩn thanh này. Tên, glyph và vùng focus không đổi vì badge.
Compare: artboards d-x-light/dark, d-c-light/dark, comp-rail, comp-bell, comp-badge cho rail lịch sử.
```

## 3. Popover thông báo

[#602](https://github.com/loveoverflowcom/vink/issues/602) cập nhật đầu popover và trang hộp thư
theo [HTML R1](https://drive.google.com/file/d/1wRQ9W1kNWNrRi6fdqioGqxsl-BKqLrgT/view).
HTML là concept được chọn cho vùng này. App dùng glyph và token của Vink.

```text
Overlay #notif-popover — dialog modal (OverlayLayer), neo #bell (đỉnh thẳng đỉnh chuông), nền surface, bo 32
  Column
    Row #pop-head — padding 16/8/8/22
      Heading #pt "Thông báo" (Literata 30; tabindex=-1; focus khi mở)
      IconButton #read-all Double Check (Standard; chỉ khi có dòng chưa đọc)
      IconButton #pop-close "Đóng thông báo" (48)
    Region #pop-body — cuộn riêng, đệm 6/8/8
      Ghi chú ngoại tuyến hoặc lỗi đếm; “Thử lại” chỉ hiện khi số đếm cũ hoặc chưa tải được
      List #groups — hàng có key ổn định, tiêu đề ngày xen giữa theo lịch thiết bị
        Hôm nay | 7 ngày qua | Cũ hơn; Sắp tới hoặc Chưa rõ thời gian nếu cần
        NotificationRow × n
      Toggle “Ngày giờ đầy đủ” sau danh sách
      Region #state — Skeleton | StatePanel(empty) | thông báo lỗi trên nền chung + Button “Thử lại”
      Region #load-more — Xem thêm | Chưa tải thêm được + Thử lại
    Link #pop-more "Mở trang Thông báo →" (khi còn next_cursor)
Hierarchy: tiêu đề → dòng chưa đọc → đọc/đã đọc
Sizing: rộng 400 (tối đa 100% − 24); cao tối đa viewport − 24 − đáy neo; chỉ #pop-body cuộn
Responsive: không dùng < 760 hoặc chữ ≥ 150% (chuyển sang §4)
Scroll: #pop-body cuộn; header + footer cố định; vòng focus không bị cắt nhờ đệm ≥ 6 px
Overlays: anchor = #bell; dismiss = Esc / Đóng / bấm ngoài (không scrim làm tối); focus vào #pt,
  Tab xoay trong popover. Đóng → trả focus #bell. Resize < 760 → đóng, focus #noi-dung.
  Escape khi tooltip đang mở chỉ ẩn tooltip. Nếu nút mất sau khi xác nhận và đang giữ focus, trả về #pt.
Themes: surface + hàng đọc surface-container-low + chưa đọc primary-container (sáng/tối cùng cấu trúc)
States: tải, trống, 503, stale, offline, tải thêm lỗi, đang đánh dấu — comp-states-light/dark
Interaction: mở → tải trang đầu → Seen(snapshot) → xác minh lại badge, dòng vẫn chưa đọc; bấm dòng → Read rồi tới đích;
  Double Check gửi MarkAllRead theo snapshot. Trạng thái chưa đọc giữ đến khi đọc lại.
  Tooltip và tên truy cập cùng dùng "Đánh dấu tất cả là đã đọc". Glyph 24 và vùng chạm ≥48 CSS px, nền trong suốt.
  [Đồng hồ trình bày](human-time.md) cập nhật nhãn và nhóm ngày trong danh sách đang mở.
  Số mới cập nhật trên chuông. Lần mở kế tiếp lấy trang đầu mới. Danh sách đang mở giữ thứ tự, vị trí cuộn và mốc Seen.
  scale .92 → 1 (320 ms, ease.spatial); reduced-motion: hiện ngay
Accessibility: role=dialog aria-modal aria-labelledby=#pt; danh sách ul/li, mỗi dòng một link
Invariants: một modal tại một lúc; popover không nằm trong cây AX khi đóng; Seen không đánh dấu Read
Compare: d-x-light/dark, d-c-light, d-c-dark(stale), comp-states-*, comp-inter, cmp-pop
```

## 4. Trang `/thong-bao`

```text
Screen #thong-bao — mặc định < 760 và deep link; desktop dùng cùng nội dung trong #noi-dung
  Column #np (≤ 640; gutter 24 | 16 hẹp)
    IconButton #back
    Header #np-head
      Row — Heading h1 "Thông báo" (Literata 30) · IconButton #read-all Double Check (Standard)
      Count "{n} chưa đọc" (neutral)
    Region #np-body — giống #pop-body: note · groups · state · load-more
Hierarchy: h1 → #read-all (cùng hàng với h1) → dòng chưa đọc
Sizing: nhãn ngày chỉ khi > 1 nhóm; nhóm cách nhau 20; hàng đầu cao ≥ 48
Responsive: dòng chuyển "xếp chồng" khi bề rộng danh sách / hệ số chữ < ≈ 215 px (320 px ≥ 150%);
  h1 xuống dòng bên trái nút Double Check; thanh dưới theo §2, nhãn có thể xuống hai dòng
Scroll: cuộn trang chính; thanh dưới cố định; không có cuộn riêng
Overlays: không có
States: như §3 + lỗi tải thêm ngay dưới danh sách
Interaction: vào trang → Seen khi nhận snapshot; Back → trang trước, hoặc Khám phá nếu vào trực tiếp.
  Khi trở về đích gốc từ Inbox, ưu tiên focus điểm mở Thông báo đang hiện, rồi tới #noi-dung.
  Double Check cùng hợp đồng popover. Nếu nút mất khi đang giữ focus, trả về h1.
Accessibility: một h1, landmark main = #noi-dung. Phiên đủ quyền thấy Thông báo được chọn.
Invariants: không cuộn ngang ở 320 CSS px, chữ 200%; tối đa 3 dòng nhan đề; dấu tiếng Việt không bị cắt
Compare: d-page-*, m-inbox-*, m-states, cmp-stress, m-inbox-200
```

## 5. Header mobile sau #608

Khám phá và Bàn viết giữ header với tiêu đề và các công cụ của trang. Hero Tài khoản giữ avatar,
bút danh và các hành động hồ sơ. Xem §2 cho đích Thông báo.
Các artboard `m-heads`, `m-heads-dark`, `m-heads-wrap` và AC-01 của #486 ghi lịch sử chuông rời
trước thay đổi này. Specimen popover ở Preview vẫn dùng dữ liệu mẫu và minh họa riêng hành vi rail.

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
    Region #reason "Lời nhắn kiểm duyệt" — đúng revision/version, ẩn khi bản duyệt cũ thiếu lời nhắn; bản từ chối thiếu lý do ghi rõ sự thiếu vắng
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
| Chuông và badge | `NavUtility` trên rail, `NavItem` có badge và glyph active trong thanh dưới. `NotificationBadge` và `ShapeMark` thuộc `vink-ui`. Shell và rail lấy tên từ `BadgeState::trigger_name` và cùng owner Web. |
| Dòng và nhóm | `NotificationRow` và `NotificationGroup` thuộc `vink-patterns`. Caller chọn glyph, avatar, ngày, excerpt được phép và đích. |
| Preview | `vink-ui-preview`, hai kịch bản ở §0. Mẫu dùng kiểu chữ có sẵn, cỡ glyph 34 và icon 16. |
| Tích hợp | Shell, overlay neo rail, trang hộp thư, màn quyết định và người Thấm nằm trong lát Web. |

## 9. Tên truy cập và trạng thái

- Chuông lúc ẩn badge đọc “Thông báo”. Fresh dương đọc “Thông báo, {n} mục mới” hoặc
  “Thông báo, hơn 99 mục mới”. Stale dương thêm “có thể chưa cập nhật”. Stale bằng 0 và Unknown chỉ đọc “Thông báo”.
  Panel nêu tình trạng đồng bộ và có nút thử lại kể cả khi chỉ count lỗi. Badge trang trí, không là live region.
- Dòng đọc theo thứ tự “Chưa đọc.” (nếu có), câu, nhan đề, trích đoạn trong ngoặc kép nếu có,
  bản gửi, thời gian và “Cần xử lý” nếu có. Trích đoạn dành cho nhận xét gốc hoặc phản hồi
  đã duyệt mà Gateway còn cho phép đọc.
- Lỗi tải trang hoặc tải thêm có lối thử lại và giữ phân biệt với danh sách trống. Khi quyền
  thay đổi, màn đích dùng trang không khả dụng của owner.
- Mục tiêu nghiệm thu specimen gồm 320/390/834/1024/1440 CSS px, sáng/tối, cỡ chữ
  100/150/200%, không tràn ngang, dấu tiếng Việt nguyên vẹn, vùng chạm ≥ 44 CSS px,
  focus thấy được và reduced motion giữ đủ tín hiệu. Ảnh và số đo specimen chưa chạy.
