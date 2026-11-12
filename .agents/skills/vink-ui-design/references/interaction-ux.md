# Đặc tả và tiêu chuẩn Interaction UX Vink

Tài liệu này do `vink-ui-design` sở hữu, định nghĩa chuẩn mực mô tả, đánh giá và nghiệm thu
trải nghiệm tương tác (Interaction UX) cho toàn bộ hệ thống giao diện Vink (đặc biệt Compose
Multiplatform và Leptos Web).

---

## 1. Khung mô tả một Interaction (Interaction Contract)

Mọi tương tác từ mức thành phần đến luồng màn hình phải được mô tả mạch lạc theo 9 yếu tố:

1. **Mục tiêu người dùng và ngữ cảnh:** Người dùng muốn đạt được điều gì (ví dụ: tìm tác giả,
   thu gọn bộ lọc, cuộn bài thơ, xác nhận xóa)? Thành phần và luồng liên quan là gì?
2. **Input modality:** Tương tác hỗ trợ những phương thức nhập liệu nào: cảm ứng (touch), con trỏ
   (pointer/mouse), bàn phím cứng (keyboard), hay công cụ trợ năng (screen reader: TalkBack,
   VoiceOver, NVDA/Orca)?
3. **Chuỗi trạng thái tương tác:**
   - **Trigger:** Thao tác khởi phát (tap, click, keydown, drag, swipe, focus).
   - **Phản hồi tức thời (Immediate Feedback):** Hiệu ứng phản hồi tức thì (pressed state, ripple,
     độ dịch chuyển theo tay, ring focus) để người dùng biết hệ thống đã nhận tín hiệu.
   - **Commit / Cancel:** Điểm quyết định hành động: người dùng thả tay qua ngưỡng, bấm Enter, hay
     kéo ngược lại, bấm Escape, chạm vùng ngoài để hủy.
   - **Trạng thái cuối (Final State):** Kết quả thị giác và ngữ nghĩa sau khi hoàn tất hoặc hủy.
4. **Quyền sở hữu trạng thái (State Ownership):**
   - Ai sở hữu state thật (source of truth): ViewModel, host Activity, Leptos Resource hay DB?
   - Phần nào chỉ là biểu diễn tạm thời (transient visual state) ở local component?
5. **Vòng đời Focus:**
   - Focus ban đầu đặt ở đâu khi thành phần xuất hiện?
   - Focus di chuyển thế nào trong suốt thao tác?
   - Điều kiện phục hồi focus (focus restoration) khi đóng hoặc hủy: trả về trigger ban đầu nếu
     trigger còn tồn tại và đóng bình thường; **không** trả focus về trang cũ nếu đóng do điều hướng
     sang màn hình mới.
6. **Bàn phím, IME, Window Insets và Cuộn:**
   - Insets và chiều cao bàn phím do tầng nào quản lý (host scaffold hay local component)? Tránh
     xử lý padding/insets hai lần (double insets).
   - Bàn phím có che mất vùng nhập liệu, nút CTA hoặc thông báo lỗi không?
   - Vị trí cuộn (scroll anchor) được giữ nguyên ra sao khi bàn phím bật/tắt?
7. **Xung đột cử chỉ (Gesture Conflict) và ngắt quãng (Interruption):**
   - Cử chỉ có tranh chấp với cuộn dọc/ngang của danh sách cha (nested scrolling), chọn văn bản
     (text selection) hoặc cử chỉ Back hệ thống (system Back) không?
   - Hành vi khi cử chỉ bị ngắt (interrupted, ví dụ cuộc gọi đến hoặc cử chỉ hệ thống chen ngang)
     hoặc người dùng đảo chiều kéo đột ngột.
8. **Reduced Motion và phương án thay thế không dùng cử chỉ:**
   - Khi hệ điều hành hoặc người dùng bật Giảm chuyển động (`prefers-reduced-motion` hoặc
     `LocalVink.current.motion.reduced`), hiệu ứng phải chuyển sang tức thời hoặc tối giản, các vòng
     lặp vô tận (infinite loops) phải dừng ở khung hình tĩnh có nghĩa.
   - Luôn cung cấp phương án thay thế bằng nút bấm, phím tắt hoặc Accessibility Action cho mọi thao
     tác cử chỉ cảm ứng.
