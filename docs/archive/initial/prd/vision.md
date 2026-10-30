# Vink — Việt Ink: Vision sản phẩm

**Trạng thái:** Bản nháp định hướng; các đề xuất và câu hỏi mở bên dưới cần được xác thực trước khi triển khai.

## Tầm nhìn

Xây dựng một cộng đồng văn học hiện đại, trẻ trung nhưng có chiều sâu, nơi người Việt có thể viết, đọc, học hỏi, chia sẻ và phê bình tác phẩm. Vink ưu tiên chất lượng nội dung hơn số lượng và tương tác ảo, hướng tới một cộng đồng nhỏ nhưng có giá trị thay vì một diễn đàn đại trà.

## Trải nghiệm sản phẩm

- **Sáng tác:** Truyện ngắn, tiểu thuyết, thơ, tản văn, nhật ký và bài tập văn.
- **Cộng đồng:** Bình luận, góp ý, đánh giá tác phẩm và theo dõi tác giả.
- **Học viết:** Thử thách sáng tác, đề bài định kỳ, trao đổi kỹ thuật viết và phê bình văn học.
- **Khám phá:** Trải nghiệm tạp chí với các tác phẩm được tuyển chọn, đẹp và hữu ích ngay cả khi cộng đồng còn ít nội dung.
- **Thiết kế:** Material 3 Expressive, hiện đại và giàu cảm xúc, thân thiện với giới trẻ, đồng thời phù hợp đọc văn dài và giữ nhịp ngắt dòng của thơ.

## AI tuyển chọn và kiểm duyệt — định hướng dài hạn

AI đóng vai trò hỗ trợ biên tập và kiểm duyệt. Bài viết và bình luận có thể được đánh giá trước khi xuất bản; hệ thống hướng đến việc phê duyệt nội dung chân thành, có ý nghĩa, sáng tạo hoặc đóng góp xây dựng, đồng thời lọc spam, nội dung sao chép kém chất lượng, công kích và tương tác câu kéo.

Việc đánh giá cần xét ngữ cảnh và thể loại, không đòi hỏi người mới tập viết đạt trình độ chuyên nghiệp. Nội dung không chắc chắn được chuyển sang kiểm duyệt thủ công, có giải thích và quyền khiếu nại. Về lâu dài, AI có thể góp ý văn phong, chỉ ra điểm mạnh và đề xuất cải thiện.

**Nguyên tắc:** AI giúp nâng chất lượng cộng đồng, không áp đặt một gu văn học duy nhất hay bóp nghẹt sáng tạo.

## Định hướng kiến trúc

- **Repository:** [loveoverflowcom/vink](https://github.com/loveoverflowcom/vink); nhánh mặc định `develop`.
- **Community engine:** Discourse mã nguồn mở.
- **Dữ liệu:** PostgreSQL và Redis, độc lập với Turso.
- **Frontend:** Leptos và Material 3 Expressive, xây dựng UX riêng.
- **API:** Discourse REST API; Rust/Axum khi cần nghiệp vụ mở rộng.
- **Quản trị:** Discourse Web Admin; bổ sung dashboard kiểm duyệt AI về sau.
- **Cantos:** Sản phẩm độc lập, có thể liên kết Cantos Theatre để chuyển thể tác phẩm thành audio.

## Lộ trình định hướng

1. **MVP:** Sáng tác → Đăng bài → Kiểm duyệt → Đọc → Bình luận → Đánh giá.
2. **Giai đoạn 2:** AI hỗ trợ kiểm duyệt, thử thách viết, tuyển chọn nội dung, hồ sơ tác giả và tác phẩm nhiều chương.
3. **Giai đoạn 3:** AI kiểm duyệt tự động theo chính sách rõ ràng, AI Writing Coach, đề xuất nội dung chất lượng và tích hợp audio.

Đây là lộ trình định hướng, chưa phải cam kết về phạm vi, thời gian hay thứ tự triển khai chi tiết.

## Các đề xuất cần nghiên cứu

Những ý tưởng sau được tổng hợp từ tài liệu phản biện đính kèm; chúng là phương án để kiểm chứng, chưa thay thế các quyết định trong vision ở trên.

### Cộng đồng và trải nghiệm

- Cân nhắc **Xưởng Viết** cho bản nháp, bài tập và góp ý, cùng **Tạp Chí** cho tác phẩm được tuyển chọn; tiêu chí hiển thị phù hợp từng không gian.
- Cân nhắc ghi chú bên lề, đánh dấu đoạn yêu thích và khung góp ý có cấu trúc; đánh giá lợi ích và rủi ro của chấm điểm đối với người mới trước khi chọn cơ chế đánh giá.
- Cho phép tương tác nhẹ như lưu tác phẩm hoặc bày tỏ yêu thích để giảm áp lực phải viết bình luận dài.
- Ưu tiên typography hỗ trợ tiếng Việt, chế độ đọc dễ chịu và hiển thị thơ phù hợp trên màn hình nhỏ.

### Kiểm duyệt và sáng tác có AI

- Khảo sát gợi ý nhẹ ngay trong khung soạn thảo và rubric theo thể loại, có cơ chế xử lý khi AI không chắc chắn hoặc dịch vụ không khả dụng.
- Cân nhắc đưa AI hỗ trợ kiểm duyệt cơ bản vào MVP để giảm thời gian chờ; cần đánh giá chi phí, độ trễ, độ chính xác và khả năng vận hành trước khi quyết định.
- Không dựa riêng vào công cụ phát hiện văn bản AI để kết luận về tác giả. Có thể khuyến khích lịch sử bản nháp và đối thoại về quá trình sáng tác, đồng thời làm rõ quyền riêng tư và tính tự nguyện.

### Kỹ thuật và audio

- Đánh giá Leptos SSR và Axum làm backend-for-frontend để phục vụ SEO, metadata chia sẻ, tổng hợp dữ liệu Discourse và cache trang tạp chí.
- Thiết kế ánh xạ tác phẩm, chương và bình luận vào Discourse trước khi mở rộng: một đề xuất là mỗi chương là một topic riêng và được nhóm theo tác phẩm; ghi chú bên lề cần chiến lược neo vào văn bản khi nội dung thay đổi.
- Cân nhắc dùng trust levels của Discourse để điều chỉnh quy trình kiểm duyệt, nhưng cần xác minh cấu hình, quyền hạn và tác động đến người dùng trước khi áp dụng.
- Khảo sát tuyển chọn tác phẩm để chuyển thể cùng Cantos Theatre từ giai đoạn sớm, với sự đồng ý của tác giả; tích hợp audio đầy đủ vẫn thuộc định hướng dài hạn.

## Câu hỏi mở

- MVP dùng hình thức đánh giá nào để khuyến khích góp ý mà không tạo áp lực điểm số?
- Bài viết và bình luận nào cần duyệt trước; trải nghiệm chờ, giải thích và khiếu nại hoạt động ra sao?
- Tiêu chí tuyển chọn, quyền quyết định của biên tập viên và cách đánh giá sai lệch của AI là gì?
- Phạm vi của Xưởng Viết, Tạp Chí và tích hợp Cantos trong từng giai đoạn là gì?
- Mô hình dữ liệu, SSR, BFF và các giới hạn của Discourse cần được kiểm chứng bằng thử nghiệm kỹ thuật nào?

## Tuyên ngôn sản phẩm

**Vink không chạy theo số lượng bài đăng. Vink tạo ra một không gian nơi mỗi bài viết đáng được đọc, mỗi bình luận có điều để đóng góp và mỗi tác giả có cơ hội trưởng thành.**
