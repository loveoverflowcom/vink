# Search R3 — Khám phá và Bàn viết

Triển khai có phạm vi cho [#90](https://github.com/loveoverflowcom/vink/issues/90),
base `develop@f7bd0c4`. Kế thừa [LayoutSpec R3 ngày 07/11/2026](https://drive.google.com/file/d/1N_9HjDK8A78VW9CT74McKFHTQkQbbaNs/view),
hai comparison board sáng/tối và state board cùng thư mục. Đây là lựa chọn của agent
trong phạm vi được giao, chưa có usability test hoặc quyết định user-approved.
R1/R2, logo, shell và điều hướng giữ owner hiện hành.

## DecisionMap

| Quyết định | Chọn / lý do | Đánh đổi và hướng loại |
| --- | --- | --- |
| CHOICE-01 | A: search liền khối trước H1 Khám phá; scope ngay dưới. Một kính lúp submit, clear ở cuối. Bàn viết dùng cùng anatomy ở công cụ danh sách, sau Viết tiếp. | Tăng độ nổi và chiều cao đầu trang; đo first-content tại 320/390. B chiếm thêm ngang cho chữ “Tìm” ở text200. C ít nổi, thêm reflow cạnh heading mà danh sách hiện chỉ có 12/100 mục. Không ship switch A/B/C. |
| CHOICE-02 | A: mở rộng `SearchField` bằng biến thể `SearchVariant::Bar`, optional native name/clear link và callback. Dùng lại `text_control` và hai consumer thật. | Giữ biến thể mặc định và preview cũ; API/visual qualification Leptos riêng. B thêm composition quanh TextField cần slot/focus shell nữa dù native control hiện đã có owner dùng được. Không thêm router, DTO hay fetch vào primitive. |

## LayoutSpec scoped

```text
#shell (owner cũ)
  #main
    Khám phá #explore
      #discovery-tools / #public-search (native GET /, role=search)
        SearchField Bar
          #search-shell (tonal, min-height56, một surface)
            #submit (kính lúp, type=submit, target>=48)
            #query [real label + native input name=q]
            #clear (a href=/ thật, target>=48, chỉ khi có chữ)
          #public-query-help (visible scope, aria-describedby)
      #explore-heading
      #community-stream (heading/count/rows/end) + #context
    Bàn viết #desk
      #heading (H1 + create)
      #resume (Viết tiếp, owner cũ)
      #drafts
        #draft-list-title
        #draft-tools
          form role=search -> SearchField Bar (không name/URL persistence)
          #draft-sort + #refresh-drafts
        #draft-scope + result count + rows / no-results
```

Một instance input, không remount khi theme/resize; nhãn intrinsic hai hàng khi focus/filled,
empty blur đặt nhãn trong shell. Không fixed height, không cắt dấu tiếng Việt. Query min-width0;
hai nút có chỗ riêng, helper wrap. Search/heading/list dùng trục lề hiện hành; thanh tối đa48rem.
Ở Bàn viết, search full-width trên sort/reload để scope không làm lệch trục controls.

Màu từ `--vk-field-container`, `--vk-on-surface`, `--vk-on-surface-variant`, `--vk-focus-ring`;
không sửa token source/generated output. Input focus vẽ một perimeter inset trên shell;
clear/submit chỉ vẽ perimeter của chính target. Forced-colors dùng CanvasText/Highlight;
không animation nhãn, tôn trọng motion contract hiện hành.

## Hành vi và impact map

- Khám phá: buffer đang gõ khác q đã submit. Form/URL/reader return vẫn do page sở hữu;
  GET chỉ có `q`, no-JS clear là `/` thật, hydrated clear xóa buffer/q và trả focus.
- Bàn viết: chỉ tiêu đề trong tối đa100 nháp đã tải, live filter/sort; không query URL/history.
  Giữ create/reconcile/resume. Refresh tạm lỗi giữ last-success và query, vẫn báo lỗi thật;
  lỗi authority không giữ dữ liệu riêng bị từ chối.
- Scope/count, empty/no-result/unavailable/loading vẫn tách biệt. Không thêm backend search,
  history/trending/voice/filter; Khám phá chỉ title/author của tối đa12 public cards.
- Leptos `SearchField` cũ và preview cần regression; renderer CMP không đổi API/hình học,
  biến thể Bar là Web-only, CMP/native NOT_RUN. Không suy parity từ build Web.
- Bằng chứng runtime, lệnh, ảnh và các gap được ghi trong handoff phát triển của cùng PR;
  prototype PNG không được dùng để tick nghiệm thu ứng dụng.

[Bằng chứng local và phần nghiệm thu còn mở](../development/search-r3-evidence.md).
