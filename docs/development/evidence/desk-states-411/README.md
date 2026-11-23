# Kiểm chứng trạng thái Bàn viết #411

Source `b4fe62fa` cho App SSR và WASM hydrate, cộng stylesheet và browser smoke tại
`3356c271`. Base `develop@4b2fb157`. Chrome 157 headless trên macOS arm64, App profile
`local` và PostgreSQL scratch. Cỡ 100/150/200 là root font-size đặt từ CDP, trong đó
200 là stress reflow.

| Chiều | Kết quả trên route `/ban-viet` |
| --- | --- |
| Functional | PASS `desk-guest` với App tắt đăng nhập, không request danh sách riêng. PASS `desk-empty` với phản hồi danh sách được giữ, kho rỗng, lọc không khớp, lỗi 503/502, retry và đổi tài khoản. |
| Geometry | PASS các ma trận khách và kho rỗng ở 320/390/834/1440, sáng/tối, 100/150/stress 200. Retry nằm trong vùng danh sách và cuộn tới được ở 320/200. |
| Visual | Self-review ảnh khách, lần đầu, skeleton và lỗi ở dưới. |
| Accessibility/Responsive | PASS tên truy cập của liên kết tài khoản theo trạng thái khách, tương phản chữ ở vùng kho rỗng, vùng chạm và reflow trong các ca Chrome trên. Screen reader thật chưa chạy. |
| Interaction | PASS focus rail khách, giữ node/caret ô tìm qua loading và retry, composition qua CDP, dữ liệu cũ giữ sau lỗi đọc với `author-works`. IME hệ điều hành chưa chạy. |

Smoke tập trung dùng `node apps/vink-web/e2e/browser-smoke.mjs --phase desk-guest|desk-empty|author-works`
với App scratch, Chrome và ba output state riêng. App và bundle dựng từ source của PR. CSS và
smoke cập nhật cuối được chạy lại trong `desk-empty`; CSS tại vùng khách không đổi sau
`desk-guest`. Phần đăng nhập đang mở và toàn bộ lane Web `main` chưa chạy trong lượt này.

Cặp khách dùng App tắt đăng nhập và cùng kích thước, theme, cỡ chữ. Mốc trước là
`b062860d` của #410, khi thay đổi chỉ tác động hàng tác phẩm. Lần đầu, loading và lỗi
có ảnh sau trong lượt này.

- Khách, 390 × 844, tối, 100%. [Trước](truoc-khach-390-toi-100.png) · [Sau](sau-khach-390-toi-100.png).
- [Khách, 390 × 844, tối, 150%](sau-khach-390-toi-150.png)
- [Lần đầu, 390 × 900, tối, 150%](sau-lan-dau-390-toi-150.png)
- [Skeleton khi giữ phản hồi, 390 × 900, sáng, 100%](sau-dang-tai-390-sang-100.png)
- [Lỗi với kết quả rỗng được giữ, 1440 × 900, sáng, 100%](sau-loi-1440-sang-100.png)
- [Đã cuộn tới nút Thử lại, 320 × 900, tối, stress 200%](sau-thu-lai-320-toi-200.png)

### Kiểm bổ sung khi bật đăng nhập

Ca Chrome `desk-guest-open` dùng App SSR trên `03730853`, bundle hydrate của
`b4fe62fa`, CSS tại `3356c271`, Identity native và PostgreSQL thử nghiệm. PASS cả
24 tổ hợp kích thước, theme và cỡ chữ. Trang khách gửi 0 request danh sách riêng,
có một nút chính “Đăng nhập” tới `/toi?next=/ban-viet` và liên kết “Khám phá tác phẩm”.
