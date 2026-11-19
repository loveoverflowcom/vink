# Nghiệm thu Follow tác giả và danh sách quan hệ #294

[PR #331](https://github.com/loveoverflowcom/vink/pull/331) · [LayoutSpec Social r2](../../../design/social-289.md) · [Hợp đồng App](../../../../services/vink-app/contracts/http/v1/social-graph.md).

Nhánh tích hợp `develop@46bb5e58e5669f39c2655994e47742abec79821c` bằng merge thường. Mã kiểm chứng:
`5bff20bc5f33c77200ccd21174ee29de58a9f505`. Commit bằng chứng tiếp theo chỉ đổi tài liệu và ảnh.

## Điểm sửa từ review

- Quyền list/count theo projection mới thay cho dữ liệu được phủ lên bằng receipt. Focus không
  được giữ lại link/số đếm đã thu hồi. Own list và own summary chỉ dùng bằng chứng đúng phiên.
- Read kiểm quyền trước cursor/validator; cursor của danh sách riêng buộc theo phiên và chỉ
  tiếp tục nếu tìm lại được cạnh mốc trong snapshot hiện tại. Response sang phiên khác không
  khôi phục các dòng cũ.
- Hai đích danh sách có route cố định, không còn route tổng quát chặn tài nguyên `/pkg/`.
  Giữ các smoke Auth đã gộp, cùng Style token của Reader. Lượt Chrome dùng bundle build tại chỗ.
- Thêm rollback khi receipt không ghi được; vẫn giữ số đếm, cạnh và order cũ. Hai thứ tự chờ
  khóa set/retire, cặp Follow hai chiều và kiểm migration nằm trong bộ PostgreSQL.

## Lệnh đã chạy trên Mac

| Lệnh | Kết quả |
| --- | --- |
| `python3 ./tools/local-budget run -- cargo xtask verify unit app --filter follow` | PASS 16 test khớp bộ lọc. |
| `python3 ./tools/local-budget run -- cargo xtask verify unit web --filter follow` | PASS 9 test khớp bộ lọc. |
| `python3 ./tools/local-budget run -- cargo xtask verify quick` | PASS 13/13 bước (chạy lại trên đúng SHA). |
| `python3 ./tools/local-budget run -- cargo xtask verify postgres` | PASS 29/29 bước: Store 6, App 242, Identity 52; 7 probe App opt-in bị bỏ qua. |
| `cargo xtask verify web --scenario password` | PASS: Auth, Follow, Profile và Handle; run ID `1795093619404850000-10465`. |
| `git diff --check`; `node --check apps/vink-web/e2e/author-follow-smoke.mjs` | PASS. |

Chạy tuần tự với `CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0`.
macOS 15.6 arm64; PostgreSQL 17.11 dùng cluster scratch localhost và database riêng cho App, Identity và test HTTP;
Web launcher dùng Chrome 157.0.8103.79, Wasm và App/Identity thật, password fixture và outbox cục bộ.
Lane Web chạy ngoài local-budget vì launcher không quản lane này. Lần Draft bị local-budget từ chối
(exit 75) trước khi gọi Cargo, không tính vào kết quả.

## Bằng chứng theo chiều

| Chiều | Kết quả trong phạm vi chạy |
| --- | --- |
| Functional | PASS: trạng thái đã xác nhận, Mutual chỉ cho hai chủ thể, không self-follow; kiểm list/count độc lập, phân trang giữ dòng khi lỗi và xóa dòng khi thu hồi, cursor riêng gắn cùng phiên. PostgreSQL kiểm lệnh/cạnh/receipt trong một giao dịch và quyền trước ETag/cursor. |
| Geometry | PASS: 40 tổ hợp của hai bề mặt, 320/390/834/1024/1440 CSS px, sáng/tối và cỡ chữ gốc 100/200%; không tràn ngang trong sai số đo 1 px, vùng đích kiểm ≥44×44 CSS px. Đây không phải browser zoom hoặc device text scaling. |
| Visual | PASS khi đối chiếu LayoutSpec, `NO_APPROVED_REFERENCE`. Đã mở xem cả bốn ảnh. Danh tính, Follow và số đếm tách lớp; danh sách không có hàng lồng link; ở 320 px/tối/chữ 200% tiêu đề dài xuống dòng, không cắt chữ. Ảnh mobile ghi vùng đầu danh sách; đích Tải thêm được kiểm bằng cuộn/bàn phím, không suy từ ảnh. Ảnh profile ghi trạng thái nút không khả dụng của fixture. |
| Accessibility/Responsive | PASS: bàn phím, focus-visible, link và nút có nghĩa đúng; tương phản chữ profile tối thiểu [6,98 sáng](follow-profile-contrast-light.json) / [8,16 tối](follow-profile-contrast-dark.json), danh sách [6,77 sáng](follow-list-contrast-light.json) / [7,87 tối](follow-list-contrast-dark.json); vòng focus 5,78 / 10,83. Không đo thay cho trình đọc màn hình. |
| Interaction | PASS: Follow bằng bàn phím giữ focus, xác nhận mới đổi count, Back khôi phục đích, chạm CDP; ACK muộn không ghi đè lệnh mới hoặc trả về cạnh của phiên A cũ sau A→B→A; focus làm mới link/count đã bị thu hồi. |

### Ảnh đã xem

- Hồ sơ: [1440 sáng, chữ 100%](follow-profile-1440-light-100.png), [320 tối, chữ 200%](follow-profile-320-dark-200.png).
- Danh sách: [1440 sáng, chữ 100%](follow-list-1440-light-100.png), [320 tối, chữ 200%](follow-list-320-dark-200.png).

`NOT_RUN`: production, thiết bị thật, trình đọc màn hình, browser zoom và BFCache hit.
Không thay đổi phạm vi #295/#296, xóa xuyên hai kho #297, hạn chế riêng theo SOC-DEP-02 hay preview #336.
