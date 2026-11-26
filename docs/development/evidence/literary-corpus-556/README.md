# Corpus văn học và nghiệm thu Web Quality · #556

Phạm vi là PR 4/4 của [Epic #554](https://github.com/loveoverflowcom/vink/issues/554).
Corpus Product đặt tại [fixture E2E](../../../../apps/vink-web/e2e/fixtures/literary-review.mjs).
Corpus Preview dùng dữ liệu riêng trong [host Leptos](../../../../apps/vink-preview/leptos/src/screens.rs).
Hai host cùng kiểm khả năng trình bày chữ Việt, nháp và trạng thái Desk. Product lưu qua App và
đọc lại từ PostgreSQL scratch. Preview render trạng thái mẫu và textarea controlled qua UI System.
Đây là phương án B cho CHOICE-01. CHOICE-02 dùng phương án A, mở rộng assertion của CDP trước
khi gọi capture.

## Corpus và điều kiện chụp

| Ca | Dữ liệu và điều kiện |
| --- | --- |
| Desk lần đầu | Tác giả vừa đăng ký qua Identity. Đọc author works thành công với 0 hàng. CTA first-use bật sau khi Desk đọc xong. |
| Desk có tác phẩm | Ba nháp riêng, một bản chờ duyệt của ca `submissionStructure` và một tác phẩm đã duyệt trong `publication`. Kiểm ID, phiên bản, title, state từ API và từng hàng tương ứng trong DOM. |
| Văn xuôi | “Tiếng chèo qua xóm dưới”, sáu đoạn có dấu tiếng Việt. Mở từ đúng hàng, tải lại và đối chiếu toàn văn trong CodeMirror. |
| Thơ | “Nắng qua hàng rào”, ba khổ, chín dòng thơ. So sánh source chính xác gồm xuống dòng và khoảng cách giữa khổ. |
| Markdown | “Qua bậc cửa”, tám đề mục H1–H6, H2 trùng tên, emoji, tiếng Việt tổ hợp và một dòng có dấu thăng trong fence. Sau khi nạp lại, kiểm label, cấp, offset UTF-16 và AST hoàn tất. Chọn H2 thứ hai trên desktop, kiểm active và caret, rồi kiểm sheet mobile đóng và trả focus. |
| Đối chứng | Cùng một CodeMirror đã mount, fixture sai nội dung và fixture ngoài tập đã xác minh đều bị từ chối trước khi gọi capture. Ca Default với cùng source có dấu thăng và giao dịch save/version conflict thuộc `editor-format` từ #567. |
| Preview Leptos | `screen.desk/long` trình bày nhãn mẫu, `screen.editor/long` giữ sáu dòng thơ trong hai khổ. Runner kiểm không có CodeMirror và không phát sinh request API Product khi đổi specimen. |

## Artifact CSR release

Dùng một checkout trên macOS 15.6 arm64, Cargo/Rust 1.99.1, Node 22.15.0, npm 10.9.2,
wasm-bindgen 0.2.129, Apple gzip 457.140.3 và Brotli 1.1.0. Hai lần dùng
`VINK_SPA_REVISION=261ce09c445ff340c6b7adb1da0d6c501b3fdfac cargo xtask build spa --release`.
Biến này cố định prefix của thí nghiệm. Mỗi site được stage riêng vào thư mục Pages scratch.

| CSS, JS, WASM | `261ce09c` trước Epic | `403e9add` sau ba slice | Chênh lệch |
| --- | ---: | ---: | ---: |
| Raw | 7.469.535 | 7.271.063 | −198.472 |
| gzip `-n -9` | 2.004.335 | 1.957.408 | −46.927 |
| Brotli `-q 11` | 1.449.509 | 1.420.764 | −28.745 |

Cộng từng file CSS/JS/WASM, gồm các snippet JS. File tĩnh khác có trong manifest Pages.
Cùng toolchain, variant `csr`, profile `wasm-release`, prefix và mức nén ở hai mốc.
Baseline làm mới 263 đơn vị Cargo WASM. Mốc sau làm mới 1, dùng lại 262 và dùng lại editor
JS. Khác trạng thái cache nên thời gian hai lượt không có giá trị so sánh. Kết quả là
kích thước artifact, không suy ra thời gian tải hay mức dùng bộ nhớ của trình duyệt.
[Byte, hash và digest Pages từng mốc](release-comparison.txt)

Các build chạy guard route và năm normal graph. Quét tám file phát hiện bảy marker UI staff
ở baseline. Gói sau không có các marker đó và vẫn có `desk-review-retry`. Hai gói đều stage
thành công. Khi sao chép gói sau và chèn `vink-preview.css`, Pages từ chối đúng asset trước
khi tạo thư mục output. `python3.13 -B -m unittest test_release -q` đạt 27 test.

## Capture thực tế

Đã mở đủ 11 ảnh Product và 2 ảnh Preview. [Manifest từng ảnh, SHA, hash PNG, fixture,
route, viewport, theme và cỡ chữ](captures.json)

- Product dùng Chrome 158.0.8127.89 ở commit `19f29afe168d3ff5c90c6a08f18640163df0a67e`.
  Desk desktop hiển thị cả sáu hàng gồm ba nháp riêng, hai bản chờ duyệt và một tác phẩm đã
  đăng. Mobile hiển thị đầu danh sách. Nút “Xem kiểm duyệt” là đích chi tiết của tác giả.
  [Lần đầu](product/literary-desk-first-use-1440x900-light-100.png) ·
  [Desk nhiều trạng thái](product/literary-desk-populated-1440x900-light-100.png) ·
  [Desk mobile](product/literary-desk-populated-390x844-dark-100.png)
- Văn xuôi kéo dài qua mép dưới khung hình. Bài thơ hiện ba khổ và đầy đủ ngắt dòng
  ở 390 × 844. [Văn xuôi](product/literary-editor-prose-1440x900-light-100.png) ·
  [Thơ](product/literary-editor-verse-390x844-dark-100.png)
- Mục lục desktop sáng chụp sau mở lại, đủ H1–H6. Desktop tối chụp ở vị trí sau khi nhảy
  tới H2 trùng thứ hai, với đầu trang nằm ngoài khung. Bốn khung mobile thể hiện sheet.
  Ở 320 × 800 chữ 200%, phần cuối mục lục nằm dưới mép vùng cuộn của sheet. Cả tám mục
  đã được kiểm qua DOM. Các mức chữ dùng cài đặt DOM của app trong Chrome headless.
  [Desktop sáng](product/literary-editor-markdown-1440x900-light-100.png) ·
  [Desktop tối](product/literary-editor-markdown-1440x900-dark-100.png) ·
  [Mobile sáng](product/literary-editor-markdown-390x844-light-100.png) ·
  [Mobile tối](product/literary-editor-markdown-390x844-dark-100.png) ·
  [Chữ 150%](product/literary-editor-markdown-390x844-dark-150.png) ·
  [320 px và chữ 200%](product/literary-editor-markdown-320x800-dark-200.png)
- Preview Leptos CSR chạy trên host tĩnh từ commit `3e7c9c4ea157b28aa45b99ba34ca72a19cc52328`.
  Phần đầu bài thơ hiện trong textarea; phần sau nằm trong vùng cuộn của textarea. Assertion
  đọc cả sáu dòng. Inspector và nhãn mẫu nhận diện rõ host. Diff đến commit Product chỉ đổi
  oracle E2E Product. [Desk mẫu](preview/screen.desk--long--light--1440x900.png) ·
  [Editor mẫu](preview/screen.editor--long--dark--390x844.png)

## Nghiệm thu tích hợp

| AC #556 | Bằng chứng |
| --- | --- |
| AC-01 | **PASS.** Product ghi ba văn bản xác định qua App và kiểm phiên tác giả, head trống, head đã có dữ liệu, nháp riêng, tác phẩm chờ duyệt và đang công khai. |
| AC-02 | **PASS.** Kiểm đọc lại từ server, đúng URL/hàng/title/source và một CodeMirror. H1/H2/H3/H2 ở 0/35/83/98, theo sau là H3–H6. Fixture sai hoặc thiếu dừng trước khi chụp. Desktop nhảy đến cuối H2 thứ hai và đánh dấu active. Sheet mobile đóng và trả focus về editor. Lượt đầu của #556 bị FAIL vì oracle tìm `aria-current` trên nút, trong khi thuộc tính nằm ở phần tử bọc. Lượt sau PASS với đúng phần tử đã render. |
| AC-03 | **PASS cho khung đã chọn.** Product và Preview chạy trên origin và package riêng. Manifest lưu 13 ảnh đã mở. Ảnh review của tác giả tiếp tục có trong scenario Publication và evidence #555. Ảnh staff từ lịch sử không dùng cho corpus này. |
| AC-04 | **PASS cho normal graph, CSR release, package CSS/JS/WASM và Pages local. NOT_RUN cho image OCI.** Đọc `infra/Containerfile`: stage Web build theo target Product và runtime chỉ copy binary, site và file revision. Podman trên Mac trả connection refused khi các VM Vink đã dừng. Không khởi động build OCI thứ hai khi checkout chỉ còn khoảng 8,6 GiB trống. |
| AC-05 | **PASS cho mapping dưới đây.** Một PR 4/4, tái sử dụng runner hiện có và hai host theo #552. |

| Slice | Mapping đến Epic và evidence trước đó |
| --- | --- |
| #552 / #562 | AC-01/05/06/07/08 của #552 dùng owner Preview đã tách, inventory, CSS và graph guard, staging Pages, đối chứng asset và số đo CSR ở trên. CMP giữ kết quả riêng từ PR1. Epic AC-01 có bằng chứng host Leptos build/run và không có chiều phụ thuộc ngược. [Inventory Web/Preview](../../web-preview-boundary.md) · [PR #562](https://github.com/loveoverflowcom/vink/pull/562) |
| #553 / #567 | AC-01/02/04 dùng runtime SSR/hydrate và `NOT_REPRODUCED` cho blank kéo dài. AC-03/05/06/07 dùng corpus, negative control và capture giới hạn ở đây. Ca giữ/chặn module, partial AST và save/reopen conflict đã ghi rõ transport. Epic AC-02 tiếp tục dùng evidence đã merge. [Báo cáo và ảnh #553](../desk-editor-553/README.md) |
| #555 / #568 | AC-01–04 của #555 và AC-02–04 của #552 dùng phân quyền API, ba route cũ, author history/appeal, contextual retry và session fencing. Publication chạy lại với phiên Identity thật. Quyết định exact từ thông báo và test PostgreSQL owner dùng evidence #555. Epic AC-03 có bằng chứng Product runtime và marker release. [Báo cáo và ảnh #555](../product-moderation-555/README.md) |
| #556 | Epic AC-04/05 có corpus, ảnh theo host, byte/digest và mapping này. Bằng chứng là các frame đã mở và test đã chạy. Thiết bị mobile thực, browser zoom thật, screen reader và OCI image chưa được kiểm ở slice này. |

## Tái kiểm

Chạy trong checkout Vink. Với `VINK_CHROME` và `VINK_TEST_DATABASE_URL` đã cấu hình tới
scratch, dùng `cargo xtask verify web --scenario publication` cho corpus Product, và
`cargo xtask verify web --scenario desk --scenario editor-format --scenario review --capture checks`
cho assertion liên slice. Publication run `1795697287753450000-70128`, regression run
`1795697424099499000-70741`.

Với Preview, build bằng `cargo xtask build preview`. Lượt đo dùng
`CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_DEV_INCREMENTAL=false` qua launcher local budget.
Serve `target/ui-preview/leptos` qua HTTP loopback rồi gọi:

```sh
node apps/vink-preview/leptos/e2e/preview-smoke.mjs --literary-only 1 \
  --base http://127.0.0.1:3019/ --chrome "$VINK_CHROME" \
  --out target/ui-preview/leptos/literary-captures \
  --catalog packages/ui-system/shared/preview-catalog.json
```
