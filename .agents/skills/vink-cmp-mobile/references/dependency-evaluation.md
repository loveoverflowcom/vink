# Quy trình và tiêu chuẩn chọn thư viện Compose Multiplatform (CMP)

Tài liệu này do `vink-cmp-mobile` sở hữu, định nghĩa quy trình khách quan, có trách nhiệm và dựa
trên bằng chứng thực tế khi xem xét, đánh giá hoặc tích hợp thêm thư viện cho các module Kotlin
Compose Multiplatform trong Vink.

> [!IMPORTANT]
> Danh sách thư viện nêu trong tài liệu này là các **ứng viên nghiên cứu có điều kiện**, tuyệt đối
> **không phải danh mục bắt buộc phải cài đặt**. Không thêm bất kỳ dependency nào nếu chưa có bài toán
> cụ thể và consumer thực tế trong mã nguồn.

---

## 1. Thứ tự xem xét ba tầng (Three-tier Evaluation Order)

Trước khi tìm kiếm giải pháp bên ngoài, agent bắt buộc phải rà soát theo thứ tự nghiêm ngặt sau:

1. **Tầng 1 — Thành phần, mẫu hình và chính sách Vink hiện có:**
   - Tra cứu [API coverage](../../../../packages/ui-system/shared/api-coverage.json) và
     [Preview catalog](../../../../packages/ui-system/shared/preview-catalog.json).
   - Kiểm tra các primitives trong `packages/ui-system/cmp/ui`: `VinkTheme`, `VinkValues`,
     `VinkActions`, `VinkFields`, `VinkNavigation`, `VinkFeedback`, `VinkMedia`.
   - Kiểm tra các mẫu hình trong `packages/vink-patterns/cmp`.
   - Kiểm tra hệ thống token `packages/ui-system/shared/tokens.json` (`VinkTokens`).
   - Nếu năng lực hiện hữu hoặc việc mở rộng nhỏ một component hiện có giải quyết được bài toán:
     **Dừng tại đây, không thêm thư viện mới**.

2. **Tầng 2 — Bộ công cụ chuẩn của Compose Multiplatform:**
   - Xem xét các API chính thức của JetBrains Compose Multiplatform tương ứng với phiên bản đang ghim
     trong repo:
     * `androidx.compose.foundation`: Layouts, gestures, drag/drop, scrollable, text, interaction.
     * `androidx.compose.animation`: AnimatedVisibility, Transition, updateTransition, Animatable.
     * `androidx.compose.material3`: Các component Material 3 chuẩn (Scaffold, TopAppBar, NavigationBar,
       ModalBottomSheet, Dialog, ExposedDropdownMenuBox, Slider, ProgressIndicator).
     * `androidx.compose.material3.adaptive`: Phân chia layout đa màn hình và kích thước cửa sổ.
   - Nếu các API chuẩn này đáp ứng được yêu cầu nghiệp vụ: **Sử dụng API chuẩn, không thêm thư viện ngoài**.

3. **Tầng 3 — Thư viện bên ngoài (Third-party Library):**
   - Chỉ được xem xét khi có khoảng trống tính năng (feature gap) cụ thể mà Tầng 1 và Tầng 2 không thể
     đáp ứng một cách hợp lý, VÀ có một consumer thực tế trong sản phẩm đang chờ sử dụng.

---

## 2. Tám tiêu chí bắt buộc cho một quyết định Dependency (Checklist)

Mỗi đề xuất thêm hoặc thay đổi dependency bắt buộc phải lập bản đánh giá ngắn gồm đủ 8 tiêu chí:

1. **Vấn đề cụ thể và Consumer thực tế:**
   - Vấn đề nghiệp vụ hoặc kỹ thuật cần giải quyết là gì?
   - Thành phần nào trong `packages/ui-system/cmp/ui`, `packages/vink-patterns/cmp` hoặc host nào sẽ
     trực tiếp tiêu thụ (consumer)? Từ chối thêm dependency "để sẵn đó" hoặc chỉ để làm mẫu minh họa.
2. **Artifact ID và Phiên bản tương thích với Checkout hiện tại:**
   - Ghi rõ tọa độ Maven (`group:artifact:version`).
   - Phải tương thích với phiên bản Kotlin (2.2.20), Compose Multiplatform (1.9.0) và Gradle (8.14.3)
     của checkout hiện hành.
3. **API khả dụng ở commonMain và từng Target:**
   - API nào có thể gọi trực tiếp trong `commonMain`?
   - Có đòi hỏi cấu hình hay phụ thuộc nền tảng nào ở `androidMain`, `iosMain`, hoặc `desktopMain` không?
4. **Trạng thái ổn định (Stability Status):**
   - API và artifact đó là Stable, Experimental, Alpha hay Beta? Nếu là Experimental, có cần OptIn
     annotation nào không?
5. **So sánh với giải pháp sẵn có:**
   - Tại sao không thể tự triển khai bằng Compose Foundation/Animation? Đánh đổi giữa việc tự viết
     vài chục dòng mã với việc ôm thêm một dependency là gì?
6. **Chi phí tích hợp, Kích thước và Khả năng bảo trì:**
   - Giấy phép nguồn mở (ưu tiên Apache 2.0, MIT; từ chối GPL nếu vi phạm ranh giới sở hữu).
   - Tác động đến kích thước file nhị phân (AAR, Framework, JAR), thời gian build Gradle.
   - Thư viện có được cộng đồng duy trì tích cực không?
