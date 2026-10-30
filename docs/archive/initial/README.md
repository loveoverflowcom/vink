# Vink — Việt Ink

**Mạng xã hội văn học và sáng tác dành cho người Việt.**

Vink là không gian để viết, đọc, học hỏi, chia sẻ và góp ý tác phẩm. Sản phẩm hướng tới một cộng đồng hiện đại, trẻ trung và có chiều sâu, ưu tiên chất lượng nội dung hơn số lượng bài đăng hay tương tác ảo.

## Trải nghiệm hướng tới

- **Sáng tác:** Truyện ngắn, tiểu thuyết, thơ, tản văn, nhật ký và bài tập văn.
- **Cộng đồng:** Bình luận, góp ý, đánh giá tác phẩm và theo dõi tác giả.
- **Học viết:** Thử thách sáng tác, đề bài định kỳ, trao đổi kỹ thuật viết và phê bình văn học.
- **Khám phá:** Giao diện tạp chí tuyển chọn tác phẩm, vẫn hấp dẫn khi cộng đồng còn ít nội dung.

Thiết kế hướng tới **Material 3 Expressive**, giàu cảm xúc, thân thiện với giới trẻ và phù hợp với việc đọc văn dài.

## Định hướng kỹ thuật

Vink dự kiến dùng **Discourse** làm community engine, **PostgreSQL và Redis** độc lập với Turso, **Leptos** cho giao diện riêng và **Discourse REST API** để kết nối; **Rust/Axum** bổ sung nghiệp vụ khi cần. Quản trị ban đầu tận dụng Discourse Web Admin. Cantos là sản phẩm độc lập, có thể kết nối Cantos Theatre để chuyển thể tác phẩm thành audio về sau.

## Tài liệu

- [Vision và định hướng sản phẩm (bản nháp)](prd/vision.md)

> Vink không chạy theo số lượng bài đăng. Vink tạo ra một không gian nơi mỗi bài viết đáng được đọc, mỗi bình luận có điều để đóng góp và mỗi tác giả có cơ hội trưởng thành.
