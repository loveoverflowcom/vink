# Review độc lập PR #283 — 17/11/2026

Base `29185dd0eceb3a6395fb9c1abfece5a3f2f7068c`; PR ban đầu
`d0f296d991d68860eb8e080a35f5b69b67376634`. Checkout riêng trên Mac;
không dùng kết luận tác giả làm PASS của lượt review, không đọc/chờ/rerun CI.

## Findings và sửa chữa

- **P2 — oracle mô phỏng A/B/C bỏ qua response bị từ chối.** Key Reader dùng tên scenario,
  còn `Publication.work_id` dùng `234-N`, `mix-N`, `tiny-N`. `Coordinator::finish` trả
  `Unavailable`, nhưng helper bỏ kết quả rồi gọi `follow_seed` với fixture tự cấp, khiến
  mô phỏng có vẻ thành công. Thêm assertion đúng waiter và response từ production seam
  tái hiện FAIL; sửa key theo `work_id` và seed từ reply đã được chấp nhận.
  Cache hit dùng navigation start `0` giống browser, không gia hạn validation bằng giờ hit.
- **P2 — kết luận GET/cache hit vượt raw.** B có 1/6 Reader miss ở normal revisit và 2/3
  ở mixed revisit; không phải 100% hit. C có 22/19/5 GET list tổng ở ba corpus, cao hơn
  B lần lượt 21/18/4. Reader GET đếm riêng. Đã sửa README, giữ raw gốc để audit.
- **Giới hạn evidence:** CSV cũ đo `struct_stats` thay vì hàm weigh production;
  lần sửa gọi `snapshot_weight_bytes` và xuất cột `production_weight_B` riêng.
  Probe Chrome gốc không có script, source patch/digest B/C; không thể chứng nhận nguồn
  chính xác của probe chỉ từ JSON. Các số cũ là historical observation, không final-HEAD PASS.
  Test cancel thực tế làm ticket cũ bị `Discarded`; không gọi nhánh park sau Fresh.

Runtime của PR không đổi trong bản sửa review. Không thấy regression P0/P1 qua source audit;
kiểm final source và giới hạn runtime sẽ được ghi dưới đây sau khi chạy.

## Boundary và quyết định

Trọng số UTF-8/container chỉ bao optional `SnapshotCache`; active view, resource handoff,
clones, decode, flight và metadata/fence không nằm trong tổng. `get` không cập nhật seq;
đây là thứ tự admission/release (`put`), không LRU cho mọi lần `status/get`.
Cache eviction không xóa coordinator authority; NoOp/over-budget có thể refetch nhưng
không cắt văn bản đang mở. INV-01/07/08, permission và model M1/M3/M4 không đổi.
Model ReadOwner/PageChain không bao `Store/Coordinator`; pure conformance không thay
browser/SQL evidence. Không thêm model/runner cạnh tranh.

READ-01/02 và DISC-WEB giữ DESIGN_PROPOSED; VDR offline-age, snapshot-storage và
read-lifecycle giữ PENDING. Không signoff, không đổi 15 phút/24 giờ, không bật IndexedDB.
