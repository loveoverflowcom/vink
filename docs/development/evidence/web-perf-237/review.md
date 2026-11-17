# Review độc lập PR #283 — 17/11/2026

Target develop `29185dd0eceb3a6395fb9c1abfece5a3f2f7068c`; merge-base ban đầu `afb402008b94924cccf43d5b5e81b1c60743855a`; PR ban đầu
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
- **P3 — recipe NoOp một dòng không khớp type thật.** `Owner.coord` cố định generic
  mặc định `InMemoryCache`, nên chỉ đổi constructor sang `NoOpCache` không biên dịch.
  Đã ghi cả hai điểm cần đổi và yêu cầu kiểm trên diff rollback; rollback UNBOUNDED
  chỉ đổi đối số budget. Không biến NoOp thành một công tắc runtime mới.
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

## Phép đo độc lập

Source test/runtime: `e93058ceb3b777fab0d23bb98df2b179382f12b0` (commit chỉ sửa test/docs;
runtime cache giữ nguyên từ `d0f296d`). Lệnh benchmark chạy trên cùng bytes trước/trong lúc
commit này được ghi, không đổi inputs. macOS arm64; Rust 1.99.1; Node 22.15.0;
`CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0` cho các lệnh Cargo.
Build/test hỗ trợ chạy qua `./tools/local-budget run`; browser theo ngoại lệ UNSUPPORTED
của launcher, bố trí tuần tự, `CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2`.

- **PASS** `cargo test --offline --locked --release -p vink-web --lib --features ssr
  measure_237_corpus_and_retention_tradeoff -- --ignored --nocapture`: 1 test, 24 dòng corpus
  và 3 dòng mô phỏng; [raw log](corpus-release-fixed.log.gz), [CSV sửa](review-corpus-release.csv).
  `walk_us` đo hàm production, một sample native mỗi case (không phải distribution hay timing
  browser). `vi_nfd` là stress DTO NFD thô từ generator #229; App thật chuẩn hóa NFC trước
  public projection, nên không coi số scalar cột input là số scalar DTO sau NFD.
- **FAIL có chủ đích**: [oracle trước sửa key](benchmark-oracle-negative.log.gz),
  `Unavailable` thay vì Publication; [bỏ removal fence](removal-fence-negative.log.gz),
  assert seed cũ bị chặn thất bại; **PASS sau khôi phục**
  [removal fence 1/1](removal-fence-restored.log.gz).
- **PASS trên PR gốc `d0f296d`**: `cargo test --offline --locked -p vink-web --lib
  --features ssr`, 240 passed / 1 ignored, [raw](web-lib-initial.log.gz).

| Corpus / 50 vòng | A peak retained proxy | B peak retained proxy | A / B / C hit:miss |
| --- | ---: | ---: | --- |
| 16 bài #234 + list | 16.168 B | 16.168 B | 49:51 / 49:51 / 0:100 |
| 16 bài mixed 200k + list | 8.584.186 B | 2.059.586 B | 49:51 / 49:51 / 0:100 |
| 4 bài tiny-blocks 200k + list | 39.208.029 B | 5.641 B | 95:5 / 49:51 / 0:100 |

Đây là coordinator với fake origin reply đã kiểm subject; mỗi miss là ý định fetch,
không phải wire HTTP GET/304. A trong mô phỏng dùng **cùng ownership mới**, chỉ bỏ byte budget;
Chrome A lịch sử dùng runtime cũ `afb4020`, nên hai so sánh không hoán đổi được.
Ở corpus 16 Reader + list vượt soft16 nên vòng round-robin mất Reader cả A/B; hot list
vẫn hit 49/49 revisit. B bỏ 50/50 tiny snapshots; chấp nhận thêm 46 miss so với A để tránh
39 MB retention. Điều này giữ lựa chọn B, không dùng tỷ lệ hit tuyệt đối làm luận cứ.

## Reconcile và registry

Merge latest develop vào branch review tại `45616e74a83d0cc2be5f266c55d4099d24f7fe78`,
không xung đột. Bao gồm fix session-ready PublicationPanel #281 và đối soát #282.
Các file `public_reads.rs`, `freshness.rs`, `read_cache.rs`, `Cargo.lock` giữ nguyên SHA-256
so với lúc chạy benchmark/negative control; không gọi kết quả đó là một lần rerun trên merge.

`decision validate`: **PASS**, 127 revisions, 88 refs, snapshot
`8354df2cde868013df8324e1934351d094f93739728e485c4f59a1e1d46462b8`.
`decision refs`: **PASS**, problems `[]`. [Raw query](review-decisions.json.gz)
xác nhận READ-01/02/DISC-WEB DESIGN_PROPOSED; O-03/O-17 r2 và USER-02/09 r1
PRODUCT_CONFIRMED; ba VDR nêu trên PENDING/current=null. Merging không signoff.

**PASS Quick 13/13 trên integrated code `45616e7`:** fmt, 3 clippy targets,
25 lawful + 34 compile-fail doctests, Reader SSR boundary 1/1, workspace unit/contract
(vink-web 240 passed / 1 ignored, xtask 83 passed / 1 ignored), repository checks.
[Raw final-code Quick](quick-integrated.log.gz), 185,71 s; sampled RSS cộng lớn nhất
2.314.112 KiB, pressure normal; soft budget không là trần RSS vật lý.
[Quick trước reconcile](quick-final-code.log.gz) cũng PASS 13/13, 299,06 s.

**PASS corpus canonical #229**, `cargo test --offline --locked -p vink-app --test
document_budget`: 3/3 trên `45616e7`, 49,78 s; validator/scalar boundaries, independent
JSON byte/hash oracle và oversized optional persistence miss. [Raw](canonical-corpus.log.gz).
