# Vink — Việt Ink

**Mạng xã hội văn học và sáng tác dành cho người Việt.**

Vink là không gian để viết, đọc, học hỏi, chia sẻ và góp ý tác phẩm. Sản phẩm hướng tới một cộng đồng hiện đại, trẻ trung và có chiều sâu, ưu tiên chất lượng nội dung hơn số lượng bài đăng hay tương tác ảo.

**Trạng thái repository:** Đang xây nền tảng sản phẩm và kỹ thuật. Bộ tài liệu v0.2 là đề xuất để review; chưa có ứng dụng, môi trường chạy hay lệnh build/test được triển khai.

## Trải nghiệm hướng tới

- **Sáng tác:** Truyện ngắn, tiểu thuyết, thơ, tản văn, nhật ký và bài tập văn.
- **Cộng đồng:** Bình luận, góp ý, đánh giá tác phẩm và theo dõi tác giả.
- **Học viết:** Thử thách sáng tác, đề bài định kỳ, trao đổi kỹ thuật viết và phê bình văn học.
- **Khám phá:** Giao diện tạp chí tuyển chọn tác phẩm, vẫn hấp dẫn khi cộng đồng còn ít nội dung.

Ba không gian đề xuất là **Bàn viết** để soạn nháp riêng, **Xưởng Viết** để chủ động chia sẻ và nhận phản hồi, **Tạp Chí** để khám phá tác phẩm được biên tập tuyển chọn. AI hỗ trợ kiểm duyệt từ MVP; mức tự động hóa cần được đánh giá trước khi mở.

Thiết kế hướng tới **Material 3 Expressive**, giàu cảm xúc, thân thiện với giới trẻ và phù hợp với việc đọc văn dài.

## Định hướng kỹ thuật

Phương án ưu tiên để kiểm chứng là **Hybrid**: Vink sở hữu tác phẩm, bản nháp, revision và publication trong **PostgreSQL**; **Discourse** sở hữu thảo luận và dùng PostgreSQL/Redis theo topology được hỗ trợ. **Leptos SSR + Axum** phục vụ giao diện riêng, cùng worker nền cho kiểm duyệt và đồng bộ. Vink cần màn vận hành tác phẩm riêng bên cạnh Discourse Admin.

Các lựa chọn này chưa phải ADR Accepted; editor, identity và tích hợp Discourse cần vượt qua các spike trong decision log. Cantos là sản phẩm độc lập, có thể nhận handoff chuyển thể audio về sau với sự đồng ý riêng của tác giả; không dùng chung database, password hay session.

## Tài liệu

- [Bản đồ tài liệu và bộ foundation v0.2](docs/README.md)
- [Vision và định hướng sản phẩm](prd/vision.md)
- [Requirements và tiêu chí nghiệm thu](prd/requirements.md)
- [Tech stack và so sánh kiến trúc](docs/architecture/tech-stack.md)
- [Quyết định mở, bất biến và integration spikes](docs/architecture/decisions.md)
- [Nguồn và giới hạn bằng chứng](docs/references/sources.md)
- [Hướng dẫn agent](AGENTS.md) và [các skills của Vink](.agents/skills/README.md)

Bản README và vision có sẵn trước lần nhập foundation được giữ nguyên tại [docs/archive/initial/](docs/archive/initial/README.md).

> Vink không chạy theo số lượng bài đăng. Vink tạo ra một không gian nơi mỗi bài viết đáng được đọc, mỗi bình luận có điều để đóng góp và mỗi tác giả có cơ hội trưởng thành.
