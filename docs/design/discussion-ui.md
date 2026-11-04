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
        Region #reply-list — một mức thụt lề; flat chronological replies
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

Desktop/tablet giữ cột đọc; mobile 320/390 có một khoảng lệch `space-3` cho toàn bộ
replies, không lồng card theo target. Action wrap và input co theo chiều rộng; không
đặt chiều cao chữ cố định. Light/dark cùng cây, dùng semantic tokens hiện có. Không
có overlay mới. PR #57 đã đưa canonical Google manifest/pilot vào base; tích hợp
picker/caret/IME còn là phần chưa triển khai, API typed sticker/reaction chưa có.
Unicode gõ trực tiếp vẫn giữ nguyên. Context trong danh sách dùng
anchor có quyền; context ngoài trang chỉ hiện nhãn API cùng giải thích chưa có
endpoint tra cứu đúng scope, không quét toàn bộ hội thoại hoặc đoán nội dung.

## Ma trận tương tác và trạng thái

| Scenario | Facts / kết quả trình bày | Bảo toàn và hành động |
| --- | --- | --- |
| D2T-03-public | SSR chỉ gọi anonymous read, roots + tối đa 3 preview từ provider | Không serialize own-pending hoặc identity; bài vẫn đọc được nếu discussion lỗi |
| D2T-03-sharing-open / sharing-closed | Sharing chỉ là intent, comments mode là fact riêng | Giữ input ở cùng vị trí; Closed chặn gửi, không xóa chữ/thread |
| D2T-03-no-authority | Production chưa có authority/session | Input soạn cục bộ, gửi disabled có lý do; không nút login giả, không dựng principal |
| D2T-03-paging | Cursor opaque, roots/replies cũ → mới | Dedupe theo ID, không suy has-more từ count; lỗi chỉ ở nhánh liên quan |
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

Discussion HTTP có deadline 30 giây cho mỗi transport attempt. Timeout/abort giữ outcome chưa rõ và snapshot/key; timer bị hủy cùng feature owner. Thu ô trả lời chỉ đổi trình bày, không hủy transport. Scope đã được kiểm ở controller; production chưa có luồng đổi account/session để nghiệm thu runtime.
