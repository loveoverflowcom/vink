# Vision Vink — Việt Ink

**Phiên bản:** 0.2 · **Ngày:** 30/10/2026 · **Trạng thái:** Đề xuất sản phẩm để review.

## 1. Lời hứa của sản phẩm

**Vink giúp người Việt viết đều hơn, được đọc với sự tôn trọng và trưởng thành qua những phản hồi có ích.** Người đọc tìm thấy tác phẩm được trình bày có chăm chút, tiếng nói mới và một cộng đồng có thể tham gia bằng nhiều mức độ: đọc, lưu, chia sẻ cảm nhận hoặc góp ý kỹ thuật.

Sự khác biệt cần được thể hiện trong trải nghiệm hằng ngày: một người có đoạn tản văn chưa hoàn chỉnh vẫn có thể chia sẻ; một người viết truyện giữ được các phiên bản và biết độc giả đang đọc bản nào; một người đọc mới hiểu vì sao một tác phẩm được giới thiệu.

Thước đo giá trị là vòng lặp **viết → được đọc → nhận phản hồi → viết tiếp**. Lượng bài đăng và tương tác chỉ có ý nghĩa khi hỗ trợ vòng lặp này.

## 2. Người dùng trọng tâm

| Nhóm | Nhu cầu chính | Lời hứa Vink cần thực hiện |
| --- | --- | --- |
| Người mới tập viết | Có nơi thử nghiệm, xin góp ý mà không chịu áp lực xếp hạng | Dễ bắt đầu, được hướng dẫn nhẹ, nội dung hợp lệ có cơ hội tiếp cận |
| Người viết thường xuyên | Soạn, lưu bản nháp, chỉnh sửa, xuất bản và theo dõi tác phẩm | Bản thảo đáng tin cậy, phiên bản rõ ràng, phản hồi gắn với nội dung |
| Người đọc yêu văn chương | Tìm bài phù hợp tâm trạng, thể loại và thời gian đọc | Trình đọc dễ chịu, tuyển chọn có lời dẫn, dễ lưu và quay lại |
| Người góp ý và biên tập | Giúp tác giả phát triển, giới thiệu tiếng nói mới | Có ngữ cảnh, quy tắc phản hồi và quyền hạn rõ ràng |

**Ưu tiên beta:** người viết cá nhân và người đọc chủ động; bắt đầu với thơ, tản văn, truyện ngắn, nhật ký công khai và bài tập sáng tác. Tiểu thuyết nhiều chương là hướng phát triển có chủ đích, cần được chuẩn bị trong mô hình nội dung từ đầu.

“Trẻ trung” là định hướng thẩm mỹ và giọng giao tiếp. Việc nhận người dùng vị thành niên là quyết định sản phẩm riêng, ảnh hưởng đến privacy, nội dung, liên hệ giữa thành viên và nhân lực kiểm duyệt. Nhóm beta đầu có thể là người trưởng thành được mời; đây là đề xuất, chưa phải giới hạn tuổi đã chốt.

## 3. Ba không gian của Vink

Đây là ba phạm vi trải nghiệm, có thể dùng chung trang tác phẩm và các component. Không bắt buộc triển khai thành ba ứng dụng, ba tab cố định hoặc ba dashboard độc lập.

### Bàn viết

Bàn viết là nơi riêng tư để soạn, lưu nháp và sửa. Người dùng phải biết bản nào đã lưu, bản nào đang chờ duyệt và bản nào đang công khai. Lưu nháp không đồng nghĩa gửi bản thảo cho AI hay xuất bản.

Người viết có thể đặt mục tiêu cho tác phẩm và chọn cách nhận phản hồi khi chia sẻ. Những ghi chú riêng hoặc lịch sử thử nghiệm không trở thành hồ sơ công khai mặc định.

### Xưởng Viết

Xưởng Viết dành cho tác phẩm được tác giả chủ động chia sẻ và đáp ứng chính sách cộng đồng. Bài viết còn vụng về, ngắn, thử nghiệm hoặc chưa đạt chuẩn tạp chí vẫn có chỗ đứng.

Tác giả chọn tín hiệu phản hồi: **Chỉ chia sẻ**, **Muốn nghe cảm nhận** hoặc **Xin góp ý kỹ thuật**. Tín hiệu này giúp người đọc hiểu mong đợi, không tạo quyền miễn trừ trước báo cáo vi phạm hoặc mọi ý kiến trái chiều.

