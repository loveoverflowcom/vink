# Biên bản phản biện R4.3 · lượt đối soát #414

**Self-review theo source · `develop@2712145b08a6a4c7a12c1d57033ecd2a5bd33b26`.**
Tên file giữ theo handoff #414. [Comment của lượt trước](https://github.com/loveoverflowcom/vink/issues/414#issuecomment-7102229465)
báo một reviewer độc lập ở `2c4f5fb`. Commit và biên bản đó chưa truy được. Các finding dưới đây
được kiểm lại bằng source và issue, với ID riêng cho lượt này. Review độc lập A3 của delta mới
đang chờ. Canvas/Drive/Stitch `NOT_INSPECTED`.

| ID | Loại / mức / độ tin cậy | Quan sát và tác động | Disposition và phép kiểm |
| --- | --- | --- | --- |
| R43-S01 | DEFECT docs / P2 / cao | [Audit §5/§7](../redesign-r4-ban-viet-kham-pha.md) trộn hàng phẳng, SplitButton và topology mới. Owner có thể áp sai trước khi xem lời chốt. | **fixed**. [Spec §2 và §5](layout-spec.md) dùng thẻ rời, H1 Be Vietnam Pro và Select. Audit giữ nhãn lịch sử, design index trỏ một spec. |
| R43-S02 | DEFECT docs / P2 / cao | [Shell](../../../apps/vink-web/src/shell.rs#L228) còn Giao diện và [appearance](../../../apps/vink-web/src/appearance.rs#L153) còn 200%. Lời chốt đích #413 dễ bị đọc thành hiện trạng. | **fixed** trong [spec §3](layout-spec.md#3-shell-rail-và-thanh-dưới). #413 sở hữu runtime và test bootstrap/cả rail, #427 sở hữu reflow/AX, #428 giữ câu hỏi editor. |
| R43-S03 | ACCEPTANCE GAP / P2 / cao | [Desk đóng dock](../../../apps/vink-web/src/pages/desk.rs#L162) giữ q, [sort](../../../apps/vink-web/src/pages/desk.rs#L670) ẩn khi mở và [recovery](../../../apps/vink-web/src/pages/desk.rs#L311) có nhiều đích focus khác comment #408. | **deferred** tới owner Desk, [delta #408](https://github.com/loveoverflowcom/vink/issues/408#issuecomment-7102205637). Thử query đã gửi rồi đóng bằng nút/Esc/Back, thử lại có và không dock, kiểm `Invalid`/`NotFound` và không remount ô. |
| R43-S04 | ACCEPTANCE GAP / P2 / cao | [Desk first use](../../../apps/vink-web/src/pages/desk.rs#L454) còn search launcher và [tools](../../../apps/vink-web/src/pages/desk.rs#L670); #411 giữ DQ-04 mở. Tự ẩn có thể bỏ đường tìm/phục hồi. | **deferred** DQ-04 / #411. [Spec §5.2](layout-spec.md#52-khách-lần-đầu-đang-tải-và-lỗi) giữ EmptyState/Select đã có và mô tả điều kiện proposal. Kiểm kho rỗng, query miss, lỗi và node/focus. |
| R43-S05 | ACCEPTANCE GAP / P2 / cao | [ExploreModes](../../../apps/vink-web/src/pages/discovery.rs#L326) có hai bộ link và CSS đổi theo breakpoint. Đọc DOM chưa chứng minh bàn phím, AX tree và các ngưỡng ở 150%. | **deferred** #427. Cần Tab/AX, URL q khi SSR, đóng/Esc, cảnh active rail và cỡ chữ 100/150/stress 200 riêng. |
| R43-S06 | SUGGESTION / P3 / cao | [#407](https://github.com/loveoverflowcom/vink/issues/407) là hợp đồng trích đoạn domain; quyền canvas và ma trận rộng không kiểm thuộc tính dấu kết câu. | **fixed** ở handoff [F-05](review-414.md#2-disposition-f-01f-06). Dùng unit excerpt, kiểm ETag/caller nếu bị tác động. Body issue cũ vẫn là lịch sử riêng. |
| R43-S07 | ACCEPTANCE GAP / P2 / cao | [Comment R4.3](https://github.com/loveoverflowcom/vink/issues/414#issuecomment-7102229465) nêu Canvas và biên bản cũ nhưng không cung cấp bản mở được của `2c4f5fb`. Thiếu revision nên chưa đối chiếu pixel/finding cũ. | **deferred** đến khi có commit hoặc Markdown gốc và đúng asset revision. Làm xong đối soát độc lập với nguồn hiện có. Không gán trạng thái cho từng finding cũ. |
| R43-S08 | SUGGESTION / P3 / cao | [#429](https://github.com/loveoverflowcom/vink/pull/429) giữ ngữ cảnh bản mới/bản công khai, 0 và “—”. Nhãn kiểu một từ một cách cứng nhắc làm mờ audience và moderation. | **fixed** ở [spec §5.3](layout-spec.md#53-thẻ-tác-phẩm). Mỗi sự thật một nhãn; giữ chữ cần cho ngữ cảnh và tên chỉ số. Tái kiểm khi đổi recipe hoặc màu DQ-10. |

## Ranh giới bằng chứng

| Chiều | Lượt này |
| --- | --- |
| Functional | Trace source và đối chiếu body PR đã merge. Chạy lại browser `NOT_RUN`. |
| Geometry | Chỉ nêu mục tiêu token/breakpoint có nguồn. Computed style ở SHA này `NOT_RUN`. |
| Visual | `NO_APPROVED_REFERENCE`. Ảnh của audit và PR lịch sử giữ nguyên provenance. Mở/render ảnh mới `NOT_RUN`. |
| Accessibility–Responsive | Có tên/role/focus để review trong source. Tab/AX, forced colors, screen reader, touch và reflow matrix ở SHA này `NOT_RUN`. |
| Interaction | Có trace focus/cursor/IME guards. Browser tương tác và IME hệ điều hành `NOT_RUN`. |
