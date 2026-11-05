# Ghi nhận văn học (LVE) — yêu cầu sản phẩm

**Phiên bản:** 0.1 · **Ngày:** 05/11/2026 · **Trạng thái:** Đề xuất; lát lõi thuần đã hiện thực,
chưa có API, lưu trữ, giao diện hay auth authority.
**Nguồn:** epic [#73](https://github.com/loveoverflowcom/vink/issues/73), body ngày 04/11/2026 (bản
tóm tắt PRD r1 trên Drive; file Drive không được đọc lại khi promote). Tài liệu này **thay thế**
snapshot đó cho FR/UC/INV; issue vẫn là tracker kế hoạch.

Tài liệu này sở hữu ý nghĩa sản phẩm và các ID `LVE-FR`, `LVE-UC`, `LVE-INV`. Các lựa chọn
`CHOICE-01..12`, semantics đã chọn cho từng lát và hợp đồng cho lát sau nằm ở
[decision record](../docs/architecture/literary-endorsement.md); phần đã hiện thực, bằng chứng và
gap nằm ở [registry](../docs/development/literary-endorsement-core.md). ID `LVE-*` không thay thế
D/INV/S/COM hiện hành; LVE kế thừa COM-03..08, MOD-01 và PUB-02/03 trong
[requirements](requirements.md).

## 1. Mục tiêu và ranh giới

Tạo tín hiệu **ghi nhận giá trị tác phẩm và chất lượng lời góp ý**, tách khỏi cảm xúc (reaction),
lưu bài và báo cáo. Tín hiệu giúp tìm nội dung đáng đọc và phản hồi hữu ích mà không biến Vink
thành bảng điểm văn chương. Một lượt ghi nhận **không chứng minh người ghi nhận là chuyên gia**,
không là điểm nghệ thuật khách quan và không cấp quyền kiểm duyệt, xuất bản hay tuyển chọn.

Đã chốt từ yêu cầu của epic:

- Bao phủ **tác phẩm, bình luận gốc và phản hồi**; giữ hai tầng, không sinh “đáp án đúng”.
- Vink sở hữu bằng Rust + PostgreSQL; Discourse chỉ để học hỏi. Không thêm service, Redis hay
  framework reputation/CQRS mặc định.
- Core thuần; actor và quyền do server xác định, kiểm lại tại ranh giới ghi bền.
- Giữ SSR công khai, nháp riêng, Unicode, nội dung chưa lưu, COM-07 (bật/tắt bình luận) và quyền
  kiểm duyệt.
- Không tự mở downvote, sao 1–5, bảng xếp hạng tác giả, huy hiệu chuyên gia, AI chấm văn hoặc đăng
  bài theo số phiếu. Mỗi phần đó cần yêu cầu riêng.
- Không dùng biểu tượng trái tim. Hướng icon (ngòi bút + máy bay giấy hướng lên) còn OPEN.

Tiêu chí thành công (đo khi thử nghiệm, chưa có số liệu): người thử phân biệt được ghi nhận với
Lưu/Gửi/biểu cảm; hiểu trạng thái đang chờ và đã xác nhận; bỏ hoặc sửa được; tìm góp ý mà không
mất mạch đọc. Không bịa mức tăng tương tác hay hiệu năng.

## 2. Yêu cầu chức năng

Cột “Lát lõi” chỉ nói phần quyết định thuần đã có; chưa có yêu cầu nào đạt nghiệm thu runtime.

