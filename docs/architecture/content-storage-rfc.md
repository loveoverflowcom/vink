# RFC #231 · Độ hạt lưu văn bản

**Đề xuất giữ JSONB nguyên khối (A).** PostgreSQL tiếp tục sở hữu nháp, revision và bản đang công
khai. Đề xuất [VDR-CONTENT-STORAGE@r1](../../decisions/proposals/VDR-CONTENT-STORAGE.r1.toml)
ở trạng thái `PENDING`. Phạm vi PR gồm RFC và phép đo PostgreSQL scratch. Runtime
`persistence-api`/`web` vẫn dùng representation hiện hành.

Khi tác giả sửa bài đã đăng, người đọc tiếp tục thấy đúng bản đã công khai. Một transaction có
thẩm quyền chuyển pointer sang bản phù hợp. Nếu bài chuyển riêng tư hoặc bị gỡ, lần đọc tiếp theo
tuân theo trạng thái mới, kể cả khi client còn ETag của bản cũ.

## Căn cứ tại develop

Source base `f0ef35193b2bef2fc9ded9f66b1482a4557ac03e`. Tra `cargo xtask decision query` tại
snapshot registry `6e77cb0a7f4eb67033c6e0df6d8a670a84eb61b3095a307ad0615b771c91dd9b` trước
khi thêm proposal:

<!-- vdr: active USER-09@r1 -->
<!-- vdr: active USER-02@r1 -->
<!-- vdr: active ADM-06@r1 -->
<!-- vdr: active O-08@r1 -->
<!-- vdr: active O-17@r2 -->
<!-- vdr: active D-02-CONTENT@r1 -->

| Nguồn quyết định | Phạm vi áp dụng |
| --- | --- |
| `USER-09@r1 PRODUCT_CONFIRMED`, [WR-04 DEFERRED](../../prd/requirements.md) | Một bài ngắn là đơn vị người dùng. Payload/chunk là chi tiết lưu trữ bên trong cùng một tác phẩm. |
| `USER-02@r1 PRODUCT_CONFIRMED`, AUD-01/02 và PUB-01/02 | Hai audience, lưu nháp riêng và công khai đúng snapshot có ý định của tác giả. |
| `ADM-06@r1 PRODUCT_CONFIRMED`, PUB-06 và MOD-07..10 | Chỉ sử dụng quyết định còn hiệu lực cho đúng snapshot, đúng policy và theo thứ tự authority hiện hành. |
| `O-17@r2 PRODUCT_CONFIRMED`, `O-08@r1 OPEN` | Trang thay thế sau kiểm duyệt có contract riêng. Retention/purge nội dung sau khi xóa cần quyết định riêng. Quyền truy cập lịch sử công khai cũng còn mở trong #231. |
| `D-02-CONTENT@r1 DESIGN_PROPOSED` | PostgreSQL là owner đang chạy. Hướng storage #231 là đề xuất kỹ thuật riêng. |
| `VDR-PUBLIC-SNAPSHOT-STORAGE@r1`, `VDR-PUBLIC-OFFLINE-AGE@r1` | Cả hai `PENDING`; giữ gate #203 và chính sách đọc hiện hành. |

