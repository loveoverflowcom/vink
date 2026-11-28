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
Phần này ghi geometry lịch sử R3.1. [#616](#ngữ-cảnh-và-trục-phản-hồi--616)
cập nhật riêng hai biên và nền nhóm. Controller, API, authority và recovery vẫn giữ owner.

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
| D2T-03-load-retry | Đọc ban đầu lỗi: bài vẫn đọc được | “Thử lại” gọi lại resource nhận xét tại chỗ sau hydration |
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
| `SendEmphasis`: Tonal khi trống, Filled khi có chữ, Progress khi gửi; lý do khóa từ `SendIntent` liên kết qua `aria-describedby` | `discussion_state`, `FeedbackComposer.send_variant` | Biến thể nút “Gửi” |
| `ReplyTarget`: Root “Trả lời bình luận gốc” / Reply “Đang trả lời X” + × về gốc (chỉ khi gốc nhận trả lời) / Unavailable “Chọn bình luận khác”; không suy “đã xóa”; đổi đích giữ chữ và ACK muộn không xóa chữ | `discussion_state` | Chip chữ, `IconButton` ×, nút chuyển focus |
| Thu ô giữ chữ: `collapsed_with_work` + `reopen` | `discussion_state` | “Bản trả lời được giữ lại · Mở lại” |
| “Bạn đã gửi” + `OwnChip` (chữ + icon + tone) | `discussion_reads` | Heading đổi tên, chip `data-tone` |
| Gỡ có xác nhận tại chỗ: `ask_remove`/`confirming`/`take_confirmed`; một mục mỗi lúc; tải lại, lỗi đọc, mất `can_remove`, đổi phiên đều hủy; DELETE chỉ sau “Gỡ” | `discussion_reads`, `OwnRemoveAction`, `RemoveConfirmation` | “Giữ lại” nhận focus, “Gỡ” `Danger` |
| Đọc: rỗng khi có kết quả không lỗi. Phân trang theo cursor | `pages/discussion.rs` (`shows_empty`, `has_more_roots`) | “Chưa có nhận xét nào được hiển thị.” và `#root-list[data-has-more]` |
| Bình luận đã tắt: “Bạn đã tắt…” chỉ khi API `viewer_is_work_author` | `closed_comments_message`, `DiscussionController.viewer_is_author` | Dòng trạng thái và lý do khóa ô soạn |

Chưa làm trong lát này: “Sao chép nháp” (clipboard, cần browser thật), skeleton trễ 300 ms, góc
tác giả đủ năm trạng thái ảnh 08 (đang đổi/xung đột/hệ thống khóa vẫn dùng `mutation_*` hiện có),
trang ẩn ảnh 10 với nhãn Ưng dạng chữ, anatomy/geometry R4 của lát A và CMP product. Phản hồi chờ
duyệt hiện cạnh ô soạn và trong “Bạn đã gửi”, không chèn hàng tạm vào nhánh.

## LayoutSpec R4 — lát UI (#359, #360; 20/11/2026)

Áp dụng trực tiếp lên vùng `#feedback-region` của Reader Web và `DiscussionThread` /
`FeedbackComposer` trong `vink-patterns`, không mở rộng sang CMP product hay Đề xuất 08 (nền trang
ngoài thẻ vẫn giữ `--vk-surface`).

### Cây vùng và ranh giới

Delta [#328](https://github.com/loveoverflowcom/vink/issues/328) ngày 26/11/2026 đưa Auth về lần bấm Gửi và giảm copy trong trạng thái mở. [LayoutSpec Auth Reader](auth-reader-web.md) sở hữu dialog.

- **`#feedback-region.vk-discussion`** (`max-inline-size: var(--vk-measure)`, giữ trong vùng đọc):
  - **`.vk-discussion-head`**: `<h2 id="feedback-heading">` `"Nhận xét"`. Thứ tự cũ đến mới có trong DOM của danh sách.
  - **`.vk-discussion-readonly-notice`** (chỉ hiện khi `ReplacementView` ẩn / gỡ sau kiểm duyệt): hộp tonal `"Trang này chỉ đọc"`, nêu rõ không gửi mới/bấm `Ưng`, vẫn xem bình luận và gỡ mục của chính mình nếu còn quyền.
  - **`#comments-mode.vk-discussion-mode`** giữ mode đọc được trong DOM và nút Mở/Tắt bình luận theo quyền tác giả. Khi mở, khách và người đọc không có quyền đổi thiết lập sẽ đi thẳng vào ô soạn. Hàng trạng thái chỉ chiếm chỗ khi có thao tác, thông báo quan trọng hoặc nhãn fixture. Lỗi đọc nền có một “Thử lại”. Xung đột hoặc lần đổi chưa xác nhận dùng “Kiểm tra trạng thái bình luận” để đọc lại trạng thái. Cả hai nằm cạnh thông báo của vùng nhận xét.
  - **`.vk-discussion-composer` (`Root` / `Reply`)**: thẻ `--vk-surface-container-low` (radius `28px`), khay `FloatingTonal` `--vk-surface-container-high` ở trạng thái nghỉ và `--vk-surface` + viền trong `2px solid var(--vk-primary)` khi focus; `FieldMode::ReadOnly` khi `Closed` để giữ node, giá trị và selection. Khi `Sending`, textarea vẫn nhận chữ mới. Lần gửi dùng snapshot cố định, nút Gửi khóa đến khi có kết quả.
    - Hàng đích (chỉ ô trả lời): chip `"Trả lời bình luận gốc"` hoặc chip tonal `--vk-primary-container` `"Đang trả lời X"` + nút `×` (`>=44px` vùng chạm) + nút `"Thu ô · Giữ chữ"`; khi thu ô hiện pill `.vk-discussion-reply-kept` `"Bản trả lời được giữ lại"` + `"Mở lại"`. Ô nhập đã mở vẫn giữ cùng node và vùng chọn khi thu. Mở lại đưa focus về đúng ô đó. [Ảnh khi thu/mở giữa một POST trên `bb25af4`](evidence/discussion-r4-366/fold-bb25af4.jpg).
    - Đích không còn hiển thị: khối cảnh báo `--vk-warning-container` (`"Bình luận được trả lời không còn hiển thị"`, `"Sao chép nháp"` dùng `navigator.clipboard.writeText`, `"Chọn bình luận khác"` Filled), không suy `"đã xóa"`.
    - Vùng biên nhận / từ chối: `CommittedPending` (hổ phách `--vk-warning-container`), `Uncertain` (xanh `--vk-info-container` + `"Kiểm tra lần gửi"` Filled + lý do khóa `Gửi` qua `aria-describedby`), `Refused` (đỏ `--vk-error-container`), `SignInAvailable` (cùng nút Gửi, mở Auth sau khi người đọc kích hoạt với chữ đủ điều kiện), `SignInClosed` (`lock` + lý do khóa cạnh nút `Gửi` qua `aria-describedby`), `Closed` (khối khóa `lock` + chữ `"Bạn đã tắt bình luận cho bài viết này"` khi `viewer_is_author` + `ReadOnly` draft + `"Chỉ đọc lúc này"` + `"Sao chép nháp"`).
    - Chân ô soạn: ghi chú kiểm duyệt (`verified-user` + `"Duyệt trước khi hiện công khai"`) và đúng một nút `Gửi` (`SendEmphasis::Tonal` khi trống, `Filled` khi có chữ; chặn kích hoạt khi toàn khoảng trắng hoặc đang composition; khi `Sending`, `.vk-ui-button__status` `LoadingIndicator` thay chỗ icon `send` tại vị trí leading để không nhảy bề rộng).
  - **`#own-comments` (“Bạn đã gửi”)** có tiêu đề và “Chỉ bạn thấy trước khi được duyệt”. Chip Pending, Rejected, ApprovedOffPage và Unavailable giữ nghĩa R4. ApprovedOffPage cho biết bình luận đã duyệt nằm ngoài phần công khai đang tải. Chỉ hiện “Gỡ” khi máy chủ cấp quyền. Lỗi đọc riêng hoặc lỗi xác nhận quyền tạm thời có một “Thử lại” tại vùng này.
  - **`.vk-discussion-remove` (`OwnRemoveAction`)**: khi mở xác nhận, chuyển thành khung cảnh báo viền `2px solid var(--vk-error)` tại chỗ với `"Gỡ nhận xét này? Sau khi gỡ, bạn không thể khôi phục."`, `"Giữ lại"` (nhận focus) và `"Gỡ"` (`ButtonVariant::Danger`), không thêm modal hay toast hoàn tác.
  - **Đọc / Skeleton / Lỗi / Phân trang**:
    - Skeleton tĩnh (`use_loading_gate`: trễ `300 ms`, hiện tối thiểu `500 ms`; không dùng pulse/shimmer, không mang class `.vk-discussion-comment`) + `role="status"` cho screen reader.
    - Lỗi đọc lần đầu hiện thẻ “Lỗi tải nhận xét” và “Thử lại” sau hydration. Rỗng hiện “Chưa có nhận xét nào được hiển thị.” Nhận xét không còn được cấp hiện trạng thái không khả dụng.
    - Giữ giới hạn nhập, cảnh báo mất nháp và disclosure kiểm duyệt. Ô soạn nhận Unicode và giữ Enter xuống dòng.
    - Lỗi tải thêm nằm ngay trên nút `"Xem thêm nhận xét"`, giữ nguyên các nhận xét đã tải và cursor cũ; lỗi nhánh nằm trong `.vk-discussion-reply-group` kèm `"Thử lại"`.
  - **`#root-list` (`.vk-discussion-root` + `.vk-discussion-thread`)**:
    - Metadata `data-order="oldest-first"` và `data-has-more` nằm ở danh sách công khai. Nút tải thêm dùng cùng cursor.
    - Thẻ root `--vk-surface-container-low` (radius `28px`, padding `16px`).
    - Nhóm thao tác root `.vk-discussion-actions`: **`Ưng · n` trước → `Trả lời` sau** (+ `OwnRemoveAction` tách sau nhóm). Trên root (khi hai nút cùng hiện), cặp `Ưng | Trả lời` dùng cụm pill tonal ghép sát (`gap: 2px`, góc ngoài `24px`, góc trong `6px`, chiều cao chạm `>=48px`; mặc định `--vk-secondary-container`, khi `Ưng` `data-pressed="true"` hoặc `Trả lời` `aria-expanded="true"` chuyển `--vk-primary-container`). Khi tác giả chuyển tác phẩm sang riêng tư hoặc tác phẩm bị rút qua kiểm duyệt (`ReplacementView`), mỗi nhận xét hiện nhãn chữ tĩnh `Ưng` không số, kể cả khi trước đó đã có số xác nhận. Ẩn nút Ưng, Trả lời và ô soạn. Chỉ hiện “Gỡ nhận xét của tôi” khi `can_remove=true`.
    - Nhóm `.vk-discussion-reply-group`: hộp tonal lồng một tầng `--vk-surface-container` (radius `20px`), tiêu đề `"Phản hồi"` (không số đếm giả); phản hồi bên trong dùng nút chữ nhỏ theo thứ tự `Ưng · n` → `Trả lời` (+ `OwnRemoveAction`), `min-block-size: 44px`. Đích trả lời đang chọn được đánh dấu viền/nhãn `"Đang là đích trả lời"`.
    - **Responsive**:
      - `< 1024px`: thân nhận xét, nhóm thao tác, `.vk-discussion-reply-group` và `.vk-discussion-reply-composer` dùng cùng hai biên thẻ root, không thụt lề phụ; hai nút ở `320px`/chữ `200%` tự wrap, không cắt nhãn hay tràn ngang (`scrollWidth <= innerWidth + 1`).
      - `>= 1024px`: ở cấp root, thân, nhóm thao tác, nhánh và ô trả lời thẳng cột tên tác giả. Xem [hợp đồng trục tên R5](#danh-tính-nhận-xét--615) để lấy kích thước đang áp dụng.

### Đối chiếu trước / sau

| Bề mặt | Trước lát UI R4 | Sau lát UI R4 |
| --- | --- | --- |
| Icon & nhãn | Còn nhãn `"Hữu ích"` ở vài biến thể/rời rạc, thiếu `send`/`reply`/`copy`/`schedule`/`visibility-off`/`verified-user`/`lock`/`cancel` | Icon Material chính thức qua `icons.json` (Web + CMP); `EndorsementAction` dùng `heart-outline`/`heart` + `"Ưng"` |
| Cụm root & desktop | `Trả lời` trước `Ưng`; không thẻ tonal; mọi màn cùng biên trái | `Ưng · n \| Trả lời` pill liền góc trong `6px`; desktop `>=1024px` thụt thân/nhánh/ô trả lời theo cột tên |
| Ô soạn & trả lời | 1 hộp delivery đơn sắc; Gửi vẫn dùng `ArrowBack` xoay + spinner cạnh phải; thiếu `"Sao chép nháp"` clipboard ở `Closed` | 9 trạng thái tonal đúng ảnh 04; vòng tiến trình thay icon `send`; chip đích + `"Sao chép nháp"` clipboard thật; `ReadOnly` giữ node & text |
| `"Bạn đã gửi"` & Gỡ | Chip màu trung tính cho mọi `ApprovedOffPage`; confirm gỡ kiểu inline thô | Thẻ `20px`, chip `Success` xanh lá cho `ApprovedOffPage`, lý do riêng màu đỏ cho `Rejected`, xác nhận gỡ viền `2px solid var(--vk-error)` |
| Đọc, lỗi, tác giả, trang ẩn | Fallback `"Đang tải…"`, 1 dòng mode, trang ẩn vẫn hiện nút `Ưng`/composer | Skeleton tĩnh sau `300 ms`; mode 5 trạng thái tác giả + xung đột; `ReplacementView` hiện chữ tĩnh `Ưng` không số |



### Đối chiếu R4 trên Reader Web Linux

Đã đối chiếu ảnh R4 01–08 và 10 cùng các biến thể desktop với Reader. Ảnh 04 giữ textarea editable khi đang gửi. Riêng ảnh 10, Mạnh đã chốt nhãn tĩnh “Ưng” không số cho cả tác phẩm chuyển riêng tư và tác phẩm bị rút. Reader giữ quyền gỡ theo `can_remove`. Hợp đồng đọc và quyền truy cập được giữ nguyên. Phép đo tối kiểm màu chữ của receipt trên chính nền thông tin; nhãn field và hai pill phải đạt 4,5:1 cả trong khi đổi theme. [Ảnh trước/sau, SHA và lệnh kiểm](../development/discussion-ui-evidence.md).

## Nhận xét R5 · #561

[Hướng đã duyệt và ảnh tham chiếu](https://github.com/loveoverflowcom/vink/issues/561) ·
[Semantic UI #560/#570](../development/web-semantics.md#từ-yêu-cầu-tới-bằng-chứng) ·
[Auth theo ý định #328/#572](auth-reader-web.md). R5 thay phần trình bày R4. Quyền
ghi, premoderation, xác nhận gỡ, cursor, thứ tự và biên nhận vẫn theo hợp đồng
đang áp dụng.

```text
#feedback-region · cùng measure với Reader
  h2 Nhận xét
  #comments-mode · trạng thái ngoại lệ, thao tác tác giả hoặc retry khi cần
  #root-composer · nhãn Viết nhận xét, textarea tonal, Gửi có nhãn
  #own-comments · lượt tải riêng, mục riêng, lỗi hoặc phân trang thật
  #root-list · gốc/neo theo thứ tự cũ → mới
    Bút danh và thời gian · nội dung có xuống dòng · Ưng · Trả lời
    Trục phản hồi · phản hồi phẳng có ngữ cảnh khi trỏ tới phản hồi khác
      Ô soạn · icon Trả lời, Trả lời [bút danh], Đóng, textarea Viết phản hồi, Gửi
  Phân trang khi còn cursor, trạng thái rỗng/lỗi/phục hồi đúng ngữ cảnh
```

- Reader phối hợp state và transport trong `RootThreadView` và dùng lại
  `DiscussionThread`/`FeedbackComposer`. Nhận xét gốc dùng nền trang và khoảng
  cách. Bút danh dùng Be Vietnam Pro đậm hơn phần nội dung Literata. Tên, thời
  gian và hành động được xuống dòng ở chữ lớn mà vẫn cùng cột đọc.
- Gửi dùng nút Filled có chữ khi sẵn sàng. Giới hạn xuất hiện cùng lý do cần sửa.
  Khi có chữ, nhắc giữ nháp và duyệt công khai theo ngữ cảnh. Bản nháp, selection,
  IME, lần gửi và khả năng đối soát thuộc cùng controller. Đóng trả focus về
  đích trả lời hoặc control phục hồi của nhánh.
- Dùng `data-mode-kind` cho policy, `aria-expanded` cho nút mở ô trả lời,
  nhãn hành động đã xác nhận của Ưng và mô tả textarea trỏ vào đích hiện tại.
  Chữ tương đối được tính tại lúc trình bày ở trình duyệt. `time[datetime]`
  giữ dữ liệu thật cùng nhãn thời gian tuyệt đối đọc được qua trợ năng.

### Danh tính nhận xét · #615

- Gốc dùng avatar Reader 44 px và phản hồi dùng Medium 40 px. Avatar tròn, giữ đường kính
  khi thiếu chỗ. Khe avatar và cụm tên lấy từ `--vk-space-3` (12 px ở chữ 100%).
  Bút danh tối đa hai dòng, handle tóm tắt một dòng. Tên và handle đầy đủ vẫn nằm trong
  tên trợ năng của cùng một liên kết hồ sơ. Nhãn trung tính giữ avatar người mặc định
  và văn bản thuần. Thời gian nằm ngoài liên kết, cạnh tên khi vừa chỗ và xuống hàng
  theo trục tên khi thiếu chỗ.
- `--vk-discussion-root-author-inset` ở owner `patterns.css` bằng đường kính avatar gốc
  cộng khe của bút danh (56 px ở chữ 100%). Từ 1024 CSS px, thân gốc, thao tác, lý do
  chung và trục phản hồi lấy cùng vị trí này. Dưới breakpoint, phần gốc ở biên
  bài đọc và trục phản hồi dùng gutter giới hạn theo #616.

Kiểm ảnh Chrome của Product ở sáng/tối, 1440/100%, 390/100–150% và 320/200%.
Ca trả lời mất đích và response phân trang dùng fixture có nhãn. Tách ảnh
Preview khỏi kết quả kiểm Product.

### Ngữ cảnh và trục phản hồi · #616

[Issue #616](https://github.com/loveoverflowcom/vink/issues/616) ghi hướng chủ sản phẩm
chọn ngày 27/11/2026. [Ảnh c248](https://drive.google.com/file/d/1vnhc2zJ50wQ5pUkI82Ztstyg-SPc72fG/view)
là ảnh hiện trạng của ô trả lời, dùng để đối chiếu phân cấp. Cập nhật này thay riêng
yêu cầu đồng biên và nhóm tonal của #134/#128 cùng kiểu trình bày ngữ cảnh lịch sử.
Thứ tự cũ đến mới, ownership hai tầng và quyền công khai tiếp tục theo
[ADR](../architecture/discussion-ownership.md). `VDR-DISCUSSION-R4@r1` vẫn sở hữu
vùng riêng, xác nhận gỡ, biên nhận chờ duyệt và thông báo đóng.

- `DiscussionThread` sở hữu một wrapper trình bày từ phản hồi đầu đến điểm cuối
  của phản hồi, phân trang hoặc bản soạn còn hiển thị. Ở desktop từ 1024 CSS px,
  wrapper nằm trên trục `--vk-discussion-root-author-inset` (56 px ở chữ 100%).
  Màn hẹp thụt theo `--vk-space-6`, giới hạn 20–28 CSS px. Đường dọc 1 CSS px dùng
  `--vk-outline-variant` giữa gutter, bên ngoài chữ, avatar và vùng focus. Khi nhánh
  rỗng và ô soạn đóng, wrapper không chiếm chỗ. Neo gốc trung tính dùng cùng lane.
- Các phản hồi chia chung một biên, kể cả khi phản hồi trỏ tới một phản hồi khác.
  Nhánh dùng nền Reader. Dấu tonal của đích đang chọn vẫn theo trạng thái editor.
  `ul` có tên “Phản hồi theo thứ tự cũ đến mới”. Pseudo-element trang trí không nhận pointer
  hay focus và không thêm node vào cây trợ năng.
- Đầu ô soạn dùng icon Reply trang trí phía trước, “Trả lời” regular ở
  `on-surface-variant`, tên semibold ở `on-surface`, nút đóng ở cuối. Ngữ cảnh đã đăng
  dùng cùng phân cấp chữ với “Đang trả lời”. Tên dài giới hạn hai dòng. Textarea vẫn nhận tên đầy đủ của đích qua `aria-describedby`. Tên đã đăng có
  link hồ sơ khi projection cấp `ProfileLink`. Hành động đến nhận xét chỉ xuất hiện
  khi đích công khai đã tải. Ô soạn dùng tên dạng văn bản lấy từ draft hợp lệ,
  và mô tả phục hồi dùng cùng ID khi đích mất.
- `RootThreadView` đưa ô soạn và bản thu gọn vào slot cố định của nhánh. Controller
  tiếp tục giữ nháp, selection, focus, IME, biên nhận và quyền gửi. Khi thu ô, native
  textarea được giữ. Mở lại đưa focus về đúng field. Đường nối dừng ở toàn bộ khối
  cuối đang hiển thị, kể cả khi khối đó là ô soạn hoặc “Bản trả lời được giữ lại”.

[Ánh xạ DOM/AX](../development/web-semantics.md#từ-yêu-cầu-tới-bằng-chứng) ·
[Lệnh và phạm vi evidence](../development/discussion-ui-evidence.md).