| ID | Kết quả cần có | Lát lõi (05/11/2026) |
| --- | --- | --- |
| LVE-FR-01 | Người có quyền thật ghi nhận/bỏ ghi nhận tác phẩm, bình luận gốc hoặc phản hồi; cardinality theo CHOICE-01 | Quyết định cho cả ba loại đích; quyền thật cần auth (D-05) |
| LVE-FR-02 | Tách tổng số có nguồn với trạng thái của chính người xem; “không khả dụng” không thành 0 | `Summary::Unavailable` khác `Count(0)`; `OwnView` riêng |
| LVE-FR-03 | Trạng thái mong muốn, không bật/tắt mù; thử lại, chạm đúp, nhiều tab, ACK đảo thứ tự không tạo hiệu ứng trùng hay hồi sinh ý định cũ | Version chấp nhận ý định (cả khi trùng trạng thái) làm hàng rào; model + Kani |
| LVE-FR-04 | Ghi nhận qua sửa/xuất bản lại có anchor rõ, truy được bản đã được đánh giá (CHOICE-02) | Anchor theo revision người đọc đã đọc |
| LVE-FR-05 | Đọc/ghi/tổng số/cursor cùng bảo vệ visibility; không lộ nháp, bình luận chờ duyệt hoặc tác phẩm đã rút | Quyết định và projection; truy vấn SQL/cursor chưa có |
| LVE-FR-06 | Sắp xếp “hữu ích” chỉ khi có policy/capability thật; không đảo thứ tự phản hồi hoặc giật hội thoại đang đọc | Chưa (CHOICE-06 OPEN); lõi không đổi thứ tự nào |
| LVE-FR-07 | Ghi nhận tách biểu cảm Noto, lưu bài, báo cáo; không đổi COM-07 bằng nhãn UI | Module riêng, không dùng catalog reaction |
| LVE-FR-08 | Các trạng thái mặc định/đã chọn/đang chờ/không chắc/bị từ chối/không khả dụng/vô hiệu có lý do; bàn phím, focus, chữ 200%, giảm chuyển động | Chưa (UI) |
| LVE-FR-09 | Dữ liệu chuẩn và dựng lại/đối soát được; chống lạm dụng/lưu giữ có owner/version; bộ đệm RAM không là nguồn bền | Số đếm là projection dựng lại được; SQL/rebuild chưa có |
| LVE-FR-10 | Skeleton/fixture không giả thành công; nghiệm thu production cần auth/authority/persistence thật | Không có route hay stub |

**Copy khởi đầu:** tác phẩm “Đánh giá cao”; nhận xét/phản hồi “Hữu ích”. Khi đã chọn, tên truy cập
là “Bỏ đánh giá cao”/“Bỏ đánh dấu hữu ích”. Không diễn giải số đếm thành “X chuyên gia đánh giá”
hay “Đạt chuẩn văn học”. Copy trạng thái đề xuất: *Đánh giá cao / Hữu ích / Chưa xác nhận được /
Kiểm tra lại / Không thể đánh giá lúc này*; không hiện “Đã ghi nhận” chỉ vì vừa bấm.

## 3. Use cases và tình huống lỗi

| ID | Tình huống | Hành vi và oracle | Lát lõi |
| --- | --- | --- | --- |
| LVE-UC-01 | Khách đọc | Đọc bài/tổng số được phép; chưa có auth thì không giả đăng nhập hay ghi nhận thành công | Projection công khai; HTTP chưa có |
| LVE-UC-02 | Ghi nhận tác phẩm | Đúng đích/anchor; chỉ xác nhận sau kết quả bền/đọc lại đúng người | Quyết định + model |
| LVE-UC-03 | Ghi nhận bình luận gốc/phản hồi | Chỉ đích đó đổi; không cộng dây chuyền lên gốc/tác phẩm; vẫn một tầng con | Quyết định + model (gốc và phản hồi) |
| LVE-UC-04 | Bỏ ghi nhận / lặp thao tác | Lặp trạng thái mong muốn không thêm hiệu ứng; delta đúng, không âm | Quyết định + Kani |
| LVE-UC-05 | Bấm nhanh khi đang chờ | Tuần tự hóa/gộp theo hợp đồng; ACK cũ không đè ý định mới; đường bỏ rõ | Hàng rào phía server; nghĩa vụ client ghi ở decision record |
| LVE-UC-06 | Mất ACK/timeout/abort | Không chắc → đọc lại trạng thái hoặc gửi lại cùng yêu cầu; timeout không chứng minh rollback | `AlreadyCurrent`, đọc lại trong model |
| LVE-UC-07 | Nhiều tab/thiết bị | Version có owner; yêu cầu đến trễ không tự thắng; không hứa biết “ý cuối” giữa nhiều máy | Model có hai thiết bị của một người |
| LVE-UC-08 | Đổi actor/session/route | ACK/đọc cũ không đổi UI/số đếm của session/đích mới; cache cá nhân cô lập | Chưa (consumer) |
| LVE-UC-09 | Đích bị ẩn/xóa/rút giữa chừng | Kiểm lại khi commit; từ chối không lộ thông tin; projection/cursor không hồi sinh nội dung/danh tính | Quyết định + model; cursor chưa có |
| LVE-UC-10 | Xuất bản phiên bản mới | Theo CHOICE-02; không âm thầm chuyển ghi nhận sang nội dung khác | `RevisionChanged`, bản trước không được đếm |
| LVE-UC-11 | Tác giả tự ghi nhận | Theo CHOICE-08; tác giả không sửa/xóa ghi nhận của người khác; không tự cộng | Bị chặn |
| LVE-UC-12 | Tắt bình luận | COM-07 vẫn chặn tạo chữ; ghi nhận theo CHOICE-09 riêng | Chặn ghi nhận mới trên bình luận; gỡ vẫn được |
| LVE-UC-13 | Sắp xếp “hữu ích”/phân trang | Chỉ khi có capability; cursor/epoch/làm mới/tie-breaker rõ; phản hồi theo thời gian | Chưa |
| LVE-UC-14 | DB/mạng/tổng số lỗi | Giữ bài/góp ý; thử lại/lỗi cục bộ; không biến lỗi thành 0 hay đã xác nhận | `Summary::Unavailable`; application chưa có |
| LVE-UC-15 | Spam/trùng/nhiều tài khoản | Policy/giới hạn có owner; actor do server xác định; không kết luận gian lận chỉ từ IP | Chưa |
| LVE-UC-16 | Bàn phím/cảm ứng/chữ 200%/giảm chuyển động | Tên/pressed/busy không chỉ bằng màu/hover; focus ổn định; không lặp hoạt ảnh trong reader | Chưa |
| LVE-UC-17 | Xóa/cách ly tài khoản, dựng lại | Lưu giữ/hiệu lực theo policy; không đếm trùng hay hồi sinh; visibility hiện hành thắng snapshot | Ràng buộc không xóa hàng khi bỏ chọn; vòng đời tài khoản chưa có |
| LVE-UC-18 | Góp ý chuyên sâu | Không bắt viết luận để ghi nhận; facets/rubric tự nguyện nếu CHOICE-01 B được chọn; ý định không là ACL | Không áp dụng (CHOICE-01 A) |

