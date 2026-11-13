# Vùng công khai của Bàn viết — LayoutSpec r1

Phạm vi: **PUB-AUTHOR-WEB-01**. Vùng nối Bàn viết với API publication có sẵn
([publication-runtime](../development/publication-runtime.md)) và trang kiểm duyệt của tác giả
([moderation-runtime](../development/moderation-runtime.md#7-web-kiểm-duyệt)). Không đổi quyết định
sản phẩm; không phải signoff. Spec này do agent triển khai viết **trước** khi có mã UI, ở trạng
thái `proposed` (không có người duyệt riêng); phần “Kết quả đối chiếu” ghi những gì đã thực sự
chạy sau khi triển khai.

```text
LayoutSpec: Vùng công khai tại chỗ, r1 — proposed (agent triển khai, cùng người giao PUB-AUTHOR-WEB-01)
Context: tác giả đã đăng nhập, mở /ban-viet/{id}. Nội dung thật của tác giả không đưa vào tài liệu;
  dữ liệu mẫu trong fixture/e2e mang nhãn [Mẫu].
Intent: viết/duyệt trạng thái — vùng đọc trạng thái kèm một hành động chính. Tác giả phải làm được:
  (1) biết bản nào đang được người đọc đọc, bản nào đang chờ, quyết định của bản đó; (2) chọn
  “Công khai” và xác nhận gửi đúng bản vừa lưu; (3) chuyển “Chỉ mình tôi” mà không phải lưu hay bỏ
  chữ đang viết; (4) hiểu và xử lý mất phản hồi, xung đột, đổi phiên.
Sources: develop@23fffd8 (base; checkout sạch, relevant dirty inputs: không có). Đã đọc
  apps/vink-web/src/pages/desk.rs, desk_state.rs, tag_state.rs, pages/work_info.rs,
  pages/moderation.rs, style/vink.css (.vk-work-info, .vk-editor-recovery, .vk-notice__actions),
  packages/ui-system/leptos (RadioGroup, Button, InlineNotice, SupportingDetails).
  Gap: chưa có token/CSS riêng cho vùng này; dùng lại .vk-work-info làm công thức.
References: không có mốc được duyệt (NO_APPROVED_REFERENCE). Nhịp, tầng nền và chiều rộng kế thừa
  .vk-work-info (cùng trang, cùng surface-container-low) — chỉ ràng buộc nhịp/phân cấp, không phải
  mục tiêu so pixel. Ảnh tự chụp sau triển khai không thành golden.

Screen #draft-editor — /ban-viet/{id}
  Region #existing-editor — giữ nguyên: tiêu đề, thân bài, thẻ (#work-info), Lưu, Xem trước, rời trang
  Region #publication — section, aria-labelledby=publication-title, cuối trang sau thông tin tác phẩm
    Heading #publication-title — “Công khai” (h2, cùng kiểu .vk-work-info__heading)
    Region #publication-summary — chỉ dữ kiện máy chủ trả (role=status khi đổi)
      #publication-public    — “Đang công khai: «tiêu đề»” | “Chưa công khai. Chỉ bạn đọc được.”
      #publication-requested — “Đang gửi: «tiêu đề» — <trạng thái>” (+ lý do, nếu có)
      #publication-draft     — “Chữ chưa lưu không nằm trong bản đã gửi.” (chỉ khi editor còn chữ chưa lưu)
      #publication-unsent    — “Có N bản đã chụp nhưng chưa gửi duyệt.” (chỉ khi có)
    Region #audience-choice — RadioGroup “Ai đọc được tác phẩm”: Chỉ mình tôi / Công khai
      help: “Máy chủ đang áp dụng: …” (tách lựa chọn đang chọn khỏi trạng thái đã xác nhận)
    Region #publication-actions — đúng một hành động chính theo trạng thái + “Tải lại”
    Region #publication-recovery — thông báo mất phản hồi/xung đột/lỗi kèm đúng hành động phục hồi
    Region #review-detail — Link “Xem kết quả kiểm duyệt và phản đối” → /ban-viet/{id}/kiem-duyet
```

## Phương án đã chọn (và phương án bỏ)

| Quyết định | Chọn | Bỏ | Lý do |
| --- | --- | --- | --- |
| CHOICE-01 Lưu và gửi | **B**: “Lưu và gửi duyệt” gọi chính hàm lưu của editor (`DraftSession`) rồi chỉ **quan sát** kết quả; chụp snapshot đúng `draft_version` + `metadata_version` được máy chủ xác nhận | A: bắt tác giả bấm “Lưu” riêng rồi mới được gửi | Không sao chép logic lưu, không sửa editor. Nếu có chữ/thẻ mới sau khi bấm thì **dừng**, không chụp (xem Interaction). A vẫn là hành vi khi thẻ chưa tải hoặc lưu đang lỗi/xung đột: nút khóa kèm lý do |
| CHOICE-02 Trình bày | **A**: vùng gọn, mở rộng tại chỗ sau thông tin tác phẩm | B: nút trạng thái + dialog | Trạng thái “chưa xác nhận”, “xung đột” phải còn thấy và thao tác được sau khi gửi; ở chữ 200% và 320 CSS px vùng tại chỗ chỉ xếp dọc trong luồng trang, không thêm vùng cuộn hay bẫy focus; dialog che trạng thái lưu của editor mà tác giả đang cần đọc |
| CHOICE-03 Làm mới | **A**: đọc khi mở, khi quay lại tab/cửa sổ, sau mỗi lệnh và bằng “Tải lại” | B: A + polling hữu hạn | Kiểm duyệt không có SLA; polling thêm hẹn giờ, chồng request và rủi ro cướp focus/cuộn. Đảo được khi có số liệu |

Các lựa chọn trên là triển khai, không phải quyết định sản phẩm.

## Hierarchy

Thứ tự đọc/focus = thứ tự DOM: tiêu đề → tóm tắt → lựa chọn → hành động → phục hồi → liên kết kết
quả. Mỗi trạng thái có **một** điểm nhấn: hành động chính (Filled) *hoặc*, khi có sự cố, thông báo
phục hồi (vùng `.vk-editor-recovery`). Khi có sự cố thông báo đứng ngay dưới tóm tắt (trước lựa
chọn) để không bị đẩy xuống dưới hai nút khi chữ 200%. Hành động chính theo trạng thái:

| Điều kiện (từ dữ kiện máy chủ + editor) | Hành động chính |
| --- | --- |
| Chọn “Công khai”, editor sạch | “Gửi duyệt bản đã lưu” |
| Chọn “Công khai”, editor còn chữ/thẻ chưa lưu | “Lưu và gửi duyệt” |
| Chọn “Công khai”, đã có bản đang công khai/đang gửi | “Gửi bản mới” (cùng luồng) |
| Chọn “Chỉ mình tôi”, máy chủ đang công khai | “Chuyển riêng tư” |
| Chọn đúng trạng thái máy chủ đang áp dụng | không có hành động; chỉ giải thích một câu |
| Đang chờ phản hồi | cùng nút ở trạng thái pending (`ActionState::pending`), các nút khác khóa |
| Mất phản hồi | “Kiểm tra lại” |

Chọn radio **không gửi request**. Nút nói đúng việc sẽ làm; không dùng “Đăng”, “Xuất bản”.

## Sizing

Kế thừa `.vk-desk--editor` (chiều đọc `--vk-measure` + 2×`--vk-space-6`). Surface
`--vk-surface-container-low`, bo `--vk-radius-lg`, padding `clamp(--vk-space-3, 3vw, --vk-space-6)`,
khoảng cách `--vk-space-3`; vùng phục hồi dùng `.vk-editor-recovery` (error/warning container).
Không viền trang trí, không card/pill lồng nhau. Tiêu đề bản dài (200 ký tự) và lý do từ chối dài
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
- **Focus:** không tự chuyển focus khi làm mới. Gỡ thông báo (“Đã hiểu”) trả focus về hành động chính;
  xác nhận thành công giữ focus tại nút vừa bấm (nút vẫn còn trong DOM, đổi nhãn/trạng thái).
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
