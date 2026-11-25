# Vùng công khai của Bàn viết — LayoutSpec r3

> [Editor R4](editor-r4.md) đặt vùng này trong Tùy chọn bài viết và dùng công tắc thay cho RadioGroup. Công tắc đổi ý định; nút xác nhận vẫn gửi lệnh riêng. Các trạng thái và quy tắc snapshot bên dưới tiếp tục áp dụng; nút Xem trước đã bỏ theo delta 24/11 (#475).

Phạm vi: **PUB-AUTHOR-WEB-01**. Vùng nối Bàn viết với API publication có sẵn
([publication-runtime](../development/publication-runtime.md)) và [chi tiết kiểm duyệt trong Bàn viết](redesign-r4/desk-moderation.md)
([moderation-runtime](../development/moderation-runtime.md#7-web-kiểm-duyệt)). Không đổi quyết định
sản phẩm; không phải signoff. Spec này do agent triển khai viết **trước** khi có mã UI, ở trạng
thái `proposed` (không có người duyệt riêng); phần “Kết quả đối chiếu” ghi những gì đã thực sự
chạy sau khi triển khai.

**r2 (13/11/2026, `codex/client-state-interaction-alignment`)** chỉ đổi ba điều, không đổi cấu trúc
hay hành động chính: (1) ở vùng hẹp so với chữ, thẻ phục hồi lồng bỏ bớt padding, hành động xếp dọc
và nút theo công thức container hẹp của `vk-state-panel`; (2) pha `acknowledged` (đã ghi nhận,
chưa đọc lại được kết quả); (3) hợp đồng focus khi control vừa bấm biến mất. Chi tiết ở mục
“Rà soát client-state” của [owner doc](../development/publication-author-web.md).

**r3 (25/11/2026, `refactor/ban-viet-cong-khai-ui`)** tinh giản trình bày tại chỗ trong Tùy chọn bài
viết mà không đổi domain, API, state machine hay quy tắc kiểm duyệt:
- Giữ công tắc **Công khai** (`#audience-public`) và bỏ nhãn trạng thái Bật/Tắt (`show_state=false`).
- Bỏ nút tải lại (`#publication-refresh`) cùng tooltip; đồng bộ tự động khi mở, đổi phiên, quay lại
  tab/cửa sổ (`focus`/`visibilitychange`), có mạng trở lại (`online`) và sau mỗi lệnh.
- Bỏ dòng “Máy chủ đang áp dụng…” (`#audience-applied`) và các dòng giải thích dư thừa.
- Gom trạng thái máy chủ thành một dòng ngắn gọn duy nhất (`#publication-status`).
- Đổi nhãn “Gửi duyệt bản đã lưu” thành “Gửi duyệt”, giữ nguyên hành vi.
- Liên kết “Xem kết quả kiểm duyệt” (`#editor-review-status`) chỉ hiện khi tác phẩm có ngữ cảnh
  kiểm duyệt (đang công khai, đang gửi, có ticket mở, có bản đã từng gửi/từ chối/phản đối).
- Vùng `#publication` dùng chung nền với Bàn viết (`bg-transparent` trên `bg-content`), không
  border, không container màu riêng; giữ `tabindex="-1"` làm đích trả focus trong vùng khi nút chính
  và công tắc đang ẩn/khóa.

```text
LayoutSpec: Vùng công khai tại chỗ, r3 — proposed
Context: tác giả đã đăng nhập, mở /ban-viet/{id}. Nội dung thật của tác giả không đưa vào tài liệu;
  dữ liệu mẫu trong fixture/e2e mang nhãn [Mẫu].
Intent: viết/duyệt trạng thái — vùng đọc trạng thái gọn kèm một hành động chính. Tác giả phải làm
  được: (1) biết ngay trạng thái công khai/kiểm duyệt hiện tại; (2) bật “Công khai” và xác nhận gửi
  đúng bản vừa lưu; (3) tắt “Công khai” → “Chuyển riêng tư” mà không phải lưu hay bỏ chữ đang viết;
  (4) hiểu và xử lý mất phản hồi, xung đột, đổi phiên.

Screen #draft-editor — /ban-viet/{id}
  Region #work-options — Tùy chọn bài viết (bg-content, cùng mặt nền với Bàn viết)
    Region #publication — section (tabindex=-1, nền trong suốt, không border, không container riêng)
      Region #audience-choice — Switch “Công khai” (#audience-public, không nhãn Bật/Tắt)
      Region #publication-live — một dòng role=status cho tiến trình/xác nhận (ẩn khi rỗng)
      Text #publication-status — một dòng trạng thái ngắn gọn từ dữ kiện máy chủ
      Region #publication-recovery — thông báo mất phản hồi/xung đột/lỗi kèm hành động phục hồi (chỉ khi có sự cố)
      Region #publication-actions — đúng một hành động chính theo trạng thái (ẩn khi không có hành động)
      Link #editor-review-status — “Xem kết quả kiểm duyệt” → /ban-viet?review={id} (chỉ hiện khi cần)
```

## Phương án đã chọn (và phương án bỏ)

| Quyết định | Chọn | Bỏ | Lý do |
| --- | --- | --- | --- |
| CHOICE-01 Lưu và gửi | **B**: “Lưu và gửi duyệt” gọi chính hàm lưu của editor (`DraftSession`) rồi chỉ **quan sát** kết quả; chụp snapshot đúng `draft_version` + `metadata_version` được máy chủ xác nhận | A: bắt tác giả bấm “Lưu” riêng rồi mới được gửi | Không sao chép logic lưu, không sửa editor. Nếu có chữ/thẻ mới sau khi bấm thì **dừng**, không chụp (xem Interaction). A vẫn là hành vi khi thẻ chưa tải hoặc lưu đang lỗi/xung đột: nút khóa kèm lý do |
| CHOICE-02 Trình bày | **A**: vùng gọn, mở rộng tại chỗ sau thông tin tác phẩm | B: nút trạng thái + dialog | Trạng thái “chưa xác nhận”, “xung đột” phải còn thấy và thao tác được sau khi gửi; ở chữ 200% và 320 CSS px vùng tại chỗ chỉ xếp dọc trong luồng trang, không thêm vùng cuộn hay bẫy focus; dialog che trạng thái lưu của editor mà tác giả đang cần đọc |
| CHOICE-03 Làm mới | **A**: tự động đồng bộ khi mở, đổi phiên, quay lại tab/cửa sổ (`focus`/`visibilitychange`), có mạng trở lại (`online`) và sau mỗi lệnh (không có nút tải lại thủ công trên tiêu đề; chỉ giữ “Tải lại” trong khung phục hồi khi đọc lỗi) | B: A + polling hữu hạn | Kiểm duyệt không có SLA; polling thêm hẹn giờ, chồng request và rủi ro cướp focus/cuộn. Đảo được khi có số liệu |

Các lựa chọn trên là triển khai, không phải quyết định sản phẩm.

## Hierarchy

Thứ tự đọc/focus = thứ tự DOM: tiêu đề → tóm tắt → lựa chọn → hành động → phục hồi → liên kết kết
quả. Mỗi trạng thái có **một** điểm nhấn: hành động chính (Filled) *hoặc*, khi có sự cố, thông báo
phục hồi (vùng `.vk-editor-recovery`). Khi có sự cố thông báo đứng ngay dưới tóm tắt (trước lựa
chọn) để không bị đẩy xuống dưới hai nút khi chữ 200%. Hành động chính theo trạng thái:

| Điều kiện (từ dữ kiện máy chủ + editor) | Hành động chính |
| --- | --- |
| Chọn “Công khai”, editor sạch | “Gửi duyệt” |
| Chọn “Công khai”, editor còn chữ/thẻ chưa lưu | “Lưu và gửi duyệt” |
| Chọn “Công khai”, đã có bản đang công khai/đang gửi | “Gửi bản mới” (cùng luồng) |
| Chọn “Chỉ mình tôi”, máy chủ đang công khai | “Chuyển riêng tư” |
| Chọn đúng trạng thái máy chủ đang áp dụng | không có hành động; chỉ giữ một dòng trạng thái ngắn gọn |
| Đang chờ phản hồi | cùng nút ở trạng thái pending (`ActionState::pending`), các nút khác khóa |
| Mất phản hồi | “Kiểm tra lại” |

Chọn radio **không gửi request**. Nút nói đúng việc sẽ làm; không dùng “Đăng”, “Xuất bản”.

## Sizing

Nằm trực tiếp trong `.vk-work-options__body` của Bàn viết, dùng chung nền `bg-content` (`background:
transparent`), không border ngăn cách và không container màu riêng; khoảng cách `--vk-space-2`;
vùng phục hồi dùng `.vk-editor-recovery` (error/warning container) khi có sự cố.
Không viền trang trí, không card/pill lồng nhau. (r2) Thẻ phục hồi là phần tử lồng duy nhất; khi
vùng hẹp so với chữ (container `vk-publication` ≤ 16rem, tính theo gốc rem nên chạm tới ở chữ 200% trên
điện thoại, không ở 100%) nó còn `--vk-space-2` padding, hành động xếp dọc, và nút dùng công thức
`vk-state-panel` (bớt padding ngang để vừa nguyên từ tiếng Việt). Nút phục hồi không có icon dẫn đầu. Tiêu đề bản dài (200 ký tự) và lý do từ chối dài
xuống dòng (`overflow-wrap: anywhere`), không cắt. Hành động xuống hàng (`flex-wrap`), mục tiêu chạm
`--vk-size-hit-target`.

## Responsive

Một cột ở mọi viewport; không breakpoint riêng. 320/390 CSS px: tóm tắt, radio (xếp dọc), nút và
thông báo xếp dọc theo DOM, nút tự xuống hàng. Desktop: cùng cấu trúc, nút nằm một hàng nếu đủ chỗ.
Không có phần ẩn sau “mở rộng”: vùng luôn hiện, chỉ **dài ra** theo trạng thái.

## Scroll

Cuộn của trang; không vùng cuộn riêng, không sticky, không `position: fixed`. Làm mới trạng thái
không đổi `scrollY` và không chuyển focus (cập nhật tại chỗ, nút giữ nguyên node).

## Overlays

Không có overlay mới. Dialog “rời trang khi còn chữ chưa lưu” của editor giữ nguyên chủ sở hữu
(`NavigationGuard`): vùng công khai không đăng ký điều hướng riêng và không giữ trang vì một lệnh
đang chờ.

## Themes

Light/dark dùng chung cấu trúc và token của `.vk-work-info` / `.vk-editor-recovery`
(`--vk-surface-container-low`, `--vk-on-surface`, `--vk-on-surface-variant`, `--vk-error-container`,
`--vk-warning-container`). Trạng thái không chỉ bằng màu: có chữ và biểu tượng của `InlineNotice`.
Focus dùng viền của primitive. Không thêm token mới.

## States

| Trạng thái (client) | Hiển thị |
| --- | --- |
| `reading` (chưa có dữ kiện) | “Đang đọc trạng thái công khai…” (`role=status`); editor không bị che |
| `unavailable` | thông báo + “Tải lại”; mất quyền/không tìm thấy xóa dữ kiện riêng cũ, mất mạng giữ dữ kiện gần nhất |
| `unreadable` | “Chưa hiểu trạng thái máy chủ trả về. Không gửi gì. Hãy tải lại.” — khóa mọi lệnh |
| `ready` | tóm tắt + lựa chọn + hành động |
| `saving` | “Đang lưu bản nháp…” (trạng thái lưu vẫn thuộc thanh editor) |
| `capturing` / `requesting_public` | nút pending “Đang gửi duyệt…”; mọi nút khác khóa |
| `making_private` | nút pending “Đang chuyển riêng tư…” |
| `uncertain(stage)` | thông báo cảnh báo: chưa rõ lệnh đã được ghi hay chưa; “Kiểm tra lại” |
| `conflict` | nêu đúng loại: nháp / thông tin tác phẩm (thẻ) / trạng thái công khai; “Đã hiểu”; khi đã chụp bản và xung đột là của trạng thái công khai: thêm “Gửi lại bản đã chụp” |
| `failed` | lý do ngắn (phiên hết, bị từ chối, bản đã bị từ chối, gõ thêm khi đang lưu…) + cách tiếp |
| `acknowledged` (r2) | máy chủ đã ghi nhận lệnh nhưng lần đọc xác nhận chưa thành công: thông báo nói đúng điều đó + “Tải lại”; không còn “Đang gửi…”, chưa nói bản nào đang công khai; hành động chính khóa kèm lý do |
| `reconciled` | một câu xác nhận lấy từ trạng thái đã đọc lại (`role=status`), tự gỡ ở lần thao tác sau |

Bản đang công khai r1 và bản gửi r2: tóm tắt **luôn nói r1 vẫn đang được đọc** khi r2 chờ hoặc bị từ
chối; không hiển thị như toàn tác phẩm bị ẩn. “Đã được duyệt” và “Đang công khai” là hai câu khác
nhau: chỉ con trỏ công khai của máy chủ mới cho “Đang công khai”. Không có trạng thái “cần sửa” riêng,
không SLA, không thông báo giả; `needs_admin` đọc là “Đã chuyển cho người duyệt” — không phải “AI đang
duyệt” hay “bị từ chối”. Giá trị lạ → `unreadable`, không suy ra.

## Interaction

- **Chọn → xác nhận:** chọn radio chỉ đổi lựa chọn cục bộ (không request, không announce). Bấm nút
  hành động là xác nhận. Chuyển riêng tư không xóa bài; câu giải thích một dòng nằm cạnh nút.
- **Bấm “Lưu và gửi duyệt”:** (1) ghim thế hệ (`generation`) đang hiển thị và dấu “đã bấm” của
  editor (đếm sửa + lựa chọn thẻ); (2) nếu còn chữ chưa lưu, gọi đúng hàm “Lưu” của editor;
  (3) chờ máy chủ xác nhận **đúng** chữ + thẻ đó; (4) chụp snapshot ở đúng phiên bản được xác nhận;
  (5) gửi `request-public` với đúng `revision_id` vừa chụp; (6) đọc lại trạng thái. Bất kỳ chữ/thẻ
  nào phát sinh sau bước 1 **dừng** việc gửi với câu “Bạn đã gõ thêm khi đang lưu. Chưa gửi bản
  nào; lưu rồi bấm gửi lại.” — không chụp, không gửi bản khác.
- **IME:** chọn radio không đọc thân bài; bấm nút khi body đang tổ hợp **không bắt đầu gì** (vùng nhận
  `is_composing` từ editor, như phím tắt lưu chờ commit); sau commit cùng nút lưu và chụp đúng chữ đã
  commit. Enter không kích hoạt nút vùng này (vùng nằm trong khung `.vk-editor` nhưng mọi nút là
  `type=button`, nên Enter không gửi form).
- **Chống gửi trùng:** nút khóa ngay khi bắt đầu (không chờ response); Enter/chạm lặp bị bỏ.
- **Mất phản hồi:** “Kiểm tra lại” giữ nguyên key + payload; sau mất phản hồi của `request-public`/
  `set-private` trang đọc trạng thái trước, chỉ phát lại nếu thế hệ đã ghim vẫn còn hiệu lực.
- **Rút bài:** không chờ editor, không lưu, không chụp; ngăn gửi tiếp của lần gửi đang chờ.
- **Focus:** không tự chuyển focus khi làm mới. Khi control vừa bấm biến mất sau khi vùng dựng lại
  (“Đã hiểu”, “Kiểm tra lại”, “Gửi lại bản đã chụp”, hay hành động chính khi rút bài xong và không còn
  gì để bấm) focus **không** rơi về `<body>`: về hành động chính nếu còn, nếu không về tiêu đề
  `#publication-title` (`tabindex=-1`, không cuộn). Focus đang ở nơi khác không bị lấy. Khi nút còn
  trong DOM (đổi nhãn/trạng thái) focus ở nguyên tại nút.
  Escape/Back thuộc editor/NavigationGuard; vùng này không có overlay.
- **Reduced motion:** không có chuyển động mới; indicator pending của primitive dùng cơ chế sẵn có.

## Accessibility

`<section aria-labelledby="publication-title">`, tiêu đề h2. Radio là `fieldset role=radiogroup` có
legend. Thay đổi trạng thái đã xác nhận qua vùng `role=status` (polite, atomic) — một vùng duy nhất;
lỗi cần xử lý dùng `InlineNotice` với `announce` do primitive quyết định, không đọc lại hai lần.
Nút pending có tên truy cập “Đang …”. Liên kết kết quả là link thật. Thứ tự focus = DOM. Chữ 200% và
320 CSS px: mọi nhãn/hành động tới được bằng cuộn trang, không cuộn ngang.

## Invariants

Kiểm bằng browser thật trong `verify web` (phase `publication-author`), tọa độ CSS px:

1. `#publication` không cuộn ngang: `scrollWidth ≤ clientWidth` của `#publication` và của
   `document.documentElement` ở 320 CSS px với chữ 200% (dung sai 0; không có ngoại lệ).
2. Mọi nút/radio/liên kết trong `#publication` có vùng nhìn tới được bằng cuộn trang và cao ≥
   `--vk-size-hit-target` (dung sai 1 px làm tròn sub-pixel).
3. Không nhãn, lý do hay tiêu đề bị cắt: `scrollHeight` của phần tử chứa chữ ≤ `clientHeight` + 1 px
   (không `text-overflow: ellipsis`, không `overflow: hidden` trên chữ).
4. Làm mới trạng thái không remount editor: nút `#save` và vùng soạn giữ nguyên node và
   `scrollY` (dung sai 0); focus không đổi.
5. Chữ tổ hợp tiếng Việt và tiêu đề 200 ký tự hiển thị đủ dấu, xuống dòng không tràn.
6. (r2) Nhãn hành động không xuống dòng thành từng từ một: tối đa `max(2, ⌈số từ / 2⌉)` dòng ở mọi
   ô của ma trận (đo bằng `Range` rect của `.vk-ui-button__label`).
7. (r2) Sau khi một lệnh ổn định, `document.activeElement` không là `<body>` và nằm trong
   `#publication` hoặc ở nơi khác tác giả đã tự đặt.

## Compare

Đối chiếu theo ID vùng: hiện diện, cha–con, thứ tự, CTA, responsive, theme, trạng thái. Ma trận:
320 / 390 / desktop (≥ 1024) × sáng / tối × chữ 100% / 200% (ghi riêng) × {pending, uncertain,
conflict, public-r1 + pending-r2, private}. Báo riêng Functional, Geometry, Visual,
Accessibility/Responsive, Interaction. Ảnh phải được mở và đọc; ảnh tự chụp không thành mốc.

## Kết quả đối chiếu

Đối chiếu bản triển khai (`develop@23fffd8` + lát này) với spec, theo vùng. Bằng chứng chạy và ảnh:
[publication-author-web](../development/publication-author-web.md#bằng-chứng).

| Vùng / điểm | Kết quả |
| --- | --- |
| `#publication`, thứ tự DOM, một cột ở mọi viewport | Khớp |
| `#publication-summary` chỉ dữ kiện máy chủ; r1 công khai + r2 chờ nói r1 vẫn được đọc | Khớp |
| `#audience-choice`: chọn không gửi request; dòng “Máy chủ đang áp dụng” tách khỏi lựa chọn | Khớp |
| `#publication-actions`: một hành động chính theo bảng Hierarchy | Khớp |
| `#publication-recovery`: nằm sau tóm tắt, trước lựa chọn | Khớp; nút phục hồi là dạng Text trong khung `.vk-editor-recovery` (đổi sau vòng 1: dạng Filled tràn 19 px ở 320 px/200%) |
| `#review-detail`: liên kết `#editor-review-status` (id giữ nguyên cho phase cũ) | Khớp; dùng `LinkVariant::Standalone` để đủ mục tiêu chạm |
| Vùng nằm trong khung `.vk-editor`, ngoài `<form>` | **Lệch có chủ ý:** nằm *trong* `<form>` (cuối khung) với mọi nút `type=button`; Ctrl/Cmd+S vẫn dùng được khi focus ở vùng |
| Thêm so với spec | `#publication-live` (một vùng `role=status` cho tiến trình/kết quả), nút làm mới biểu tượng ở tiêu đề, dòng giải thích cạnh nút (riêng tư / gửi), dòng lý do khi nút bị khóa vì lệnh chưa rõ |
| IME | Nhấn khi đang tổ hợp không bắt đầu gì (thêm `is_composing` truyền từ editor); kiểm bằng IME mô phỏng |
| Invariants 1–5 | PASS ở 12 ô × 5 trạng thái, trừ ghi chú còn mở F-7 (nhãn “Kiểm tra lại” xuống ba dòng ở 320 px/200%) |
| Reduced motion, screen reader, bàn phím-only toàn luồng, cảm ứng thật | **NOT_RUN** |