Góp ý tốt có thể bắt đầu từ một chi tiết: “Đoạn này khiến tôi dừng lại”, “Tôi chưa hiểu động cơ của nhân vật”, “Khổ thơ cuối đổi nhịp khá mạnh”. Giao diện gợi mở mà không bắt người đọc điền biểu mẫu dài.

### Tạp Chí

Tạp Chí là mặt tuyển chọn: một chủ đề, một tuyển tập, một bài được giới thiệu cùng lý do biên tập. Tác phẩm có thể xuất hiện ở cả Xưởng Viết và Tạp Chí; không cần tạo hai bản nội dung độc lập.

Tuyển chọn là sự giới thiệu có trách nhiệm. Nó gắn với **phiên bản cụ thể** đã được đọc và chọn. Một bản sửa mới không tự động thừa kế sự tuyển chọn của bản cũ. Khi tác giả rút tác phẩm, quyền hiển thị hiện tại vẫn được tôn trọng; tuyển chọn trước đó không giữ quyền công khai vô thời hạn.

**Mặc định pilot đề xuất:** mục Tạp Chí chỉ hiển thị khi snapshot được chọn vẫn là bản công khai hiện hành. Khi tác giả đăng bản mới, mục tuyển chọn tạm ngừng cho đến khi biên tập xác nhận lại; Xưởng Viết tiếp tục hiển thị bản mới. Chưa cần xây kho phiên bản công khai trong MVP.

| Khía cạnh | Bàn viết | Xưởng Viết | Tạp Chí |
| --- | --- | --- | --- |
| Mục đích | Sáng tác riêng | Chia sẻ và phát triển | Khám phá có biên tập |
| Ai chủ động đưa nội dung vào? | Tác giả | Tác giả gửi để đăng | Biên tập chọn nội dung đã đủ điều kiện và được phép giới thiệu |
| Điều kiện | Quyền của chủ bản thảo | Chính sách cộng đồng và ý định xuất bản | Điều kiện của Xưởng cộng tiêu chí tuyển chọn |
| Cách đọc | Preview riêng | Tác phẩm đang công khai | Snapshot được tuyển chọn, kèm ngữ cảnh |

## 4. Trải nghiệm đầu tiên cần đáng nhớ

Một người mới đến Vink nhìn thấy một tuyển tập nhỏ, một đề bài đang mở và lời dẫn rõ ràng. Họ đọc một bài trọn vẹn mà không cần đăng nhập, lưu hoặc bày tỏ cảm nhận sau khi tạo tài khoản, rồi thử viết một đoạn của mình.

Ở lần đầu gửi bài, Vink giải thích ngắn rằng nội dung sẽ được kiểm tra theo chính sách cộng đồng. Trạng thái chờ và cách sửa hoặc khiếu nại đều dễ tìm. Khi bài được đăng, người viết được mời chọn loại phản hồi mong muốn; họ không bị xếp vào một bảng điểm tác giả.

Người trở lại nhìn thấy bản thảo đang viết, phản hồi mới và một gợi ý để viết tiếp. Trang cá nhân làm rõ tác phẩm và hoạt động sáng tác; số lượt thích không chiếm vị trí chính.

## 5. Cộng đồng có chiều sâu khi còn ít nội dung

Trang chủ nên là một bố cục hữu hạn, đủ tốt ngay khi chỉ có một tuyển tập nhỏ: bài mở đầu, tuyển chọn theo chủ đề, đề bài tuần và lời mời đọc/góp ý. Khối trống được thay bằng hướng dẫn tham gia phù hợp, không kéo dài một feed rỗng.

**Thử nghiệm vận hành đề xuất:** mời một nhóm 15–25 người viết, chuẩn bị nội dung có sự đồng ý sử dụng, tổ chức một đề bài chung và phân công người phụ trách phản hồi cho bài xin góp ý. Những con số này là giả thuyết thử nghiệm, không phải benchmark hay cam kết tăng trưởng.

Biên tập viên có thể chọn một bài cũ để đọc lại với lời dẫn mới. Sức sống của cộng đồng đến từ việc có người thật đọc và trao đổi, không cần liên tục sản xuất bài mới. Không giả lượt đọc, tài khoản, bình luận hoặc đưa bài AI lên như tiếng nói của thành viên có thật.

Nếu bài và bình luận đều chờ duyệt trong pilot, phải có người phụ trách cùng khung xử lý được thông báo cho thành viên. Đo thời gian chờ để biết quy trình có đang làm đứt vòng phản hồi; một hàng đợi kỹ thuật hoạt động chưa bảo đảm người viết nhận được phản hồi kịp lúc.