9. **Nền tảng kiểm thử và bằng chứng bắt buộc:**
   - Nêu rõ ranh giới đã kiểm: kiểm logic thuần (JVM test), kiểm layout/render offscreen (Desktop
     preview PNG), hay kiểm tương tác thực trên thiết bị (Android emulator/device, iOS simulator,
     trình duyệt thật). Không suy diễn parity giữa các nền tảng khi chưa chạy trên host đó.

---

## 2. Bản rút gọn cho sửa một Control đơn lẻ

Đối với một thay đổi hoặc sửa lỗi nhỏ ở một control (ví dụ: nút bấm, ô tìm kiếm, chip lọc), không
cần lập văn bản toàn bộ quy trình thiết kế. Sử dụng bản rút gọn 4 dòng:

```text
Control: [Tên control, vai trò, vị trí trên màn hình]
Input & Focus: [Modality; trạng thái focus ban đầu, sau thao tác và khi thoát]
Trigger → Commit/Cancel: [Thao tác kích hoạt → phản hồi tức thời → commit khi nào → hủy khi nào]
Fallback & Bounds: [Cách thao tác bằng phím/a11y; hành vi khi reduced motion; insets/scroll liên quan]
```

---

## 3. Các nguyên tắc Interaction cốt lõi

### A. Chuyển động (Motion)

1. **Phân biệt Spatial Motion với Effects:**
   - *Spatial motion:* Chuyển đổi vị trí, kích thước, bo góc, container transform (dùng curve
     `--vk-ease-spatial` / `VinkMotionTokens.EaseSpatial`). Phải gắn liền với cấu trúc vật lý của màn
     hình, tạo cảm giác đối tượng mở rộng từ nguồn thực.
   - *Effects:* Đổi màu sắc, độ mờ (opacity), elevation (dùng `--vk-ease-emphasized` /
     `VinkMotionTokens.EaseEmphasized`).
2. **Motion Role có mục đích và tiết chế ở Reader/Editor:**
   - Chuyển động phải giúp người dùng hiểu thứ tự không gian hoặc xác nhận hành động, không phục vụ
     mục đích trang trí thuần túy.
   - Ở Bàn viết (Editor) và Trình đọc (Reader): không gian cần sự tĩnh lặng, tập trung cao độ vào
     chữ và thơ. Hạn chế tối đa các chuyển động ngoại vi, rung lắc hoặc chuyển cảnh giật cục.
   - Không biến "Material 3 Expressive" thành việc mọi thành phần đều nảy, đàn hồi quá đà hoặc lượn sóng.
3. **Tách rời Animation Completion khỏi cam kết nghiệp vụ (Decoupling):**
   - Tuyệt đối **không** để việc hoàn thành animation quyết định việc gửi request, lưu dữ liệu hay
     chuyển trạng thái nghiệp vụ. Dữ liệu và tác vụ nghiệp vụ phải được kích hoạt ngay khi người dùng
     commit; animation chỉ là phản ánh trực quan đi kèm.
   - Cho phép người dùng ngắt (interrupt), đảo chiều (reverse) hoặc thao tác tiếp mà không phải chờ
     animation chạy hết thời lượng.
4. **Reduced Motion và xử lý vòng lặp (Loops):**
   - Khi bật Reduced Motion: chuyển đổi thời gian chuyển cảnh về 0ms hoặc giá trị tối thiểu (ví dụ
     shortMs: 0ms), nhưng **không làm mất trạng thái**.
   - Các vòng lặp chuyển động vô tận (như spinner tải, skeleton pulse, dots loading) phải **dừng hẳn**
     (loopsRun = false, `loop.iterations: 0`) thay vì quay với tốc độ 0ms gây tốn pin và nghẽn CPU.
     Tại trạng thái dừng, thành phần phải hiển thị nhãn trạng thái rõ ràng bằng chữ hoặc icon tĩnh.
