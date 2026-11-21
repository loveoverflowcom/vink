# Kiểm chứng sau review chéo PR #367

[Draft PR #367](https://github.com/loveoverflowcom/vink/pull/367) · [Issue #353](https://github.com/loveoverflowcom/vink/issues/353) · [Ảnh và log lượt trước](README.md)

Base `ab5ecaff9685938dc689b52d98957375fbbfdf12`, head được review `03f2b1dbcc56adba1d532b0e533d0e16271286c0`. Mã và fixture lượt này `16095755d21a10bc855a73075e86cf0a9d0940ca`, cwd `/private/tmp/vink-353-follow-profile`.

| Finding | Bản sửa và kiểm chứng |
| --- | --- |
| 1 · AC-01 | PASS. Chrome chỉ nhận 503 tại Graph viewer read. Phiên chính chủ đọc hồ sơ riêng và giữ Chỉnh hồ sơ. |
| 2 · AC-02 | PASS. PUT đã xác nhận với cả bốn quyền ẩn gỡ link và tổng ngay. Phép thử giữ một PUT thật, làm mất ACK và chỉ trả 503 cho lần đọc summary. Vùng quan hệ bỏ dữ liệu cũ và thử lại đúng quyền. Link Người theo dõi không tổng và số 0 Đang theo dõi không link được kiểm đồng thời. |
| 3 · AC-02/04 | PASS. Back đọc lại cursor để mở trang chứa hàng đã tải thêm. Khi hàng mất, focus chuyển về H1. History giữ cursor cùng định vị liên kết. Việc đọc lại dùng phiên hiện tại và cursor được provider kiểm phạm vi. |
| 4 · AC-03/04 | PASS. Chrome giữ DTO alias thật 11 giây, sau đó xác nhận focus trên H1 canonical. Lượt thứ hai đổi đích sang Tôi trước khi trả DTO cũ. Phép thử Node trên mô-đun điều hướng kiểm thêm đổi đích trước khi URL mới commit. |
| 5 · AC-02 | PASS. Append đọc lại quyền hướng kia, gỡ link trong lúc kiểm và sau khi quyền bị ẩn. Lỗi 503 ở riêng phép đọc này đưa ra Thử lại tại chỗ, các hàng đang đọc giữ nguyên. |

## Lệnh và phạm vi

- PASS `cargo xtask verify web --scenario password`, run `1795185305086785000-97026`. Chrome 158.0.8127.36, App/Identity thật, PostgreSQL scratch 17.11, fixture #331. Các lỗi theo từng URL do Chrome CDP tạo.
- PASS (13/13 test đã chọn) `python3.13 ./tools/local-budget run -- cargo xtask verify unit web --filter follow`.
- PASS `cargo fmt --all -- --check`, `node --check` trên hai smoke và mô-đun điều hướng, `git diff --check`.

Ảnh lượt trước giữ nguyên provenance tại `8ac0943`. Lượt này tập trung vào quyền, focus và điều hướng ở các finding. `NOT_RUN` cho submit no-JS ở hai trang danh sách, BFCache hit thật, trình đọc màn hình và ma trận toàn Epic #351. Ca 410 tiếp tục dùng App scratch; xóa xuyên hai DB thuộc #297. [Log kiểm chứng lại](review-checks.txt)