## 6. Tương tác và đánh giá

Pilot nên ưu tiên **lưu tác phẩm và góp ý có ngữ cảnh**; theo dõi tác giả mở ở public beta, cảm nhận nhẹ là phần bổ sung tùy nguồn lực. Không đặt điểm sao hay tổng điểm chất lượng ở vị trí trung tâm. Nếu cần đánh giá kỹ thuật cho hoạt động học viết, dùng rubric theo mục tiêu bài tập và giữ tách khỏi xếp hạng công khai.

Tác giả có thể đánh dấu một góp ý là hữu ích. Tín hiệu này hỗ trợ cải thiện trải nghiệm và ghi nhận người góp ý; chưa nên trở thành tiền tệ, bảng xếp hạng hoặc điều kiện bắt buộc để được đăng.

Quy tắc thảo luận hướng vào văn bản và lựa chọn sáng tác. Chỉ ra điểm chưa thuyết phục được khuyến khích; công kích con người, quấy rối hoặc huy động tương tác để hạ uy tín cần có công cụ báo cáo và xử lý.

## 7. AI có ba vai trò khác nhau

| Vai trò | Được hỗ trợ việc gì? | Ranh giới |
| --- | --- | --- |
| Kiểm duyệt | Phát hiện dấu hiệu vi phạm chính sách, spam, rủi ro cần người đọc ngữ cảnh | Không lấy “viết chưa hay” hoặc điểm chân thành làm lý do cấm đăng |
| Hỗ trợ tuyển chọn | Gợi ý bài đáng để biên tập đọc theo tiêu chí công khai | Không tự quyết định toàn bộ gu Tạp Chí hoặc đồng nhất lượt thích với giá trị |
| Writing coach | Phản hồi về kỹ thuật khi tác giả yêu cầu | Không tự sửa, tự gửi bản nháp ra ngoài hoặc tự xuất bản |

AI hỗ trợ kiểm duyệt ngay từ MVP. Trong pilot, kết quả có thể chỉ là gợi ý cho người duyệt; tự động thông qua các nhóm nội dung phù hợp chỉ mở sau khi có đánh giá trên dữ liệu tiếng Việt và khả năng giám sát. Không đưa cam kết duyệt cố định 3–5 giây vào vision.

Những trường hợp như lời thoại nhân vật, trào phúng, bạo lực trong hư cấu, phương ngữ, thơ và trích dẫn cần ngữ cảnh. Kết quả mơ hồ hoặc dịch vụ lỗi chuyển sang người duyệt, với lý do và trạng thái rõ ràng. Người dùng có quyền phản hồi và khiếu nại quyết định.

Chính sách sáng tác có AI cần khuyến khích minh bạch phù hợp: phân biệt hỗ trợ chính tả, góp ý, đồng sáng tác hoặc tạo phần lớn nội dung. Không kết luận gian lận chỉ từ một điểm phát hiện văn bản AI; không bắt tác giả công khai toàn bộ lịch sử nháp để chứng minh bản thân.

## 8. Quyền tác giả, riêng tư và chuyển thể

**Nhật ký là thể loại; riêng tư là quyền truy cập.** MVP đề xuất hai mức rõ ràng: nháp riêng và bản xuất bản công khai. Nhóm góp ý kín, liên kết không niêm yết và đồng biên tập cần được thiết kế quyền riêng trước khi mở.

Tác giả phải có cách xuất tác phẩm và lịch sử cần thiết, hiểu nội dung nào đang công khai, và rút bản xuất bản. Chính sách lưu bản ghi kiểm duyệt, thời hạn xóa và xử lý trích dẫn/báo cáo cần được chốt trước public beta; không hứa xóa tức thì mọi bản sao đã được người đọc tải xuống.

Cantos là hướng mở rộng cho audio. Việc đăng một truyện trên Vink không mặc nhiên cho phép chuyển thể. Mỗi yêu cầu audio cần sự đồng ý riêng, snapshot nguồn, phạm vi sử dụng, ghi công và cách xử lý khi tác giả rút yêu cầu hoặc quyền thay đổi. Bản chữ sửa sau đó không tự biến thành bản audio mới.

## 9. Ngôn ngữ thiết kế

M3 Expressive được áp dụng vào điều hướng, nút hành động, biểu đạt trạng thái, phân cấp và hình khối. Trình đọc ưu tiên typography, khoảng trắng và nhịp văn bản. Hiệu ứng và màu nhấn tập trung ở điểm cần hành động, không cạnh tranh với nội dung.

