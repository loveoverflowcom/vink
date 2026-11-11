# Publication intent core — PUB-CORE-01 (#150)

**Phạm vi:** Rust thuần + application planning seam, chưa publication runtime. Nguồn luật là
AUD-01/02, PUB-01/02/05, MOD-05 và INV-01/02/07/08 trong [PRD](../../prd/requirements.md),
[đặc tả](../architecture/publication-moderation.md), [DecisionDelta](../architecture/product-decisions-20261110.md).
Không đổi D-06/D-07/D-12 thành Accepted và không quyết định các điểm OPEN.

## Owner và lựa chọn

- **CHOICE-01 B:** aggregate `Publication`, private fields, `restore(PublicationFacts)` fallible.
  Dữ liệu được kiểm work/revision, duplicates, intent/generation, approval/pointer và terminal
  consistency trước khi dùng. Đánh đổi: restoration API lớn hơn hàm facts đơn, đổi lại caller
  không thể thay state bằng field setter hoặc Deserialize. Restore kiểm cấu trúc, không chứng
  minh authenticity/audit của một adapter cung cấp facts.
- **CHOICE-02 B:** `application::publication::plan_publication` là owner-local use case nhỏ,
  xác thực bound principal/standing rồi gọi `Publication::decide_author`. Có test caller thật cho
  r1/r2/private/stale. Chưa cần thêm command port khi chưa có persistence consumer; không đổi
  public-read port `PublicationStore` hiện hữu và không có generic save-state API.
- **CHOICE-03 B:** chuyển canonical M1 về `behavior/publication`, giữ toàn bộ scenario và hai
  negative controls; runner chung và pin Quint không đổi. Replay subset theo bảng action trong
  [owner README](../../services/vink-app/behavior/publication/README.md), không claim whole model.

Baseline kỹ thuật được chọn, **không nâng thành PRODUCT_CONFIRMED**: tạo private ở generation 1;
accepted audience command luôn tăng generation kể cả reaffirmation; stale luôn từ chối; overflow
không plan. Inactive lifecycle chỉ là facts đầu vào, không có lệnh khôi phục/xóa/gỡ ở PR này.

## API tối thiểu cho #151

Source: [`domain/publication.rs`](../../services/vink-app/src/domain/publication.rs) và
[`application/publication.rs`](../../services/vink-app/src/application/publication.rs).

| API/type | Contract |
| --- | --- |
| `Snapshot` | `WorkId` + `RevisionId` + canonical `RevisionContent`; hash v1 giữ nguyên |
| `PublicationFacts` / `Publication::restore` | Audience, lifecycle, generation, exact intent, public pointer với generation đã cho phép nó, revision-bound moderation facts |
| `AuthorCommand::SaveSnapshot` | Thêm immutable snapshot Unsubmitted; không evaluation, không sửa pointer/generation |
| `RequestPublic { revision, expected }` | Owner + active + existing exact revision + current generation; rejected không plan; Unsubmitted thành Pending + EvaluationRequest; Pending/NeedsAdmin chỉ chờ; Approved promote |
| `SetPrivate { expected }` | Tăng generation; clear intent/pointer; giữ content và moderation facts |
| `AuthorTransition` / `Effects` | State mới, closed outcome, tối đa một revision/hash evaluation request và typed pointer effect; chưa dispatch/commit |
| `ReviewedRevision` / `record_review` | **Đầu vào đã admitted**, work/revision/hash, expected moderation và ReviewOutcome; kiểm binding/lifecycle/current fact rồi giữ quyết định và chạy guard cho đúng revision |
| `promotion_eligible(PublishIntent)` | Active + Public + exact current revision/generation + Approved |
| `readable_revision()` | Active + Public + pointer của revision Approved; không đòi pointer = current intent |

`ReviewedRevision` **không phải evaluator output, capability hoặc quyền admin**. #151 phải xác minh
schema, job, policy, decision version, admin override và reason trước khi gọi; `expected: Moderation`
không chống decision-version ABA và không thay được version guard của #151. Input không mang raw
provider result hay authority bit. Review một revision khác current intent chỉ lưu fact; không
được promote revision khác như side effect. Quyết định không-Approved trên revision đang public
clear đúng pointer đó; reject r2 không clear r1. Đây là application/domain seam, không có service
consumer, provider, admin role/grant hay route mới.

## Promotion khác public read

| Facts | Promote | Đọc hiện hành |
| --- | --- | --- |
| r1 Approved + pointer r1, intent r2 Pending/Rejected/NeedsAdmin, Public/Active | r2 không đủ; r1 không được promote bằng intent r2 | r1 |
| r2 Approved + exact current intent r2 | r2 | r2 sau transition |
| Private, không intent, callback Approved của r2 đã admitted | Không | Không; quyết định Approved được giữ |
| Intent generation cũ, revision lạ/khác work, owner sai | Refusal, không plan/effect | Không đổi state cũ |
| AuthorDeleted/Removed | Không | Không; không có lệnh hồi sinh |

