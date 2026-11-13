# LayoutSpec · Quên mật khẩu (#181)

**Trạng thái:** đề xuất cho triển khai sau; không có UI/ảnh runtime mới trong PR này.
Nguồn: [đặc tả recovery](../architecture/password-recovery.md),
[Auth Password Web](auth-password-web.md), `/toi` và component hiện có tại `4c78601`.
Không có mốc thị giác recovery đã duyệt: `NO_APPROVED_REFERENCE`.

Hai hướng cùng ca khó “gửi lại mã nhưng thư cũ tới muộn”: A, một luồng inline trong `/toi`,
nhãn “Dùng mã cho yêu cầu đang mở trên trình duyệt này”, đường xin lại và quay về login; B, route recovery riêng
với cùng step/error, dễ deep-link nhưng thêm routing/history trong khi không dùng link token.
Đề xuất A vì không cần route/token URL mới, tái dùng form owner/AuthGate; không là signoff.

```text
Screen #page-account — giữ trang Tôi, Giao diện và điều hướng
  Region #password-login
    Action #forgot-password — “Quên mật khẩu?”
  Region #password-recovery — thay vùng form, không modal lồng
    Heading — “Đặt lại mật khẩu”
    Step #recovery-request — email khôi phục → “Yêu cầu mã”
      Notice — “Dùng email bạn đã xác minh cho tài khoản trước đây.”
      Action — “Quay về đăng nhập”
    Step #recovery-code — code → “Tiếp tục”; chỉ kiểm định dạng tại đây
      Status — “Nếu email đủ điều kiện, bạn sẽ nhận được mã đặt lại.”
      Action — “Yêu cầu mã mới”; Retry-After; “Đổi email”
    Step #recovery-password — code còn giữ trong form + mật khẩu mới → “Đặt lại mật khẩu”
      Hint — O-12; toggle hiện/ẩn dùng shared password field
      Notice — đề xuất: “Bạn sẽ cần đăng nhập lại trên các thiết bị.”
    Step #recovery-result — thành công được server xác nhận / chưa rõ kết quả
      Action — “Về đăng nhập” / “Kiểm tra kết quả”
  Region #recovery-email-settings — mục đích attach email trước khi quên, dependency chưa có
    Chưa gắn — “Chưa có email khôi phục”; “Thêm email khôi phục” (đề xuất tùy chọn)
    Đang xác minh — email + code; fresh reauthentication; không báo verified trước commit
    Đã gắn — chỉ trong phiên đúng người, trạng thái server; đổi email là use case riêng
```

**Hierarchy/sizing.** Giữ baseline của Auth: một nhiệm vụ chính/bước, form tối đa 28rem, label
hiển thị độc lập placeholder; không lồng card/pill hoặc thêm border idle. Shared FloatingTonal,
Button, notice và password reveal hiện có; không tạo primitive bằng tên suy đoán. Không hỏi
username hoặc bút danh trong reset. Không hiện “không tìm thấy tài khoản”.

**Responsive/theme/scroll.** 320/390/834/1440 CSS px × sáng/tối × chữ100/200; một cột co theo vùng
chứa, nút xuống hàng, không thu font. Cuộn trang chính, không cuộn lồng hoặc sticky CTA;
keyboard/insets không che field đang focus. Lavender/token hiện có dùng chung hai theme.
Không có overlay mới. Giữ truy cập mọi action/lỗi bằng cuộn, không tràn ngang quá 1 CSS px
(dung sai làm tròn), không cắt dấu tiếng Việt.

**Trạng thái/copy.** Unread/idle, requesting, request recorded, code input, submitting,
completed, uncertain, closed, rate-limited và unavailable riêng. Spinner trong nút có accessible
name, giữ bề rộng và chặn duplicate. Không nói đã gửi thư khi chỉ `202`. Lỗi format tại email,
code sai tại code, O-12 tại password; lỗi mạng ở vùng form có retry. Request đóng dùng câu chung
“Mã không còn dùng được. Hãy yêu cầu mã mới.” Chưa có/mất hộp thư: “Nếu chưa thêm email khôi phục
hoặc không còn vào được hộp thư, bạn chưa thể đặt lại theo cách này.” Không chẩn đoán account
cụ thể cho khách, không hứa hỗ trợ/manual reset không tồn tại.

**Interaction/focus/accessibility.** Chọn “Quên mật khẩu?” đưa focus email; bước mới focus heading
hoặc field đầu, lỗi liên kết `aria-describedby` và status live phù hợp, không thông báo giả “đã xác
minh” ở bước code local. Enter chỉ submit form hiện tại khi không composition. Paste/autofill đọc
DOM thực, `autocomplete=email`, `one-time-code`, `new-password`; code không autosubmit.
Toggle password không submit, giữ caret/selection khi click; Tab vẫn tới button. Quay lại giữ dữ
liệu trong cùng form generation khi an toàn; đóng/rời/đổi phiên xóa secret local, không lưu storage.
AuthGate giữ write đang bay tới khi kết thúc/đối soát; response cũ không update form mới.
Back/đóng không hứa hủy server effect. Hai tab dùng chung cookie: nếu request ID form không còn
khớp cookie, dừng form và nêu “Yêu cầu đã thay đổi trên trình duyệt này”, cho quay về login hoặc
bắt đầu lại; không thử mã vào request khác. Response cũ có Set-Cookie vẫn có thể làm mất ngữ cảnh,
không hứa AuthGate một mình ngăn được. Không animation dịch chuyển bắt buộc; reduced-motion giữ
cùng nghĩa/focus. Chưa render nên contrast/fidelity/IME không được tuyên bố PASS.

**Phiên và dữ liệu viết.** Reset không tự sign-in, không đặt một SessionOwner khác. Sau commit,
refresh nguồn phiên hiện hữu; giữ chữ cho cùng người theo `text_survives`, không xóa bản nháp đã
lưu hoặc đưa chữ/receipt sang người khác. Cảnh báo revoke mọi thiết bị là đề xuất chờ product signoff.

**Đối chiếu khi triển khai.** Capture/open mỗi bước và lỗi ở ma trận trên; keyboard, forced-colors,
zoom, reduced motion, giữ request/ACK rồi đổi mode/phiên, native IME và password manager ghi riêng.
Ảnh chỉ chứng minh bố cục, không chứng minh proof/privacy/atomicity. Mọi dữ liệu trong hình là
fixture/outbox cục bộ có nhãn, không email/mật khẩu/mã thật của người dùng.
