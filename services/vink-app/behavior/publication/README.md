# Publication — canonical M1 và scoped Rust replay (#150)

M1 đã chuyển nguyên model, vectors và hai negative control từ `../target`; không có policy copy.
`publication.qnt` vẫn chứa author, evaluator, admin và lifecycle. Trạng thái là
`PARTIAL_PURE_CORE_CONFORMANCE`, **không** phải whole-model conformance hay runtime acceptance.

Runner chung ở [`../target/verify.mjs`](../target/verify.mjs) giữ Quint **0.33.1** và lockfile duy
nhất của `../discussion`: typecheck, simulation TypeScript seed **41**, **1.000 samples × tối đa
40 bước**, hai scenario (40 + 11 bước), hai mutant. `--write` chỉ đổi vị trí/metadata fixtures
trong lần chuyển này; transition/oracle không đổi. Vectors được xuất từ ITF thật, không JS reducer.

`cargo xtask verify discussion` đòi chọn đủ hai test mới trong `REPLAYS` rồi chạy core tests:

- `quint_publication_author_inputs_and_admitted_decision_effects_replay_through_production`:
  22 author commands và 8 publication effects của admitted decisions gọi `decide_author` /
  `record_review` production; so sánh exact audience, generation, intent, pointer, read eligibility,
  per-revision decision và effect plan. 19 job/policy/authority actions chỉ kiểm projection không
  đổi; **không** claim code validation. Hai lifecycle mutations chỉ nạp terminal fixture.
- `the_withdrawn_intent_counterexample_is_refused_by_the_production_publication_guard`: ba author
  inputs chạy thật; approval cuối được giả định đã admitted. Mutant xuất bản r1 sau khi private;
  production giữ Approved nhưng không pointer, không evaluation/pointer effect.

| Action trong M1 | Rust coverage của #150 | Còn thiếu |
| --- | --- | --- |
| `snapshot` | `decide_author(SaveSnapshot)`, không evaluation | SQL snapshot/metadata binding |
| `request_public`, `set_private` | Owner, lifecycle, exact revision, generation, moderation, pointer guard | Transaction + actor standing tại commit; HTTP/UI |
| `ai_result`, `admin_decide` đã được admitted | Chỉ `record_review`: hash/subject/current moderation + publication effect | #151: job, policy, decision version, schema, authority/admin override/reason |
| Result từ chối/timeout, `reevaluate`, `policy_bump` | Model-only; Rust projection không đổi | Evaluator/job/retry owner #151 |
| `author_delete`, `admin_remove` | Model-only transition; Rust terminal restored facts chặn publish | Delete/remove authority/commands, persistence và tombstone |
| `mutatedIntentStep` | Model NC + production guard refusal | Durable commit race |
| `mutatedOverrideStep` | Model NC được giữ; **không** Rust authority replay | #151 |

Pure facts sử dụng UUID/content hợp lệ do harness cấp; actor/session/authentication, serialized
commit, job authority và policy là giả định. Simulation có biên không là exhaustive proof;
Apalache **NOT_RUN**. PostgreSQL, HTTP, browser và production **NOT_RUN** trong scope này.
API, choices, AC và bước nối tiếp: [publication-core](../../../../docs/development/publication-core.md).
