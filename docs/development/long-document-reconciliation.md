# Đối soát Long Document #229–#231

**LOCAL GATES PASS:** sau phục hồi dung lượng, fmt, decision validate và full Quick
đã thực thi PASS tại `dcd36ba6883b49bf764124e08e5d78278a485e48`. Raw FAIL trước đó giữ nguyên; không đổi nhãn kết quả cũ.
[Raw và exit codes mới](evidence/long-document-reconciliation/resume-20261117T093542Z/results.json).
Merge/checklist/closure vẫn cần GitHub xác nhận; tài liệu ghim đánh giá trước merge.

Biên bản ngày 17/11/2026 tại **`develop@607ef83aa2824a16ec54a6f25bd10eb4c137d62f`**;
source Quick **`dcd36ba6883b49bf764124e08e5d78278a485e48`**; corpus/PG **`11925bfc3cfd901e6b5b7e2c46ec08fb8ee8957e`**, chỉ thêm regression corpus.
Đánh giá 24 AC theo nghĩa trong issue: baseline đo đạc (#229), metadata projection (#230),
RFC và prototype giới hạn (#231). Không dùng merge/checkmark làm bằng chứng thực thi.
Checklist và trạng thái GitHub sống tại các issue; tài liệu này ghim kết quả đối soát tại SHA trên.

Chủ repo đã merge checkpoint [#278](https://github.com/loveoverflowcom/vink/pull/278) khi Mac
mất kết nối và Quick cuối còn `UNKNOWN`. Sau phục hồi, raw xác nhận lượt đó **FAIL disk**;
lượt tiếp theo tại `afb8a09` cũng FAIL do strip hết đĩa tạo binary UI preview hỏng; test UI preview chưa thực thi. Lượt đầu trên `d352dba` gặp
strip hết đĩa ở App, tạo file data 3312 B; App unit tests chưa thực thi, repository checks NOT_RUN.
Follow-up này bổ sung bundle thật và verification mới trên develop đã có #277/#278/#279; giữ nguyên
PR #278 và checkpoint lịch sử, không biến merge trước đó thành gate PASS.

Kết luận trong phạm vi: **#229 7/7, #230 8/8, #231 9/9 AC đủ bằng chứng**. Có thể đóng sau khi
PR evidence follow-up được kiểm/merge và fresh-read issue không phát hiện thay đổi acceptance.
F1/F3 và lỗi re-review có owner riêng bên dưới. Không nghiệm thu production performance,
candidate storage schema hoặc policy đang `PENDING` qua việc đóng ba deliverable này.

## Tích hợp và độ mới của bằng chứng

GitHub đã xác nhận `merged=true`; cả bốn merge commit là ancestor của base đối soát:

| Issue | PR | Merge commit trong develop |
| --- | --- | --- |
| #229 | [#232](https://github.com/loveoverflowcom/vink/pull/232) | `a81333c71faf6033014a00ec26c9f7b90109d1d9` |
| #229 | [#249](https://github.com/loveoverflowcom/vink/pull/249) | `e632c352546ea7840fa59668c25c88e1fc7dfa56` |
| #230 | [#248](https://github.com/loveoverflowcom/vink/pull/248) | `f0ef35193b2bef2fc9ded9f66b1482a4557ac03e` |
| #231 | [#253](https://github.com/loveoverflowcom/vink/pull/253) | `eaa0348f8631af5add17609bf4c8c1406923042b` |

[Audit máy đọc được](evidence/long-document-reconciliation/audit.json) kiểm ancestry, checksum,
raw arrays và fingerprint source. Không thấy revert; delta owner và giới hạn được ghi rõ:

BASE đầu phiên là `6287fbfa2817fd457f3b93e710463a7a9cc7e84f`, nơi chạy corpus/focused PG và
reproducer đầu. Sau #276 là `f6b8846`; khi chủ repo merge #278, develop lên `d352dba`, gồm cả
#277 (Editor yên tĩnh). Nhánh follow-up mới xuất phát từ SHA này, không tái dùng nhánh PR đã merge.
Sau đó nhánh fast-forward lên `92b2eb3` khi #279 merge; không sửa source/hash của owner #255.
#277 bỏ export `.txt`, đổi UI/CSS/browser harness. #279 thêm source/hash v2 và reader DTO cô lập,
writer/API/fixtures sản xuất vẫn v1. `revision.rs` chia sẻ helper hex; luồng byte hash v1 và length
prefix giữ nguyên. Test corpus hiện có nay so **21 hash + toàn bộ kích thước/shape/envelope**
với raw #249 bất biến, thay vì chỉ so hai hash của cùng implementation. Ba hàm `text_to_document`,
`document_to_text`, `DocumentView` giữ byte identity; `editor.rs` chỉ đổi comment export.
Audit ghi whole-file khác biệt, helper/refactor và hash từng hàm; không gọi các file đó unchanged.

Tại `11925bf` (sau #280): **corpus3 PASS, PostgreSQL17 PASS / 2 opt-in ignored và Quick13 PASS**.
Sau #281, source `c2a3073` từng FAIL disk ở doctest; later SSR/unit/repository khi đó NOT_RUN.
Sau parent cleanup có bảo toàn evidence, source `dcd36ba` chạy mới **fmt, decision validate
và full Quick PASS**. Không lấy kết quả lịch sử làm PASS cho source này.
Backend toàn bộ `services`, codec/DTO/cache và lock/registry có OID giống hệt source focused;
reuse corpus/PG vẫn ghi source11925bf, không gán nhãn SHA mới. [Proof #281](evidence/long-document-reconciliation/upstream-281.json).
#280 Reader/Back và #281 UI chờ phiên được tích hợp nguyên; không sửa hoặc lấy browser metrics
của chúng làm evidence Long Document. Raw các source trước giữ riêng, kể cả compile lỗi assertion
mới tại f4e917b đã sửa tại c27286d. [Run-source](evidence/long-document-reconciliation/quick-run-source.json)
và [verification](evidence/long-document-reconciliation/verification.json) ghim từng gate.

- #229: validator, corpus generator, PgStore save, cache adapter và document-budget harness
  giống byte với source đo `4f288363`; hash v1 được kiểm bằng oracle lịch sử, các hàm Web giữ như trên.
  Save/retry/conflict vẫn giữ live buffer; export không còn là yêu cầu hiện hành sau #277.
  File benchmark chỉ đổi import sang
  `super::document_corpus` cùng corpus khi tích hợp #230; helper đo/reset/assertions giữ nguyên.
  Owner-state đã được #248 thay loader: các timing history cũ chỉ mô tả baseline trước tối ưu.
- #230: publication/discovery/moderation/evaluation/appeals, domain owners và hai test owners
  giống byte với HEAD #248 `7651416e`. HTTP API/provider-consumer DTO/fixtures v1 không đổi; contract README thêm sáu dòng PENDING v2 từ #279.
  Map tin cậy vẫn là một summary GET và bảy call sites full-content cho command/worker/appeal.
- #231: probe, corpus và proposal giống byte với HEAD #253 `041d5699`. Raw hai lượt giữ hash
  đã công bố; giới hạn binding/GC/upgrade của candidate vẫn được ghi `NOT_RUN`.
- Reader SSR shell đã có công việc khác sau các phép đo cũ. Quick có regression SSR hiện hành;
  điều này không biến browser timing lịch sử thành số đo tại HEAD mới. PR follow-up chỉ
  thêm oracle trong test hiện có và docs/evidence, không sửa renderer, HTTP, schema, transition hoặc contract.

## Ma trận #229

Nguồn chính: [ngân sách](long-document-budget.md), [raw v2](evidence/long-document-229/followup/README.md),
[review/mutation #249](evidence/long-document-229/review-249/README.md). Cột “Đủ đóng?” đánh giá
AC trong phạm vi baseline, không tuyên bố GitHub đã đóng hoặc mọi finding đã được sửa.

| AC_ID | Requirement | Implementation owner | Source path | Evidence/test | Status | Remaining gap | Action | Đủ đóng? |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| AC-01 | Corpus thật; scalar/UTF-8/block/line/envelope | App document/hash + test corpus | [document.rs](../../services/vink-app/src/domain/document.rs), [corpus](../../services/vink-app/tests/support/document_budget.rs) | Corpus tại 11925bf: 3 PASS; 21/21 byte/hash/shape bằng raw v1/v2 | PASS | Corpus tổng hợp, không đại diện production | Giữ seed 229 và provenance | Đủ scope; chờ merge |
| AC-02 | PG latency/WAL/heap/TOAST/index/history | PgStore + benchmark test | [postgres.rs](../../services/vink-app/src/persistence/postgres.rs), [probe](../../services/vink-app/tests/postgres/document_budget.rs) | Raw PG v2: 420 saves, history 1/10/100, EXPLAIN/LSN/sizes; 4 budget regressions tại 11925bf PASS | PASS baseline PG | Không đo lại performance ở HEAD; relation totals gồm reset/churn | Không dùng median v1 làm changed-save | Đủ scope; chờ merge |
| AC-03 | Tách network/SSR/hydration/DOM/memory/snapshot | Web Reader/editor | [DocumentView](../../apps/vink-web/src/document.rs), [CDP phase](../../apps/vink-web/e2e/document-budget.mjs) | Historical Chrome: 42 Reader + 84 editor scenes; navigation/SSR/resources/DOM/V8/getText/save riêng | PASS baseline Chrome | Browser mới, mobile/OS IME, total RSS/WASM peak NOT_RUN | Giữ bounds; F3 → #274 | Đủ scope; chờ merge |
| AC-04 | Default/pglz/LZ4, old/new rows | Scratch SQL compression probe | [compression_probe](../../services/vink-app/tests/postgres/document_budget.rs) | Raw fresh identical corpus, old pglz/new LZ4 rows, WAL/size/latency; checksum khớp | PASS baseline SQL | Không qualification adoption/rewrite production | Giữ default; không production DDL | Đủ scope; chờ merge |
| AC-05 | Biên, no truncation, conflict, chữ chưa lưu | Domain + HTTP/PgStore + editor | [pure boundaries](../../services/vink-app/tests/document_budget.rs), [HTTP regression](../../services/vink-app/tests/postgres/document_budget.rs) | Tại 11925bf: limit±1, valid oversized rejection/409 và exact durable before/after PASS; browser v2 giữ full buffer | PASS baseline/regression | F1 vẫn tồn tại, valid tiny_blocks200k chưa lưu được qua HTTP | #273 sửa transport; không giảm product limit | Đủ scope; chờ merge |
| AC-06 | Oversized cache miss/fallback, gate, không ACK giả | Public persistence #203 | [adapter](../../apps/vink-web/src/public_persistence.rs), [IndexedDB](../../apps/vink-web/public-cache/indexeddb.js) | Pure oversized tại 11925bf PASS; historical Rust/Memory/NoOp/IDB 800.554 B miss, không durable ACK; gate tắt | PASS adapter/gate | Offline rollout/policy chưa nghiệm thu | Reuse #203, giữ gate/proposal | Đủ scope; chờ merge |
| AC-07 | Report/raw/budgets/go-no-go/operations | Owning report + evidence | [report](long-document-budget.md) | Raw/checksum/source audit; thresholds đề xuất, TOAST/MVCC/rewrite/rollback bounds; findings có owner | PASS | Không SLA/mobile/production tuning | Sửa references stale; giữ JSONB có điều kiện | Đủ scope; chờ merge |

315 mẫu changed của v2 đều đổi hash trước/sau; 105 mẫu no-op giữ hash; mọi mẫu có exact durable
readback. Baseline v1 là workload thực nhưng phân loại 12/15 mẫu changed sai vì không reset.
Giữ raw v1, không dùng nó như năm edit độc lập. Hash canonical v1, NFC, khổ/dòng/tab và version
không đổi. Cache quota không trở thành giới hạn nội dung.

## Ma trận #230

Nguồn chính: [projection/trust map](publication-state-read.md),
[raw PostgreSQL](evidence/publication-state-230/README.md),
[review #248](evidence/publication-state-230/review-248.md).

| AC_ID | Requirement | Implementation owner | Source path | Evidence/test | Status | Remaining gap | Action | Đủ đóng? |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| AC-01 | Inventory read/command/moderation/integrity callers | App publication port + adapter | [publication.rs](../../services/vink-app/src/persistence/publication.rs) | Owner trust map; summary GET và 7 full-content call sites được đọc lại | PASS | Commands/worker vẫn tốn full history | Chỉ đo/tối ưu riêng nếu có evidence | Đủ scope; chờ merge |
| AC-02 | Owner-state không tải/parse/hash history body | Typed metadata projection | [STATE_REVISIONS_SQL/loader](../../services/vink-app/src/persistence/publication.rs), [test](../../services/vink-app/tests/postgres/publication_state.rs) | Tại 11925bf: role không SELECT document vẫn trả 106 history rows; invalid body schema không parse; runtime SQL 5 SELECT, NULL placeholder | PASS | Metadata/history vẫn O(N), một số validate O(N²) | Giữ đủ history, không silent cap | Đủ scope; chờ merge |
| AC-03 | Public+pending/rejected/private/withdrawn/bindings | Shared restore invariants | [publication domain](../../services/vink-app/src/domain/publication.rs), [moderation](../../services/vink-app/src/domain/moderation.rs) | Historical full PG + domain differential oracle; Tại 11925bf: snapshot race, private/late-result và metadata-only binding PASS | PASS | Re-review old approved/new policy là defect khác | #275; không dùng projection để cấp quyền | Đủ scope; chờ merge |
| AC-04 | Owner/privacy/no-store/standing/generation/receipt/304 | HTTP + command commit fences | [API](../../services/vink-app/src/http/api.rs), [conditional](../../services/vink-app/src/http/conditional.rs) | Historical full PG 200 tests trên runtime tương ứng; Tại 11925bf: foreign/missing owner, corruption/wildcard, private fences + 3 conditional tests PASS | PASS | Full lane không chạy lại trong reconciliation docs-only | Giữ current-access trước validator, standing trước receipt | Đủ scope; chờ merge |
| AC-05 | Integrity đúng boundary, corruption/cross-work/late result | Full-content loader + public exact-body hash | [discovery](../../services/vink-app/src/persistence/discovery.rs), [state regressions](../../services/vink-app/tests/postgres/publication_state.rs) | Tại 11925bf: corruption/schema/hash/command rollback, FK/job digest, private late-approval PASS; commands/worker/appeals vẫn full loader | PASS | Summary không audit bit rot trong history body | Giữ boundary có chủ ý, không bỏ guard | Đủ scope; chờ merge |
| AC-06 | History/receipt/audit và compatibility không mất | Metadata response v1 | [contract](../../services/vink-app/contracts/http/v1/README.md), [state test](../../services/vink-app/tests/postgres/publication_state.rs) | Tại 11925bf: 106 rows/ties/legacy NULL; historical 300 rows; DTO/fixtures unchanged; không migration/delete/truncate | PASS | Metadata lớn chưa pagination | Tách contract/cursor nếu đo thấy cần | Đủ scope; chờ merge |
| AC-07 | Body size × revision count, before/after PG | Existing opt-in test owner | [publication_state_budget.rs](../../services/vink-app/tests/postgres/publication_state_budget.rs) | 13 checkpoint mỗi source; 30k/200k × 1/10/100, thêm 300 revisions; SQL/EXPLAIN/logical bytes và raw medians tính lại | PASS historical measurement | Không đo lại latency HEAD/wire/RSS/production | Chỉ trích số lịch sử có nguồn | Đủ scope; chờ merge |
| AC-08 | Local regressions thật, provenance và bounds | App/PG + verification owner | [new run bundle](evidence/long-document-reconciliation/README.md) | Tại 11925bf: 6 projection + 3 conditional regressions PASS; historical full PG/Quick; current commands/source/target trong bundle | PASS | Browser/production/model riêng NOT_RUN | Không chương/CAS/offline/provider | Đủ scope; chờ merge |

**4.386,58 ms → 1,647 ms** là median warm lịch sử ở Mac M4 dev, 200k Unicode scalars × 100
revision, source trước `20ed13e` + harness và sau `0147bda`. Không gọi đó là phép đo HEAD mới,
p95, SLA hay wire bytes. Projection vẫn dùng cùng restore invariants, không là capability của
`Publication`/`WorkModeration`. Public reader recompute hash của đúng revision trả ra trước ETag.

## Ma trận #231

Nguồn chính: [RFC](../architecture/content-storage-rfc.md),
[phương pháp/raw](evidence/content-storage-231/README.md). Đánh giá RFC + bounded prototype;
production binding/GC/migration không nằm trong deliverable này.

| AC_ID | Requirement | Implementation owner | Source path | Evidence/test | Status | Remaining gap | Action | Đủ đóng? |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| AC-01 | Current code/decisions, không ngầm chốt chapter/CAS/claims | RFC + decision registry | [RFC](../architecture/content-storage-rfc.md), [proposal](../../decisions/proposals/VDR-CONTENT-STORAGE.r1.toml) | Current CLI snapshot; USER-09/USER-02/ADM-06 giữ status, storage current=null/PENDING; source map khớp | PASS RFC | Cần owner signoff nếu muốn accept proposal | Không tự signoff | Đủ scope; chờ merge |
| AC-02 | So A/B/C với corpus/cost hoặc đúng NOT_RUN | Scratch storage probe | [content_storage_rfc.rs](../../services/vink-app/tests/postgres/content_storage_rfc.rs) | Audit 768 raw samples/2 runs, 4 corpus × 4 layouts × 24 revisions; relation/WAL/prep+commit/read thống kê lại | PASS bounded probe | Release/representative workload/per-layout RSS NOT_RUN | Chọn đề xuất A, chưa production qualification | Đủ scope; chờ merge |
| AC-03 | Đầu/giữa/cuối/đúng biên/delete/reorder | Pure chunk controls + histories | [boundary test](../../services/vink-app/tests/postgres/content_storage_rfc.rs) | Boundary_shift tại 11925bf PASS, gồm inner/exact insertion, delete/reorder/oversized/repeated; histories có undo và edits | PASS | Reuse trên synthetic corpus không dự đoán manuscript thật | Không suy 95% hoặc mọi edit đều 0% | Đủ scope; chờ merge |
| AC-04 | Payload khác revision/metadata/moderation/Thấm/appeal | RFC identity contract + v1 hash | [revision.rs](../../services/vink-app/src/domain/revision.rs), [RFC identity](../architecture/content-storage-rfc.md#danh-tính-và-sql-binding) | Raw exact document/hash across layouts; V1 corpus tại 11925bf; scoped work/codec/digest và auth/compatibility được mô tả | PASS RFC | B/C chưa runtime | Giữ IDs/hash v1, không dedup business decisions | Đủ scope; chờ merge |
| AC-05 | Composite binding/seal/atomic pointer/fences và SQL/race plan | RFC + bounded scratch constraints | [RFC verification](../architecture/content-storage-rfc.md#verification-trước-rollout), [probe constraints](../../services/vink-app/tests/postgres/content_storage_rfc.rs) | Scratch constraints/commit visibility tại 11925bf PASS; RFC có same-work/unit/revision, ordinal/seal, intent/standing/receipt và two-connection plan | PASS design/probe bounds | Candidate composite FK/GC/public pointer/upgrade qualification NOT_RUN | Không coi scratch test là production PASS | Đủ scope; chờ merge |
| AC-06 | Visibility trước cache/304; public/private/history | Existing read owner + RFC | [visibility/cache](../architecture/content-storage-rfc.md#visibility-và-cache) | RFC đủ state/control/404/no-store; Conditional/corruption/private tests tại 11925bf PASS | PASS RFC | Public historical access policy chưa được chốt | Không immutable-year/hash-only auth | Đủ scope; chờ merge |
| AC-07 | Retention roots/GC/crash races, reuse #203 | App retention + cache owner | [reachability](../architecture/content-storage-rfc.md#reachability-và-vận-hành) | Roots/lease/work-lock/FK RESTRICT/crash/retry/rollback plan; quota/epoch/incarnation/age và disabled gate giữ nguyên | PASS RFC | O-08 OPEN; GC implementation/races NOT_RUN | Không bật CAS/offline hoặc store song song | Đủ scope; chờ merge |
| AC-08 | Anchors/SSR/hydration/CMP/FTS phân scope trung thực | RFC scope + current Web owners | [anchors/Reader/search](../architecture/content-storage-rfc.md#neo-reader-và-tìm-kiếm) | V1 không stable IDs; ambiguity unresolved; browser/native anchors/selection/FTS bounds ghi rõ | PASS RFC | Browser/CMP/annotation/search candidate NOT_RUN | Không đổi chương/native/search requirements | Đủ scope; chờ merge |
| AC-09 | Lựa chọn/raw/gates/rollback; PENDING đúng signoff | RFC + registry | [rollout/backout](../architecture/content-storage-rfc.md#rollout-và-rollback-có-điều-kiện), [current query](evidence/long-document-reconciliation/decisions.json) | A đề xuất, raw checksum và gates B/C, additive/backfill/dual-read/JSONB fallback/reconstruction plan; current=null | PASS RFC | Signoff và production adoption không được cấp | Đóng deliverable RFC vẫn giữ PENDING | Đủ scope; chờ merge |

Audit tính p95 nearest-rank của **24 thao tác hỗn hợp** sau khi cộng prep+commit từng sample.
Không cộng hai percentile, không gọi là distribution production. Ở corpus Việt nhiều đoạn 190k,
C giảm relation/WAL mạnh nhưng p95 ghi 25,20/19,41 ms vượt A 9,05/9,62 ms ở hai lượt dev.
Scratch constraint test chỉ chứng minh schema probe/commit visibility đã chạy; kế hoạch FK ghép,
GC-vs-publish và upgrade/backout của schema ứng viên vẫn là kế hoạch.

Mở lại B khi exact repeats thực trong cùng work, sau nén và overhead, giảm ≥20% relation/WAL.
Mở lại C khi giảm ≥30% so phương án đơn giản nhất và p95 đọc/ghi trong 10% baseline hoặc có
budget đích chấp nhận đánh đổi rõ. Các ngưỡng kỹ thuật tạm của RFC cần workload đại diện/release,
integrity, vận hành và quyết định đúng revision trước adoption. Không mở lại WR-04 từ benchmark.

## Findings còn tồn tại và owner

Inventory GitHub đã phân trang đủ **272 issue/PR open+closed** trước create; đã đọc candidate
#150/#186/#238/#264. Không có issue riêng cùng outcome cho ba finding. Các follow-up đã tạo và
fresh-read/bind/readback, mỗi issue có reproducer/source map, CHOICE, acceptance và local gates:

| Finding | Evidence hiện hành | Owner và hành động |
| --- | --- | --- |
| F1: valid tiny_blocks200k > HTTP 2 MiB | HTTP regression tại 11925bf PASS khi xác nhận 422 và durable rollback; pure validator chấp nhận >3,2 MB document | [#273](https://github.com/loveoverflowcom/vink/issues/273), App HTTP/contract + Web save; upper-bound/finite route budget, không giảm product limit |
| F3: Reader có thể dựng 200k paragraph | DocumentView/corpus unchanged; raw Chrome/SSR lịch sử xác nhận cost, browser mới NOT_RUN | [#274](https://github.com/loveoverflowcom/vink/issues/274), Web Reader/render; release/target budget và selection/find/copy/a11y; #238 chỉ vùng phụ, #264 format parity |
| Approved → private → policy mới → request-public revision cũ | Desired-behavior test tại 6287fbf **FAIL 101**; PostgreSQL ghi `work_revision_moderation_pending_check`; test tạm đã phục hồi byte-identical | [#275](https://github.com/loveoverflowcom/vink/issues/275), App publication/moderation + migration compatibility; migration additive, không nới guard/reset version |

Finding #275 có trước projection **theo source**: method `decide_author_with` và applied migration
giống byte với `20ed13e` trước #248. Full flow trên baseline cũ vẫn `NOT_RUN`; runtime failure
mới ghim ở 6287fbf; source transition/schema không đổi đến 607ef83. F2 (owner-state full history body) đã được #248 xử lý; metadata
O(N)/validation O(N²) và full-content command cost vẫn có giới hạn rõ, chưa có evidence buộc
thêm cursor/framework trong reconciliation.

## Verification, quyết định và rollback

Commands, raw output, environment/source và từng `PASS/FAIL/NOT_RUN` nằm tại
[bundle reconciliation](evidence/long-document-reconciliation/README.md). Known-policy failure
được giữ riêng, không đổi thành PASS và không phải regression do oracle/docs này tạo ra. Không chạy lại
benchmark khi workload/implementation tương ứng không đổi. Không đọc, chờ hoặc rerun GitHub CI.

CLI tại source kiểm có snapshot
`8354df2cde868013df8324e1934351d094f93739728e485c4f59a1e1d46462b8` (127 revisions/88 refs);
snapshot trước #279 `9dbad856…` giữ riêng:
`USER-09@r1`, `USER-02@r1`, `ADM-06@r1` vẫn `PRODUCT_CONFIRMED`; `WR-04 DEFERRED` theo PRD;
`D-02-CONTENT@r1 DESIGN_PROPOSED`; `O-08@r1 OPEN`, `O-17@r2 PRODUCT_CONFIRMED`.
`VDR-CONTENT-STORAGE@r1` **PENDING/current=null**, payload
`4dfde6cc9fb3bae44badbafc05d19ad7ec5edfd97ccb0336badc0ded6f533f41`.
`VDR-DOCUMENT-SOURCE-FORMAT@r1` cũng **PENDING/current=null**, payload
`ac39f4df098244634e78d76f6b338d4689cb087bf9ad9abd4c50a8ce2b91683a`. Writer v2 chưa mở;
raw/probe storage ở đây thuộc v1. Nếu writer/encoding đổi về sau, phải đo lại gates B/C trên
workload đó trước adoption, không dùng timing v1 làm qualification v2.
Hai proposal public cache/offline cũng `PENDING`. Không thêm signoff, không đổi registry,
Quint/Kani SSOT, policy hoặc transition để tạo vẻ hoàn tất.

Rollback PR reconciliation là revert oracle test và docs/evidence; không có production migration, deployment,
content change hoặc data backfill cần hoàn tác. Giữ nguyên raw/source lịch sử và ba issue
follow-up. Mỗi issue gốc chỉ được tick/close sau fresh-read, comment evidence và GitHub readback;
không dùng `Closes #...` để thay quy trình nghiệm thu.