Các nguyên tắc thiết kế đề xuất:

- Font có tiếng Việt đầy đủ, dấu rõ ở cỡ nhỏ và lớn; kiểm thử văn bản thật thay cho lorem ipsum.
- Giữ xuống dòng, khổ thơ và ngắt đoạn có chủ ý; thiết kế mobile không ép thơ thành đoạn văn.
- Chế độ sáng/tối và tùy chỉnh đọc cần nhất quán; tương phản, focus bàn phím, screen reader và cỡ chữ phải được kiểm chứng.
- Ngôn từ ngắn, thân thiện; trạng thái lỗi và chờ phải chỉ ra việc người dùng có thể làm tiếp.
- UI kế thừa nguyên tắc/tokens có chọn lọc từ Cantos, đồng thời xây bản sắc Vink và kiểm chứng riêng cho đọc dài, editor, bình luận.

## 10. Phạm vi phát triển

| Giai đoạn | Giá trị cần kiểm chứng | Phạm vi tiêu biểu |
| --- | --- | --- |
| Pilot có mời | Người viết lưu bài tin cậy, được đọc và góp ý | Tác phẩm một phần, nháp riêng, kiểm duyệt có AI hỗ trợ, đọc, bình luận, tuyển tập nhỏ |
| Public beta | Vòng lặp có thể vận hành bền vững | Tự động duyệt có giới hạn, nhiều chương cơ bản theo quyết định phạm vi, hồ sơ và theo dõi tác giả, vận hành khiếu nại |
| Mở rộng | Tăng chiều sâu sáng tác và khám phá | Ghi chú bên lề theo revision, đề bài/nhóm góp ý, writing coach, cá nhân hóa có kiểm soát |
| Hệ sinh thái | Tác phẩm đi sang hình thức khác có quyền rõ | Handoff audio Cantos, liên kết tài khoản tùy chọn, liên kết bản chữ và bản chuyển thể |

MVP không đồng thời xây công cụ tiểu thuyết chuyên nghiệp, cộng tác thời gian thực, chợ bản quyền, thanh toán tác giả, chat riêng và ứng dụng mobile native. Mỗi phần được mở khi có nhu cầu cụ thể và người chịu trách nhiệm vận hành.

## 11. Tín hiệu thành công

Chỉ số trọng tâm đề xuất là **số người viết hoàn thành một vòng viết–phản hồi–viết tiếp trong một chu kỳ quan sát**. Cần định nghĩa “phản hồi hữu ích” bằng phản hồi tự nguyện của tác giả và một mẫu review biên tập; không suy ra chỉ từ độ dài bình luận.

Theo dõi thêm tỷ lệ bài xin góp ý nhận được phản hồi, thời gian đến phản hồi đầu, người viết quay lại, bản thảo được sửa sau góp ý, tỷ lệ người đọc quay lại tác giả và tuổi hàng đợi kiểm duyệt. Chi phí AI, quyết định bị đảo sau khiếu nại và tải công việc của người duyệt là các điều kiện vận hành đi cùng tăng trưởng.

Chưa đặt KPI tăng trưởng hay tỷ lệ duyệt tự động cố định. Pilot cần tìm hiểu vì sao người dùng tiếp tục viết và đọc trước khi tối ưu quy mô.

## 12. Các quyết định sản phẩm cần chủ sở hữu lựa chọn

1. Nhóm beta tập trung người trưởng thành, sinh viên hay mở cả học sinh ngay từ đầu?
2. Tác phẩm nhiều chương có phải lý do chính để người dùng chọn Vink trong bản đầu, hay có thể mở ở public beta?
3. MVP chỉ cần nháp riêng và xuất bản công khai, hay nhóm góp ý kín là bắt buộc?
4. Tạp Chí do một nhóm biên tập tuyển chọn, có cộng đồng đề cử, hay kết hợp cả hai? AI chỉ hỗ trợ đến đâu?
5. Trong beta, phần thảo luận nâng cao có được chuyển sang giao diện Discourse để giảm khối lượng phát triển không?

Các mặc định đề xuất và điều kiện đổi hướng nằm trong [Architecture Decisions](../docs/architecture/decisions.md). Tài liệu vision này là tổng hợp và đề xuất từ tài liệu người dùng, không phải báo cáo khảo sát thị trường; nguồn đầu vào được ghi ở [Sources](../docs/references/sources.md).
