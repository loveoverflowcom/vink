# Bằng chứng đo DISC-02/03 phần 2 — đọc bình luận (#120)

Quyết định và bảng kết quả: [discussion-reads §5](../../../architecture/discussion-reads.md#5-số-đo-postgresql-trước-khi-tối-ưu).
Dữ liệu tổng hợp, không có nội dung thật. Chạy trên một cluster PostgreSQL scratch (ví dụ 17.x tại
`127.0.0.1:55432`, user `vink_test`), **không** trên database chung.

| File | Vai trò |
| --- | --- |
| [seed.sql](seed.sql) | Dataset tái lập (`setseed`): hot thread 2 000 root/~21 500 reply, 300 thread nền, decision rows |
| [power_user.sql](power_user.sql) | Viewer P với 120 lời pending/rejected rải trên timeline |
| [queries.sql](queries.sql) | Đúng các câu SQL production (`roots`, `previews`, `replies`, `own_list`, `own_exact`) dạng `PREPARE` |
| [explain.py](explain.py) | `EXPLAIN (ANALYZE, BUFFERS)` 7 lần/workload, lấy median |
| [candidates.sh](candidates.sh) | So từng index ứng viên với baseline (tạo, đo, kích thước, xóa) |
| [latency.py](latency.py) | p50/p95/p99 bằng pgbench `-M prepared`, 4 client, 10 s |
| [wal.sh](wal.sh) | WAL của 2 000 lần gửi + duyệt, sau `CHECKPOINT`, có/không index |
| `plans-*.txt` | Plan rút gọn đã đo: baseline, `own_partial` (được chọn), `own_full`, `public_replies` |

Quy trình (đường dẫn script là thư mục này; chạy từ worktree của PR):

```bash
createdb -h 127.0.0.1 -p 55432 -U vink_test vink_bench
```

```bash
VINK_MIGRATE_DATABASE_URL=postgres://vink_test@127.0.0.1:55432/vink_bench cargo run -p vink-app -- migrate
```

```bash
VINK_PROFILE=local DATABASE_URL=postgres://vink_test@127.0.0.1:55432/vink_bench cargo run -p vink-app -- dev-seed
```

Sau đó `psql … -f seed.sql`, `psql … -f power_user.sql`. Số baseline được đo **trước** khi có
migration `20261113000000_own_comment_overlay`; để lặp lại baseline, chạy file rollback của nó
trước, rồi `python3 -I explain.py baseline`, `sh candidates.sh`, `python3 -I latency.py LABEL`,
`sh wal.sh`.

Môi trường đã ghi: macOS (Darwin 25.5, Apple Silicon), PostgreSQL 17.11 Homebrew, `fsync=off`,
`shared_buffers` mặc định, cache ấm, source `develop@8a58089` + thay đổi của PR. Thời gian dưới
1 ms dao động ± 0,3 ms giữa các lần; dùng số buffer để so sánh. Đây không phải SLA production.
