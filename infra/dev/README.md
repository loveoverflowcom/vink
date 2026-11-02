# infra/dev (đã chuyển)

`infra/dev/compose.yaml` đã được thay bằng định nghĩa container duy nhất tại
[services/postgres](../../services/postgres/README.md). Project `vink-dev`, image
`postgres:17.11-alpine`, volume `vink-dev_vink-postgres`, database `vink` và cổng
`127.0.0.1:55432` được giữ nguyên.

Nếu container cũ đang chạy, chỉ cần chạy lệnh mới; không dùng `down -v` và không xóa volume.
Cách chuyển, kể cả khi đã chạy Compose với `-p`, nằm ở
[services/postgres/README.md](../../services/postgres/README.md#chuyển-từ-infradev).
