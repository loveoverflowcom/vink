# Tích hợp policy re-review và document canonical

Commit `36efcf652738731c76b2772bce367b22f6623460` ghép develop
`2e27e585036353a68ddc18a5eb36a8075eb9cc78` (#287) vào #285. Dữ liệu kiểm chứng
là fixture tổng hợp trên PostgreSQL 17.11 riêng của #285. Kết quả từng lane trên head
cuối, số assertion Chrome, phiên bản Kani và giới hạn kiểm chứng nằm trong
[biên bản PR #285](https://github.com/loveoverflowcom/vink/pull/285#issuecomment-6967734211).

| Mốc | Chứng cứ |
| --- | --- |
| `20261121000000` | Seed nháp và revision ở đúng shape cũ, thêm bốn trạng thái kiểm duyệt, job, case, receipt và decision history. Mỗi revision fixture mới nhận document và digest từ sample v1 lúc INSERT. |
| `20261121120000` → `20261121120100` | Áp từng migration policy; đối chiếu ledger và nguyên các hàng trước/sau. Khi có revision cũ, cutover `20261122000000` từ chối và giữ nguyên các hàng. |
| Backout policy | Pending v3 làm transaction từ chối, giữ đủ hai migration trong ledger. Sau v4, down/up giữ nguyên lịch sử v2–v4, digest và revision. Trên DB trống đã canonical, chạy down canonical trước hai down policy rồi up lại đủ ba theo thứ tự. |
| Runtime canonical | Author gửi lại đúng revision theo policy mới. Lỗi ở bước ghi receipt rollback cả transaction. Retry cùng key replay; worker duyệt đúng revision, hash từ `public.content` khớp digest ticket. |

Trên source tích hợp, [focused policy](policy.log.gz) PASS 6/6,
[canonical](focused.log.gz) PASS 9/9, [HTTP loopback](http.log.gz) PASS 3/3
và [corpus](corpus.log.gz) PASS 5/5. File [SHA256SUMS](SHA256SUMS) băm từng log nén.
R2 giữ đúng digest phê duyệt. Cutover trên dữ liệu thật, deploy và mọi phương án
đổi lịch sử bất biến nằm ngoài lượt kiểm này.