## Acceptance và evidence

| AC | Source/check thật |
| --- | --- |
| AC-01 | `PublicationFacts`, hai variant Audience, typed generation/intent/pointer/lifecycle, per-revision Moderation; restore tests |
| AC-02 | SaveSnapshot effects NONE, exact revision/hash EvaluationRequest; unit + author replay |
| AC-03 | `readable_revision` vs `promotion_eligible`; r1/r2 table, private retains decisions; unit + application + replay |
| AC-04 | fallible restoration, wrong owner/work/hash/revision, duplicate, stale/reaffirmation, overflow; tests trả Err không transition/effects |
| AC-05 | withdrawn intent unit + NC replay; terminal fixture guards; không claim delete/tombstone runtime |
| AC-06 | `plan_publication` gọi core thật trong use-case test; PublicationStore, revision/hash v1 và metadata code giữ nguyên |
| AC-07 | Canonical M1, required FILTERS/REPLAYS, hai tên test bắt buộc và exact coverage counts; action matrix owner README |
| AC-08 | Registry/feature map/AGENTS/đặc tả tách pure core với runtime gap |

Môi trường kiểm: macOS arm64, rustc 1.99.1, Node 22.15.0, npm 10.9.2, Quint 0.33.1.
Base `origin/develop@e16b1cc94aa87aa90aab145a133b5a1d7425933c`; worktree
`/Users/manhblue/Documents/Codex/2026-11-10/task-9/vink`, branch `codex/publication-intent-core`.
Inputs là base cộng diff #150 (publication domain/application/tests, canonical model relocation,
runner selection và tài liệu). Implementation SHA nằm ở PR để không tự tham chiếu commit.
Build dùng `CARGO_BUILD_JOBS=2 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0` để giảm
artifact trên máy ít dung lượng; không đổi source/profile repository. npm dùng cache local có sẵn (`npm_config_offline=true npm_config_cache=/tmp/vink-task8-npm-cache`).
Lượt quick cuối chạy với quyền Unix socket loopback/local cho Identity test. Các lượt trước đã
**FAIL** vì sandbox chặn socket và vì hết dung lượng ở archive Rust; đã dọn đúng orphan object /
test binary cũ trong target của task, giới hạn 2 build jobs rồi chạy lại lane đầy đủ. Không bỏ test.
Logs local: `/tmp/vink150-focused.log`, `/tmp/vink150-quick-final.log`,
`/tmp/vink150-discussion-final.log`; FAIL cũ ở `/tmp/vink150-quick.log`,
`/tmp/vink150-quick-local-sockets.log`, `/tmp/vink150-discussion.log`.

| Check | Kết quả | Boundary |
| --- | --- | --- |
| `cargo test --locked -p vink-app --lib publication` | PASS (9 tests; cũng được lane quick chọn trên diff cuối) | Pure core, application caller và scoped replay |
| `cargo xtask verify quick` | PASS (9/9 bước, 47 doctest, 579 unit/contract tests) | Repo, server/wasm clippy, doctests, unit/contract |
| `cargo xtask verify discussion` | PASS (5/5 bước, 62 tests, đủ 13 required replays) | Baseline models + canonical M1/target drift + required production replays |
| PostgreSQL / HTTP / browser / Kani | NOT_RUN | Ngoài scope #150; không SQL/UI/Kani change |
| GitHub CI | NOT_RUN, NOT_REQUIRED | Không đọc/poll Actions/checks |
| Production | NOT_RUN | Không deploy/activation |

## Tích hợp tiếp

[#151](moderation-core.md) (`domain::moderation`, `application::moderation`) nối contract evaluator,
`ScriptedEvaluator` (9 lớp `evaluator-vectors.json`), guard precedence, `DecisionVersion` và
`AdminAuthority` vào `Publication::record_review`, mở rộng replay M1 lên 46/51 bước cùng cả hai
negative control (`mutatedIntentStep`, `mutatedOverrideStep`). PR persistence sau phải reload
owner + **principal standing fence**, revision/hash + current decision/version, current audience,
intent/generation và lifecycle trong cùng transaction; gọi policy trên facts mới, capture metadata
bằng `capture_metadata_snapshot`, bind revision + metadata trong cùng pointer row và commit
receipt/audit/outbox cùng effect. Không sửa public reads sang working_drafts. Public access hiện
hành vẫn cần check ở read/commit/cache boundary; pure PASS không chứng minh SQL locks, race,
rollback hay authorization. Sau đó mới nối HTTP/SSR/UI, delete/tombstone, worker và evaluator thật.
