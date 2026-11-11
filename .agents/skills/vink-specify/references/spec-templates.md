# Mẫu đặc tả, bản ghi quyết định và kịch bản đối soát (`vink-specify`)

Khi spec dẫn một quyết định đã chuyển, dùng [kho TOML](../../../../decisions/README.md) để tra
ID và revision. Chỉ ghim trong spec cần theo dõi freshness. Bàn giao ghi snapshot và đối chiếu
ngữ nghĩa ngay cả khi pin còn đúng.

## 1. Mẫu bản ghi quyết định (Decision Record)

Dùng mẫu này cho bàn giao hoặc quyết định còn lưu ở Markdown. Với ID đã chuyển, lấy dữ liệu
tương ứng cho proposal và ghi phần căn cứ/tác động vào bàn giao.

```markdown
### [ID quyết định] — [Tên ngắn gọn, phạm vi áp dụng]

- **Câu hỏi cần giải quyết:** [Tình huống thực tế và vấn đề sản phẩm/kiến trúc cần chốt]
- **Lựa chọn đã chốt & lý do:** [Phương án được chọn, hệ quả chính, người quyết định (`PRODUCT_CONFIRMED` bởi chủ sản phẩm hoặc `DESIGN_PROPOSED` bởi agent)]
- **Nguồn xác nhận:** [Ngày, ngữ cảnh cuộc trao đổi / PR / issue]
- **Quyết định cũ bị thay thế (nếu có):** [ID/mục cũ bị thay thế một phần hay toàn bộ; phần nào của quyết định cũ vẫn giữ nguyên]
- **Tài liệu sở hữu cần cập nhật:** [Danh sách đường dẫn `prd/requirements.md`, domain spec, registry, feature map]
- **Điểm còn mở & điều kiện xem lại:** [Các mục `OPEN` liên quan và thời điểm/lát cắt bắt buộc phải chốt]
```

## 2. Mẫu đặc tả 7 phần theo miền

Chỉ dùng các mục có liên quan tới phạm vi thay đổi; giữ ngắn gọn và dẫn link thay vì chép lại:

```markdown
## 1. Mục tiêu và nghiệm thu
- **Yêu cầu sở hữu:** [ID trong `prd/requirements.md`]
- **Trạng thái 4 chiều:**
  - Quyết định sản phẩm: `PRODUCT_CONFIRMED` | `DEFERRED` | `OPEN`
  - Quyết định kiến trúc: `DESIGN_PROPOSED` | `Provisional` | `Accepted`
  - Mức triển khai: `NOT_IMPLEMENTED` | `core` | `persistence-api` | `worker-provider` | `web` | `cmp`
  - Bằng chứng: [lệnh/lane + SHA hoặc `NOT_RUN`]
- **Nghiệm thu quan sát được:** [Hành vi thành công và các nhánh từ chối/lỗi]

## 2. Thuật ngữ domain
| Tên trong sản phẩm (Tiếng Việt) | Tên trong code / API / DB | Trạng thái (`existing` / `proposed`) | Ghi chú ranh giới |
| --- | --- | --- | --- |

## 3. Quyền và trách nhiệm
| Actor | Hành động | Đối tượng & trạng thái | Điều kiện cho phép | Kiểm tra bền vững tại commit |
| --- | --- | --- | --- | --- |

## 4. Trạng thái và chuyển trạng thái
- [Các trục trạng thái độc lập, version/generation fence, idempotency receipt, điều kiện từ chối stale]

## 5. Trải nghiệm người dùng (UX contract)
- [Hiển thị theo trạng thái, copy tiếng Việt, xử lý chờ/lỗi/xung đột]
- **Ranh giới thiết kế trước runtime (nếu áp dụng):**
  - *Target UX vs hành vi đang chạy:* [...]
  - *Dữ liệu mẫu vs API thật:* [...]
  - *Phụ thuộc runtime cần hoàn tất trước khi nghiệm thu E2E:* [...]

## 6. Hợp đồng dữ liệu, quyền riêng tư và AI
- **Lưu trữ & vòng đời xóa:** [Bảng/trường sở hữu, hành vi khi chuyển riêng tư / tác giả xóa / kiểm duyệt gỡ]
- **Ranh giới AI provider (nếu có):**
  - Chỉ gửi khi có hành động gửi duyệt rõ ràng (không bao giờ gửi khi lưu nháp riêng `MOD-05`/`INV-08`).
  - Chỉ gửi đúng snapshot bất biến đang xét; không gửi thông tin định danh thừa.
  - Không ghi nội dung bài/bình luận riêng tư vào log vận hành.
  - Xác định rõ thời gian lưu kết quả đánh giá và hành vi khi bài bị xóa.

## 7. Triển khai, bằng chứng và gap còn lại
- **Code owners:** [`services/vink-app/src/...`, `apps/vink-web/src/...`]
- **Kiểm chứng:** [`cargo xtask verify quick|discussion|postgres|web|kani`]
- **Gap còn lại:** [Phần chưa triển khai hoặc chưa chạy kiểm thử]
```

