# Đối soát Long Document #229–#231

**Checkpoint 17/11/2026; PR draft, chưa final nghiệm thu.** Mac mất kết nối lúc full Quick đang chạy unit/contract. Kết quả Quick cuối **UNKNOWN**, không merge hoặc đóng issue. Raw mới đã ghi ở worktree Mac nhưng chưa upload/đọc lại đầy đủ; xem [execution checkpoint](evidence/long-document-reconciliation/checkpoint.json) và [cách tiếp tục](evidence/long-document-reconciliation/README.md).

BASE đầu `6287fbfa2817fd457f3b93e710463a7a9cc7e84f`; đã fresh-fetch và fast-forward worktree riêng lên **`f6b88467c53a82c39e95ac79329cd22d7242212c`** sau #276. Delta #276 là Empty State Web/CMP/CSS/e2e, không đổi App document/hash/publication/HTTP/test corpus hoặc Web DocumentView/editor/cache owners. Tests mới chạy tại 6287fbf; reuse theo input fingerprints, không đổi SHA của lần chạy. Những đánh giá PASS dưới đây có boundary riêng; không đại diện full Quick PASS.

## Tích hợp và source

| Issue | PR GitHub xác nhận merged | Merge commit là ancestor của develop đã audit |
| --- | --- | --- |
| #229 | [#232](https://github.com/loveoverflowcom/vink/pull/232) | a81333c71faf6033014a00ec26c9f7b90109d1d9 |
| #229 | [#249](https://github.com/loveoverflowcom/vink/pull/249) | e632c352546ea7840fa59668c25c88e1fc7dfa56 |
| #230 | [#248](https://github.com/loveoverflowcom/vink/pull/248) | f0ef35193b2bef2fc9ded9f66b1482a4557ac03e |
| #231 | [#253](https://github.com/loveoverflowcom/vink/pull/253) | eaa0348f8631af5add17609bf4c8c1406923042b |

Source inspection/audit không thấy revert: #229 validator/hash/corpus/PgStore/DocumentView/editor/cache/harness byte-identical với source đo 4f288363; benchmark chỉ đổi import sang cùng document_corpus khi #230 tích hợp. #230 runtime/test owners, API/contract/DTO byte-identical với HEAD #248 7651416e. #231 probe/corpus/proposal byte-identical với HEAD #253 041d5699. Reader SSR shell có thay đổi ở PR khác: giữ browser timing cũ ở source lịch sử, không gọi đó là performance ở HEAD mới. Full fingerprint JSON còn chờ upload từ Mac.

## #229 — baseline

