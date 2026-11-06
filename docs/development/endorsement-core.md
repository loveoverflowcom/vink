# Literary endorsement — core và registry LVE-PR-01

Baseline ban đầu `develop@be2b69e490d7637f6021c4b97062ed3859e09189`; lần merge #80 hợp nhất
`develop@69f0a3e376c3ab3db9a9e7fb146451d7aa4de27b` (gồm #78/#79), macOS arm64, 05/11/2026.
[PRD](../../prd/requirements.md#12-literary-endorsement-lve),
[quyết định/transaction contract](../architecture/literary-endorsement.md),
[epic #73](https://github.com/loveoverflowcom/vink/issues/73).
Owner: `vink-app::domain::endorsement`; PR2/PR3 phải dùng cùng seam, không viết lại luật trong
handler/SQL. Consumer transaction/HTTP có từ #81 ([runtime](literary-endorsement-runtime.md));
chưa có UI consumer và auth thật: epic AC-03 **PARTIAL**.

## Registry traceability duy nhất cho LVE

Các ID dưới đây giữ nguyên nghĩa từ epic. Mapping là coverage, không đồng nghĩa mọi lớp đã PASS.
Tên harness trong `src/domain/endorsement/proofs.rs` viết tắt bỏ tiền tố `endorsement_` trong bảng.

| INV / FR / UC | Quint | Production Rust / regression | Kani harness | Boundary gap |
| --- | --- | --- | --- | --- |
| LVE-INV-01 / FR-01 / UC-02/03 | `acceptedFence`, scoped pair; hai revision/root/reply | `EndorsementTarget`, `EndorsementTargetFacts::comment`, `matches_scope`; `work_root_reply…`, `foreign_facts…` | `comment_scope_and_visibility_reuse_two_tier_facts`, `eligibility_and_scoped_private_removal_fail_closed` | Thread→work và revision→work do adapter/FK; không prove bằng client fields |
| LVE-INV-02 / FR-01/10 / UC-01/15 | `authority` fact, guard trong `setWith` | `decide_endorsement` nhận server actor; `can_endorse` guard | `eligibility_and_scoped_private_removal_fail_closed` | Auth/capability hiện hành và transaction revoke chưa có |
| LVE-INV-03 / FR-01/02 / UC-03/04 | Một row cho pair; `exactCounts` | Một bool/actor/full target; `same_state_new_key…` | `acceptance_fences_every_new_intent_including_same_state` | Durable unique key chưa có |
| LVE-INV-04 / FR-03 / UC-04/06 | Receipt exact command; `refusalAndReplayDoNotWrite` | `EndorsementReceipt`, `Replay{committed,observed}`; `replay_is_historical…` | `receipt_replays_history_separately_and_rejects_changed_payload` | Atomic receipt/write, concurrent khác key/target, retention chưa có |
| LVE-INV-05 / FR-03 / UC-05/07 | `acceptedAgainstLatest`, `acceptedFence`, `retained`; mutant no-op | `AcceptedOrder`, `StoredEndorsement`; `off_noop_fences…`, replay và ABA tests | `noop_then_delayed_on_and_remove_recreate_cannot_reset_order`, `acceptance_fences…` | SQL state không reset, client ACK fence chưa có; không latest creation intent xuyên thiết bị |
| LVE-INV-06 / FR-05 / UC-09/17 | Hide/withdraw/authority shell, public projection | `visible`, own-removal guard; `hidden_withdrawn_missing…` | `eligibility_and_scoped_private_removal_fail_closed`, `comment_scope…` | Recheck current facts tại commit/read, account lifecycle còn OPEN |
| LVE-INV-07 / FR-02/09 / UC-04/14/17 | `exactCounts` theo selected source; count 0..3 | `EndorsementCount::apply`, `EndorsementDelta`; `exhausted_tokens_and_counts…`; `EndorsementState::restore` chỉ nhận trạng thái đạt được (selected ⇔ version chẵn), `accepted_states_keep_selection_parity…` | `scalars_and_count_arithmetic_are_total`; `acceptance_fences…` kiểm restore đúng parity và mọi state được chấp nhận restore lại được | Query/transaction aggregate, reconciliation/rebuild, freshness chưa có |
| LVE-INV-08 / FR-05 / UC-01/09 | `hiddenCountsStayHidden` | `public_count` → Option; private own state chỉ đúng scope | `eligibility…`, `comment_scope…` | Cache/HTTP/SQL/voter consent chưa có, CHOICE-12 OPEN |
| LVE-INV-09 / FR-06 / UC-03/13 | Ngoài model | Không đổi discussion order/structure/reaction | Harness cũ giữ nguyên | Ranking/cursor slice sau; không nhận hoàn tất invariant này |
| LVE-INV-10 / FR-03/08 / UC-05/08/14/16 | Không model transport ACK | Core tách replay history/current; không có UI consumer | Không proof ACK/session UI | Consumer phải bind actor/session/target/intent generation; toàn bộ UI còn thiếu |
| LVE-INV-11 / FR-07 / UC-11/18 | Không output cấp quyền | Outcome chỉ state/delta/receipt; không gọi publish/moderate/curate | Không proof hệ thống quyền | Caller/auth/authority cần tests riêng; count không được dùng làm ACL |
| LVE-INV-12 / FR-04/07 / UC-10/11/12/18 | `noSelfEndorsement`; đổi publication, close/reopen shell | Revision identity, author check, không có comments-mode gate; `current_revision…`, `work_root_reply…` | `eligibility…`, `comment_scope…` | Content edit/purge/reuse lifecycle và production policy qualification chưa có |

## Caller production (#81)

Use case, HTTP API v1 và `PgStore` gọi `decide_endorsement` trong transaction; schema giữ hàng/order
và receipt. Bằng chứng SQL/HTTP/race và gap còn lại ở
[runtime](literary-endorsement-runtime.md); các cột "gap" PostgreSQL/API trong bảng trên được thu hẹp
tới các mục ở đó (auth thật, thu hồi capability, retention, purge tài khoản, UI).

## Quint và replay

Dùng **cùng runner/package pin** ở `services/vink-app/behavior/discussion`, không thêm lane
`verify endorsements`/`votes`. `endorsement.qnt` được typecheck cùng ba model cũ; seed **41**,
backend **TypeScript**, **1.000 samples × tối đa 40 transitions**. Ba actor, bốn target:
work/revision1, work/revision2, root và flat reply; keys 1..12, expected gần order hiện tại.
Mỗi action giả định commit đã serialized. Order là mathematical integer; run 40 bước đạt tối đa
41, version <= order; scalar exhaustion toàn `i64` ở Rust/Kani, không fake giới hạn số nhỏ là production.

Scenario `adversarialStep`: **33 transitions**, đủ chín outcome (new acceptance changed/unchanged,
replay, conflict, key reuse, unavailable target, own content, authority unavailable và synthetic
facts change). Bao phủ OFF/no-op đến trước ON cũ, same-state stale key mới, lost ACK/retry sau remove,
ABA, root/reply riêng, comment-close độc lập, hide/withdraw/đổi revision, authority revoke và own-removal.
`serial` là ghost specification clock độc lập actual order, không phải field production.

`endorsement-fixtures.json` sinh từ ITF thật. Node chỉ project/sort integer/map; không tính lại
transition. Test `quint_endorsement_inputs_replay_against_production` nạp **command inputs** vào
`decide_endorsement`, áp checked delta rồi giữ state/receipt trong shell RAM. Tất cả rows, counts,
public counts, receipt count, exact outcomes và historical state được so **sau** quyết định thật.
Environment changes là synthetic facts shell, không giả auth/publication implementation.

Negative control: `mutatedNoopStep` bỏ tăng actual order khi accepted same-state. B=OFF@1 được nhận
nhưng vẫn order1; A=ON@1 cũ sau đó được nhận và resurrect ON. Ở transition 2,
`acceptedAgainstLatest` vi phạm (ghost serial đã 2). Runner đòi exit1 + invariant violation + ITF
đúng hành vi ON hồi sinh; parse/tool failure không tính PASS. Counterexample commit tại
`endorsement-noop-counterexample.json`. Production Rust mutation bỏ fence của no-op cũng phải làm
regression/replay đỏ; evidence của phiên ở bảng dưới.

Không chạy Apalache/model checking; simulation **không** là exhaustive proof. Không claim liveness
hoặc SQL concurrency từ các serialized traces.

## Kani và registry

Pin theo repo: **0.67.0**, bundled compiler `nightly-2025-11-21`.
`services/vink-app/kani/lib.rs` mount đúng production `src/domain`; sáu harness mới nằm tại
`src/domain/endorsement/proofs.rs`, `cfg(kani)`, không rule copy. `xtask/src/kani.rs` nay đăng ký
**cả hai file** discussion/proofs và endorsement/proofs cho App manifest; tổng phải là **19**
harness App (13 discussion, gồm hai harness #79, + 6 endorsement), cùng 3 harness expressions. Module bị compile-out/zero harness hoặc
summary thiếu harness là FAIL. Không tạo runner thứ hai.

- `scalars_and_count_arithmetic_are_total`: mọi i64 token/count, checked +1/-1, cover underflow,
  overflow và miền giữa; không assume sẵn count hợp lệ ngoài constructor.
- `acceptance_fences_every_new_intent_including_same_state`: mọi selected/wanted, version/order/
  expected i64 hợp lệ (1 <= version <= order), cover add/remove/no-op/conflict/exhaustion.
- `noop_then_delayed_on_and_remove_recreate_cannot_reset_order`: chuỗi cố định bốn lệnh thật,
  chặn ON@1 sau OFF/no-op và sau remove/recreate, cover OFF cuối.
- `eligibility_and_scoped_private_removal_fail_closed`: mọi authority/visibility/own/foreign facts,
  private own-removal và không gỡ record actor khác, projection hidden; covers own/refusal và hidden removal.
- `receipt_replays_history_separately_and_rejects_changed_payload`: production acceptance tạo
  receipt, replay ON lịch sử/current OFF; khác payload/actor bị chặn, cover historical != observed.
- `comment_scope_and_visibility_reuse_two_tier_facts`: root/reply, approved/pending, present/removed,
  work visibility, self; thread mismatch và comment mismatch bị chặn; covers public root/reply.

Không vòng lặp/collection trong harness mới; default unwind, **giữ unwinding assertions**.
IDs hằng là các equivalence class bằng/khác cần thiết; proofs không chứng minh UUID generation,
principal source, work/thread FK, Unicode, SQL, I/O, auth, UI hay account lifecycle. Lần merge PR #80 đã chạy verifier thật: 19/19 harness App và 3/3 expressions đạt;
40 cover App (26 discussion + 14 endorsement) và 2 cover expressions đều SATISFIED.
Giữ unwinding assertions; không có failure hoặc cover bị bỏ.

## Bằng chứng phiên và acceptance

Source base ở đầu trang; commit chính xác của diff và log local được ghi trong PR handoff.
Rust **1.99.1**, Node **22.15.0**, npm **10.9.2**, Quint **0.33.1**, macOS arm64.

| Command / boundary | Kết quả |
| --- | --- |
| `cargo test --locked -p vink-app --lib domain::endorsement` | PASS — 8 regression + 1 production-input Quint replay (33 bước) |
| `node services/vink-app/behavior/discussion/verify.mjs --write` | PASS — typecheck, simulations/scenarios + meaningful negative control; fixture sinh rồi kiểm drift riêng |
| `cargo xtask verify quick` | PASS — fmt, clippy server/browser/preview, 33 doctests, 307 unit/contract tests và repository checks |
| `npm_config_cache=/tmp/vink-lve-npm-cache cargo xtask verify discussion` | PASS — 4 model, drift/scenarios/5 negative controls, 27 Rust tests gồm 4 replay bắt buộc |
| `cargo xtask verify kani` (PATH trỏ kani-tools/bin) | PASS sau cài pin 0.67.0 — App 19/19, expressions 3/3; 42/42 cover toàn lane SATISFIED |
| Production no-op fence mutation | PASS sensitivity — mutant compile/run exit101: 3 assertion failures (`off_noop_fences…`, `same_state_new_key…`, replay step1); source đã khôi phục, quick chạy sau restoration |
| Apalache/model checking, PostgreSQL/API/browser/native | NOT_RUN — ngoài pure slice / không có adapter hoặc UI mới |

Cache npm mặc định không ghi được; dùng cache riêng `/tmp/vink-lve-npm-cache`, không sửa cache của
người dùng. Cargo/Quint dependency được tải theo lockfile, không nâng toolchain/dependency.

AC-P1-01: canonical owners có; AC-P1-02: pure work/root/reply + typed outcomes có;
AC-P1-03/04: core contract + regression/model/replay/negative control, không nhận SQL/consumer;
AC-P1-05 **ĐÃ ĐẠT** sau chạy Kani 0.67.0 tại lần merge #80; AC-P1-06 giữ source/fixtures của #79 và kiểm regression qua lanes hiện hành.
Epic AC-03 chỉ PARTIAL; không mark/đóng #73, không merge/deploy.

Tái lập proof từ repo root trên host hỗ trợ, đủ disk (phiên merge dùng install root riêng `../kani-tools`):

```sh
cargo install --locked kani-verifier --version 0.67.0
cargo kani setup
cargo xtask verify kani
```

Yêu cầu 19/19 App harness và 3/3 expression harness, mọi cover SATISFIED, không unwinding failure.
Sau khi sửa proof/source, chạy lại quick/discussion cần thiết; negative control trên harness
acceptance được ghi riêng trong bằng chứng merge. Downstream: auth/capability thật + transaction
adapter/schema giữ preconditions, receipt retention, query/rebuild và exact API conformance trước UI.


## Nghiệm thu merge #80 sau #78/#79

Input upstream: `69f0a3e376c3ab3db9a9e7fb146451d7aa4de27b`, PR head gốc `0f748d2`.
Kani cài pin 0.67.0 vào `../kani-tools`, bundled compiler `nightly-2025-11-21`; không đổi
workspace Rust 1.99.1. Dùng `PATH=../kani-tools/bin:$PATH`. Đặt `CARGO_PROFILE_DEV_DEBUG=0`,
`CARGO_PROFILE_TEST_DEBUG=0`, `CARGO_INCREMENTAL=0` cho quick/discussion để giảm artifact disk.
Các flags này không đổi source, guards hoặc unwinding assertions.

- `cargo xtask verify kani`: PASS 19/19 App, 3/3 expressions; 40 + 2 cover SATISFIED.
- `cargo xtask verify quick`: PASS — 33 doctests, 313 unit/contract, fmt/clippy/repo checks.
- `npm_config_cache=/tmp/vink-lve-npm-cache cargo xtask verify discussion`: PASS — 31 tests, sáu replay bắt buộc, 4 model/drift và 7 negative controls.
- Kani production no-op fence mutant: PASS sensitivity — exit1, đúng assertion
  `after.order().get() == order + 1` FAIL, 0/1 harness; source khôi phục byte-exact.
- Source/fixtures reaction/settings, application/persistence và PostgreSQL tests của #79
  khớp byte với `origin/develop@69f0a3e`; không đảo sửa fence đã merge.
- Canonical owner/registry hợp nhất; các module work/comment cũ và model/counterexample
  của seam #78 được thay bằng seam/model #80, không để hai luật cạnh tranh.

Không đọc/chờ GitHub Actions/status/logs. PostgreSQL/browser/native không chạy lại vì
#80 không đổi source các boundary đó so với develop. Không deploy hoặc đóng epic #73.
