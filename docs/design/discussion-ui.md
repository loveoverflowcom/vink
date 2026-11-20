# Thảo luận hai tầng — LayoutSpec D2T-03

Ngày 03/11/2026; hướng đã được chọn trong issue #45 và handoff. Base `develop`
`14c33c95f6ddc77e2990f37b741841df942ea3c5`. Đây là đặc tả phần triển khai được từ
contract hiện tại, không phải bằng chứng nghiệm thu production.

Người đọc muốn trao đổi sau tác phẩm; tác giả có quyền đổi comments mode từ API.
Thứ tự đọc giữ nguyên hướng đã chọn, bổ sung đúng giới hạn provider:

```text
Screen #reader
  Region #reader-paper — tiêu đề, tác giả, bản công khai
  Region #feedback-region
    Row #feedback-heading — Nhận xét; thứ tự cũ → mới mà API hỗ trợ
    Region #comments-mode — mở/đóng từ settings; author control khi API cấp quyền
    Region #root-composer — input có nhãn; lý do chưa gửi được; receipt riêng
    Region #root-list
      Region #root-card — root hoặc neo trung tính
        Region #root-body — tác giả thật, UTC, visibility, body escaped, own-remove
        Region #reply-group — một mặt tonal; heading Phản hồi một lần
          Region #reply-list — flat chronological replies, không thụt thêm
          Region #reply-item — context → nội dung → action
        Region #reply-pagination — cursor thật, lỗi cục bộ
        Region #reply-composer — một composer/root, target đổi không mất chữ
    Region #root-pagination — cursor thật hoặc điểm kết thúc
    Region #reply-recovery — nhánh ngoài page giữ text/lần gửi chưa rõ; receipt đã nhận có status riêng
```

`ReaderHeader`, `FeedbackComposer`, `FeedbackItem`/`FeedbackThread` và pattern
thảo luận thuộc `packages/vink-patterns`; primitive từ `vink-ui`. App adapter giữ
DTO, transport, scope và draft. Không tạo account/avatar/feedback intent từ tên hay
work ID. Provider chưa có feedback intent, root total hay ranking: không dựng số
tổng, selector “Mới nhất/Nổi bật” hoặc intent của tác giả. Sharing trong mẫu độc lập
với comments mode ở cả hai renderer. `FeedbackIntent::Unspecified` không có nhãn riêng (tiêu đề `#feedback-heading` và nhãn textarea đã gọi tên form); các intent có tên giữ nhãn của chúng.

Desktop/tablet giữ cột đọc sau rail. Root/neo, nhóm Phản hồi và composer cùng hai biên;
không thụt replies theo target. Action wrap và input co theo chiều rộng; không
đặt chiều cao chữ cố định. Light/dark cùng cây, dùng semantic tokens hiện có. Không
có overlay mới. PR #57 đã đưa canonical Google manifest/pilot vào base; tích hợp
picker/caret/IME còn là phần chưa triển khai, API typed sticker/reaction chưa có.
Unicode gõ trực tiếp vẫn giữ nguyên. Context trong danh sách dùng
anchor có quyền; context ngoài trang chỉ hiện nhãn API cùng giải thích chưa có
endpoint tra cứu đúng scope, không quét toàn bộ hội thoại hoặc đoán nội dung.


## Nhận xét R3.1 / desktop R1.3 — #134, 10/11/2026