## 4. Bất biến đề xuất

Bảng truy vết từng bất biến tới model, code, Kani, PostgreSQL và gap nằm ở
[registry](../docs/development/literary-endorsement-core.md#registry-truy-vết-lve-inv).

| ID | Bảo đảm |
| --- | --- |
| LVE-INV-01 | Đích có kiểu đúng loại và đúng phạm vi tác phẩm/bình luận |
| LVE-INV-02 | Chỉ principal/quyền hiện hành mới được ghi; không tin actor hay số đếm từ client |
| LVE-INV-03 | Một trạng thái hiệu lực trên mỗi actor + đích theo cardinality đã chọn |
| LVE-INV-04 | Lặp/thử lại trạng thái mong muốn có tối đa một hiệu ứng; nếu dùng receipt thì khóa/payload lệch bị từ chối |
| LVE-INV-05 | Yêu cầu/version cũ không thắng cái mới; bỏ/tạo lại không gây ABA hay reset version |
| LVE-INV-06 | Thu hồi/quyền/visibility được xét tại commit và khi đọc; cache/event trễ không hồi sinh quyền |
| LVE-INV-07 | Số đếm khớp nguồn hiệu lực, không âm/tràn; projection có freshness/version và dựng lại được |
| LVE-INV-08 | Không lộ nội dung riêng/chờ duyệt hay danh tính người ghi nhận ngoài policy; trạng thái của người xem không vào cache dùng chung |
| LVE-INV-09 | Sắp xếp gốc có hợp đồng; phản hồi giữ thứ tự thời gian và hai tầng |
| LVE-INV-10 | ACK/đọc áp đúng actor/session/đích/thế hệ ý định; lỗi không làm mất bản góp ý |
| LVE-INV-11 | Ghi nhận không tự cấp quyền xuất bản/kiểm duyệt/tuyển chọn/chuyên môn |
| LVE-INV-12 | Revision/tự ghi nhận/tắt bình luận theo policy version đã chọn, không trộn semantics |

## 5. Ngoài phạm vi mặc định

Auth/SSO hoàn chỉnh, cuộc thi/trả thưởng, social graph, thông báo toàn hệ thống, danh hiệu chuyên
môn, ứng dụng native, tự động chọn bài cho Tạp Chí. Auth là dependency: không được dựng giả để báo
production PASS.