7. **Phương án dự phòng, Kế hoạch kiểm thử và Điều kiện gỡ bỏ (Exit Strategy):**
   - Nếu thư viện ngừng hoạt động hoặc phát sinh lỗi trên một nền tảng (ví dụ iOS crash), giải pháp
     fallback là gì?
   - Cách kiểm thử tự động trên Desktop JVM và thiết bị thật.
   - Điều kiện nào sẽ dẫn đến việc gỡ bỏ dependency (ví dụ: khi CMP chính thức hỗ trợ tính năng đó)?
8. **Nguồn chính thức đã kiểm tra cùng ngày/phiên bản:**
   - Dẫn link tài liệu chính thức từ JetBrains / Google Android Developers, hoặc repository chính thức
     (GitHub/GitLab) của tác giả thư viện, được kiểm tra tại thời điểm đánh giá. Tuyệt đối không trích
     dẫn câu trả lời từ chat cũ hoặc suy đoán trí nhớ.

---

## 3. Các ứng viên nghiên cứu có điều kiện

Dưới đây là các ứng viên phổ biến trong hệ sinh thái Compose Multiplatform và điều kiện xem xét tương ứng:

| Ứng viên | Ngữ cảnh xem xét | Điều kiện và Ranh giới áp dụng |
| --- | --- | --- |
| **MotionScheme / M3 Expressive** | Cấu hình chuyển động theo chuẩn M3 Expressive | Chỉ áp dụng khi Material3 CMP chính thức cung cấp API ổn định tương thích phiên bản hiện tại mà không ép nâng toàn bộ Kotlin/Gradle toolchain. Hiện tại Vink quản lý chuyển động thông qua `VinkTokens` và `VinkMotion`. |
| **Navigation Compose / Navigation 3** | Điều hướng màn hình trong ứng dụng native | Thuộc về tầng Application/Host (`apps/vink-mobile` hoặc diagnostic hosts), **không** nằm trong thư viện primitive `packages/ui-system/cmp/ui`. Hiện tại thư viện UI dùng pure policy `VinkNavigation.kt` độc lập với framework điều hướng. |
| **Coil 3** | Tải và giải mã hình ảnh từ mạng/disk | Thuộc về tầng Data Adapter hoặc Host Application. **Không** đưa Coil vào làm dependency mặc định của module `:ui` dùng chung để tránh ép mọi consumer phải mang theo network client. Module `:ui` nhận `Painter` hoặc `ImageBitmap` trừu tượng. |
| **Compose Unstyled** | Các control headless/unstyled | Chỉ xem xét khi các component chuẩn của Material3 có giới hạn cấu trúc không thể tùy biến để khớp với Anatomy, Token hoặc khả năng trợ năng của Vink. |
| **Telephoto** | Xem ảnh có cử chỉ zoom/pan đa điểm | Chỉ xem xét khi có tính năng xem ảnh chi tiết (ví dụ: Reader truyện tranh hoặc ảnh minh họa văn học khổ lớn). Không cài đặt khi chưa có màn hình sử dụng. |
| **Compottie** | Chạy hoạt ảnh Lottie trong Compose | Chỉ xem xét khi biểu cảm Noto (NotoExpressions) hoặc đồ họa minh họa phức tạp cần chạy animation Lottie vector native. Hiện tại biểu cảm dùng SVG và dotLottie Web player. |
| **Reorderable** | Kéo và sắp xếp lại danh sách (drag-to-reorder) | **Cực kỳ thận trọng:** Tuyệt đối không đưa tính năng đã hoãn (như tác phẩm nhiều chương `WR-04` hay sắp xếp danh mục) vào làm use case giả định chỉ để biện hộ cho việc thêm thư viện này. Chỉ đánh giá khi có tính năng được phê duyệt `PRODUCT_CONFIRMED` cần sắp xếp thứ tự. |

---

## 4. Các nguyên tắc cứng (Hard Rules)

1. **Không lấy phiên bản AndroidX làm bằng chứng tương thích CMP:**
   - Việc một artifact AndroidX (ví dụ `androidx.navigation:navigation-compose:2.8.x`) đã phát hành
     trên Maven **không chứng minh** nó có phiên bản tương thích đa nền tảng cho iOS/Desktop trên
     JetBrains Compose Multiplatform. Phải kiểm tra chính xác artifact đa nền tảng (thường mang group
     `org.jetbrains.androidx.*` hoặc repository đa nền tảng riêng).
2. **Không đóng đinh một nhận định "chưa hỗ trợ" thành sự thật vĩnh viễn:**
   - Hệ sinh thái CMP phát triển nhanh. Nhận định "chưa hỗ trợ iOS" trong một tài liệu 6 tháng trước
     có thể đã lỗi thời. Luôn tra cứu trạng thái phát hành mới nhất từ repo chính thức của thư viện.
3. **Không yêu cầu nâng toàn bộ Stack chỉ vì một hiệu ứng nhỏ:**
   - Tuyệt đối không đề xuất nâng phiên bản Kotlin của toàn bộ dự án, nâng Gradle hoặc chuyển sang
     bản Compose EAP/Dev chỉ để sử dụng một hiệu ứng visual chuyển cảnh đơn lẻ.
4. **Điểm xuất phát tra cứu chính thức:**
   - Tài liệu chính thức Android Material 3: https://developer.android.com/develop/ui/compose/designsystems/material3
   - Cổng thông tin JetBrains Compose Multiplatform: https://www.jetbrains.com/lp/compose-multiplatform/
   - Kho lưu trữ chính thức JetBrains Compose Multiplatform: https://github.com/JetBrains/compose-multiplatform
