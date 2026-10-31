# Verification scope and evidence

Use actual manifests, commands and CI from the current checkout. The implemented lanes are
`cargo xtask verify quick|postgres|web|all`; what each covers, its prerequisites and its limits
are defined once in [docs/development](../../../../docs/development/README.md). CI runs the same
lanes. There is no Quint model or behavior lane; do not report one as available.

Find route-level scenarios and existing assertions in the
[feature map](../../../../docs/development/feature-map.md), then confirm the relevant source.
The map records coverage, not current PASS results. For requested maintenance of stale maps,
instructions or repeated checking workflows, use
[vink-maintain-verification](../../vink-maintain-verification/SKILL.md).

## Pick lanes by change

| Change | Lanes | Notes |
| --- | --- | --- |
| Docs, skills, metadata only | `quick` (repository checks) or a diff/link review | No app run needed |
| Domain, application, contract, config, `desk_state` | `quick` | Add a focused unit or fixture test |
| SQL, migration, transaction, ownership, seed | `quick` + `postgres` | A fake store is not evidence of PostgreSQL semantics |
| Leptos view, route, SSR, hydration, desk interaction | `quick` + `web` (+ `postgres` if data access changed) | Also open captures for visual claims |
| Worker or process lifecycle | `quick` + `postgres` | The lane starts/stops the worker and checks config exit codes |

## Select evidence by the claim

| Change or claim | Suitable check | Limit |
| --- | --- | --- |
| Docs, skills, metadata, path moves | Diff/whitespace review, local links, YAML and referenced-file validation | Does not exercise an app |
| Pure validation, guard or state transition | Exact example/table tests; properties or bounded enumeration when they add coverage | Does not prove database or transport behavior |
| Snapshot binding and publication races | Pure tests plus real PostgreSQL concurrency/rollback tests | A mock store cannot establish transaction semantics |
| Job lease, outbox/inbox, crash/replay | Named failure points, restart/reclaim and uncertain-result reconciliation | Local locking cannot fence an in-flight remote request |
| Discourse create/edit/review/identity | Pin release/plugin/config; test enabled entrypoints and direct access | A webhook fixture cannot prove pre-publication moderation |
| SSR and hydration | Public HTML/metadata and private-data checks, then actual browser hydration/navigation | A native string render proves no browser interaction |
| Editor | Vietnamese IME, paste, undo, save/reload, conflicts, poetry round-trip on target browsers | Programmatic text entry alone proves no real IME compatibility |
| UI appearance | Open same-scenario captures with viewport, theme and text scale | Pixels alone prove no keyboard, screen-reader or motion behavior |
| Behavior model | Tool/version, properties, run inputs, seed/bounds and counterexamples | Model results and implementation conformance stay separate |

Keep tests independent of implementation where feasible: assert an observable requirement,
exact failure or durable state, rather than repeating the same algorithm as the expected result.
Do not add tests that merely mirror document wording or reversible formatting edits.

## Timing and unavailable dependencies

Use the cheapest focused check while developing. At handoff, run the checks that cover the
final inputs; reuse valid results instead of repeating an already covered suite. Broaden when
new changes or failures justify it. No fixed formal-method or full-stack campaign applies to
every task.

If PostgreSQL, Discourse, a browser or a model tool is unavailable, complete independently
testable work. Preserve real runtime semantics and report the missing layer; never replace
authentication, publication or persistence with fake success to make a runtime appear ready.
For a failure, preserve the command and relevant output, distinguish environment/pre-existing
failure from a regression, and test one supported hypothesis before rerunning.

## Results

Use `PASS` for an executed successful check, `FAIL` for an executed failure, `NOT_RUN` for a
check not executed, and `UNSUPPORTED` when the target/tool cannot perform it. `xtask` prints
the same words; a missing prerequisite is a `FAIL`, and steps after a failure are `NOT_RUN`.
A workflow file is not evidence that GitHub Actions ran; cite the run when claiming CI. Preserve the
runner's own output and clarify any mapping. For each material result, name the exact command,
source inputs and boundary: pure core, fake adapter, real PostgreSQL, pinned Discourse, browser
or deployed environment. Record model seed/bounds or browser/device details when relevant.

An opened screenshot can be reported as inspected; a captured but unopened image cannot.
A model PASS does not accept a requirement, test production code or establish a database race.
Handoff should state the unverified claim, reason, prerequisite and next useful check when a
missing layer affects acceptance. Keep the report short for small changes.
