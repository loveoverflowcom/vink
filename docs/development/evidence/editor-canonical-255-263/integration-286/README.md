# Tích hợp contract nguồn với ngân sách PUT của #286

Source Mac `61215bd026139d2dd5ac68ecaf3b8f41246de194` hợp nhất PR #285 tại
`fd5a922dd2b81d24381120ea64e041b9c1be41ea` và develop
`a19afdc4339f34cbce13ff86f8ce81441e261627`. [Review trước #286](../mac-review/README.md)
ghim các kết quả lịch sử trên đầu cũ.

- Chủ sản phẩm trả lời “Duyệt phạm vi” cho proposal tại `fd5a922`. Blob của commit đã trình,
  tệp checkout và kết quả query r2 cùng SHA-256
  `478a09f823816984d5f92228046c9cf30ab23b78ffae70febf272dbc49643738`. Sau signoff,
  [query](decision-r2.json) trả `ACCEPTED`; [validate](decision-validate.log) kiểm 128
  revisions và 88 references. Phạm vi dữ liệu thật và P1 rehash vẫn cần ủy quyền riêng.
- #286 giữ 8 MiB riêng PUT nội dung. [Cách tính theo DTO](../../../draft-http-budget.md)
  cho cận 4.199.232 B. [Probe JS compact UTF-8](compact-json.log) đo hai source đúng
  2.097.152 byte, default có title NFD 200 scalar sau NFC và Markdown bỏ trường title.
- [Probe PostgreSQL 17.11](cutover-preflight.log) chạy cùng fixture qua SQL ở `fd5a922`
  và đầu tích hợp. Bảng fixture, constraint và function nằm trong schema riêng, rollback
  sau từng ca. Hai draft hợp lệ qua cả hai. SQL cũ chấp nhận sáu hàng mà writer cũ không
  tạo được, trong đó block rỗng làm mất cấu trúc. Bản tích hợp từ chối cả sáu trước UPDATE.
- Reader nhận CR, LF và CRLF làm ranh giới dòng. Draft Markdown thiếu H1 tiếp tục lưu được,
  capture trả lỗi validation ở title. Gate Markdown giữ bản default đã công khai ở đúng
  revision. Các ca PostgreSQL mới kiểm cả hai tình huống.

## Kiểm chứng trên Mac

| Phép kiểm trên `61215bd` | Kết quả |
| --- | --- |
| Corpus và serializer compact Web → App | [PASS 5/5](corpus.log.gz), gồm CRLF đúng 2 MiB, escaping, title NFD và Markdown bỏ title |
| Quick | [PASS 13/13 bước](quick.log.gz), App 343, Web 240 + 1 ignored, xtask 83 + 1 ignored |
| PostgreSQL 17.11 | [PASS 29/29 bước](postgres.log.gz), App 216 + 7 ignored, store 6, Identity 48, role/backup/restore |
| Web main | [PASS 6/6 bước, 5.867 assertions](web-main.log.gz), Chrome thực trên macOS |
| Canonical + draft HTTP | Trong cùng lane PostgreSQL: 8/8 canonical, 3/3 loopback HTTP và 1/1 biên durable của corpus |

Cluster test riêng trên loopback 55585. Cargo dùng `CARGO_INCREMENTAL=0`,
`CARGO_PROFILE_DEV_DEBUG=0`, `CARGO_PROFILE_TEST_DEBUG=0`, hai build jobs và một libtest thread.
Log và kết quả này chỉ áp dụng cho các SHA đã ghi.

## Thay đổi sau lượt kiểm toàn bộ

Tại `40589725c13bf9643554aa85051cad97b475d4fb`, [probe backout trước và sau](backout-preflight.log)
dùng một schema PostgreSQL 17.11 riêng trong transaction. SQL ở `61215bd` nhận ba draft đã
drift khỏi domain dù qua CHECK JSON. SQL sửa từ chối cả ba (control, 200.001 scalar, title 201
scalar). Có regression SQLx kiểm thêm title chứa LF và readback nguyên hàng. Cargo/SQLx với
regression này, Chrome budget ba corpus và Kani tại head mới **NOT_RUN** ở lượt bàn giao Mac
cho reviewer #287. Kani 3+33 thuộc lượt cũ ở `0543e34`. Không chạy GitHub Actions hay
migration trên dữ liệu thật.

## Điểm giao với #275

PR #287 đang được review riêng. Tại `6627553` migration policy `20261121120000` đứng
sau `20261121000000` và trước canonical `20261122000000`. Phép thử upgrade/backout của
#275 gọi `migrate()` tới latest và tạo fixture bằng `submit()` của binary hiện hành trong
schema cũ. Khi kết hợp phải dùng data fixture đúng từng schema, kiểm trạng thái ledger/hàng ở
từng mốc, bảo toàn revision và digest khi canonical từ chối, và down ngược thứ tự các
migration thực sự đã áp dụng. Trên schema canonical, kiểm công khai lại cùng revision bằng
`RevisionContent::new(public.content)`. Chạy lại focused, Quick và PostgreSQL cho chính đầu
kết hợp. Kết quả hai PR độc lập không xác nhận được phần này.