## 3. Sáu kịch bản chuẩn để kiểm bộ đôi `vink-shape` + `vink-specify`

1. **Làm rõ hành vi trên trang thay thế sau khi tác giả xóa bài (`O-03`):**
   - `vink-shape` nhận diện `PUB-04/05` đã chốt 4 trường hợp A/B/C/D, còn `O-03` (quyền ghi trên
     tombstone) có baseline kỹ thuật trong `domain::publication` (`#157`: chặn bình luận mới, cho
     người viết tự gỡ bình luận cũ) nhưng vẫn `OPEN` ở mức sản phẩm. Khi làm route/UI tombstone,
     agent nêu rõ baseline hiện tại và hỏi chủ sản phẩm trước khi khóa hợp đồng công khai.
2. **Làm rõ quan hệ giữa sửa bình luận và “Hữu ích” (`COM-10`, `LVE-FR-04`, `O-06`):**
   - `vink-shape` đặt tình huống bình luận có 20 lượt “Hữu ích” rồi được sửa và duyệt, so sánh hai
     phương án (giữ nguyên tổng số theo `CommentId` vs neo theo revision bình luận và bắt đầu lại số
     đếm công khai cho bản viết lại), khuyến nghị phương án neo theo revision (`O-06`).
3. **Cập nhật tài liệu đăng nhập username + mật khẩu và phát hiện lệch ký tự vs byte (`ACC-03`, `O-12`):**
   - `vink-specify` cập nhật `prd/requirements.md` và `product-decisions-registry.md` để phản ánh
     backend `#152` đã có, tách phần Web UI / recovery / production unlock còn thiếu, đồng thời ghi
     rõ độ lệch giữa PRD (`≥ 10 ký tự`) và `Password::parse` (`10–128 byte` UTF-8) mà không tự ý sửa
     luật sản phẩm để hợp thức hóa code. Khi chủ sản phẩm ủy quyền chốt, luật mới được ghi bằng
     proposal + signoff (`O-12@r2`: 10–128 ký tự sau NFKC) trước khi đổi `Password::parse`.
4. **Sửa một lỗi giao diện đơn giản mà không kích hoạt quy trình hỏi không cần thiết:**
   - Khi yêu cầu chỉ là sửa tràn chữ tiêu đề dài hoặc khoảng cách nút trên Bàn viết, `vink-ui-fix`
     hoặc `vink-work-item` xử lý trực tiếp cùng LayoutSpec rút gọn, không gọi `vink-shape`.
5. **Thay đổi một quyết định cũ và cập nhật lại PRD, đặc tả, registry:**
   - Khi chủ sản phẩm đổi một quy tắc (ví dụ từ tiền kiểm thủ công sang trợ lý AI duyệt mặc định ngày
     10/11/2026), `vink-specify` ghi rõ quyết định mới thay thế phần nào, giữ lại các bất biến còn
     hiệu lực (`COM-07/08`, `MOD-01/05`, `INV-01..08`), giữ nguyên bằng chứng lịch sử theo SHA cũ và
     đồng bộ cả đầu lẫn cuối registry để không còn hai đoạn mâu thuẫn nhau trong cùng một file.
6. **Thiết kế một màn hình cho tính năng chưa có runtime nhưng ghi rõ các phụ thuộc cần hoàn tất:**
   - Khi dùng `vink-ui-design` thiết kế màn Bộ sưu tập cá nhân (`COL-01..06`) hoặc trạng thái xuất
     bản của tác giả, `vink-specify` bảo đảm đặc tả tách rõ target UX vs hành vi đang chạy, dữ liệu
     mẫu vs API thật, và danh sách migration/API/worker cần có trước khi nghiệm thu thật.