5. **Phân biệt Motion Timing với các mốc thời gian khác:**
   - Motion duration (160ms/320ms): dành cho mắt người theo dõi chuyển động.
   - Debounce gõ phím (300ms - 500ms): trì hoãn phát tín hiệu tìm kiếm/lưu.
   - Loading Gate (300ms delay tránh nhấp nháy, 500ms min-visible tránh chớp tắt): chính sách UX
     phản hồi tải nội dung.
   - Network timeout (10s - 30s) và thời hạn nghiệp vụ (SLA): thuộc tầng transport/domain. Không
     dùng chung các hằng số này.

### B. Cử chỉ (Gesture)

1. **Thao tác trực tiếp (Direct Manipulation) và đường thoát (Cancel Path):**
   - Khi người dùng chạm và kéo một thành phần (sheet, slider, dismissable item), thành phần phải di
     chuyển đồng bộ 1:1 theo ngón tay, mang lại cảm giác làm chủ vật lý.
   - Luôn có đường hủy rõ ràng: nếu người dùng kéo chưa vượt ngưỡng kích hoạt (threshold) và thả
     tay, thành phần phải trượt êm ái về vị trí cũ.
2. **Bốn giai đoạn rõ rệt của cử chỉ:**
   - *Đang kéo (Dragging):* Phản hồi bám sát ngón tay, cung cấp chỉ báo trực quan về ngưỡng.
   - *Vượt ngưỡng (Threshold crossed):* Thay đổi nhẹ về visual/haptic để báo hiệu hành động sẽ xảy
     ra nếu thả tay.
   - *Thả tay (Released):* Kiểm tra vị trí cuối; nếu đạt ngưỡng thì thực thi, nếu chưa thì hồi phục.
   - *Hành động được xác nhận (Confirmed Action):* Trạng thái sau khi hoàn tất.
3. **Tôn trọng hệ thống và cử chỉ lồng nhau (Nested Scrolling):**
   - Cử chỉ tùy biến trong thành phần không được cướp mất cử chỉ cuộn nội dung tự nhiên, cử chỉ chọn
     văn bản (text selection), hoặc cử chỉ Back cạnh màn hình của Android/iOS.
   - Áp dụng cơ chế NestedScrollConnection đúng quy cách: cho phép thành phần con tiêu thụ cuộn
     trước, phần dư chuyển lên cha; hoặc ngược lại tùy thuộc vào hành vi sheet/list.
4. **Phương án trợ năng thay thế:**
   - Mọi thao tác bằng cử chỉ vuốt/kéo phải có nút bấm, menu hành động hoặc phím tắt tương đương để
     người dùng bàn phím hoặc screen reader thao tác đầy đủ.
5. **Cử chỉ phá hủy là quyết định sản phẩm (Product Decision):**
   - Tuyệt đối không tự ý thêm cử chỉ "vuốt để xóa" (swipe-to-delete), "vuốt toàn màn hình để đổi
     tab" (full-screen swipe navigation), hoặc "vuốt để bỏ bản nháp" (swipe-to-discard) chỉ vì thấy
     tiện lợi. Những cử chỉ này dễ gây mất dữ liệu ngoài ý muốn và phải được chủ sản phẩm phê duyệt
     qua TOML registry (`open_product`).

### C. Tiêu điểm (Focus) và Bàn phím/IME

1. **Phân biệt rõ 4 loại Focus:**
   - *Selected:* Trạng thái dữ liệu được chọn (ví dụ: Tab đang active, Radio đã check).
   - *Pressed:* Trạng thái đang được nhấn giữ vật lý bởi chuột hoặc ngón tay.
   - *Input / Keyboard Focus:* Tiêu điểm nhận ký tự từ bàn phím cứng hoặc ảo (có caret nhấp nháy,
     kết nối IME buffer).
   - *Accessibility Focus:* Vòng tiêu điểm màu xanh/vàng của TalkBack/VoiceOver đang đọc thành phần.
   Bốn trạng thái này độc lập và không được đánh đồng với nhau.
2. **Không cướp Focus (Zero Focus Stealing):**
   - Tuyệt đối không cướp focus của người dùng khi: màn hình tự động tải lại (silent refresh),
     request ngầm hoàn tất, hoặc thành phần recompose / re-render.
   - Khi chuyển trang hoặc tải xong dữ liệu, chỉ đặt focus vào thành phần đầu tiên nếu đó là luồng
     chủ động của người dùng (ví dụ: mở Dialog, chuyển qua màn hình mới); không tự động nhảy focus
     khi người dùng đang đọc ở lưng chừng trang.
