# Kiểm chứng Web Thông báo R4 (#486)

- Issue triển khai [#486](https://github.com/loveoverflowcom/vink/issues/486), cùng [LayoutSpec](../../../design/notifications.md), [bộ thiết kế R4](https://drive.google.com/drive/folders/1gLJYaMmg35WYpKJ0iXGp-p8JPpUs1Qhn) và [registry](../../notifications.md#11-web-r4-486).
- Mã kiểm chứng `cbcd65a6bcaf0ae4ee4e71b8c70e50736c907e8d`, rebase trên `develop@d040d017`.
- Ảnh trước lấy từ `478dcbf0` với bản sửa lỗi biên dịch đã vào `develop` và harness chụp ảnh. So sánh trên Khám phá với cùng fixture shell. Popover và hộp thư xuất hiện ở ảnh sau.
- Chrome 158 headless trên macOS, viewport cao 900 CSS px, chữ 100/150/200% mô phỏng bằng root font-size. Dữ liệu Thông báo là fixture hợp đồng phát qua CDP vào UI SSR/hydrate thật. Hai kịch bản hồi quy Reader/Bàn viết dùng App và PostgreSQL scratch.

## Lệnh

| Check | Kết quả | Ranh giới |
| --- | --- | --- |
| `cargo xtask verify web --scenario notifications` | PASS · `cbcd65a6` | Chrome, Leptos/Axum thật, fixture race bằng CDP |
| `cargo xtask verify web --scenario notifications --scenario endorsement --scenario review` | PASS · `b28ec3cc` | Chrome, PostgreSQL với hai kịch bản hồi quy Reader/Thấm và Bàn viết/kiểm duyệt |
| `python3 ./tools/local-budget run -- cargo test --locked -p vink-app --test postgres notification_gateway` | PASS 10/10 · `ba6f3e16` | App Gateway và Discussion trên PostgreSQL, inbox transport fixture |
| `python3 ./tools/local-budget run -- cargo xtask verify quick` | PASS · `cbcd65a6` | Workspace SSR/hydrate/preview lint, unit/contract/doctest, token, SSR Reader, oracle |
| `git diff --check`, `node --check`, đối chiếu link tương đối và TOML | PASS | Tĩnh |

## AC-01–05

| Tiêu chí | Bằng chứng và ranh giới |
| --- | --- |
| AC-01 | Bell dưới Bàn viết, account chỉ còn Tôi, popover neo theo chuông, đóng bằng Esc/nút/ngoài modal, focus qua breakpoint; mobile có ba đích, khách không thấy chuông. |
| AC-02 | Badge fresh/stale/503/99+, chỉ count khi đóng. Browser mô phỏng pause/resume và A→B→A kèm phản hồi đến muộn. Unit đo jitter và giới hạn backoff. PostgreSQL kiểm Gateway từ chối đọc/ghi lần cũ khi credential thay đổi. |
| AC-03 | Seen theo snapshot giữ nguyên unread. Read bắt đầu ngay dưới URL cũ, điều hướng tới Reader trước khi nhận ACK. Bulk đợi xác nhận và đọc lại, dữ liệu mới hiện banner, lỗi cursor giữ trang và thử lại. |
| AC-04 | Mở đúng quyết định theo revision/version, nút sửa/phản đối theo quyền, không hiện thân nhận xét đã gỡ. Gateway kiểm comment bị gỡ và anchor còn phản hồi công khai. Browser Reader kiểm danh sách Thấm ở dialog và trang. |
| AC-05 | 30 tổ hợp 320/390/834/1024/1440 × sáng/tối × 100/150/200%, 0 px tràn ngang, 186 phép đo vùng chạm với kích thước nhỏ nhất 48 × 48 CSS px. Kiểm Tab/Enter/Space/Escape, focus khi quay lại, reduced motion, HTML hộp thư no-store và không có dữ liệu riêng. |

## Ảnh và review giao diện

- [Shell trước và sau](shell-before-after.png)
- 30 ô của ma trận sau triển khai [mobile sáng](matrix-mobile-light.png), [mobile tối](matrix-mobile-dark.png), [rail sáng](matrix-rail-light.png), [rail tối](matrix-rail-dark.png)
- [Quyết định tác phẩm](notifications-work-decision.png) và [quyết định nhận xét](notifications-comment-decision.png)
- [Số đo JSON](notifications-geometry.json) và [tóm tắt log](checks.txt)

| Chiều | Kết quả | Phạm vi |
| --- | --- | --- |
| Functional | PASS | Lệnh và điều hướng trong kịch bản Chrome, kiểm quyền ở Gateway |
| Geometry | PASS | Neo đầu popover, tràn ngang, vùng chạm ở 30 tổ hợp |
| Visual | Đối chiếu LayoutSpec / `NO_APPROVED_REFERENCE` | Mở ảnh đối chiếu sáng/tối, thứ tự chuông/rail, nhịp nhóm dòng và chữ Việt ở 200% |
| Accessibility/Responsive | PASS phần đã đo | Chrome bàn phím, ổn định focus, vùng chạm và các breakpoint. Đọc màn hình, browser zoom, thiết bị thật và đo lại mọi cặp contrast trên màn runtime chưa chạy |
| Interaction | PASS | Mở/đóng/resize và Read trước khi điều hướng, tải tiếp và khôi phục focus, tắt chuyển động với reduced motion |

## Tự review

- Bell, popover và inbox dùng chung SessionOwner, OverlayLayer, NavUtility, NotificationBadge và NotificationRow. Thay đổi route đi qua NavigationGuard để giữ kiểm tra rời trang của editor. Lượt xác minh đầu phát hiện capture listener đóng popover trước khi dòng gửi Read. Bản sửa cho dòng bắt đầu Read trước, và kịch bản giữ ACK tới khi Reader đã mở.
- Màn quyết định dùng khoảng đệm từ token hiện có. Dòng gom nhóm dùng AvatarGroup và giữ initials khi ảnh thiếu. Kiểm lại inset và fallback initials trong browser.
- Polling chỉ tải count khi tab hiện và có mạng. Inbox giữ dữ liệu trong tab, ràng buộc từng request vào credential, kiểm generation trước khi nhận dữ liệu. Đọc tất cả tải lại số trang đã mở sau ACK. Đích nhận xét dùng kiểm quyền Discussion và tra đúng ID ở PostgreSQL.
- Ranh giới chưa chạy gồm Chrome với Identity và Notification Service sống cùng lúc, Firefox/Safari, screen reader, thiết bị thật và rollout production.
