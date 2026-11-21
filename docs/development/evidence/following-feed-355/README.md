# Đang theo dõi · #355

Trước sửa là develop a9d6a4f9fb0a35b1e0e56f843132d4e3fe5dbb20, với smoke mở rộng để chụp cùng corpus.
Bản kiểm Web sau sửa là 83f1137efa8ad3c0be7d915e94f5699a23cf022f. Chrome 158.0.8127.36
trên macOS arm64, DPR 1, viewport cao 900 CSS px, App và Identity thật với PostgreSQL scratch
17.11, tài khoản và bài thơ kiểm thử. Hai lượt tạo Handle/WorkId riêng bằng cùng fixture.
200% là hệ số font gốc do CDP đặt. [Issue #355](https://github.com/loveoverflowcom/vink/issues/355)

- [Ảnh trước và sau](truoc-sau.jpg) gồm 390 sáng/tối, 320 tối chữ 200% và 1440 sáng.
- [Trạng thái và focus](trang-thai-va-focus.jpg) gồm hai loại rỗng, lỗi tải thêm, Back về nhan đề
  và Back về tiêu đề khi bài đã rút.
- Xem PNG Drive 296-01..04, 293-01..03, 297-02/03 và 99. Các khung ngày 20/11 có trạng thái
  DESIGN_PROPOSED / NO_APPROVED_REFERENCE. Ngày và thể loại ở trên nhan đề theo Lavender R3.
  VDR-SOCIAL-CONTRACT@r1 và VDR-SOCIAL-GRAPH@r1 vẫn PENDING ở mốc kiểm.
  [Chỉ mục thiết kế](https://drive.google.com/file/d/1E2OZaT9OJG_cKGuxeUewBxqPcOBru1Rm/view)

## Nghiệm thu Web trên SHA kiểm

| AC | Kết quả | Bằng chứng |
| --- | --- | --- |
| AC-01 | PASS | Chọn đúng mode, Tab theo thứ tự mode → nhan đề → bút danh. So từng WorkId, ngày và thứ tự với work_first_publications trong PostgreSQL sau Unfollow/Follow lại và tái đăng. Trích đoạn có dòng thơ và ngắt khổ. |
| AC-02 | PASS | Khách đến trang đăng nhập với return nội bộ. Feed rỗng dẫn đến Tác giả, tìm được hồ sơ chưa có bài, Follow có receipt sau mất ACK và trở về đúng trạng thái chưa có bài. Loading, lỗi đầu, hết phiên và lỗi tải thêm có trạng thái riêng. Append giữ các hàng trước đó và cho thử lại, đổi A→B→A bỏ response và chain cũ. |
| AC-03 | PASS | Reader Back đọc Feed theo phiên hiện tại. Lỗi mạng chưa kết luận mất anchor, thử lại phục hồi đúng nhan đề từ trang tải thêm. Rút bài khi Reader đang mở và Unfollow khi bài còn public đều về H2. SSR đã đăng nhập trả placeholder không chứa danh sách và no-store. Feed không gọi endpoint tác phẩm công khai trong ca kiểm. Kiểm mã xác nhận Feed gọi HTTP riêng, ngoài PublicKey và IndexedDB. |
| AC-04 | PASS trên Chrome/CDP | Ma trận 320/390/834/1024/1440 × sáng/tối × 100/200% có 0 px overflow ngang. Chữ tối thiểu 6,72:1 sáng và 8,16:1 tối. Focus trên mode đạt 5,78:1 sáng và 10,83:1 tối. Tab/Enter/Space, reduced motion và phần cuối danh sách với safe area mô phỏng đều đã kiểm. |

## Năm chiều UX

| Chiều | Kết quả và phép kiểm |
| --- | --- |
| Functional | PASS. Hành trình Feed rỗng → Tác giả chưa có bài → hồ sơ → Follow → Feed → Reader → Back. Smoke byline đi từ thẻ, Reader và nhận xét đến đúng hồ sơ, có ca thành viên Follow từ phản hồi rồi mở danh sách người theo dõi theo quyền. |
| Geometry | PASS. Thanh dưới có ba đích, pill 64 × 32 CSS px đúng khung 99, chênh tâm ngang lớn nhất 0,008 px, chênh trục dọc 0 px. Rail 96/220 px giữ hai link chính và hành động Tôi. Ở 320/200% nhan đề sau Back nằm từ y=186 đến y=424 px, H2 fallback từ y=176 đến y=290 px, phía trên thanh dưới bắt đầu tại y=756 px. |
| Visual | PASS theo LayoutSpec r2. Đã mở ảnh trước/sau và ảnh trạng thái. Nền danh sách phẳng, dòng thơ và bút danh còn đọc được. Ở 320/200% nhóm mode xuống hàng và phải cuộn để đọc bài đầu. Fidelity NO_APPROVED_REFERENCE. |
| Accessibility/Responsive | PASS trong phạm vi DOM, bàn phím và CDP đã đo. Safe area mô phỏng 24 px dưới và 20 px hai bên. Bộ gõ của hệ điều hành và trình đọc màn hình trên thiết bị là NOT_RUN. |
| Interaction | PASS cho đọc lại trước focus, mất ACK, lỗi mạng, đổi phiên và response muộn. Back trong hydrated document và lượt đi qua document khác đã chạy. BFCache hit tự nhiên là NOT_RUN, vì lần đo không có pageshow.persisted=true. |

## Lệnh và dữ liệu

- PASS trước sửa với cargo xtask verify web --scenario password, lượt 1795242176868354000-53196.
  15 PNG Feed. Lượt chuẩn bị trước đó lỗi quyền cache npm dùng chung, phục hồi bằng cache riêng.
- PASS sau sửa với cargo xtask verify web --scenario password --scenario return-position,
  lượt 1795242508473992000-61815. 19 PNG Feed và manifest. Lượt return-position công khai là đối chứng.
- Log local, PNG gốc và manifest giữ ở thư mục /private/tmp/vink-355-evidence.OcqmKp0a.
