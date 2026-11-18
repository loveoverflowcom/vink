# Quyết định #239 giữ điều hướng Reader theo click

Base `9fcd75ec03229a27464123eac1d68104e8447b19`, harness `f91daf2efe4228b08174fe9322eada9a2dcf1c9b`. Chrome 157.0.8103.78/Linux, 1280×900, DPR 1, loopback, release/wasm-release, App khách và PostgreSQL 17.11. Hai DB scratch dùng cùng một work/card, thay duy nhất revision bài ngắn bằng source #229 có 200.000 đoạn. Mỗi cấu hình có 10 lượt cold-key (5 pointer, 5 keyboard). Mỗi lượt thử intent bị bỏ rồi đi thật. Resource Timing đo response end của GET. Không giữ transport bằng CDP ở phép đo này.

Ngưỡng lab chốt trước spike: chỉ thử B nếu median chờ GET đạt 16 ms và lớn hơn IQR click → article. Giới hạn B dự kiến 1 candidate, 1 flight, 1 request, 32 KiB body/intent và 250 ms. Wasted body trung bình cho intent bỏ ≤8 KiB. Thiếu giới hạn byte tin cậy trước fetch thì bỏ candidate. Các ngưỡng độc lập với trần RAM retention của #237.

| Corpus | GET Reader ms, median (min–max, p95 lab) | Click → article ms, median (min–max, p95 lab) | IQR click ms | Body B | GET khi bỏ / khi đi thật |
| --- | ---: | ---: | ---: | ---: | --- |
| Một đoạn | 8.65 (6.10–10.20, p95 10.20) | 64.45 (59.60–73.30, p95 73.30) | 3.10 | 584 | 0 / 1 mỗi lượt |
| 200.000 đoạn | 55.05 (42.40–72.10, p95 72.10) | 1452.65 (1314.30–1721.60, p95 1721.60) | 119.60 | 1,000,522 | 0 / 1 mỗi lượt |

**No-go.** Ngay cả khi che được toàn bộ thời gian GET, bài ngắn vẫn dưới mức lợi ích 16 ms. Response dài gấp hơn 30 lần ngân sách. Card công khai không có dữ liệu để chứng minh trần byte trước khi gửi. Chọn A. Trong A, cả 20 intent bỏ có 0 Reader request, 0 response byte và 0 công việc Reader speculative. Thời gian server thực thi không được instrument trong phép đo này.

| AC | Trạng thái và ranh giới |
| --- | --- |
| 01 | PASS. Đã đo GET tách với render article, áp dụng ngưỡng chốt trước trên hai kích thước. |
| 02–03 | Không áp dụng cho outcome A. Không xây B vì dừng ở admission; join, fence hay hủy flight speculative đều chưa chạy. |
| 04 | PASS phần A cho intent chuột/bàn phím bị bỏ. Timeout, offline, withdraw/republish và dispose của **speculation** chưa chạy. Lifecycle của GET bình thường có evidence riêng ở #234/#237. |
| 05 | Đã đo A và wasted bytes cùng fixture; không có before/after/rollback B. Quyết định dừng là outcome cho phép trong #239. |

Tại gốc repo, phục vụ App guest với release của base và cơ sở dữ liệu scratch theo [fixture #234](https://github.com/loveoverflowcom/vink/issues/234). Chạy một lần mỗi corpus

```sh
node apps/vink-web/e2e/browser-smoke.mjs --base "$SCRATCH_ORIGIN" --chrome "$VINK_CHROME" --state "$ARTIFACTS/state.json" --phase reader-intent-cost
```

Trace và ledger từng lượt nằm trong [bài ngắn](small.json.gz) và [bài dài](blocks.json.gz). Không có timing ngoài lab hay dữ liệu người dùng.
