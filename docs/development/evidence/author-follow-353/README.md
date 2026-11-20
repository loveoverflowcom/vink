# Kiểm chứng Web cho Theo dõi và trạng thái hồ sơ #353

[Issue #353](https://github.com/loveoverflowcom/vink/issues/353) · [Draft PR #367](https://github.com/loveoverflowcom/vink/pull/367) · [LayoutSpec Social r2](../../../design/social-289.md) · [Bộ khung đề xuất](https://drive.google.com/file/d/1E2OZaT9OJG_cKGuxeUewBxqPcOBru1Rm/view)

Base `ab5ecaff9685938dc689b52d98957375fbbfdf12`. Source Web kiểm chứng `8ac0943025f556bb5bd340f1293551d472170f22`.
Worktree `/private/tmp/vink-353-follow-profile`. Mac arm64, macOS 15.6, Chrome headless
158.0.8127.36, DPR 1, root text 100%/200%, Be Vietnam Pro và Literata tải cục bộ.
Phiên App/Identity thật, PostgreSQL scratch 17.11 và outbox cục bộ của kịch bản `password`.
Mỗi lượt test tự cấp handle ngẫu nhiên cho cùng fixture của #331. Ca cursor hết hiệu lực ở lượt sau kích hoạt lại một cạnh, làm hàng đó lên đầu danh sách.

## Lệnh

- `python3.13 ./tools/local-budget run -- cargo xtask verify unit web --filter follow`
  trên `b4c64f4944cb4aa7274ae09de1d0302138d0b267`. PASS 12/12 test được chọn.
- `cargo xtask verify web --scenario password` trên base. PASS, lượt `1795182220193360000-81726`.
- `cargo xtask verify web --scenario password` trên source Web. PASS, lượt `1795183423621392000-89791`..
- `cargo fmt --all -- --check`, `node --check` trên hai smoke và mô-đun điều hướng, `git diff --check`.
  PASS.

## Đối chiếu

| Tiêu chí | Bằng chứng |
| --- | --- |
| AC-01 · Follow | Khách có return nội bộ, chủ có Chỉnh hồ sơ. Giữ ACK tại server, đo cùng DOM và focus ở 320 px/chữ 200%, Enter lặp không tạo PUT. Mất ACK đọc receipt, thông báo một lần. ACK cũ và phiên A→B→A không phục hồi trạng thái cũ. |
| AC-02 · Danh sách | Kiểm bốn tổ hợp list/count theo cả hai hướng, count 0 thật, lỗi 503, Mutual trong overlay riêng, quyền bị thu hồi và cursor hết hiệu lực. Giữ hàng, giữ nút khi thử lại và đưa focus đến hàng mới hoặc H1 tùy kết quả. |
| AC-03 · Trạng thái hồ sơ | PASS GET/HEAD 301/404/410/503. Alias trong app đưa focus về H1 chuẩn. 410 bỏ định danh cũ. 503 thử lại cùng URL/cursor. Tab và Enter ở 404/410 cùng Enter ở 503 được kiểm trong Chrome tắt JavaScript. 410 dùng một hồ sơ giả lập trong PostgreSQL scratch theo fixture #331. Phối hợp xóa qua hai DB thuộc #297. |
| AC-04 · UI Web | PASS ở 320/390 px × sáng/tối × root text 100/200%. Các đích được đo đạt ít nhất 44×44 CSS px. Sau khi cuộn, nút tải thêm và Thử lại 503 đều nằm trên thanh dưới.. |

| Chiều | Kết quả self-review |
| --- | --- |
| Functional | PASS với phiên khách/chủ/viewer, quyền danh sách và số đếm, các kết quả resolver và phục hồi. |
| Geometry | PASS. Nút Follow giữ nguyên 218,75×90,59 CSS px khi giữ ACK ở 320 px/chữ 200%. |
| Visual | PASS theo LayoutSpec với nền phẳng Lavender R3, `NO_APPROVED_REFERENCE`. Đã mở ảnh. Ở 320 px/chữ 200%, tiêu đề danh sách dài đẩy hàng đầu xuống dưới màn đầu. |
| Accessibility/Responsive | PASS trong Chrome cho hành động, nhãn và focus đã đo. Tương phản văn bản ≥4,5:1. Vòng focus 5,78:1 sáng và 10,83:1 tối. Một số ảnh danh sách bắt đầu tại liên kết quay lại do runner cuộn đến đầu đích. |
| Interaction | PASS với Enter/Space, giữ node/focus khi gửi và thử lại, Back về link người và hướng, focus về H1 khi mất quyền hoặc hết hạn cursor. |

## Ảnh và số đo

| Cảnh | Trước | Sau |
| --- | --- | --- |
| Danh sách 1440×900, sáng, 100% | [PNG](before/follow-list-1440-light-100.png) | [PNG](after/follow-list-1440-light-100.png) |
| Hồ sơ 320×900, tối, 200% | [PNG](before/follow-profile-320-dark-200.png) | [PNG](after/follow-profile-320-dark-200.png) |
| Lỗi đọc hồ sơ 1280×900, sáng, 100% | [PNG](before/author-public-retry.png) | [PNG](after/author-public-retry.png) |

Link không có tổng ở 390×900 [sáng](after/follow-list-without-count-390-light-100.png) và [tối](after/follow-list-without-count-390-dark-100.png). Ca 503 ở 390×900 [sáng](after/profile-read-unavailable-390-light-100.png) và [tối](after/profile-read-unavailable-390-dark-100.png). Chrome tắt JavaScript ở [404](after/profile-missing-390-light-100.png) và [410](after/profile-gone-390-light-100.png).

[Log các phép đo và kiểm tra](checks.txt). `NOT_RUN` đối với trình đọc màn hình, zoom trình duyệt, BFCache hit thực, thiết bị thật và lane browser toàn epic.
