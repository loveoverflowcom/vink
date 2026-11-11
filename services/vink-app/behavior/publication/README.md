# Publication — canonical M1 và scoped Rust replay (#150, #151)

M1 đã chuyển nguyên model, vectors và hai negative control từ `../target`; không có policy copy.
`publication.qnt` vẫn chứa author, evaluator, admin và lifecycle. Trạng thái là
`PARTIAL_PURE_CORE_CONFORMANCE`, **không** phải whole-model conformance hay runtime acceptance.

Runner chung ở [`../target/verify.mjs`](../target/verify.mjs) giữ Quint **0.33.1** và lockfile duy
nhất của `../discussion`: typecheck, simulation TypeScript seed **41**, **1.000 samples × tối đa
40 bước**, hai scenario (40 + 11 bước), hai mutant. `--write` chỉ đổi vị trí/metadata fixtures
trong lần chuyển này; transition/oracle không đổi. Vectors được xuất từ ITF thật, không JS reducer.

`cargo xtask verify discussion` đòi chọn đủ bốn test replay của publication/moderation trong `REPLAYS`:

- `quint_publication_author_inputs_and_admitted_decision_effects_replay_through_production`:
  46/51 bước của `publication-vectors.json` (`author`: 22, `ai_result`: 13, `admin_decide`: 6,
  `reevaluate`: 4, `policy_bump`: 1) chạy qua `Publication` và `WorkModeration` production;
  so sánh exact audience, generation, intent, pointer, read eligibility, per-revision decision + `by`,
  `policy`, `jobs` và `restore`. Chỉ 2 bước terminal lifecycle (`deleted`, `removed`) nạp fixture và
  3 bước từ chối lệnh xóa/gỡ (`outside_scope`) để dành cho `#155` (M2 tombstone).
- `the_withdrawn_intent_counterexample_is_refused_by_the_production_publication_guard`:
  replay `publication-withdrawn-intent-counterexample.json` (`mutatedIntentStep`); production trả
  `approved_unpublished` và giữ `pointer = None`.
- `the_admin_late_result_counterexample_is_refused_by_the_production_moderation_guard`:
  replay `publication-late-result-counterexample.json` (`mutatedOverrideStep`); callback AI `reject`
  đến muộn sau khi admin đã `approve` bị từ chối `overridden` và giữ nguyên quyết định admin.
- `quint_evaluator_vectors_replay_publication_steps_and_check_comment_contract`:
  chạy cả 21 bước của `../target/evaluator-vectors.json` (đủ 9 lớp `ScriptKind`) qua
  `ScriptedEvaluator`: 12 bước `publication.qnt` đi qua full consumer (`WorkModeration` +
  `Publication`), 9 bước `comment_edit.qnt` đi qua shared validator `validate_evaluation_result`
  (chỉ kiểm hợp đồng/script, chưa có comment-edit consumer `#154`).

| Action trong M1 | Rust coverage của #150 + #151 | Còn thiếu |
| --- | --- | --- |
| `snapshot` | `WorkModeration::decide_author` → `Publication::decide_author(SaveSnapshot)`, không mở job | SQL snapshot/metadata binding |
| `request_public`, `set_private` | Owner, lifecycle, exact revision, generation, moderation, pointer guard + mở `EvaluationTicket` khi `Submitted` | Transaction + actor standing tại commit; HTTP/UI |
| `ai_result` (cả hợp lệ và bị từ chối) | `WorkModeration::apply_evaluator_result` + `validate_evaluation_result` + `Publication::record_review` | Durable job/result tables, worker consumer, apply transaction fence |
| `admin_decide` (cả hợp lệ và bị từ chối) | `WorkModeration::apply_admin_decision` (`AdminAuthority`, `DecisionVersion`, `DecisionReason`) + `Publication::record_review` | Durable admin grant provisioning, CLI/HTTP, audit row |
| `reevaluate`, `policy_bump` | `WorkModeration::reevaluate` (cấp `EvaluationId` mới), `WorkModeration::bump_policy` | Policy persistence/governance, retry worker |
| `author_delete`, `admin_remove` | Model-only transition; Rust terminal restored facts chặn publish/admin/evaluator | Delete/remove commands, persistence và tombstone (`#155`) |
| `mutatedIntentStep` | Model NC + production guard refusal (`approved_unpublished`) | Durable commit race |
| `mutatedOverrideStep` | Model NC + production guard refusal (`overridden`) | Durable commit race |

Pure facts sử dụng UUID/content hợp lệ do harness cấp; actor/session/authentication, serialized
commit, durable job storage và admin grant provisioning là giả định. Simulation có biên không là
exhaustive proof; Apalache **NOT_RUN**. PostgreSQL, HTTP, browser và production **NOT_RUN** trong
scope này. API, choices, AC và bước nối tiếp:
[publication-core](../../../../docs/development/publication-core.md) và
[moderation-core](../../../../docs/development/moderation-core.md).
