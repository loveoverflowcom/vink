# Gửi bình luận và trạng thái duyệt — brief hành vi (#120, chưa design)

**Ngày:** 09/11/2026. **Nguồn:** [#120](https://github.com/loveoverflowcom/vink/issues/120) §7,
lát DISC-01; quyết định nền ở [admission bình luận](../architecture/discussion-admission.md).
Brief này chỉ mô tả **hành vi, cấu trúc và trạng thái** để chủ sản phẩm thiết kế sau. Không có
PNG, Figma, prototype hay code Leptos/CMP; triển khai UI quay về #45 khi có design. Nó mở rộng,
không thay, [LayoutSpec D2T-03](discussion-ui.md) đang chạy trên Web reader.

## Anatomy đề xuất

```text
Reader / Nhận xét
  Heading + trạng thái mở/đóng thật + số liệu được phép công bố
  Root composer: danh tính được phép + ô nhập + hành động gửi
  Kết quả gửi của chính mình / vùng khôi phục khi bình luận nằm ngoài trang đang tải
  Root list
    Root: byline → nội dung hoặc neo trung tính → thao tác theo capability
    Một cụm replies phẳng
      Ngữ cảnh “Đang trả lời …” khi còn được phép xem
      Replies ngang cấp + tải thêm cục bộ
      Một reply composer đang hoạt động trong nhánh
  Root pagination / điểm kết thúc thật
```

Không thêm tab thảo luận chính, không card lồng cho từng reply. Nhóm bằng surface/khoảng cách
Lavender; mọi màu, radius, icon, spacing và motion do design sau quyết định trên token hiện hành.
Không gradient, viền trang trí lặp hay pulse vô hạn trong trang đọc; focus/lỗi/đang chọn vẫn dễ
nhận biết.

## Trạng thái logic cần phân biệt

| Trạng thái | Nghĩa | Hiển thị cần có (chưa chốt visual) |
| --- | --- | --- |
| Draft | Chữ chưa gửi | Giữ Unicode, IME, xuống dòng thơ, caret; không mất khi trạng thái quanh nó đổi |
| Sending | Đã có key logic + snapshot bất biến của lần gửi | Có thể hiện hàng cục bộ **chỉ cho người gửi**, không giả công khai, không tăng số đếm |
| Uncertain | Timeout/mất ACK, không chứng minh thất bại | “Kiểm tra lại/Thử lại” dùng đúng key + payload cũ; chữ gõ mới là ý định riêng |
| Refused | Server từ chối trước khi lưu (phiên/đình chỉ, đóng bình luận, đích không còn) | Lý do theo mã contract; giữ chữ; không có hàng chờ duyệt nào được tạo |
| CommittedPending | Server xác nhận đã lưu, chờ người duyệt | “Đang chờ duyệt”; tải lại vẫn thấy; không spinner mạng vô hạn |
| CommittedPublic | Server xác nhận công khai | Gộp theo ID với danh sách công khai, không hai bản |
| Held (tương lai) | Đã công khai theo policy đăng-trước rồi bị hậu kiểm rút về chờ duyệt | Với người viết: quay về “Đang chờ duyệt”; với người khác: biến mất hoặc thành neo nếu còn reply công khai |
| Rejected | Bị từ chối | Lý do chỉ cho người viết; không khuyến khích gửi lại mù để né policy |
| Removed/Unavailable | Bị gỡ hoặc không còn quyền xem | Không lộ chữ/byline/ngữ cảnh đã thu hồi; root ẩn vẫn neo replies hợp lệ |

Hôm nay mọi bình luận mới là **CommittedPending** (`premoderation.v1`). Mã provenance như
`trust_stale` hay `content_flagged` là dữ liệu audit nội bộ, **không** hiển thị cho người dùng.
Held chỉ xuất hiện nếu D-08 delta được chấp nhận; design không cần màn riêng trước đó.

## Quy tắc gửi và thử lại (CHOICE-06 A)

- Key sinh ở controller cho **một ý định gửi**, không phải mỗi lần retry mạng; local correlation ID
  không thay ID bình luận do server cấp. Không ép đổi `CommandKey` hiện có sang UUIDv7.
- Sửa chữ sau timeout không gửi dưới key cũ: đối soát lệnh cũ trước, rồi xử lý ý định mới rõ ràng.
  ACK đến muộn không xóa chữ vừa gõ. Fence actor/session/work/root/attempt tương đương Web và CMP.
- Kết quả gửi ở gần composer tới khi xác nhận hoặc đối soát xong, kể cả khi bình luận nằm ngoài
  trang đã tải. Phương án B (hàng local theo submission rồi gộp theo ID server) để designer xét
  cho di động; nó cần trạng thái merge/paging/session/restart riêng.
- Muốn thử lại qua tải lại trang hoặc khởi động lại app phải chốt trước storage, thời hạn và cô lập
  tài khoản cho payload đã gửi; bộ nhớ RAM không được quảng cáo là khôi phục sau restart. Không tự
  gửi offline.
- Lỗi tải thêm nằm cạnh nút và giữ phần đã đọc; làm mới hay duyệt muộn không giật/đổi thứ tự mạch
  đang đọc. Tắt bình luận giữ lịch sử, khóa gửi mới, giữ draft. Reaction, “Hữu ích” và report là ba
  tín hiệu khác nhau; số đếm chỉ đổi theo facts server đã xác nhận.

## Ma trận cho designer

Vai trò khách/thành viên/tác giả tác phẩm/moderator đúng quyền; root/reply/ngữ cảnh mất quyền;
sáng/tối; bề rộng 320/390/834/1440; cỡ chữ 100%/200%, tên dài, tiếng Việt, thơ, IME; bàn phím,
chạm, screen reader, giảm chuyển động; focus không mất khi làm mới. Đây là danh sách cần phủ khi
design, **không** phải bằng chứng đã kiểm.
