# Bề mặt, dashboard và trạng thái

Đọc cùng [ngữ cảnh sản phẩm](product-context.md). Các bố cục dưới đây là gợi ý
thiết kế, không khẳng định tính năng đã chạy. Dùng requirement ID từ nguồn hiện hành
để liên kết acceptance; không biến gợi ý thẩm mỹ thành lỗi blocker.

## Dashboard theo vai trò

| Vai trò / bề mặt | Nội dung ưu tiên | Hành động chính | Cần tránh |
| --- | --- | --- | --- |
| Tác giả / Bàn viết | Bản đang viết, audience, trạng thái lưu/duyệt từng bản, phản hồi liên quan | Viết tiếp, lưu nháp hoặc chọn công khai snapshot | Bảng KPI lượt thích che bản thảo; hứa autosave khi chỉ lưu thủ công |
| Người đọc / Khám phá & Bộ sưu tập | Bài hợp ngữ cảnh, lọc theo thẻ/hashtag, Đề xuất / Top 100 hữu hạn, bộ sưu tập cá nhân | Đọc một bài hoặc duyệt bộ sưu tập | Feed vô hạn trống, hero quá lớn, xếp hạng văn chương bằng sao, lộ tên/số đếm bộ sưu tập riêng tư |
| Xưởng Viết & Thảo luận | Tác phẩm, loại phản hồi tác giả mong muốn, thảo luận hai tầng và “Đánh giá cao” / “Hữu ích” | Đọc, ghi nhận và góp ý | Form nhận xét dài bắt buộc, áp lực tích điểm để được đăng |
| Quản trị viên / Kiểm duyệt | Bản/bình luận `NeedsAdmin`, quyết định override có lý do, case phản đối (`MOD-09/10`) | Xem đúng revision/bình luận và quyết định theo authority | AI score thành phán quyết không thể override; hứa đã gửi thông báo khi chưa nối kênh |
| *(Hoãn)* Biên tập / Tạp Chí (`DEFERRED` 10/11/2026) | Chỉ thiết kế khi user yêu cầu riêng | — | Đưa Tạp Chí hoặc nhiều chương vào luồng mặc định khi đang `DEFERRED` |

Nếu user muốn vài dashboard, hỏi ít nhất có thể: suy ra vai trò từ ngữ cảnh trước.
Không tự nhân ba app vì Vink có ba không gian. Có thể thử ba cách bố cục cho cùng
dashboard tác giả: **tiếp tục bản thảo**, **bàn làm việc gọn với phản hồi bên cạnh**,
**hành trình từ viết đến chia sẻ**. Giữ cùng dữ liệu/trạng thái giữa các phương án.
Dùng [LayoutSpec và ví dụ](../../vink-layout-spec/references/examples.md) để trình
bày cấu trúc trước khi sinh UI; đây là hướng đề xuất, không phải mặc định bắt có
ba phương án nếu người dùng đã chọn hướng hoặc chỉ giao sửa nhỏ.

## Trang đọc và nội dung mẫu

Tác phẩm dẫn dắt: title, bút danh, metadata hữu ích rồi văn bản. Phần giới thiệu
biên tập có chỗ riêng; công cụ lưu/góp ý không chen giữa từng khổ thơ. Đặt controls
đủ gần, có tên accessible, không che đoạn đang đọc. Chỉ dùng chapter navigator khi
tác phẩm có cấu trúc nhiều chương và tính năng thuộc scope.

Dùng mẫu tự viết ngắn khi cần kiểm layout; ghi rõ là mẫu:

> [Mẫu] **Chuyện nhỏ ở ngõ chiều mưa** — bút danh **Nguyễn Hoài Thương**
>
> “Mẹ để đèn ngoài hiên,” cô nói, rồi đứng nép sang một bên để chiếc áo mưa khỏi
> chạm vào tập giấy. Có những ngày người ta chỉ cần một chỗ khô ráo để viết tiếp.

Mẫu thơ tự viết để kiểm hard line break và khoảng khổ:

```text
[Mẫu]
Mưa khẽ chạm vào ô cửa nhỏ
Một dòng còn đợi nét mực khô

Chiều đi chậm qua hàng ngói cũ
Tiếng ai về sáng cả trang thơ
```

Kiểm cả chữ hoa có dấu, `Nguyễn`, `tưởng`, `khuỷu`, `ĐẶNG`, tên dài và chuỗi có
combining marks. Không suy ra IME runtime đã đạt từ việc font render mẫu đúng.

## Bộ trạng thái theo nhiệm vụ

| Nhiệm vụ | Trạng thái cần phân biệt | Cách hiển thị / bước tiếp |
| --- | --- | --- |
| Lưu nháp | Clean, local edits, saving, server saved, failed, conflict, uncertain | Giữ text; thông báo đúng bản; retry/reconcile/export khi có implementation |
| Gửi để đăng | Draft, snapshot submitted, waiting, needs policy revision, rejected, approved | Nêu snapshot và ý định tác giả; reason/appeal theo quyền; approved không tự đổi nhãn published |
| Bản công khai | Chưa có; bản cũ còn công khai; bản mới đã thay; withdrawn | Cho biết người đọc đang thấy bản nào; không lộ bản mới riêng tư |
| Tuyển chọn | Candidate, selected snapshot, reconfirmation needed, inactive | Lý do biên tập gắn đúng bản; không xóa quyền đọc Xưởng khi chỉ mất vị trí Tạp Chí |
| Phản hồi | Intent, loading, none, pending, sent, failed | Không giả thành công; giữ nội dung comment và tránh gửi lặp |
| Đọc/khám phá | Loading, useful empty, content, access unavailable, dependency error | Phân biệt lỗi với hết bài; nội dung đã công khai vẫn là trọng tâm |
| Rút bài | Vink public access closed, remote cleanup pending/failed/complete | Nêu phạm vi thực đã hoàn tất; không hứa thu hồi ngoài hệ thống |

Các tên trên là taxonomy thiết kế. Phải đối chiếu actual enum/API trước khi viết
mapping hoặc code. UI chỉ trình bày quyết định của domain/application owner.

## Ma trận responsive có mục tiêu

Với concept chưa có thiết bị chỉ định, dùng 390px mobile và 1440px desktop làm
giả định bố cục; đây không phải acceptance device đã chốt. Khi kiểm tra layout thực,
thêm màn hẹp khoảng 320px và tăng chữ/zoom cho vùng có nguy cơ. Nêu đúng viewport,
đơn vị CSS px, theme và text scale khi báo cáo.

Không chạy mọi tổ hợp vô hạn. Chọn trạng thái xấu nhất liên quan: title dài, lỗi lưu,
bàn phím mở, khổ thơ, không có bài, góp ý dài hoặc nhiều action theo quyền. Kiểm các
quan hệ: toolbar với editor, overlay với scroll, sticky action với bàn phím, sidebar
với độ rộng đọc, focus với dialog.

## Thành công cần quan sát

- Người mới có thể nhận ra một bài để đọc hoặc một cách bắt đầu viết.
- Tác giả tìm được bản thảo và biết thay đổi đã được server lưu hay chưa.
- Người đọc hiểu tác giả muốn cảm nhận hay góp ý kỹ thuật.
- Người duyệt/biên tập biết mình đang xem đúng revision và có quyền làm gì.
- Người dùng không bị nhầm “nháp”, “được duyệt”, “đã đăng” và “được tuyển”.

Đây là câu hỏi thiết kế/kiểm thử. Chỉ gọi đã đạt sau khi có bằng chứng phù hợp,
không suy ra từ mockup hoặc từ một nút được vẽ đúng.