3. **Bảo toàn IME tiếng Việt và Controlled TextField:**
   - Với Compose, giữ `TextFieldValue` cùng selection/composition. Trên Web, `TextField`,
     `SearchField` và `TextArea` dùng DOM input/textarea với `Signal<String>` và `TextInput`:
     giữ node, selection và xử lý composition hiện có; không ghi đè giá trị đang gõ hoặc submit
     giữa composition. Chỉ editor CodeMirror giữ `EditorView` và state/selection/composition của nó;
     không đổi input thường sang CodeMirror để sửa focus/IME. Không dùng vòng chuyển đổi chuỗi làm
     mất caret hoặc bộ đệm gõ dấu tiếng Việt (Telex, VNI, dấu tổ hợp Unicode NFC/NFD).
   - Không remount hoặc thay thế node input trong cây giao diện chỉ để làm hiệu ứng transition hoặc
     đổi theme, vì sẽ làm bàn phím ảo bị đóng/mở lại và mất kết nối IME.
4. **Quy tắc phục hồi Focus (Focus Restoration):**
   - Khi đóng Dialog, Sheet hoặc Menu:
     * Nếu đóng bằng *Escape*, *Click outside* hoặc nút *Hủy*: trả focus về phần tử kích hoạt
       (trigger button) ban đầu.
     * Nếu đóng do *chuyển hướng (Navigate)* sang trang mới: chuyển focus lên tiêu đề trang mới
       hoặc vùng nội dung chính; **không** trả focus về phần tử cũ đã biến mất hoặc bị che khuất.
     * Nếu phần tử kích hoạt ban đầu đã bị xóa khỏi giao diện: chuyển focus về container gần nhất.
5. **Trách nhiệm đối với Window Insets và Keyboard:**
   - Tránh cộng dồn padding hai lần: nếu Scaffold hoặc Activity đã tiêu thụ `imePadding()` hoặc
     `safeDrawingPadding()`, thành phần con bên trong không được áp dụng lại insets đó.

### D. Trạng thái và Hiệu năng cảm nhận (Perceived Performance)

1. **Ổn định nội dung, vị trí cuộn và danh tính (Identity Stability):**
   - Khi làm mới danh sách (refresh) hoặc nhận dữ liệu mới: giữ nguyên vị trí cuộn hiện tại của
     người dùng; không đẩy người dùng về đầu trang (scroll jump).
   - Giữ ổn định khóa định danh (`key(...)` trong Compose, `Key` trong Web list) để tránh tái tạo
     lại toàn bộ danh sách, làm giật khung hình hoặc chớp trắng giao diện.
2. **Không dùng Animation để che đậy lỗi kỹ thuật:**
   - Shimmer, skeleton hoặc animation không được dùng để che giấu việc decode ảnh quá chậm, nghẽn I/O,
     hoặc xử lý nặng trên Main thread.
   - Nếu tìm kiếm bị giật: phải tối ưu thuật toán lọc, debouncing hoặc chuyển sang coroutine nền;
     không được "chữa cháy" bằng cách thêm spinner xoay vòng để che giật.
3. **Không suy diễn độ mượt từ ảnh tĩnh hoặc bản build Debug:**
   - Ảnh chụp màn hình (PNG/WebP) chỉ chứng minh bố cục tĩnh tại một thời điểm, không chứng minh
     chuyển động mượt mà (60fps/120fps).
   - Bản build JVM Desktop hoặc Android Debug chạy với cờ debug, unoptimized bytecode và tắt R8
     sẽ có hiện tượng giật cục cục bộ do recomposition tracking; không lấy đó làm thước đo hiệu năng
     cuối cùng, cũng như không tự suy diễn là bản Release trên thiết bị thật sẽ mượt.
4. **Không thêm trạng thái lạc quan (Optimistic State) trái cam kết nghiệp vụ:**
   - Không tự ý hiển thị "Đã lưu bản nháp", "Đã gửi bình luận" hay "Đã xuất bản" trước khi server
     hoặc local storage có thẩm quyền xác nhận, trừ khi nghiệp vụ đã chốt cơ chế offline/optimistic rõ ràng.
