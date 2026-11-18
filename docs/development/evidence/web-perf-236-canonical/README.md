# Khôi phục URL anchor sau khi danh sách đã vẽ

Base `9fcd75ec03229a27464123eac1d68104e8447b19`. Chrome 157.0.8103.78/Linux x86_64, DPR 1, loopback, Rust 1.99.1 release/wasm-release, PostgreSQL 17.11. Khách, fixture #100 + #234 gồm 200 work/1.000 comments. Dung sai giữ ở 2 CSS px theo [LayoutSpec](../../../design/discovery-return-position-236.md).

Tại 390×844 chữ 200%, callback cũ focus đúng nhưng mép hàng lệch **218,359375 px** trong lane chính. [Raw trước sửa](before-paint.json.gz). Một lượt [ghi thứ tự scroll và kích thước hàng](scroll-diagnostic.json.gz) bắt được scroll khi các hàng phía trước còn dùng kích thước của frame đầu. Ở frame tiếp theo chiều cao đổi. Bản sửa `2493f8b` chờ lần vẽ kế, kiểm lại scope, DOM và focus rồi mới cuộn. Cleanup hủy frame còn chờ khi effect hết hiệu lực.

| AC | Kết quả |
| --- | --- |
| 01 | PASS. URL và fragment gắn với đúng q, tags, cursor. Giữ Enter, modified/new-tab, focus người đọc và reduced motion. Fallback dùng landmark hiện hành “Tác phẩm khám phá” của #258. |
| 02 | PASS. Back/Forward và link về danh sách khôi phục đúng trang và hàng. Ở 390×844 chữ 200% và ở desktop, sai lệch 0,015625 px trong lane chính. Khi thiếu hàng hoặc lỗi, focus về landmark. |
| 03 | PASS. CDP giữ font thật và ảnh có kích thước, A→B→A rồi mới thả B, offline warm, 18 key khiến snapshot bị evict rồi offline, thiếu hàng và 503. Late refresh để nguyên focus và chữ đang nhập. Withdrawal purge thật do lane `review` tại `9fcd75e` kiểm qua App/PostgreSQL. |
| 04 | PASS. 50/50 cùng document, mỗi lần có một list và không giữ Reader rời trang. Ở vòng 5/10/20/30/40/50 trên lane chính đều 917 DOM node và 79 JS event listener sau GC. Owner READ-02 có 3 observer chung key, 1 resource, 1 subscription, 1 timer và 0 flight/waiter. |
| 05 | PASS. Đã mở ảnh cùng cảnh. Bỏ riêng thay đổi thời điểm vẽ tái hiện drift; đảo bản sửa route đã merge `c90719f` trên base, build WASM lại và chạy cùng harness tái hiện URL Khám phá với Reader vẫn mounted. Giữ URL anchor, chưa thử offset hoặc keep-alive. |

| Chiều UX | Kết quả |
| --- | --- |
| Functional | 13 ca trên App/PostgreSQL. Native new-tab/modified click, Enter, Back, Forward và return link đúng scope. |
| Geometry | [Trước](before-text200.png) target nằm thấp trong viewport dù focus đúng. [Sau](after-text200.png) tiêu đề hàng 19 gần mép trên theo scroll margin 32 px. Không tràn ngang. |
| Visual | Đã mở ảnh 390×844/chữ 200% và [desktop](after-desktop.png). Hàng tiêu đề có dấu tiếng Việt, link ở trên bottom navigation và vòng focus desktop hiện đầy đủ. |
| Accessibility/Responsive | Keyboard focus vào link đúng hàng hoặc landmark có tên; resize, reduced motion, text200 là root-font override. Screen reader và đo lại contrast riêng cho thay đổi thời điểm scroll chưa chạy. |
| Interaction | Late font/ảnh, tín hiệu tới muộn sau điều hướng và buffer input đang gõ đều được kiểm. [Rollback đỏ](rollback-red.png) và [trace rollback](rollback-red.json.gz). |

## Tái lập

Bộ đếm trực tiếp cần probe opt-in từ [PR #309](https://github.com/loveoverflowcom/vink/pull/309). Trong checkout lab riêng, ghép đúng source của hai nhánh để tái hiện lane chính. Lần đo dùng commit tích hợp local `1cd5f58e4ac990cd6bfd470502616bed20163603`, tree `ab62df5ced733712d88f6034a8965bde04b06d29`. Hai commit đầu vào đều có trên các nhánh PR.

```sh
git switch --detach acfe00369ccf016a95d1de6cbc3faf6a3f1f7c9c
git merge --no-edit 2493f8b2de9a814a374ee3920f629f332b53af4f
cargo xtask verify web --scenario return-position --release
```

[Raw lane chính](after-paint.json.gz). [Lần 1](repeat-1.json.gz), [lần 2](repeat-2.json.gz), [lần 3](repeat-3.json.gz) đều đạt 50/50 trên bản sao corpus có hai tag dùng trong filter. [Lần siết assertion cuối](final-assertions.json.gz) dùng cùng WASM (SHA-256 `14be942e6a9f96828d29c0ae387a944ad324c07752fbe3b24b562314c8593f96`), harness tại `2e467a3`. Bundle thiếu probe ghi `NOT_RUN` cho bộ đếm và `MEASURED_PARTIAL` cho coverage.

Đối chứng paint giữ cùng #234 probe và cùng harness, thay production restore bằng bản trước sửa. Đối chứng rollback route dùng một checkout riêng tại base, `git revert --no-commit c90719f`, build bundle release, rồi phục vụ bundle đó trên corpus #234 và chạy phase `return-position` từ nhánh harness. Trường `source_sha` trong raw của lượt này là SHA của harness; WASM đến từ bản revert. [Trace 50 vòng đã merge](../web-perf-234-236/README.md) là mốc trước đó.
