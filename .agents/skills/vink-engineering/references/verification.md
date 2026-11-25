# Verification scope and evidence

Use the manifests and command owners at the current checkout. The implemented lanes are
`cargo xtask verify quick|discussion|expressions|kani|postgres|web|all`; what each covers, its prerequisites and its limits
are defined once in [docs/development](../../../../docs/development/README.md). CI runs the same
lanes. On shared macOS machines, run supported heavy commands via
[local-budget](../../../../docs/development/local-resource-budget.md) (`./tools/local-budget run -- ...`, requiring Python 3.13+; note unsupported lanes such as `verify web`, Gradle/CMP and native diagnostics). `discussion` covers the structural D2T-01, lifecycle D2T-02 (with receipts), reaction D2T-INV-08 and literary endorsement (LVE #73) Quint models and real Rust replay;
there is no generic behavior lane. Canonical publication/moderation/governance model counts and
replay limits belong to the [publication owner](../../../../services/vink-app/behavior/publication/README.md).
The lane also checks the [Web read-lifecycle model](../../../../apps/vink-web/behavior/README.md)
and replays its inputs through production `ReadOwner/PageChain`. This is pure-core evidence,
not browser, storage or private-authority evidence.
The same lane runs the remaining target specifications in
`services/vink-app/behavior/target` (product decisions of 10/11/2026) as a separate step without
any Rust replay: a PASS there is model evidence, never implemented behavior. `expressions` checks the isolated EMO lifecycle model
and production policy replay. `kani` (not in `all`; needs the pinned verifier) runs the
owner-local manifests for the expression policy and the App's pure discussion and endorsement cores. No model or
proof lane verifies locking or I/O: PostgreSQL evidence remains in `postgres`, browser in `web`.

Find route-level scenarios and existing assertions in the
[feature map](../../../../docs/development/feature-map.md), then confirm the relevant source.
The map records coverage, not current PASS results. For requested maintenance of stale maps,
instructions or repeated checking workflows, use
[vink-maintain-verification](../../vink-maintain-verification/SKILL.md).

For a focused local task, use the [scoped workflow guide](../../../../docs/development/scoped-workflows.md):
`verify static`, `verify check <owner>`, `verify unit <owner> [--filter NAME]`,
`verify postgres <owner> [--filter NAME]`, `verify web --scenario NAME`, `verify preview`
and `doctor <scoped command>`. `--capture checks` on explicit Web scenarios leaves PNG/visual
evidence `NOT_RUN`. Cite runnable test count separately from Cargo compile/link cost.
Inspect dependencies with trailing `--plan`. A selected result is not a full lane PASS; `restart`
creates its own draft and carries its ID across the App restart. Full lanes and formal-method obligations below remain unchanged.

## Scoped completion claims

Keep these scoped claims distinct alongside the four status dimensions in [AGENTS.md](../../../../AGENTS.md) (they are reporting claims, not a second tracking database):

- `IMPLEMENTED(scope)`: the relevant code and required entrypoint/caller/consumer wiring exist.
- `VERIFIED(scope, inputs, target)`: applicable checks actually support the claim on those inputs and target.
- `OUTCOME_COMPLETE(journey, target)`: the declared journey works across its required boundaries on the integrated version.

A child PR may satisfy its own acceptance without claiming `OUTCOME_COMPLETE` for a multi-PR parent journey. Journey completion does not imply deployment, and verification does not require automatic GitHub labels or issue-state changes.

## Select scope during edit, review and integration

A Draft PR can carry a focused result and an explicit gap. During review, trace each changed
caller and the original evidence inputs. Before integration or release, run the outstanding
cross-owner checks and the gates required for that target. Open the feature map for opt-in
performance, native, formal, IME, security and visual obligations. An empty diff, inspected
plan or artifact HIT establishes no test result.

| Change | First check during an edit | Evidence for the boundary |
| --- | --- | --- |
| Contributor docs or skills | `verify static docs`, inspect the affected links and CLI | Run changed examples against their owner or its plan |
| CSS or shared component | `verify static tokens` and relevant repository guards | Product route at affected themes/sizes plus opened captures. Shared sources also feed Preview and may feed CMP |
| Rust module | `verify check OWNER` for types, then `verify unit OWNER --filter NAME` for a decision | Record the matched runnable tests. Cargo can still compile and link the entire selected test target |
| SQL, roles or migration | `verify postgres OWNER` on a disposable cluster | Add lifecycle, rollback, races or full PostgreSQL when shared schema or provisioning changes |
| Auth or shared HTTP contract | Start at the owning service tests and a scoped PostgreSQL fixture | Verify actual Web and service consumers, including disabled auth and negative access cases |
| Editor, SSR or hydration | Select the affected Web scenario | Exercise the real hydrated route, save/reload, focus and IME as the claim requires |
| Refined arithmetic, discussion or expressions model | Focused owner regression | Select the owning Quint and Kani lanes for sequence and exhaustive-domain claims, retain bounds |

## Select evidence by the claim

| Change or claim | Suitable check | Limit |
| --- | --- | --- |
| Docs, skills, metadata, path moves | Diff/whitespace review, local links, YAML and referenced-file validation | Does not exercise an app |
| Pure validation, guard or state transition | Exact example/table tests; properties or bounded enumeration when they add coverage | Does not prove database or transport behavior |
| Type boundary (no raw constructor, deserialization or field write) | `compile_fail` doctest plus a compiling lawful twin with the same setup | Stable rustdoc ignores `compile_fail` error codes; without the twin a broken import also passes |
| Snapshot binding and publication races | Pure tests plus real PostgreSQL concurrency/rollback tests | A mock store cannot establish transaction semantics |
| Job lease, outbox/inbox, crash/replay | Named failure points, restart/reclaim and uncertain-result reconciliation | Local locking cannot fence an in-flight remote request |
| Discussion write/moderation/visibility | Pure decision tests, then real PostgreSQL constraints, locks and direct-SQL negatives | A fake store cannot prove two-tier integrity or a create-vs-close race |
| SSR and hydration | Public HTML/metadata and private-data checks, then actual browser hydration/navigation | A native string render proves no browser interaction |
| Editor | Vietnamese IME, paste, undo, save/reload, conflicts, poetry round-trip on target browsers | Programmatic text entry alone proves no real IME compatibility |
| UI appearance | Open same-scenario captures with viewport, theme and text scale | Pixels alone prove no keyboard, screen-reader or motion behavior |
| Behavior model | Tool/version, properties, run inputs, seed/bounds and counterexamples | Model results and implementation conformance stay separate |

Keep tests independent of implementation where feasible: assert an observable requirement,
exact failure or durable state, rather than repeating the same algorithm as the expected result.
Do not add tests that merely mirror document wording or reversible formatting edits.

Distinguish **functional, visual, interaction and accessibility** evidence so each is selected and interpreted accurately; this distinction does not make interaction or accessibility optional. Prefer semantic or stable locators, without banning CSS selectors categorically: CSS and geometry assertions are valid when testing an actual contract (hit targets, clipping, focus visibility, selected indicators).

For flaky checks, investigate readiness, fixtures, input delivery, environment capabilities and the oracle before rerunning or changing the test. Never delete coverage, relax acceptance, suppress errors or move a required test to an unexecuted lane merely to obtain PASS. Screenshots do not replace save/reload, keyboard, focus or IME checks.

Required AC evidence and the broader advisory UX pilot (#174) in
[UX quality §5](../../vink-ui-design/references/ux-quality.md#5-mức-bắt-buộc-hiện-tại) are distinct: do not silently convert the pilot into a universal merge gate, and do not use pilot status to excuse an unmet mandatory AC. Propose any materially new gate with its scope and cost.

## Evidence handoff and provenance

For each required AC, the writer and reviewer use the same handoff:

`AC/claim → connected source/caller → evidence → result → gap`

For a small fix this can be one sentence. Use one row per AC only when several results need
comparison. For a run, give its original SHA/build and relevant dirty inputs, command or scenario,
target, fixture mode, result and log/capture reference. A real database or browser can use isolated
synthetic data. A component preview can establish how that component rendered in the preview.
An AC on a product route needs evidence from that route.

Record the source SHA and relevant dirty content, target/feature/profile, toolchain and flags,
dependency locks, generator/validator inputs, fixture or corpus, command, original logs and claim.
For cold/warm comparisons identify which cache began empty, which stayed warm and the measurement
boundary (process RSS or cgroup peak). Separate an artifact HIT from an executed assertion.
When copying a source snapshot into a reused build directory, preserved mtimes can predate the
previous binary. Verify the changed producer with a regression that the old binary fails.
Mark unresolved inputs `NOT_RUN` or `PARTIAL` and unsupported targets `UNSUPPORTED`.

Compare the tested inputs with the final delta. Rerun an affected check if its code, build,
fixture or target changed in a relevant way. If the delta is unrelated, record why the original
result still applies. For example, a browser trace on A can cover the same route on B after a
spelling fix to the contributor guide. Cite A as the executed revision. If applicability is
unknown, preserve the original result and mark the current claim unverified.

For UI changes, inspect relevant runtime errors and hydration behavior. Distinguish new unexpected errors from intentional failure fixtures and verified pre-existing/environment issues; never suppress errors to produce a clean report.

## Timing and unavailable dependencies

Use the cheapest focused check while developing. At handoff, run the checks that cover the
final inputs; reuse valid results instead of repeating an already covered suite. Broaden when
new changes or failures justify it. No fixed formal-method or full-stack campaign applies to
every task.

If PostgreSQL, a browser or a model tool is unavailable, complete independently
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
source inputs and boundary: pure core, fake adapter, real PostgreSQL, browser
or deployed environment. Record model seed/bounds or browser/device details when relevant.

An opened screenshot can be reported as inspected; a captured but unopened image cannot.
A model PASS does not accept a requirement, test production code or establish a database race.
Handoff should state the unverified claim, reason, prerequisite and next useful check when a
missing layer affects acceptance. Keep the report short for small changes.

## Human smoke handoff

When a change alters a main UI journey or leaves an interaction to check on a device, add a
review card. Aim for two minutes of interaction and state the setup separately.

- accessible URL/deep link and build identity (SHA/branch/mode);
- test state/fixture already prepared, or honest setup prerequisite;
- 3–4 concrete actions with expected results;
- specific remaining risk to observe.

Human execution remains `NOT_RUN` until actually observed/reported. This supplements agent verification; it is not a new approval gate unless the task or current policy requires one.
