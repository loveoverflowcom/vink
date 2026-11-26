# Lưu bài · Reader và tab Đã lưu

Người đọc lưu một bài ở cuối Reader, tìm lại trên hồ sơ của mình, mở bài rồi quay về cùng vị trí. **Hướng đã duyệt** là r2 + D1–D12 ngày 25/11/2026. LayoutSpec này dùng chi tiết R3 do chủ sản phẩm bổ sung cho r2. Miền Lưu bài đã có core (#386) và API (#387). Cặp Thấm · Lưu trên Reader đạt mức `web` (#388); tab Đã lưu trên hồ sơ (#389) vẫn `NOT_IMPLEMENTED`. [PRD §18](../../prd/requirements.md#18-lưu-bài-để-đọc-lại-saved-works) · [quyết định §8](https://github.com/loveoverflowcom/vink/issues/384) · [tám lựa chọn trong handoff R3](https://github.com/loveoverflowcom/vink/issues/384#issuecomment-7155013123).

| Mốc đối chiếu | Phạm vi |
| --- | --- |
| [Thư mục r2](https://drive.google.com/drive/folders/1yJy33JVl8u8YHU_1BMD1uc8KcxlvvQ4a) · [Preview HTML](https://drive.google.com/file/d/14lX1XPa9wDRfmn3Atz71DVcXG63uxuuZ/view) | Đã đối chiếu README, markup và 11 khung PNG: 00/01/01b là cặp nút, 03 là sheet, 04–06 là hồ sơ, 07–08 là state và chữ lớn. Khung 02 (phương án B), G2 và nhãn đổi sang “Đã lưu” là phương án lịch sử. |
| [Gói R3](https://drive.google.com/file/d/1ZFR1bS2-i1E5aaR3j8gBs8RSHjZozbYr/view) | LayoutSpec và DESIGN_SPEC §1–8 chi tiết hóa hình học, copy, focus và responsive. Ảnh là concept với dữ liệu mẫu; nghiệm thu cần Web chạy thật ở #388–389. |
| [Pen Flight](pen-flight-r2.md) · [sheet Reader](auth-reader-web.md) · [token chung](../../packages/ui-system/shared/tokens.json) | Nguồn của glyph Thấm, phiên đăng nhập và token R4/Tailwind. Trong nhóm dùng chữ `on-surface` theo D4; nếu đổi token chung phải kiểm mọi consumer chịu ảnh hưởng. |

## Cây vùng và thứ tự focus

```text
Reader /tac-pham/{work_id}
  article#reader-body → byline → thân bài → thẻ
    #work-endorsement
      #sv-pair (group “Thấm và lưu tác phẩm”) → [Thấm + số] [Lưu]
      #sv-note (aria-live polite) → liên kết “N người đã Thấm” khi khả dụng
  #discussion
  dialog đăng nhập Reader (khi khách chọn Lưu)

Hồ sơ /@handle
  định danh → bio → quan hệ → hành động
  #profile-view-slot
    khách/người khác → Tác phẩm công khai
    đúng chủ → nav “Hồ sơ của tôi” → Tác phẩm | Đã lưu (link, aria-current=page)
  /@handle/da-luu (đúng chủ)
    “Chỉ mình bạn thấy danh sách này.”
    #saved-list (ul) → li → liên kết bài nếu đọc được → nút Bỏ lưu độc lập
    #saved-tail → Xem thêm | tiến trình | lỗi và Thử lại | Đã hết danh sách
```

Ở cả CSR và SSR, xác nhận phiên và chủ hồ sơ trước khi tải hoặc hiện vùng riêng. Khách/người khác mở URL tab thấy NotFound giống đường dẫn không tồn tại. Hồ sơ công khai và nội dung SSR không chứa tab, link, count hay dữ liệu riêng. Khi phiên đổi, xóa vùng riêng, cả trang đã tải lẫn phản hồi cũ.

## Cặp Thấm · Lưu

- **D1, D3, D5.** Một nhóm ngay sau thân bài và trước nhận xét. Mỗi nút chiếm nửa chiều rộng khi nằm ngang, cao 56 px ở 100%, khe 2 px, nhóm tối đa `max(400px, 25rem)`. Góc ngoài 28 px, góc ở khe 8 px. Icon và chữ căn giữa khi chưa chọn, số ở ngay cạnh chữ “Thấm” trong một cụm. Hai thao tác có quyền, số liệu và trạng thái chờ độc lập. Bài không đọc được không có nút Lưu.
- **D4.** Dùng token R4 qua Tailwind v4, scope chữ `on-surface` trong nhóm. Pen Flight và bookmark cỡ 24 px (20 px ở vùng hẹp theo DESIGN_SPEC). Bookmark viền khi tắt, đặc + nền tonal + viên thuốc + ✓ khi bật. Giữ chữ nhìn thấy “Lưu”. Tên truy cập luôn là “Lưu”; trạng thái đọc bằng `aria-pressed` theo dữ liệu đã xác nhận (chốt 26/11/2026, xem ghi chú #388 bên dưới). Dùng cả hình và ký hiệu để phân biệt trên nền sáng/tối và forced colors. Đích tương phản chữ ≥ 4,5:1; icon và focus ≥ 3:1. Hai icon `bookmark` và `bookmark-border` trong catalog và provenance ở DESIGN_SPEC §3 giao lát Reader #388.
- **D2.** Khách bấm Lưu mở sheet “Đăng nhập để lưu bài”, lead “Bài sẽ nằm trong danh sách Đã lưu riêng trên hồ sơ của bạn.” Nút chính “Đăng nhập”, sau “Hoặc” là hai nút Tonal toàn chiều rộng theo thứ tự “Dùng mã email”, “Tạo tài khoản” theo [Auth R3 #559](auth-reader-web.md). #388 đã dùng viền 2 px cho hai nút phụ ở bản trước R3. Focus vào Đóng, giữ Tab trong dialog, nền inert; Escape/Đóng trả focus. Đăng nhập xong quay đúng bài và focus Lưu, đọc lại trạng thái của phiên mới. Nếu chưa lưu: “Bạn đã đăng nhập. Bấm Lưu để lưu bài này.” Không phát lệnh lưu khi quay về.
- **D11.** Bấm/chạm/Enter/Space gửi desired state từ trạng thái đã biết. Khi đang đọc overlay lần đầu, chưa được coi bài là chưa lưu. Pending giữ focus, icon và pressed cũ, khóa bấm lặp, spinner tại nút kèm “Đang lưu…”/“Đang bỏ lưu…”. Chỉ ACK hợp lệ mới đổi hình (`duration.medium`, 320 ms); reduced motion đổi ngay, tiến trình tĩnh có chữ. Sau lưu: “Đã lưu vào hồ sơ của bạn.” và link “Mở Đã lưu”; sau bỏ: “Đã bỏ lưu.”
- Lỗi cạnh nút: “Chưa lưu được. Bạn có thể thử lại.” hoặc “Chưa bỏ lưu được. Bạn có thể thử lại.” Đủ hạn mức D6: “Danh sách Đã lưu đã đầy. Bỏ lưu một bài để lưu thêm.” Mất ACK: “Chưa xác nhận được lượt lưu.” + “Kiểm tra lại”. Đọc lại trạng thái trước thao tác mới; không phát lại tự động. Vùng phản hồi chừa tối thiểu một dòng dưới nhóm.
- **Triển khai #388 · chủ sản phẩm chốt ngày 26/11/2026** (trả lời trên [PR #546](https://github.com/loveoverflowcom/vink/pull/546): “Tên truy cập của nút Lưu => Như bạn đề xuất”, hai điểm còn lại “OK”). Đây là quyết định thiết kế trong chat, chưa phải signoff TOML theo `VDR-DECISION-SIGNOFF@r1`.
  - *Tên truy cập:* nút Lưu giữ tên “Lưu” ở mọi trạng thái và báo trạng thái bằng `aria-pressed`; không đổi thành “Bỏ lưu” để tránh cách đọc “Bỏ lưu, đã nhấn” mà [hợp đồng ToggleAction](../../packages/ui-system/shared/components.md) đã loại. Thấm giữ tên theo hành động (“Thấm”/“Bỏ Thấm”), không có `aria-pressed`.
  - *“Mở Đã lưu”:* trỏ tới `/@handle/da-luu` theo handle hiện hành của principal, kể cả khi địa chỉ chưa hoàn tất thiết lập. Trang này thuộc #389; trước khi #389 merge, địa chỉ trả NotFound như mọi URL chưa có.
  - *Thấm của khách:* vẫn hiện lý do “Cần đăng nhập để Thấm.” (artboard R3 không có dòng này). Luồng khách của Thấm nằm ngoài #388; chỉ Lưu mở sheet đăng nhập.
  - Nhóm có id `#sv-pair`; mỗi nút có vùng phản hồi riêng (`save-work-note` cho Lưu) thay cho một `#sv-note` chung.

## Danh sách và quay lại bài

Chỉ đúng chủ thấy hai chế độ cùng vị trí tiêu đề “Tác phẩm công khai”. D6/D7/D10: mới lưu trước, 20 bài/lần, dùng “Xem thêm”; không có tổng hay cuộn tải vô hạn. D8/D9/D12 cho phép lưu bài công khai của chính mình, không thông báo và không tự hết hạn. Thứ tự khi bằng thời điểm và các trường được hiện theo [hợp đồng HTTP](../../services/vink-app/contracts/http/v1/saved-works.md).

| Tình huống | Giao diện và tương tác |
| --- | --- |
| Đọc được | Một cột thẻ cách 8 px. “{Thể loại} · lưu {dd/mm/yyyy}” (thiếu thể loại chỉ hiện ngày lưu), nhan đề, bút danh và @handle nếu có quyền. Link mở bài ≥ 44 px; nút Bỏ lưu 48 × 48 px là phần tử anh em của link, tên “Bỏ lưu: {nhan đề}”. |
| Ẩn / tác giả hoặc tài khoản tác giả đã xóa | Icon + đúng câu “Nội dung đã được ẩn” / “Nội dung đã được tác giả xoá” + Bỏ lưu với tên truy cập trung tính. Không ngày, liên kết đọc hay metadata. Admin gỡ/missing không chiếm hàng. Công khai lại thì chiếu lại như PRD. |
| Trống / tải lần đầu | Chỉ sau trang thành công rỗng mới hiện “Chưa lưu bài nào.”, hướng dẫn “Bấm Lưu ở cuối một tác phẩm để đọc lại sau ở đây.” và “Khám phá tác phẩm”. Trong khi tải, skeleton sau 300 ms, “Vẫn đang tải…” sau 8 s. Lỗi đọc lần đầu có Thử lại. |
| Tải thêm | Giữ hàng, focus và vị trí cuộn. “Đang tải thêm…”; khi lỗi “Chưa tải thêm được. Các bài đã tải vẫn ở nguyên.” + “Thử lại”. Cuối chuỗi hiện “Đã hết danh sách”. |
| Bỏ lưu | Pending trong nút, giữ hàng tới ACK. Lỗi “Chưa bỏ lưu được. Bài vẫn nằm trong danh sách.”; mất ACK “Chưa xác nhận được.” + “Kiểm tra lại”. Sau ACK chuyển focus đến link hoặc nút còn dùng được ở hàng kế, rồi hàng trước hoặc tiêu đề trống. Báo “Đã bỏ lưu “{nhan đề}”.” nếu còn được đọc, nếu không “Đã bỏ lưu.”; không có Hoàn tác. |
| Mở bài → Back | Giữ vị trí cuộn và focus của hàng theo WorkId trong cùng phiên, đối soát quyền và dữ liệu trước khi vẽ lại. Nếu hàng mất, focus về tiêu đề. |

## Responsive và nghiệm thu

Dưới 760 CSS px dùng bottom bar và sheet chạm đáy; từ 760 dùng rail và dialog giữa màn. Hai nút bằng nhau khi xếp ngang lẫn dọc. Khi nhóm hẹp dưới 20rem hoặc không đủ chỗ lúc tăng chữ, xếp dọc trước khi cụm “Thấm + số” bị cắt; ưu tiên giữ cụm trên một dòng. Nav hồ sơ dưới 22rem gập thành menu vẫn có hai link. Sheet cuộn dọc khi có bàn phím. Focus ring 3 px, offset 2 px. Hai theme dùng chung cấu trúc.

#388–389 kiểm 320/390/1024 px, sáng/tối, chữ 100/150/200%, nhan đề dài, count nhiều chữ số, Tab/Shift+Tab/Enter/Space/Escape, reduced motion và trình đọc màn hình. Kiểm không tràn ngang, nhóm cao/rộng bằng nhau (dung sai 0,5 CSS px ở 100%), đường về đúng hàng, đổi phiên, lỗi và mất ACK. Đối chiếu quyền riêng ở cả CSR standalone lẫn SSR/hydration đang ship. [Kế hoạch bằng chứng](../development/saved-works.md).
