# Bàn viết và mục lục trên bản thảo đã lưu

**NOT_REPRODUCED** cho màn trống kéo dài ở `/ban-viet` và `/ban-viet/:id`. Lượt ngày 26/11/2026 dùng product SSR/hydrate tại `a9f4280ad78331ac3b25e23cedfe027f7e8ec162`, macOS 15.6, Chrome 158.0.8127.89, native Identity và PostgreSQL 17 riêng. App chạy bằng `cargo xtask dev web --auth identity --migrate` trên `127.0.0.1:57454`. Tài khoản cùng các bản nháp đều tạo qua API product trong phiên Chrome.

## Các ảnh trước đó

Đã mở năm PNG của run `7269df7d`. Desk lần đầu có minh họa và CTA, còn Desk populated có hai tác phẩm. Editor Mặc định hiển thị tiêu đề và hai đoạn văn ở cả hai kích thước. [c189](https://drive.google.com/file/d/1As78-cyGytIiKPLT_sLulGXWu413glx5/view) · [c195](https://drive.google.com/file/d/1o8oeKljqzArVS546mW6-7y2yG8We00ea/view) · [c209](https://drive.google.com/file/d/1beUxIRlv_2NkPu8e8BKIgA8BFuT4qXU-/view) · [c213](https://drive.google.com/file/d/1wiVi8TGrSnom72qL6ONeTSxUj3IbONSD/view)

c327 hiển thị “Bài viết mới”, định dạng Markdown và “Chưa có đề mục”. Báo cáo capture cũ ghi `editor.outline-headings` là `BLOCKED / NOT_RUN`. Tên ảnh chính xác mà người dùng gặp và hành vi của Drive viewer trong phiên đó vẫn chưa xác định. [c327](https://drive.google.com/file/d/1P0SWHAqjG5EodMgxVtItsNky8l8Uf__Z/view) · [Báo cáo capture gốc](https://drive.google.com/file/d/1WfmLkrkkWEeVCdSUpYgWuK5dhfTtjgPE/view)

## Quan sát trong phiên tác giả

| Mốc từ đầu probe | Thao tác và kết quả |
| --- | --- |
| 894–1.922 ms | Khách thấy guard. Đăng ký, đặt bút danh và đọc lại session `active`. Desk của tác giả mới hiện first-use. |
| 2.864–3.685 ms | Bản Markdown mới có một EditorView và mục lục rỗng. Lưu nội dung qua API rồi mở Desk thấy hai bản. |
| 4.466–5.279 ms | Cùng source, Mặc định giữ nguyên dấu thăng, Markdown hiện H1/H2/H3/H2 tại offset UTF-16 0, 35, 83, 98. Dòng có dấu thăng trong code fence chỉ nằm trong thân bài. |
| 5.869 ms | Chọn H2 trùng tên thứ hai trên sheet di động đưa caret đến cuối dòng tương ứng và đóng sheet. |
| 6.729–7.634 ms | CDP giữ request module Editor, rồi chủ động làm request lỗi. Body và outline lần lượt báo đang tải và lỗi. Tải lại trang phục hồi source đã lưu trong một EditorView. |
| 8.665 ms | Mở nháp dài sau khi giữ module. DOM lần lượt báo đang đọc (0 mục), đang cập nhật phần còn lại (4 mục), rồi đủ 5 mục. |

Các mốc là thời gian quan sát từ đầu chuỗi thao tác. Console error và exception bằng 0. Endpoint đếm thông báo trả 503 tám lần trong host scratch không khởi động Notifications. Các GET/PUT nháp và GET danh sách thành công. [Timeline đã che ID](trace.json)

Đã mở 10 PNG của phiên Identity, gồm first-use, Desk hai bản, Markdown rỗng, Mặc định và sáu biến thể mục lục đã parse. Ma trận mục lục gồm desktop 1440×900 sáng/tối, mobile 390×844 sáng/tối, 390×844 chữ 150% và 320×800 chữ 200%. Theme và cỡ chữ được đặt trong DOM. [Ảnh desktop](markdown-1440x900-100-light.png) · [Ảnh mobile](markdown-390x844-100-dark.png)

## Tái kiểm

Với `VINK_CHROME` và `VINK_TEST_DATABASE_URL` trỏ tới môi trường scratch, chạy `cargo xtask verify web --scenario desk --scenario editor-format`. Desk giữ response đầu để kiểm skeleton sau loading gate, first-use sau head thành công, retry lỗi và phản hồi cũ khi đổi phiên. Đây là các ca transport mô phỏng có chủ đích.

Trong `editor-format`, `outlineOffsets` ghi cùng source qua API ở hai định dạng, kiểm bốn heading thật cùng offset sau khi parse đủ, thử lệnh thuộc generation cũ, nhảy tới H2 trùng, rồi lưu và mở lại sau conflict. Capture `markdown-outline-reopened` được ghi sau khi source khôi phục chính xác và AST hoàn tất. Runner dùng principal fixture loopback cho ca này. Lane native Identity ở trên kiểm thêm phiên có cookie thật.

Phạm vi PR2 là SSR/hydrate. Corpus đa trạng thái và capture catalog tích hợp thuộc [#556](https://github.com/loveoverflowcom/vink/issues/556). [Issue #553](https://github.com/loveoverflowcom/vink/issues/553)