Hướng C đã chọn từ [issue #134](https://github.com/loveoverflowcom/vink/issues/134),
[LayoutSpec R3.1](https://drive.google.com/file/d/1S4xxyEpjtt4r6g9i8G8iJviWR_c_dQ8i/view)
và [desktop R1.3](https://drive.google.com/file/d/1G18GyQHrmYdxplNd9kdsn05hKRjl5AW8/view).
Phần này thay geometry lịch sử D2T-03; controller, API, authority và recovery vẫn giữ owner.

```text
#feedback-region — cùng measure Reader
  #root-composer — một native TextArea, label thật trong tonal surface
  #root-list
    #thread-[root-id]
      #root — nền trang, tên 600; ngày/context nhỏ; avatar bên phải
      #reply-group — surface-container, heading Phản hồi
        #reply-list — một tầng cũ → mới; 16px giữa replies
        #reply-pagination — slot từ app, đúng cursor và lỗi
      #branch-endorsement-reason — chỉ gộp khi facts đồng nhất
      #reply-composer — cùng hai biên, surface-container-high, bo 28px
        #context / #collapse — giữ chữ, focus về đúng trigger
        #native-field / #send — nhãn nổi, một chủ focus, Gửi + mũi tên ngang
      #delivery — ngoài chữ đang gõ, tonal bo 20px
        Chưa rõ kết quả gửi → Dùng lại nội dung lần trước, giữ nháp.
        Kiểm tra lần gửi — filled; Gửi bị chặn có mô tả lý do
```

Inset cân 16px ở 390/100, 12px ngang ở 320; giữ px khi chữ 200%.
Khoảng vùng 12px, replies 16px, nhánh 32px. Root trên nền trang;
không viền cây hoặc divider lặp. Native field không bị thay khi đang gõ.
Nhãn `Chờ duyệt · Chưa công khai` ở đúng item/receipt; cảnh báo rời trang chỉ hiện khi có chữ.

**CHOICE-01 B:** `DiscussionThread` sở hữu anatomy và slot footer DTO-free;
`RootThreadView` giữ cursor/error/transport. **CHOICE-02 A:** `ToggleAction` có
`blocked_description` opt-in; từng target giữ count/value/gate riêng và tham chiếu
lý do chung. Chỉ gộp ≥2 target công khai, mọi facts đã có, `Idle`, cùng enum
`SignedOut` hoặc `Unavailable`; mixed, thiếu facts, sending/uncertain/failed giữ note
ở target. Own-pending không có đích endorsement. Không suy từ copy hoặc cộng count.

Pen Flight chưa có trong registry ở base #138; tiếp tục do #73 sở hữu.
#134 dùng lại slot hiện có, không thêm glyph tạm hay thay policy. #128 tiếp tục
sở hữu các delta visual khác; #129 giữ input/clipboard/recovery chưa triển khai;
#130 CMP, #131/#132 nav nằm ngoài lát này.
Bằng chứng, điều kiện render và phần chưa chạy ở [evidence](../development/discussion-ui-evidence.md).

## Ma trận tương tác và trạng thái

| Scenario | Facts / kết quả trình bày | Bảo toàn và hành động |
| --- | --- | --- |
| D2T-03-public | SSR chỉ gọi anonymous read, roots + tối đa 3 preview từ provider | Không serialize own-pending hoặc identity; bài vẫn đọc được nếu discussion lỗi |
| D2T-03-sharing-open / sharing-closed | Sharing chỉ là intent, comments mode là fact riêng | Giữ input ở cùng vị trí; Closed chặn gửi, không xóa chữ/thread |
| D2T-03-no-authority | Production chưa có authority/session | Input soạn cục bộ, gửi disabled có lý do; không nút login giả, không dựng principal |
| D2T-03-paging | Cursor opaque, roots/replies cũ → mới | Dedupe theo ID, không suy has-more từ count; lỗi tải thêm hiện cạnh nút (`#root-pagination` sau `#root-list`; lỗi replies trong `.vk-discussion-root`), giữ phần đã tải và bấm lại gọi đúng cursor cũ; `#comments-mode` giữ lỗi làm mới cả vùng. Lỗi đọc không mô tả kết quả ghi chưa rõ; lỗi phiên/quyền giữ lý do riêng |
| D2T-03-load-retry | Đọc ban đầu lỗi: bài vẫn đọc được | “Tải lại nhận xét” là Button gọi lại resource (request mới), không phải link cùng URL + fragment chỉ cuộn trang |
| D2T-03-context | Reply-to-reply vẫn thuộc root ban đầu | Chỉ đổi target, giữ text; context unavailable không tiết lộ tên/snippet cũ |
| D2T-03-submitting | Một snapshot + command key + actor/session/work/root | Không gửi đôi; người dùng gõ tiếp; ACK chỉ xóa đúng snapshot chưa đổi |
| D2T-03-pending | Receipt `pending`, hoặc trạng thái khác đúng response | Không giả public/count/participant; refresh lấy projection mới |
| D2T-03-uncertain | Network, malformed success hoặc server không rõ kết quả | Giữ snapshot/key; đối soát bằng POST lại đúng payload/key theo API, không key mới |
| D2T-03-definitive | 401/403/404/409/422 phân biệt theo contract | Giữ chữ; Closed refresh policy; target unavailable bỏ context theo chủ ý; không tự gửi lại |
| D2T-03-scope | Đổi work/actor/session hoặc response cũ | Cô lập state; response cũ không tác động scope mới; không localStorage chung |
| D2T-03-author-mode | Chỉ API `viewer_is_work_author` + authority thật | PUT expected_generation; pending/error/conflict riêng; refresh trước lần đổi kế tiếp |
| D2T-03-own-remove | Chỉ API `can_remove` | DELETE repeatable; không cho work author xóa người khác; refresh projection |

Input native/HTML giữ newline, Unicode, composition; Enter chỉ xuống dòng, không
shortcut gửi. State đổi không remount textarea. Draft là bộ nhớ của route hiện tại,
không phải đã lưu; rời trang có thể mất chữ. Không nhận localStorage/cookie hay URL
để giả auth. Chưa có nguồn actor/session production: write adapter/controller có
seam kiểm chứng riêng, không biến preview thành integration.

## Đối chiếu và giới hạn bằng chứng

Regression dùng controller production cho stale ACK, typing mới, scope đổi,
double-submit, closed policy và lost ACK. Pattern projection chia sẻ scenario IDs
Web/CMP; CMP không có service consumer/native app. Quint hiện tại chỉ sở hữu core;
không gọi test Rust consumer là model-check/Kani. Browser, persistence, OS IME,
screen reader, responsive/theme screenshots và native runtime cần evidence riêng.
PR #56 đã merge tại `27927c4` để sửa editor selection/resize; base đã tích hợp. Kết quả quick/Web/hai preview và giới hạn ở [evidence](../development/discussion-ui-evidence.md).

Trạng thái gửi/duyệt mở rộng cho thiết kế sau (Refused, Held, quy tắc thử lại, ma trận cho designer)
nằm ở [brief #120](discussion-submission-brief.md); brief không đổi phần đã triển khai ở đây.

Discussion HTTP có deadline 30 giây cho mỗi transport attempt. Timeout/abort giữ outcome chưa rõ và snapshot/key; timer bị hủy cùng feature owner. Thu ô trả lời chỉ đổi trình bày, không hủy transport. Scope đã được kiểm ở controller; production chưa có luồng đổi account/session để nghiệm thu runtime.

## Delta public/own Reader Web (10/11/2026)

SSR và browser dùng public projection; vùng “Nhận xét của bạn” (R4: “Bạn đã gửi”) sau root composer giữ own
pending/rejected hoặc kết quả gửi ngoài trang. [DecisionMap, LayoutSpec delta, scope và bằng chứng](../development/discussion-web-reads.md)
sở hữu lát này; anatomy R3.1, native composer và Pen Flight giữ nguyên.

## Nhận xét R4 — lát logic (#359, #360; 20/11/2026)

Thiết kế R4 (`DESIGN_PROPOSED`, ảnh 01–10 trong
[thư mục Drive](https://drive.google.com/drive/folders/1OpT6_uY1GG-oGvhSl6b5xsmgSsM7hVdE)) chia hai
lát: A ([#359](https://github.com/loveoverflowcom/vink/issues/359), Ưng + anatomy) và B
([#360](https://github.com/loveoverflowcom/vink/issues/360), trạng thái). Lát này chỉ đưa **logic
và facts** vào controller/patterns, nối tối thiểu vào Reader Web để hành vi quan sát được. Geometry
R4 (nhóm nút liền góc trong 6px, nhóm Phản hồi tonal, desktop thụt theo cột tên, vùng tonal theo màu,
skeleton 300 ms, vòng tiến trình thay icon) còn `NOT_IMPLEMENTED` và thuộc lát UI kế tiếp.
Quyết định: [delta “Ưng”](../architecture/literary-endorsement.md#delta-thuật-ngữ-20112026--359)
và `VDR-DISCUSSION-R4@r1` (`PRODUCT_CONFIRMED`, signoff 20/11/2026).

<!-- vdr: active VDR-DISCUSSION-R4@r1 -->

| Facts / quy tắc | Owner | Trình bày tối thiểu hiện nay |
| --- | --- | --- |
| `ComposerStatus` một trạng thái chính: `Sending` > `Uncertain` > `Closed` > khách (`SignInAvailable`/`SignInClosed`) > biên nhận (`CommittedPending`, `Refused`) > `Draft`; focus thuộc trình bày | `discussion_state` | `data-status` trên `.vk-discussion-composer` |
| `Receipt` có kiểu (Pending/Approved/Rejected/Removed/Uncertain/Refused(mã)/SessionChanged) + `NoticeTone` (waiting hổ phách, unknown xanh, adverse đỏ, neutral) thay chuỗi | `discussion_state` | `data-tone` trên `.vk-discussion-delivery`, copy cũ giữ nguyên |
| `SendEmphasis`: Tonal khi trống, Filled khi có chữ, Progress khi gửi; lý do khóa vẫn là `reason()` + `aria-describedby` | `discussion_state`, `FeedbackComposer.send_variant` | Biến thể nút “Gửi” |
| `ReplyTarget`: Root “Trả lời bình luận gốc” / Reply “Đang trả lời X” + × về gốc (chỉ khi gốc nhận trả lời) / Unavailable “Chọn bình luận khác”; không suy “đã xóa”; đổi đích giữ chữ và ACK muộn không xóa chữ | `discussion_state` | Chip chữ, `IconButton` ×, nút chuyển focus |
| Thu ô giữ chữ: `collapsed_with_work` + `reopen` | `discussion_state` | “Bản trả lời được giữ lại · Mở lại” |
| “Bạn đã gửi” + `OwnChip` (chữ + icon + tone) | `discussion_reads` | Heading đổi tên, chip `data-tone` |
| Gỡ có xác nhận tại chỗ: `ask_remove`/`confirming`/`take_confirmed`; một mục mỗi lúc; tải lại, lỗi đọc, mất `can_remove`, đổi phiên đều hủy; DELETE chỉ sau “Gỡ” | `discussion_reads`, `OwnRemoveAction`, `RemoveConfirmation` | “Giữ lại” nhận focus, “Gỡ” `Danger` |
| Đọc: rỗng chỉ khi không lỗi; điểm kết thúc chỉ khi danh sách không rỗng và hết cursor | `pages/discussion.rs` (`shows_empty`, `shows_list_end`) | Giữ nguyên copy |
| Bình luận đã tắt: “Bạn đã tắt…” chỉ khi API `viewer_is_work_author` | `comments_mode_message`, `DiscussionController.viewer_is_author` | Dòng trạng thái và lý do khóa ô soạn |

Chưa làm trong lát này: “Sao chép nháp” (clipboard, cần browser thật), skeleton trễ 300 ms, góc
tác giả đủ năm trạng thái ảnh 08 (đang đổi/xung đột/hệ thống khóa vẫn dùng `mutation_*` hiện có),
trang ẩn ảnh 10 với số Ưng dạng chữ, anatomy/geometry R4 của lát A và CMP product. Phản hồi chờ
duyệt hiện cạnh ô soạn và trong “Bạn đã gửi”, không chèn hàng tạm vào nhánh.

## LayoutSpec R4 — lát UI (#359, #360; 20/11/2026)

Áp dụng trực tiếp lên vùng `#feedback-region` của Reader Web và `DiscussionThread` /
`FeedbackComposer` trong `vink-patterns`, không mở rộng sang CMP product hay Đề xuất 08 (nền trang
ngoài thẻ vẫn giữ `--vk-surface`).

### Cây vùng và ranh giới

- **`#feedback-region.vk-discussion`** (`max-inline-size: var(--vk-measure)`, giữ trong vùng đọc):
  - **`.vk-discussion-head`**: `<h2 id="feedback-heading">` `"Nhận xét"` (trái) + `<span class="vk-discussion-order">` `"Cũ đến mới"` tĩnh (phải; không dropdown, không tổng giả).
  - **`.vk-discussion-readonly-notice`** (chỉ hiện khi `ReplacementView` ẩn / gỡ sau kiểm duyệt): hộp tonal `"Trang này chỉ đọc"`, nêu rõ không gửi mới/bấm `Ưng`, vẫn xem bình luận và gỡ mục của chính mình nếu còn quyền.
  - **`#comments-mode.vk-discussion-mode`**: năm trạng thái ảnh 08 cho tác giả / trạng thái đọc + `#discussion-refresh` (`Tải lại nhận xét`): Mở (chấm xanh `--vk-success` + `"Bình luận đang mở"` + `"Tắt bình luận"` Tonal), Đang đổi (`LoadingIndicator` + `"Đang cập nhật…"`), Xung đột/lỗi (`--vk-info-container` / `--vk-error-container` + `"Tải lại nhận xét"` Filled), Tác giả tắt (`--vk-surface-container-highest` + `lock` + `"Bạn đã tắt bình luận"` + `"Mở bình luận"` Tonal), Hệ thống khóa (`--vk-surface-container-highest` + `verified-user` + `"Hệ thống đang khóa bình luận"`). Dòng nhãn thử nghiệm cục bộ (`local fixture`) giữ nguyên nếu có.
  - **`.vk-discussion-composer` (`Root` / `Reply`)**: thẻ `--vk-surface-container-low` (radius `28px`), khay `FloatingTonal` `--vk-surface-container-high` ở trạng thái nghỉ và `--vk-surface` + viền trong `2px solid var(--vk-primary)` khi focus; `FieldMode::ReadOnly` khi `Closed` để giữ node, giá trị và selection. Khi `Sending`, textarea vẫn nhận chữ mới. Lần gửi dùng snapshot cố định, nút Gửi khóa đến khi có kết quả.
    - Hàng đích (chỉ ô trả lời): chip `"Trả lời bình luận gốc"` hoặc chip tonal `--vk-primary-container` `"Đang trả lời X"` + nút `×` (`>=44px` vùng chạm) + nút `"Thu ô · Giữ chữ"`; khi thu ô hiện pill `.vk-discussion-reply-kept` `"Bản trả lời được giữ lại"` + `"Mở lại"`.
    - Đích không còn hiển thị: khối cảnh báo `--vk-warning-container` (`"Bình luận được trả lời không còn hiển thị"`, `"Sao chép nháp"` dùng `navigator.clipboard.writeText`, `"Chọn bình luận khác"` Filled), không suy `"đã xóa"`.
    - Vùng biên nhận / từ chối: `CommittedPending` (hổ phách `--vk-warning-container`), `Uncertain` (xanh `--vk-info-container` + `"Kiểm tra lần gửi"` Filled + lý do khóa `Gửi` qua `aria-describedby`), `Refused` (đỏ `--vk-error-container` + CTA đăng nhập lại nếu mất phiên và đăng nhập đang mở), `SignInAvailable` (nhắc đăng nhập + CTA Filled `#...-auth`), `SignInClosed` (`lock` + lý do khóa cạnh nút `Gửi` qua `aria-describedby`), `Closed` (khối khóa `lock` + chữ `"Bạn đã tắt bình luận cho bài viết này"` khi `viewer_is_author` + `ReadOnly` draft + `"Chỉ đọc lúc này"` + `"Sao chép nháp"`).
    - Chân ô soạn: ghi chú kiểm duyệt (`verified-user` + `"Duyệt trước khi hiện công khai"`) và đúng một nút `Gửi` (`SendEmphasis::Tonal` khi trống, `Filled` khi có chữ; khi `Sending`, `.vk-ui-button__status` `LoadingIndicator` thay chỗ icon `send` tại vị trí leading để không nhảy bề rộng).
  - **`#own-comments` (`"Bạn đã gửi"`)**: tiêu đề `"Bạn đã gửi"` + `"Chỉ bạn thấy trước khi được duyệt"` + `#own-comments-refresh`; từng mục đặt trong thẻ `--vk-surface-container-low` (radius `20px`) kèm `OwnChip`: `Pending` (hổ phách + `schedule`), `Rejected` (đỏ + `cancel` + hộp `"Lý do chỉ bạn thấy"`), `ApprovedOffPage` (xanh lá `--vk-success-container` + `check` + `"Tải lại nhận xét"`), `Unavailable` (trung tính + `visibility-off`).
  - **`.vk-discussion-remove` (`OwnRemoveAction`)**: khi mở xác nhận, chuyển thành khung cảnh báo viền `2px solid var(--vk-error)` tại chỗ với `"Gỡ nhận xét này? Sau khi gỡ, bạn không thể khôi phục."`, `"Giữ lại"` (nhận focus) và `"Gỡ"` (`ButtonVariant::Danger`), không thêm modal hay toast hoàn tác.
  - **Đọc / Skeleton / Lỗi / Phân trang**:
    - Skeleton tĩnh (`use_loading_gate`: trễ `300 ms`, hiện tối thiểu `500 ms`; không dùng pulse/shimmer, không mang class `.vk-discussion-comment`) + `role="status"` cho screen reader.
    - Lỗi tải lần đầu (thẻ đỏ + `"Chưa tải được nhận xét"` + `"Tải lại nhận xét"` gọi GET mới) tách biệt hoàn toàn với rỗng (`chat-bubble-outline` + `"Chưa có nhận xét nào được hiển thị."`).
    - Lỗi tải thêm nằm ngay trên nút `"Xem thêm nhận xét"`, giữ nguyên các nhận xét đã tải và cursor cũ; lỗi nhánh nằm trong `.vk-discussion-reply-group` kèm `"Thử lại"`. `"Đã hết danh sách nhận xét được cấp."` chỉ hiện khi không rỗng và `next_cursor.is_none()`.
  - **`#root-list` (`.vk-discussion-root` + `.vk-discussion-thread`)**:
    - Thẻ root `--vk-surface-container-low` (radius `28px`, padding `16px`).
    - Nhóm thao tác root `.vk-discussion-actions`: **`Ưng · n` trước → `Trả lời` sau** (+ `OwnRemoveAction` tách sau nhóm). Trên root (khi hai nút cùng hiện), cặp `Ưng | Trả lời` dùng cụm pill tonal ghép sát (`gap: 2px`, góc ngoài `24px`, góc trong `6px`, chiều cao chạm `>=48px`; mặc định `--vk-secondary-container`, khi `Ưng` `data-pressed="true"` hoặc `Trả lời` `aria-expanded="true"` chuyển `--vk-primary-container`). Ở trang chỉ đọc (`ReplacementView`), bỏ `Trả lời` và hiện `Ưng · n` dạng text chip tĩnh, không có `<button>` ghi.
    - Nhóm `.vk-discussion-reply-group`: hộp tonal lồng một tầng `--vk-surface-container` (radius `20px`), tiêu đề `"Phản hồi"` (không số đếm giả); phản hồi bên trong dùng nút chữ nhỏ theo thứ tự `Ưng · n` → `Trả lời` (+ `OwnRemoveAction`), `min-block-size: 44px`. Đích trả lời đang chọn được đánh dấu viền/nhãn `"Đang là đích trả lời"`.
    - **Responsive**:
      - `< 1024px`: thân nhận xét, nhóm thao tác, `.vk-discussion-reply-group` và `.vk-discussion-reply-composer` dùng cùng hai biên thẻ root, không thụt lề phụ; hai nút ở `320px`/chữ `200%` tự wrap, không cắt nhãn hay tràn ngang (`scrollWidth <= innerWidth + 1`).
      - `>= 1024px`: ở cấp root, thân, `.vk-discussion-actions`, `.vk-discussion-reply-group` và `.vk-discussion-reply-composer` thụt `padding-inline-start` thẳng cột tên tác giả (`52px`, khớp avatar `40px` + khe `12px`); các phản hồi bên trong `.vk-discussion-reply-group` giữ thẳng biên nhóm, không thêm tầng thụt thứ ba.

### Đối chiếu trước / sau

| Bề mặt | Trước lát UI R4 | Sau lát UI R4 |
| --- | --- | --- |
| Icon & nhãn | Còn nhãn `"Hữu ích"` ở vài biến thể/rời rạc, thiếu `send`/`reply`/`copy`/`schedule`/`visibility-off`/`verified-user`/`lock`/`cancel` | Icon Material chính thức qua `icons.json` (Web + CMP); `EndorsementAction` dùng `heart-outline`/`heart` + `"Ưng"` |
| Cụm root & desktop | `Trả lời` trước `Ưng`; không thẻ tonal; mọi màn cùng biên trái | `Ưng · n \| Trả lời` pill liền góc trong `6px`; desktop `>=1024px` thụt thân/nhánh/ô trả lời theo cột tên |
| Ô soạn & trả lời | 1 hộp delivery đơn sắc; Gửi vẫn dùng `ArrowBack` xoay + spinner cạnh phải; thiếu `"Sao chép nháp"` clipboard ở `Closed` | 9 trạng thái tonal đúng ảnh 04; vòng tiến trình thay icon `send`; chip đích + `"Sao chép nháp"` clipboard thật; `ReadOnly` giữ node & text |
| `"Bạn đã gửi"` & Gỡ | Chip màu trung tính cho mọi `ApprovedOffPage`; confirm gỡ kiểu inline thô | Thẻ `20px`, chip `Success` xanh lá cho `ApprovedOffPage`, lý do riêng màu đỏ cho `Rejected`, xác nhận gỡ viền `2px solid var(--vk-error)` |
| Đọc, lỗi, tác giả, trang ẩn | Fallback `"Đang tải…"`, 1 dòng mode, trang ẩn vẫn hiện nút `Ưng`/composer | Skeleton tĩnh sau `300 ms`; mode 5 trạng thái tác giả + xung đột; `ReplacementView` chỉ đọc với `Ưng · n` dạng chữ |