[Acceptance gốc](https://github.com/loveoverflowcom/vink/issues/229), [report](long-document-budget.md), [raw v2](evidence/long-document-229/followup/README.md), [review/mutation](evidence/long-document-229/review-249/README.md).

| AC_ID | Requirement | Implementation owner | Source path | Evidence/test | Status | Remaining gap | Action | Can close? |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| AC-01 | Corpus thật: scalar/UTF-8/block/line/envelope | App document/hash + corpus | [document.rs](../../services/vink-app/src/domain/document.rs), [corpus](../../services/vink-app/tests/support/document_budget.rs) | 3 pure tests PASS tại 6287fbf; 21/21 raw byte/hash/shape equality | PASS baseline | Synthetic, không đại diện production | Giữ seed 229/source | Chờ gate/merge |
| AC-02 | PG latency/WAL/heap/TOAST/index/history | PgStore + benchmark | [probe](../../services/vink-app/tests/postgres/document_budget.rs) | V2 420 saves + history1/10/100 + plans/LSN/sizes; 4 current budget regressions PASS | PASS baseline PG | Không benchmark lại HEAD; reset/churn trong totals | Không dùng median v1 làm changed-save | Chờ gate/merge |
| AC-03 | Network/SSR/hydration/DOM/memory/snapshot tách riêng | Web Reader/editor | [DocumentView](../../apps/vink-web/src/document.rs), [CDP](../../apps/vink-web/e2e/document-budget.mjs) | Raw Chrome lịch sử:42 Reader/84 editor scenes, payload/DOM/V8/getText/save | PASS baseline Chrome | Browser mới/mobile/OS IME/RSS-WASM NOT_RUN | F3 → #274; không suy SLA | Chờ gate/merge |
| AC-04 | Default/pglz/LZ4 và old/new rows | Scratch SQL compression | [probe](../../services/vink-app/tests/postgres/document_budget.rs) | Fresh identical corpus, old pglz/new LZ4, WAL/size/latency; checksum khớp | PASS baseline SQL | Production adoption/rewrite NOT_RUN | Giữ default, không DDL production | Chờ gate/merge |
| AC-05 | Boundary/no truncation/conflict/chữ chưa lưu | Domain/HTTP/editor | [pure](../../services/vink-app/tests/document_budget.rs), [HTTP](../../services/vink-app/tests/postgres/document_budget.rs) | Limit±1/422/409/exact durable readback PASS tại 6287fbf; historical full buffer check | PASS baseline | F1 valid tiny_blocks200k còn không HTTP-save được | HTTP budget → #273; không giảm limit | Chờ gate/merge |
| AC-06 | Oversized cache fallback/gate, không ACK giả | Public persistence #203 | [adapter](../../apps/vink-web/src/public_persistence.rs), [IDB](../../apps/vink-web/public-cache/indexeddb.js) | Pure oversized PASS; historical 800.554 B miss trong Rust/Memory/NoOp/IDB; gate tắt | PASS adapter/gate | Không nghiệm thu offline rollout | Reuse #203, giữ PENDING/gate | Chờ gate/merge |
| AC-07 | Report/raw/budgets/go-no-go/operations | Owning report/evidence | [report](long-document-budget.md) | 51 checksums; source/workload audit; thresholds và rollback bounds, findings có owner | PASS report | Không production tuning/SLA | Sửa stale refs, giữ JSONB có điều kiện | Chờ gate/merge |

V2 có 315 changed saves đổi hash và 105 no-op giữ hash, mỗi sample reset ngoài timer/WAL rồi so full content/title/version sau commit. Title cố định trong helper. V1 là output thật của workload bị phân loại sai 12/15 changed samples, không dùng median đó như 5 edit độc lập. Hash v1/NFC/block-line/version và disabled persistence gate giữ nguyên. F1/F3 là follow-up, không mở rộng hồi tố acceptance baseline.

## #230 — metadata projection

[Acceptance gốc](https://github.com/loveoverflowcom/vink/issues/230), [trust map/compatibility](publication-state-read.md), [raw](evidence/publication-state-230/README.md), [review #248](evidence/publication-state-230/review-248.md).

| AC_ID | Requirement | Implementation owner | Source path | Evidence/test | Status | Remaining gap | Action | Can close? |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| AC-01 | Inventory owner-state/shared callers/trust boundaries | App ports/adapter | [publication](../../services/vink-app/src/persistence/publication.rs) | Một summary GET + 7 full-content call sites kiểm lại; tách dashboard #214 | PASS source | Commands/worker vẫn full history | Đo riêng khi có evidence | Chờ gate/merge |
| AC-02 | Owner-state không materialize/hash history body | Typed metadata projection | [SQL/loader](../../services/vink-app/src/persistence/publication.rs), [tests](../../services/vink-app/tests/postgres/publication_state.rs) | Role không SELECT document vẫn trả106 rows, corrupt schema không parse; runtime SQL5 SELECT | PASS PG | Metadata O(N), vài validator O(N²) | Giữ full history, không cap ngầm | Chờ gate/merge |
| AC-03 | Public+pending/rejected/private/withdrawn/binding | Shared restore invariants | [publication](../../services/vink-app/src/domain/publication.rs), [moderation](../../services/vink-app/src/domain/moderation.rs) | Historical full PG/domain oracle; snapshot race/private-late/metadata-only tests PASS tại 6287fbf | PASS projection | Policy re-review approved cũ là defect riêng | Theo #275, không cấp authority qua summary | Chờ gate/merge |
| AC-04 | Owner/privacy/no-store/standing/generation/receipt/304 | HTTP + commit fences | [API](../../services/vink-app/src/http/api.rs), [conditional](../../services/vink-app/src/http/conditional.rs) | Historical full PG200; current foreign/missing/corrupt-wildcard/private +3 conditional tests PASS | PASS boundaries | Full lane không chạy lại | Giữ current visibility trước validator | Chờ gate/merge |
| AC-05 | Integrity đúng boundary/corruption/cross-work/late-result | Full loader + exact public hash | [discovery](../../services/vink-app/src/persistence/discovery.rs), [tests](../../services/vink-app/tests/postgres/publication_state.rs) | Corrupt body/schema/hash rollback, FK/job digest/private-late PASS; commands/worker/appeals full loader | PASS PG/source | Summary không audit body bit rot | Giữ trust map/guards | Chờ gate/merge |
| AC-06 | Full history/receipt/audit/compatibility | Metadata response v1 | [contract](../../services/vink-app/contracts/http/v1/README.md), [tests](../../services/vink-app/tests/postgres/publication_state.rs) | 106 rows/ties/legacyNULL current PASS;300 revisions historical; DTO unchanged; không migration/delete | PASS compatibility | History còn tăng, chưa cursor | Tách cursor khi có cost/contract cần | Chờ gate/merge |
| AC-07 | PG body-size×revision before/after | Existing measurement owner | [budget](../../services/vink-app/tests/postgres/publication_state_budget.rs) | 13 checkpoints/source:30k/200k×1/10/100 +300 revisions; SQL/plans/logical bytes/raw medians | PASS historical measurement | HEAD latency/wire/RSS/production NOT_RUN | Không gán 4386.58→1.647 ms cho HEAD mới | Chờ gate/merge |
| AC-08 | Local regressions, SHA/command/target/bounds | App/PG/verification | [checkpoint](evidence/long-document-reconciliation/checkpoint.json) | 6 projection +3 conditional tests PASS tại6287fbf; historical fullPG/Quick; current source equality | PASS focused; gate pending | Final Quick UNKNOWN; raw upload pending | Khôi phục Mac trước final nghiệm thu | Chờ gate/merge |

4386.58 ms → 1.647 ms là historical local-dev median warm của 200k scalars ×100 revisions (20ed13e+harness →0147bda), không là phép đo HEAD mới/p95/SLA/wire bytes. Summary dùng shared restore invariants, không chuyển thành Publication/WorkModeration authority. Commands/worker/appeals vẫn full-content tại commit boundaries; public read kiểm exact revision/hash trước ETag. Re-review defect ngoài projection được giữ FAIL ở #275.

## #231 — RFC và bounded prototype

[Acceptance gốc](https://github.com/loveoverflowcom/vink/issues/231), [RFC](../architecture/content-storage-rfc.md), [raw/method](evidence/content-storage-231/README.md).

| AC_ID | Requirement | Implementation owner | Source path | Evidence/test | Status | Remaining gap | Action | Can close? |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| AC-01 | RFC khớp code/decisions, không chốt chapter/CAS/claims | RFC + registry | [RFC](../architecture/content-storage-rfc.md), [proposal](../../decisions/proposals/VDR-CONTENT-STORAGE.r1.toml) | Current snapshot9dbad856; USER/ADM statuses; storage current=null/PENDING | PASS RFC | Signoff/adoption chưa được cấp | Giữ PENDING | Chờ gate/merge |
| AC-02 | A/B/C corpus/cost hoặc NOT_RUN đúng | Scratch storage probe | [probe](../../services/vink-app/tests/postgres/content_storage_rfc.rs) | 768 raw samples/2runs:4corpus×4layouts×24revisions, relation/WAL/read/prep+commit tính lại | PASS bounded probe | Release/representative/RSS-layout NOT_RUN | Đề xuất A, không production qualification | Chờ gate/merge |
| AC-03 | Đầu/giữa/cuối/đúng biên/delete/reorder/undo | Chunk controls/histories | [boundary test](../../services/vink-app/tests/postgres/content_storage_rfc.rs) | Current boundary_shift PASS; insertion/exact-inner boundary/delete/reorder/oversized/repeated + historical undo | PASS controls | Synthetic reuse không dự báo manuscript thật | Không suy95% hoặc mọi edit0% | Chờ gate/merge |
| AC-04 | Payload khác revision/metadata/moderation/Thấm/appeal | RFC identity + v1 hash | [hash](../../services/vink-app/src/domain/revision.rs), [bindings](../architecture/content-storage-rfc.md#danh-tính-và-sql-binding) | Exact document/hash across layouts; current v1 corpus; scope work/codec/digest/auth/compatibility mô tả | PASS RFC/hash | B/C runtime chưa có | Giữ IDs/hash v1, không reuse quyết định từ bytes | Chờ gate/merge |
| AC-05 | Composite/seal/atomic pointer/fences và SQL/race plan | RFC + bounded constraint probe | [verification](../architecture/content-storage-rfc.md#verification-trước-rollout) | Current scratch constraints/commit visibility PASS; same-work/unit/revision/seal/receipt/standing và race plan đủ | PASS design/probe bounds | Candidate FK ghép/GC/public-pointer/upgrade NOT_RUN | Không biến plan thành production PASS | Chờ gate/merge |
| AC-06 | Visibility trước cache/304, private/history | Existing read owner + RFC | [cache](../architecture/content-storage-rfc.md#visibility-và-cache) | State/control/404/no-store design; current conditional/corruption/private guards PASS | PASS RFC | Public historical policy còn mở | Không immutable-year/hash-only auth | Chờ gate/merge |
| AC-07 | Retention roots/GC/crash races, reuse #203 | App retention/cache owner | [reachability](../architecture/content-storage-rfc.md#reachability-và-vận-hành) | Roots/lease/work-lock/FK RESTRICT/crash/retry plan; quota/epoch/incarnation/age/gate đúng | PASS RFC | O-08 OPEN; GC implementation/races NOT_RUN | Không CAS/offline/store song song | Chờ gate/merge |
| AC-08 | Anchors/SSR/hydration/CMP/FTS scope/bounds | RFC + Web owners | [anchors](../architecture/content-storage-rfc.md#neo-reader-và-tìm-kiếm) | V1 không stable IDs; ambiguity unresolved; renderer/selection/native/search qualification tách rõ | PASS RFC | Browser/CMP/annotation/FTS candidate NOT_RUN | Không mở WR-04/native/search | Chờ gate/merge |
| AC-09 | Lựa chọn/raw/gates/rollback, PENDING đúng signoff | RFC + registry | [rollout/backout](../architecture/content-storage-rfc.md#rollout-và-rollback-có-điều-kiện) | A proposal; raw checksum, B/C gates; additive/backfill/JSONB fallback/reconstruction; current=null | PASS RFC | Local final gate/raw upload chưa xong | Đóng RFC không accept architecture | Chờ gate/merge |

Raw storage: 768 samples, hash/document đúng giữa layouts; payload=new+reused, mỗi layout24 revisions. P95 nearest-rank tính sau khi cộng prep+commit từng sample của 24 thao tác hỗn hợp. C/190k nhiều đoạn: p95 ghi25.20/19.41 ms, A9.05/9.62 ms ở hai lượt dev; không distribution production. Scratch SQL constraints/commit visibility không chứng minh production binding/GC/upgrade.

Mở lại B khi exact repeats thật scoped work giảm ≥20% relation/WAL sau nén/overhead. Mở lại C khi giảm ≥30% so phương án đơn giản nhất và p95 đọc/ghi trong10% baseline hoặc có target budget chấp nhận đánh đổi. Cả hai cần representative release workload, integrity/operations và signoff đúng revision trước adoption. Không mở lại WR-04 từ benchmark.

## Findings và decisions

Inventory phân trang đủ272 issue/PR open+closed trước create, đã đọc #150/#186/#238/#264. #238 chỉ vùng phụ Reader; #264 format parity. Ba follow-up đã tạo, bind URL và readback:

| Finding còn tồn tại | Evidence/boundary | Owner/action |
| --- | --- | --- |
| F1: valid tiny_blocks200k >HTTP2MiB | HTTP422/durable preservation regression hiện hành PASS; domain JSON3.200.031 B hợp lệ | [#273](https://github.com/loveoverflowcom/vink/issues/273), App HTTP/contract + Web save; finite transport budget, không hạ product limit |
| F3: 200k paragraphs | Current DocumentView map mọi block; historical Chrome/SSR xác nhận cost; new browser NOT_RUN | [#274](https://github.com/loveoverflowcom/vink/issues/274), Web article/render budget, selection/find/copy/a11y |
| Approved→private→policy mới→request-public revision cũ | Desired test FAIL101 tại6287fbf, PostgreSQL pending constraint; temp test restored byte-identical | [#275](https://github.com/loveoverflowcom/vink/issues/275), publication/moderation + additive migration/compatibility |

#275 pre-existing **theo source**: author transition và migration byte-identical với20ed13e trước #248; baseline full-flow NOT_RUN. Không reset version hoặc nới guard để xanh. F2 đã được #248 xử lý; metadata O(N)/validation O(N²) và full-content command cost vẫn được giới hạn claim.

Registry snapshot `9dbad85666397d5d9b76d811440fc80ecf12f034328dea0ec7c22114e3596cb3`: USER-09@r1/USER-02@r1/ADM-06@r1 PRODUCT_CONFIRMED, WR-04 DEFERRED (PRD), O-08@r1 OPEN, O-17@r2 PRODUCT_CONFIRMED, D-02-CONTENT@r1 DESIGN_PROPOSED. **VDR-CONTENT-STORAGE@r1 PENDING/current=null**, payload `4dfde6cc9fb3bae44badbafc05d19ad7ec5edfd97ccb0336badc0ded6f533f41`. Public cache/offline proposals cũng PENDING. Không signoff, model/registry/production semantics change.

## Trạng thái bàn giao

24 AC được map tới evidence trong phạm vi; global final gate và full raw upload chưa xác nhận nên chưa thực hiện checklist/closure delta cho #230/#231, không gọi reconciliation MERGED/CLOSED. #229 giữ7 checkmarks đã hợp lệ; #230/#231 giữ checklist hiện hữu tới bước fresh-read/merge/readback sau verification. Không dùng Closes trong PR.

Rollback checkpoint chỉ revert docs/summary; không migration/data/deploy. Khi Mac trở lại, kiểm process/dirty state/raw Quick trước khi rerun hoặc clean, khôi phục local docs/evidence và cập nhật **cùng PR/branch**. Không reset/stash/force-push công việc khác, không tạo PR thứ hai, không chờ/rerun CI. Chỉ merge sau gates PASS, rồi xác nhận merge SHA, fresh-read issue/AC, tick/comment/close/readback đúng scope. Defect #273–#275 không tự trở thành blocker vô hạn của ba deliverable gốc.
