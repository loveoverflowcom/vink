# Bằng chứng Bàn viết Markdown và định dạng nháp đầu tiên

Base `develop@7be90f9df8c13247002bfa4a0c0f93444499f26b`.
SHA kiểm chứng Web `2b17a7613ecd057c87ce175d86739a5f6ab72afb`.
Môi trường macOS 15.6 arm64, Chrome 157.0.8103.79 headless/CDP, Node 22.15.0,
Rust 1.99.1, PostgreSQL 17.11 trên cluster scratch riêng. App chạy profile dev.

## Phạm vi và kết quả

**Sản phẩm** `PRODUCT_CONFIRMED` cho Mặc định, Markdown, H1 đầu tiên và mục lục Cách 1.
**Kiến trúc** `VDR-DOCUMENT-SOURCE-FORMAT@r2 ACCEPTED`, snapshot registry
`43865e626ee7ce88e8f453140666e10d3b5f3ecf5fe6c52d7a44b6887300fb22`.
**Triển khai** `web` qua API nháp và snapshot hiện hữu. [Signoff r2](../../../../decisions/signoffs/VDR-DOCUMENT-SOURCE-FORMAT.r2.toml) · [LayoutSpec](../../../design/editor-live-preview-256-257.md).

| Issue | Bằng chứng tại SHA đã kiểm | Delta checklist đề nghị |
| --- | --- | --- |
| [#256](https://github.com/loveoverflowcom/vink/issues/256) | Base tái hiện `RangeError` khi mở link có URL xuống dòng. Web PASS khi mở lại, dán qua handler paste, đặt caret trong/ngoài link thường và link nhiều dòng, giữ selection ngược, commit tiếng Việt qua CDP, undo hai lượt và đọc lại đúng nguồn qua API. | Ghi thêm bằng chứng cho AC-01/02/04/06. IME hệ điều hành còn cần kiểm. |
| [#262](https://github.com/loveoverflowcom/vink/issues/262) | Hai principal scratch có 0 tác phẩm tạo và reload lần lượt Mặc định/Markdown. Tab/type-ahead và CTA đạt ở 320 px/chữ 200%, sáng/tối. Chuyển đổi có preview, Hủy giữ editor, Áp dụng qua PUT thật, đối soát mất ACK bằng GET và giữ buffer khi version xung đột. | Ghi thêm bằng chứng cho AC-01/02/03/04/05/06. Menu native, touch và ma trận đầy đủ của Select ở luồng đầu tiên còn cần kiểm. |
| [#263](https://github.com/loveoverflowcom/vink/issues/263) | H1 có 201 ký tự bị từ chối, lỗi gắn vùng Nội dung, **Sửa tiêu đề** đưa caret tới H1 và lưu bản đã sửa trên cùng buffer. H1 đầu rỗng, thiếu H1 và tên Mặc định rỗng đều lưu được dưới dạng nháp. Cả ba bị `422 title` khi chụp snapshot. Tiêu đề Mặc định không hợp lệ hiện lỗi ở đúng ô rồi lưu lại bằng tên riêng. | Đề nghị nghiệm thu AC-05. Phần Reader/metadata của AC-04 thuộc PR kế tiếp. |
| [#257](https://github.com/loveoverflowcom/vink/issues/257) | PASS mục trùng, offset UTF-16 sau emoji/NFD, đích gần cuối nguồn dài hơn 195k, từ chối click snapshot cũ khi nguồn A→B→A trong cùng view, cập nhật sau undo, conflict/reload và focus sheet. Ma trận mục lục 5 bề rộng × 2 theme × chữ 100/200%, kiểm H1 Literata, H2 bán đậm và mục hiện tại Lavender. | Ghi thêm bằng chứng cho AC-01/02/03/04/05/07. Safe-area và bàn phím ảo trên thiết bị thật cùng reflow H5/H6 còn cần kiểm. |

## p95 trên Chrome

Mỗi corpus có 5 lượt làm nóng và 40 mẫu cho mỗi phép đo. Parse dựng nguyên cây CommonMark
từ nguồn. Duyệt dùng cây đã parse và hàm chiếu mục lục production. Gõ và caret đo thời gian đồng bộ
của transaction trên CodeMirror runtime mà App phục vụ. Phép đo dùng một view fixture trên trang
Bàn viết. Độ trễ tới lúc paint, Leptos DOM, request lưu và thiết bị thật nằm ngoài ranh giới đo.

| Ký tự Unicode | Parse (ms) | Duyệt mục lục (ms) | Gõ (ms) | Caret (ms) |
| ---: | ---: | ---: | ---: | ---: |
| 10.000 | 0,7 | 0,2 | 0,3 | 0,5 |
| 50.000 | 2,9 | 0,5 | 0,4 | 0,5 |
| 100.000 | 5,5 | 0,9 | 0,6 | 0,5 |
| 200.000 | 11,1 | 1,6 | 0,9 | 0,5 |

[Mẫu đo gốc](editor-markdown-p95.json), SHA-256 `e2c5f8024f6a79d7c74cfde8f45817b9bbea04faf1c96aa9ae130107cf6ea7f4`.
[Code đo](../../../../apps/vink-web/e2e/editor-markdown-budget.mjs) · [Regression Web](../../../../apps/vink-web/e2e/editor-desk-regressions.mjs).

## Review giao diện

Self-review theo `NO_APPROVED_REFERENCE`. Ảnh và manifest của lượt Web nằm ở
`target/verify-web/1795013684604278000-63646/screenshots/`.

| Chiều | Kết quả |
| --- | --- |
| Functional | PASS trên App/PostgreSQL scratch cho tạo nháp, lưu, phiên bản xung đột và từ chối snapshot thiếu tên. |
| Geometry | PASS Select 16/16/52 CSS px và CTA ở 320 px/chữ 200%. Mục lục cột phải và sheet đạt ma trận Chrome hiện hữu. |
| Visual | Đã mở ảnh Select/CTA sáng và tối ở 320 px/chữ 200%, lỗi H1 ở desktop, mục lục 1440 px/chữ 100% và sheet 320 px/chữ 200%. Các control chính và H1/H2 đọc được ở những ca này. |
| Accessibility/Responsive | PASS Chrome cho nhãn Select, Tab, focus ring 2 px, liên kết aria-describedby của lỗi H1, Escape và trả focus sheet. Screen reader, menu hệ điều hành, safe-area và bàn phím ảo thiết bị thật `NOT_RUN`. |
| Interaction | PASS paste, undo, selection, CDP composition/commit và lệnh mục lục stale. IME Telex/VNI hệ điều hành và touch thiết bị thật `NOT_RUN`. |

## Lệnh kiểm

- `node scripts/test-markdown.mjs` trong `apps/vink-web/editor` — PASS 30 ca corpus, mục lục và nguồn 200k.
- `CARGO_INCREMENTAL=0 CARGO_PROFILE_TEST_DEBUG=0 ./tools/local-budget run -- cargo xtask verify quick` — PASS toàn bộ trên SHA kiểm chứng.
- `CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2 cargo xtask verify web --scenario main` với Chrome và PostgreSQL scratch — PASS 5.957 assertions, gồm hai ca first-use và main, sạch lỗi console/exception.
