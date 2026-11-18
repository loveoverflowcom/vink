# Thử containment của danh sách nhận xét Reader

**Giữ layout thường.** Corpus ngắn không đạt lợi ích đã định. Corpus 200.000 đoạn có lợi ích đo được khi danh sách ở rất xa viewport. CSS cũng làm mép trên danh sách dịch 19 CSS px trên cả hai corpus. Chưa có số liệu tần suất trong sử dụng thực tế để chọn B cho người đọc.

## LayoutSpec và ngưỡng thử

Chỉ `#root-list` (20 nhận xét gốc và 20 phản hồi của trang đầu) tham gia thử B. Article, heading, composer và pagination dùng layout thường. Cuộn hoặc fragment đưa danh sách vào vùng nhìn, focus bàn phím đi vào đúng nút. CSS thử `@media screen` dùng `content-visibility:auto` và `contain-intrinsic-block-size:auto <chiều cao A>px`. Đo chiều cao trong một lượt hiệu chuẩn bị loại khỏi mẫu. In dùng layout thường. Bỏ thẻ style phải khôi phục A và toàn văn.

Baseline #234 đo layout + style median 16,75 ms trên bài ngắn và 44,82 ms trên bài dài, qua ngưỡng ứng viên 8 ms. Ngưỡng chọn B chốt trước thử là tiết kiệm ≥4 ms, lớn hơn IQR A và giữ hành vi native. 10 cặp trên mỗi corpus, đảo thứ tự A/B giữa các cặp. Giữ response discussion cho tới khi article hiện đủ; bắt đầu đo sau hai animation frame, dừng khi hiện đủ trang đầu và qua hai frame. Thời gian bao gồm các tác vụ của renderer trong cửa sổ này.

| Corpus | A / B | Layout + style ms, median (min–max, p95 lab) | IQR ms | Script ms, median | DOM nodes, median | V8 used MiB, median | Discussion encoded B |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| 1 đoạn | A | 19.91 (16.82–40.33, p95 40.33) | 16.98 | 79.48 | 3,325 | 2.49 | 10829 |
| 1 đoạn | B | 24.75 (17.99–38.29, p95 38.29) | 9.60 | 87.53 | 3,328 | 2.53 | 10829 |
| 200.000 đoạn | A | 39.13 (35.86–84.33, p95 84.33) | 4.97 | 79.33 | 402,877 | 17.33 | 10829 |
| 200.000 đoạn | B | 20.05 (18.31–25.71, p95 25.71) | 4.86 | 77.31 | 402,799 | 17.33 | 10829 |

[Raw bài ngắn](small.json.gz) và [raw bài dài](long.json.gz) lưu từng cặp, body/transfer byte, script/task/layout/style, DOM/heap và digest toàn văn. Heap/V8 và DOM counters gồm cả tác động GC, chưa đo RSS hoặc RAM thiết bị. Dữ liệu lab Desktop Linux x86_64 1280×900 DPR 1, CPU 1×, loopback, Chrome 157.0.8103.78, Firefox 158.0.1, PostgreSQL 17.11. Rust 1.99.1 release/wasm-release của develop `9fcd75ec03229a27464123eac1d68104e8447b19`. Harness khi thu các cặp có SHA trong raw, checksum WASM phục vụ là `b6c88befa7d719e1438b4d9997a07ea922b11d2f538790c2abd74585c4c483f4`.

| AC | Kết quả |
| --- | --- |
| 01 | PASS. B đo riêng danh sách nhận xét, cùng work và cùng nội dung article trong mỗi cặp. |
| 02 | PASS trên desktop Linux đã chạy. Chrome và Firefox giữ toàn văn, native find, copy/paste thực qua hai vùng, fragment, Tab và PDF. Orca đọc được tiêu đề, thân bài, heading Nhận xét và nội dung nhận xét đầu tiên khi B hoạt động. |
| 03 | PASS. Chữ 100/200%, hai WebP #115 (chậm/ảnh hỏng, khung 256×171), paging 20→40 root, refresh về 20 và Hidden sau revalidation. Khi ảnh/refresh/paging, anchor article lệch ≤2 px. CDP tạo Hidden để kiểm UI; quyền thu hồi thật do lane review kiểm riêng. |
| 04 | PASS. Hai khối A/B dùng cùng release/corpus/seed. Hình học, render, bytes, script và proxy heap có raw riêng. |
| 05 | PASS. Gỡ style phục hồi `content-visibility:visible` và số đoạn. CSS thử chỉ tồn tại trong trang kiểm thử. |

| Chiều UX | Kết quả đã xem hoặc thao tác |
| --- | --- |
| Functional | Toàn văn khớp SHA-256. Paging và refresh làm việc trên đúng trang hữu hạn. |
| Geometry | Mép danh sách ở 987,41 px với A và 968,41 px với B trên bài ngắn. Ở bài dài cũng chênh 19 px. Article giữ vị trí khi cập nhật vùng phụ. |
| Visual | Đã mở [A ngắn](small-A.png), [B ngắn](small-B.png), [A dài](long-A.png), [B dài](long-B.png). Trong khung đầu, title, tác giả, tag và chữ dùng cùng trục. Mép danh sách nằm dưới khung ảnh. |
| Accessibility/Responsive | [Ảnh chữ 200%](small-text200.png) và assertion overflow. Orca 50.3 + AT-SPI 2.60.4, Chrome headed qua Xvfb, bàn phím thật qua XTest. Có [speech output và kiểm nội dung PDF](native.json) cùng [kết quả Firefox](firefox.json). Chưa đo ở Safari/VoiceOver, điện thoại vật lý hoặc người nghe trực tiếp. |
| Interaction | Tìm bản nhận xét, chọn/copy từ cuối article vào nhận xét, fragment/Tab, in và tắt B đều đã chạy. Khi so clipboard, chuẩn hóa whitespace của hai engine và kiểm đủ văn bản. |

## Chạy lại

Từ gốc repo, chạy App guest trên hai DB scratch theo [hướng dẫn #234](https://github.com/loveoverflowcom/vink/pull/309). Với mỗi origin chạy

```sh
node apps/vink-web/e2e/browser-smoke.mjs --phase reader-containment --build-profile release --base "$SCRATCH_ORIGIN" --chrome "$VINK_CHROME" --state "$ARTIFACTS/state.json"
```

Trên corpus ngắn, `pdftotext print.pdf -` phải có thân bài, “Nhận xét 20” và “Phản hồi 820”. Bản in toàn bộ 200.000 đoạn chưa chạy. Để đối chiếu Firefox, mở cùng Reader và chèn rule B với chiều cao 9.371 px, kiểm find, selection từ đầu thân bài đến cuối nhận xét 1, Ctrl+C rồi Ctrl+V vào ô nhập, fragment đến comment, Tab vào list, in và gỡ rule. Để lặp Orca, bật trợ năng hệ thống, khởi động `orca --debug-file orca.log` và Chrome với `--force-renderer-accessibility`. Focus `#reader-return`, nhấn Ctrl+Home, duyệt heading, Down và paragraph qua article tới nhận xét. Đối chiếu bốn utterance trong native.json với `SPEECH OUTPUT`. Kiểm in/screen reader trên corpus ngắn, benchmark hai corpus đều có đủ article trong DOM.

Phép thử Orca diễn ra trên cùng UI Reader với bundle lab có probe #234. Các cặp A/B tính thời gian và Firefox dùng bản release trên stable-site.

[Digest SQL seed, source canonical và cấu hình artifact](fixtures.json).