[Issue #231](https://github.com/loveoverflowcom/vink/issues/231) nêu giả thuyết chương giải quyết
95% dedup, chunk 32–64 KiB giữ lại bytes sau edit, LZ4 nhanh hơn nhiều và UPDATE tại chỗ. Phép đo
dưới đây kiểm reuse theo loại edit và kiểu cấu trúc. Tỷ lệ 95% chưa có corpus đại diện. JSONB có
TOAST và MVCC giữ tuple cũ cho snapshot còn đọc. `SET COMPRESSION` áp dụng cho lần ghi sau, còn
rewrite dữ liệu đã lưu là thao tác riêng. #229 từng thấy LZ4 giảm median UPDATE của Việt NFC
200k từ 14,29 xuống 8,44 ms, trong khi tổng relation scratch tăng khoảng 42% và WAL theo mẫu
tăng khoảng 20%. Giữ default/pglz. [Ngân sách #229](../development/long-document-budget.md),
[TOAST PostgreSQL 17](https://www.postgresql.org/docs/17/storage-toast.html),
[ALTER TABLE PostgreSQL 17](https://www.postgresql.org/docs/17/sql-altertable.html).

### Source và contract

| Boundary | Owner tại base |
| --- | --- |
| Nháp riêng/version, full-body save, shape NFC | [Document v1](../../services/vink-app/src/domain/document.rs), [PgStore](../../services/vink-app/src/persistence/postgres.rs), [core migration](../../services/vink-app/migrations/20261030000000_literary_core.sql). |
| Capture, revision immutable, metadata, ý định và visibility | [Revision/hash](../../services/vink-app/src/domain/revision.rs), [publication adapter](../../services/vink-app/src/persistence/publication.rs), [lifecycle migration](../../services/vink-app/migrations/20261116000000_work_publication_persistence.sql), [runtime #158](../development/publication-runtime.md). |
| Decision và hiệu lực | [Moderation](../../services/vink-app/src/domain/moderation.rs), [evaluation worker](../../services/vink-app/src/persistence/evaluation.rs), [appeals](../../services/vink-app/src/persistence/appeals.rs), [runtime](../development/moderation-runtime.md), [Thấm](../development/literary-endorsement-runtime.md). |
| Nội dung và control công khai | [Public DB read](../../services/vink-app/src/persistence/discovery.rs), [READ-01](../../services/vink-app/src/http/conditional.rs), [HTTP v1](../../services/vink-app/contracts/http/v1/README.md), [Reader document](../../apps/vink-web/src/document.rs) và [READ-02/#203](../development/public-read-freshness.md). |
| Owner-state | [#230](../development/publication-state-read.md): restore từ metadata, dưới một read snapshot. Capture vẫn đọc draft, command/worker vẫn kiểm full body history, Reader kiểm hash đúng bản đang trả. |

#230 đã đo Việt NFC 200k × 100 revision từ 4.386,58 ms xuống 1,647 ms median warm, volume body
logic được SELECT từ khoảng 45,2 MB xuống 0, ở môi trường dev. Response metadata vẫn tăng theo
số revision. #229 đo full-body save, TOAST, SSR, browser và HTTP; follow-up #249 tách từng mẫu ghi
thay đổi khỏi no-op. Case nhiều block 200k vẫn chạm giới hạn transport 2 MiB và dựng khoảng 200k
paragraph trong Reader. Đây là nút thắt ở adapter/rendering, còn thử nghiệm storage giữ nguyên
full-body trên wire. Nếu command/worker trở thành điểm nóng, đo đường đọc target revision sau
khi xác định kiểm tra toàn vẹn nào cần giữ, rồi so với chi phí thay representation.
[Phép đo #230](../development/publication-state-read.md#đo-trướcsau-trên-postgresql-thật),
[đối soát #229/#249](../development/long-document-budget.md#đối-soát-follow-up-229).

## Ba phương án

| Phương án | Write và read | Chi phí và điều kiện cân nhắc |
| --- | --- | --- |
| **A · JSONB + TOAST** | Draft ghi toàn document theo version. Capture lưu JSONB vào revision. Reader key theo work và đọc một body, owner-state chỉ đọc metadata. | Ít join, giữ chính xác upgrade/backout hiện hành. Edit nhỏ vẫn ghi toàn body cho revision mới. Theo dõi byte hậu nén × số snapshot, WAL, dead tuples/TOAST, replica và backup. |
| **B · Payload nguyên văn trong một work** | Revision giữ ID/title/metadata/hash v1 riêng, tham chiếu payload document immutable. Một edit đổi nội dung tạo body mới. Title/tag-only hoặc undo có thể reuse byte nếu toàn document trùng. Draft trước mắt giữ JSONB. | Thêm một lookup/FK khi ghi, join khi đọc và lifecycle của payload. Đo tỷ lệ exact repeat theo work, sau nén và index, gồm nguy cơ tiết lộ sự tồn tại qua timing/metric. |
| **C · Manifest chunk nội bộ** | Mỗi revision có một manifest sealed. Chỉ ghi chunk chưa có trong scope của work; một read snapshot lắp đúng thứ tự trước khi validate/hash. Draft và save HTTP giữ document đầy đủ cho lần đánh giá đầu. | Có thể giảm WAL/storage khi edit cục bộ và lịch sử dài. Cần sort/join/assembly, index ordinal, xử lý khối lớn, GC và di trú. Network giảm chỉ khi có giao thức mới và kiểm ở boundary riêng. |

### Thử nghiệm phân biệt

[Phương pháp, ngưỡng và raw samples](../development/evidence/content-storage-231/README.md).
So A/B/C trên PostgreSQL 17 scratch, cùng JSONB/pglz. C dùng mốc theo nội dung block, min 32 KiB,
cut theo fingerprint hoặc cap 64 KiB. Greedy 60 KiB là control. Các ngưỡng tính trên compact JSON
chunk, gồm dấu phân cách. Block tự nó vượt cap đi riêng. Prototype dùng boundaries từ v1 hiện
có và recompute toàn document. Khi cần triển khai tiếp, so với nhóm block ổn định cập nhật cục
bộ để tránh rediscover boundaries, đo thêm rebalance, oversized-unit policy, ownership của
editor và version mapping. CDC dạng byte rolling là ứng viên khác, cần format/version cho
round-trip Unicode và khổ thơ. [Tham chiếu thuật toán restic](https://restic.net/blog/2015-09-12/restic-foundation1-cdc/).

### Kết quả và khuyến nghị

Trên lịch sử 24 revision, **B chỉ reuse hai body** (đổi title và undo) cho mỗi corpus. Tổng
relation và WAL giảm khoảng 5–8% và 7–8%. C giảm mạnh chi phí lưu cho các văn bản nhiều block,
nhưng p95 tổng ghi và đọc vượt ngưỡng thử nghiệm. Với văn bản 190k chỉ một block, C reuse nguyên
block trong một số thao tác chèn/xóa hoặc đổi thứ tự. Mỗi sửa một scalar *bên trong* block vẫn
ghi lại cả block. Hai biên chunk cho kết quả dung lượng bằng nhau ở ca này.

| Corpus | Layout | Tổng relation (MiB) | Tổng WAL (MiB) | Prep + commit p50 / p95 (ms) | Đọc p50 / p95 (ms) |
| --- | --- | ---: | ---: | ---: | ---: |
| Việt nhiều đoạn 30k | A / B / C | 1,17 / 1,11 / 0,66 | 1,13 / 1,04 / 0,57 | 1,62 / 2,14 · 2,03 / 2,50 · 3,36 / 4,41 | 0,62 / 0,68 · 0,63 / 0,69 · 0,69 / 0,74 |
| Việt nhiều đoạn 190k | A / B / C | 6,93 / 6,38 / 1,41 | 7,07 / 6,49 / 1,34 | 8,49 / 9,05 · 10,74 / 12,30 · 17,12 / 25,20 | 3,42 / 3,59 · 3,44 / 3,63 · 3,96 / 4,62 |
| Thơ/tab 190k | A / B / C | 7,17 / 6,61 / 1,08 | 7,33 / 6,73 / 0,99 | 8,96 / 9,60 · 11,32 / 20,97 · 17,81 / 19,50 | 4,47 / 4,79 · 4,48 / 4,67 · 5,24 / 5,60 |
| Việt một đoạn 190k | A / B / C | 6,83 / 6,30 / 4,92 | 6,96 / 6,39 / 4,94 | 8,37 / 8,91 · 11,02 / 11,56 · 22,74 / 23,75 | 3,27 / 3,43 · 3,31 / 3,47 · 3,54 / 3,77 |

C ở đây là biên theo nội dung. Greedy 60 KiB trên thơ 190k đạt 1,66 MiB relation và 1,59 MiB WAL.
Ở case equal-size, chèn ba block đầu hoặc sau block 5 làm mất toàn bộ nhóm cũ. Thêm ba block
cuối giữ cả ba nhóm cũ. Thêm đúng 40 block đầu hoặc xoay một nhóm nguyên vẹn giữ các nhóm cũ.
Trên Việt nhiều đoạn 190k, greedy giữ khoảng 86% bytes ở prepend và 0% ở reorder trong lịch sử
thử nghiệm, biên theo nội dung giữ khoảng 92% và 75%. Đây là các phép thử boundary shift, với
reuse tính theo bytes của snapshot sau từ mọi payload đã có trong cùng lịch sử. Trường hợp
đối chứng chính xác kiểm reuse từ riêng snapshot trước đó.
[Raw và giới hạn](../development/evidence/content-storage-231/README.md).

Giữ A. #230 đã bỏ đọc body khỏi owner-state. B chưa có tín hiệu đủ lớn về exact repeats. C có
tín hiệu dung lượng mạnh ở một số shape nhưng tốn prepare/assembly ngay cả với một writer và
cache khóa dedup lý tưởng. Ưu tiên đo workload đại diện và xử lý transport/Reader theo
[#229](../development/long-document-budget.md). Mở lại B khi exact repeat theo work đủ vượt
ngưỡng. Mở lại C khi chi phí sau nén, WAL/replica/backup của lịch sử thực sự vượt ngân sách
và candidate dùng biên ổn định đạt ngưỡng cả storage lẫn latency. Giữ ngưỡng và cách đo tại
[evidence](../development/evidence/content-storage-231/README.md).

## Contract nếu mở lại B hoặc C

### Danh tính và SQL binding

- `ContentHash::of` v1 là SHA-256 trên domain separator rồi title và số block/dòng cùng byte
  UTF-8 length-prefixed. Dùng `Document::parse` và `Title::parse` trước khi hash. Kiểm lại golden
  vector NFC/NFD và hash của mọi dữ liệu backfill. Hash storage có domain/codec version riêng.
  Khi trùng digest, so nội dung canonical trong scope trước khi reuse, sai khác trả integrity error.
- Revision, metadata snapshot, decision/provenance, endorsement “Thấm” và appeal giữ định danh
  riêng. Metadata-only hoặc title-only có thể tạo revision mới dùng chung body. Cùng body vẫn
  có thể có policy, metadata, manifest context hoặc quyết định người duyệt khác và phải xét
  theo `ADM-06`. Receipt là kết quả của lệnh theo actor/work/op/key + input digest, độc lập
  với digest dùng lưu byte hay revision đã match.
- B: `content_payloads(work_id, codec_version, digest, body)` khóa ghép theo ba cột đầu. Link
  cạnh revision tham chiếu `revisions(work_id, id)` và payload bằng khóa ghép có cùng `work_id`.
  Authorize theo work trước truy cập. Bind prepared values và hạn chế việc trả/log hash, existence
  hay dedup hit ra bề mặt người dùng. So timing và side channels theo owner trong lần mở lại.
- C: với một unit storage cho mỗi tác phẩm, `storage_units UNIQUE(work_id)` và
  `UNIQUE(work_id, unit_id)`. Manifest có `UNIQUE(work_id, unit_id, revision_id)` và một
  manifest sealed cho mỗi `(work_id, revision_id)`. FK ghép tới unit và `revisions(work_id,id)`;
  uniqueness một unit/work bảo đảm revision chỉ gắn unit đó. Item key
  `(work_id, unit_id, revision_id, ordinal)` và FK ghép tới manifest chính xác. Chunk key
  `(work_id, unit_id, codec_version, digest)`; item dùng cùng work/unit trong FK thứ hai.
  Cho phép lặp digest tại ordinal khác nhau. Check ordinal bắt đầu từ 0 và liên tục đến
  count−1 tại seal bằng constraint deferred hoặc lệnh seal dưới row lock, kiểm count/sizes
  và digest trên thứ tự lắp ghép. Chặn update/insert/delete item sau seal bằng quyền ghi và
  trigger ở DB. Metadata snapshot cùng revision vẫn được bind theo composite FK tại owner
  hiện hành. Dùng version để đổi encoding, đọc thứ tự có `ORDER BY ordinal`.
- `publications` tiếp tục trỏ revision, `work_publications` giữ lifecycle/audience, generation,
  intent, pointer, hidden/withdrawn. Manifest sidecar thuộc revision. Capture đóng băng document,
  metadata, sidecar và receipt cùng transaction ở đúng draft/tag version, dưới current standing.
  Publish dưới thứ tự lock hiện hành (publications → work_publications → works → principal)
  kiểm lại intent/generation, exact revision, policy/decision version và authority, rồi đổi
  pointer và audit nguyên tử. Callback cũ hoặc rollback giữa chừng giữ pointer cũ. Retry cùng
  key chỉ trả receipt đã commit khi request khớp và standing còn hợp lệ.
- Read lấy access plan và đúng revision trong một câu truy vấn hoặc cùng MVCC snapshot, kiểm
  manifest completeness, work/unit scope, metadata và hash v1 trước khi serialize/ETag. Lỗi thiếu
  chunk, sai digest hoặc thứ tự trả lỗi theo contract. Request đã bắt đầu trước commit có thể dùng
  snapshot trước commit; request đọc sau commit thấy visibility mới. Thiếu payload không chuyển
  sang latest khác hoặc cho từng phần của hai revision vào cùng response.

### Visibility và cache

| Trạng thái | Contract cần giữ |
| --- | --- |
| Public hiện hành | `/publication` và `/reader` trả đúng representation riêng, strong ETag tính trên body response đã serialize, `private, no-cache`. Chỉ xét `If-None-Match` sau public eligibility + integrity. |
| Author-hidden từng public / review-withdrawn | `/reader` trả control 200 `no-store`, không title/manuscript/metadata, bình luận theo policy. `/publication` trả 404. |
| Chưa từng public / admin removal theo lifecycle | Origin không trả body/listing/snippet cũ, 404 `no-store` theo contract; validator cũ và `If-None-Match: *` không thắng access plan. |
| Tombstone do tác giả xóa | Target PUB-04/05: trang thay thế noindex, không body/metadata và chỉ bình luận đủ điều kiện. Xóa tác phẩm còn ở scope runtime riêng. |
| Private draft, history, audit | Giữ các route private có auth và `no-store`, giới hạn bởi role/mục đích hiện hành. Quyền công khai historical body cần quyết định sản phẩm. |

Bài revocable phải revalidate ở origin theo contract. `public,max-age=31536000,immutable` sẽ cho
client dùng bản fresh trong thời gian access có thể đổi. Dùng ETag cho đúng representation gồm
title, metadata, schema/render/encoding variant nếu chúng đổi bytes. Content ID v1 độc lập với
strong validator. [READ-01/02](../development/public-read-freshness.md),
[RFC 8246](https://www.rfc-editor.org/rfc/rfc8246.html).

#203 sở hữu cache public theo origin, schema/incarnation/epoch và retention. Allowlist chỉ envelope
đầy đủ của listing/publication. Giữ transaction data/cursor/metadata, row ≤512 KiB, 32 entry và
4 MiB logical UTF-8, clear/purge fences với response đến muộn, `validated_at` từ xác nhận của
origin. LRU chỉ quyết định eviction, tuổi xác minh giữ nguyên. Adapter có gate tắt. Proposal
24 giờ/expired qua restart còn `PENDING`, khác tuổi cho lần mount mới của RAM pilot (15 phút).
Thiết bị offline chỉ có snapshot đã tải với provenance chưa xác minh, nên việc thu hồi tại
origin và sự quan sát của thiết bị theo hai mốc khác nhau. Đổi cache key theo chunk, logout/đổi
tài khoản và UX hết hạn cần được đánh giá ở #203. [Policy và adapter](../development/public-read-freshness.md).

### Reachability và vận hành

PostgreSQL App sở hữu retention. Draft hiện hành, capture/backfill đã stage với lease, các
revision còn giữ, publication/intent/hidden/withdrawn/tombstone binding, moderation/job/appeal và
audit theo retention hiện hành đều là roots (trực tiếp hoặc qua revision). Hoàn tất một sửa/xóa
work không ngầm cho phép purge theo `O-08`. Sidecar/manifest giữ cùng lifecycle của revision,
chỉ release root khi lifecycle và policy cho phép. Dữ liệu của active snapshot đang đọc được giữ
nhờ MVCC, backup/restore giữ toàn bộ binding và payload.

GC đề xuất chỉ thu payload staging mồ côi hoặc không còn reference của bất kỳ root được giữ.
Lập batch giới hạn theo work, tuổi an toàn cho staging, số hàng, byte/WAL và thời gian lock.
Dùng cùng khóa work với capture/publish/backfill khi tạo reference, đọc lại reachability sau
khi lấy lock trong transaction với snapshot hợp lệ, và FK `RESTRICT` làm hàng rào cuối. Kiểm
audit/backup retention trước khi release root. Nếu publisher thắng, GC thấy reference đã commit
và bỏ qua. Nếu GC thắng, caller tải/ghi lại đúng payload trước khi tạo reference, hoặc fail
atomic, tuyệt đối không gắn pointer thiếu body. Crash trước commit rollback cả batch/lệnh, sau
commit replay theo receipt/checkpoint. GC job theo cùng owner vận hành App, với lag, reclaim,
failures, lock time và WAL được đo trước rollout.

### Verification trước rollout

| Ranh giới | Ca cần chạy nếu làm slice lưu mới |
| --- | --- |
| Direct SQL và upgrade | Trên cluster SQLx/runner có migrator thật, thử link khác work/unit, revision thuộc work khác, ordinal trùng/khuyết/sai count, mutate item sau seal, draft version/tag stale, payload đang tham chiếu bị xóa, thiếu payload/seal và legacy revision/hash/metadata NULL. Kiểm quyền split migrator/runtime, atomic rollback và backup/restore trên cả dữ liệu trước/sau migration. |
| Transaction hai connection | Dùng barrier/row lock cho hai thứ tự GC-vs-capture/publish, publish-vs-private/remove, read đang assemble vs pointer swap, late evaluator vs admin, stale intent/lease và lost ACK + retry. Assert single complete revision hoặc control, exact receipt một lần, quyền lại tại commit. Crash trước/sau insert payload, seal, pointer, commit và cleanup trong process thật. |
| API và browser | Old ETag và wildcard với public → hidden/private/withdrawn/removed và republish, hash/metadata/representation đổi, conditional GET/HEAD, listing/history access, cache clear/epoch cùng response muộn. So cùng manuscript selection/copy/find/scroll với 200% chữ, line breaks, SSR/hydration trên browser thật nếu chạm renderer. |
| Chi phí | Workload đại diện trên môi trường release, phân bố exact repeat và edit, cold/warm, read burst/concurrent writer, app/DB RSS, backup/restore và vacuum, tăng đĩa tạm, WAL/replica lag, overhead migration/GC. Đo bytes HTTP riêng nếu đề xuất protocol delta. |

Kiểm thử scratch chạy round-trip, đo footprint, chặn thiếu payload, trùng ordinal và xóa
payload có reference. Reader qua connection khác chỉ thấy revision sau commit. FK ghép,
cạnh tranh GC/publish và upgrade/backout trên schema ứng viên `NOT_RUN`.
[Runner PostgreSQL](../development/README.md).

## Neo, Reader và tìm kiếm

Schema v1 giữ nội dung và boundary, chưa có stable block IDs. Chọn giữ nguyên v1 cho storage.
Nếu cần annotation, dùng revision + quote/position ở scope riêng; anchor mơ hồ giữ unresolved.
Lựa chọn IDs cần format version và owner editor cho split/merge/move/copy/cut/undo. Chỉ giữ ID khi
qua được các ca paste trùng, di chuyển, tái tạo từ text, khổ thơ tiếng Việt NFC/NFD, dòng tab,
và cùng quote ở nhiều vị trí. Enter xuống dòng thơ giữ stanza theo quy tắc parser/editor.
Probe kiểm document/hash, chưa kiểm khôi phục neo editor.

`content-visibility` ảnh hưởng render, bytes tải vẫn theo response. Signal tĩnh chưa bảo đảm
SSR-only. Islands/virtualization và progressive network cần evidence về hydration, selection
qua boundary, scroll, copy, find và accessibility từ renderer thật. Web đã có số đo #229, thử
nghiệm #231 không chạy browser. CMP đang ở mức host diagnostic, native selection/layout cần thử
riêng nếu có consumer. FTS tương lai lấy public-current projection, một card/work, và gỡ
index/snippet theo visibility hiện hành. Chapter, FTS, offline, editor anchors và Reader split
có điều kiện mở lại riêng với policy, contract và evidence tương ứng.

## Rollout và rollback có điều kiện

A có thể khép RFC với JSONB hiện hành. Khi một candidate qua gate, tách ba slice có thể review:

1. **App + PostgreSQL:** thêm sidecar additive trong migration mới. Backfill batch restartable theo
   ID/version, giữ JSONB canonical, so body/metadata/hash v1 và đánh dấu checkpoint. Shadow
   verify read toàn vẹn khi backfill chạy. Dừng ở đây nếu khác hash, tăng lock/WAL/lag hoặc
   không đủ đĩa. Backup và thử restore trước khi đổi nguồn đọc.
2. **Capture và Reader:** dual-write trong cùng transaction, đọc shadow rồi canary theo scope
   cấu hình. JSONB đầy đủ còn được lưu trong suốt canary. Tắt read path mới và dual-write,
   quay về JSONB đã xác minh nếu lỗi, giữ nguyên pointer/receipt và audit. Không chọn lại
   đời revision khi backout. Lặp giao thức compatibility dưới rolling deploy.
3. **Retention:** chỉ xem xét bỏ bản JSONB song song sau khi hoàn tất parity, nghiệm thu
   retention/purge và kiểm restore. Từ thời điểm bỏ bản song song, rollback đòi hỏi materialize
   lại đầy đủ JSONB ở một migration có kiểm hash, hoặc restore + replay được kiểm chứng;
   một toggle không còn đủ. Đo trước chi phí đĩa, lock, WAL, replica, vacuum và thời gian
   khôi phục. GC chạy sau khi toàn bộ reader/writer cũ đã rời và root audit được xác minh.

| Checklist #231 | Bằng chứng của PR |
| --- | --- |
| AC-01 | Decisions ở base, PRD và source map đầu RFC. |
| AC-02/03 | Scratch PostgreSQL bốn layout, raw per-edit và test đối chứng lệch biên. |
| AC-04/05 | Hash v1 round-trip đã chạy; binding, immutability, fence và negative/race plan ở contract. |
| AC-06/07 | Visibility trước validator; retention roots, GC/rollback và cache #203 ở design. |
| AC-08 | Neo, Reader/CMP, FTS và điều kiện mở lại riêng, browser/native `NOT_RUN`. |
| AC-09 | Chọn A trong proposal `PENDING`, có raw, ngưỡng mở lại và đường backout. |
