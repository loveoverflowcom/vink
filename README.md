# Vink — Việt Ink

**Mạng xã hội văn học và sáng tác dành cho người Việt.**

Vink là không gian để viết, đọc, học hỏi, chia sẻ và góp ý tác phẩm. Sản phẩm hướng tới một cộng đồng hiện đại, trẻ trung và có chiều sâu, ưu tiên chất lượng nội dung hơn số lượng bài đăng hay tương tác ảo.

**Trạng thái repository:** Đang xây nền tảng sản phẩm và kỹ thuật. Bộ tài liệu v0.3 ghi các quyết định sản phẩm đã chốt và các đề xuất kỹ thuật cần review. Đã có skeleton chạy được cục bộ: Leptos SSR + Axum, PostgreSQL, trang đọc tác phẩm công khai và Bàn viết lưu nháp riêng có kiểm phiên bản. Có API bình luận và UI đọc thảo luận hai tầng; kiểm thử gửi/gỡ/bật-tắt dùng định danh mẫu cục bộ. Đăng nhập mã email có UI và đã được kiểm cục bộ; production vẫn khóa đăng nhập mặc định. Có Web duyệt tác phẩm, phản đối và chế độ tự động; chưa có UI chọn audience/xuất bản; xem [hướng dẫn phát triển](docs/development/README.md) để chạy, kiểm chứng và biết giới hạn.

## Trải nghiệm hướng tới

- **Sáng tác:** Truyện ngắn, tiểu thuyết, thơ, tản văn, nhật ký và bài tập văn.
- **Cộng đồng:** Bình luận, góp ý, đánh giá tác phẩm và theo dõi tác giả.
- **Học viết:** Thử thách sáng tác, đề bài định kỳ, trao đổi kỹ thuật viết và phê bình văn học.
- **Khám phá:** Tìm bài theo hashtag, qua Đề xuất và Top 100.

**Bàn viết** là nơi soạn nháp riêng; **Xưởng Viết** là không gian đề xuất để chủ động chia sẻ và nhận phản hồi.

**Cập nhật 10/11/2026:** chủ sản phẩm chốt Vink là mạng xã hội các bài viết ngắn/tản văn; Tạp Chí, tác phẩm nhiều chương và quyết định nhóm tuổi được hoãn; trợ lý AI là luồng kiểm duyệt mặc định (hiện mới có đặc tả và dữ liệu kiểm thử mô phỏng). Xem [quyết định sản phẩm 10/11/2026](docs/architecture/product-decisions-20261110.md).

Thiết kế hướng tới **Material 3 Expressive**, giàu cảm xúc, thân thiện với giới trẻ và phù hợp với việc đọc văn dài.

## Định hướng kỹ thuật

Vink sở hữu tác phẩm, bản nháp, revision, publication **và thảo luận** trong **PostgreSQL**. Bình luận hai tầng (bình luận gốc + một danh sách phản hồi phẳng) là module Rust trong app, đã chốt ngày 03/11/2026 ([ADR](docs/architecture/discussion-ownership.md)); Discourse chỉ là nguồn tham khảo thiết kế, không chạy trong hệ thống. **Leptos SSR + Axum** phục vụ giao diện riêng, cùng worker nền cho kiểm duyệt. Vink tự làm màn vận hành tác phẩm và bình luận.

Ngoài owner thảo luận, các lựa chọn này chưa phải ADR Accepted; editor, identity và tiền kiểm bình luận cần vượt qua các spike trong decision log. Cantos là sản phẩm độc lập, có thể nhận handoff chuyển thể audio về sau với sự đồng ý riêng của tác giả; không dùng chung database, password hay session.

## Tài liệu

- [Bản đồ tài liệu và bộ định hướng v0.3](docs/README.md)
- [Phát triển: setup, lệnh, cấu trúc code và giới hạn skeleton](docs/development/README.md)
- [Kho quyết định TOML và cách tra cứu](decisions/README.md)
- [Vision và định hướng sản phẩm](prd/vision.md)
- [Requirements và tiêu chí nghiệm thu](prd/requirements.md)
- [Tech stack và so sánh kiến trúc](docs/architecture/tech-stack.md)
- [Quyết định mở, bất biến và integration spikes](docs/architecture/decisions.md)
- [Nguồn và giới hạn bằng chứng](docs/references/sources.md)
- [Hướng dẫn agent](AGENTS.md) và [các skills của Vink](.agents/skills/README.md)

Bản README và vision có sẵn trước lần nhập foundation được giữ nguyên tại [docs/archive/initial/](docs/archive/initial/README.md).

> Vink không chạy theo số lượng bài đăng. Vink tạo ra một không gian nơi mỗi bài viết đáng được đọc, mỗi bình luận có điều để đóng góp và mỗi tác giả có cơ hội trưởng thành.
