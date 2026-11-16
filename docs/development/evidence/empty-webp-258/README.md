# Evidence #258 — Dot Linux, 16/11/2026 UTC

**Implementation ready for draft review; browser acceptance incomplete.**
Base `develop@f1b46703ae3329ee7cd11fb06db3f8157a31e5b9`, branch `codex/empty-webp-discovery`,
worktree `/workspace/scratch/429f9ca90843/vink-258-empty-webp`.
[Final source hashes](inputs.json) pin the tested bytes; publication must keep the same tree inputs.
All results below are self-run/self-review, not independent review or historical issue evidence.

| Local command / boundary | Actual result |
| --- | --- |
| `node apps/vink-web/assets/empty/verify.mjs`; `node --test apps/vink-web/assets/empty/verify.test.mjs` | **PASS**, 3 asset regressions: original records, animation/truncation/lossless rejection, hash-drift rejection; [log](assets.log) |
| Pillow full `load()`, RGBA/alpha extrema, 512×427, one frame, all three delivered files | **PASS**; same [log](assets.log). Opened all three actual WebP pixel files and supplied light/dark board. These are artwork inspections, not app screenshots. |
| `cargo xtask verify quick` | **FAIL**: App unit group 325 PASS / 9 FAIL at unchanged Identity Unix-socket fixture `http/mod.rs:793`, `EPERM`. Ten earlier steps PASS, including asset validation, token/manifest/oracle checks, fmt, SSR/hydrate/preview clippy and doctests. Later repository checks **NOT_RUN**. [Raw log](quick.log) |
| `cargo test --locked --workspace --lib --bins pages::discovery::tests` | **PASS 4/4**, 231 other Web tests filtered; other packages select zero tests. [Log](ssr-tests.log). Actual Leptos SSR composition with explicit Router context; synthetic read facts, not SQL/browser. |
| Final `cargo clippy --locked --workspace --all-targets -- -D warnings`; `cargo clippy --locked -p vink-web --lib --target wasm32-unknown-unknown --features hydrate -- -D warnings`; `cargo fmt --all -- --check` | **PASS** after the final freshness qualifier and test-harness fix; [log](clippy-final.log) |
| `cargo xtask verify web --scenario main --scenario discovery` with isolated PostgreSQL 17.11 | **FAIL** starting Chrome: `process_singleton_posix.cc:297`, Unix `socket()` returns `Operation not permitted`. Prerequisites, visual-oracle unit tests, current-source WASM/editor/assets + App build, 78-file bundle secret scan, migration/seed and App readiness **PASS**. Browser assertions in main and later discovery **NOT_RUN**. [Raw log](web.log) |
| `python /tmp/vink-258-http-check.py` in PTY, isolated PostgreSQL + current Axum/Leptos binary/static bundle | **PASS 8 checks**, [report](http-report.json), [exact one-off evidence script](http-check-run.py). Fresh corpus, filtered miss, unknown tag 404, invalid cursor 422, three actual served WebPs and nonempty page after real dev-seed. Correct image MIME/byte count/SHA-256; site empty directory contains exactly the three derivatives. This is HTTP/SSR evidence, not browser request-count/interaction evidence. |
| `node --check` on both modified browser smoke modules; `git diff --check`; focused relative-link review | **PASS**; browser modules are prepared, not executed. |
| `./tools/local-budget plan` | **UNSUPPORTED**: launcher supplies macOS SOFT_MANAGED only. Direct Linux commands use two Cargo jobs, no incremental/debug info; one heavy lane at a time. |

Environment: Linux x86_64, Rust 1.99.1, Node 24.22.1, wasm-bindgen 0.2.129, Chromium
156.0.8078.79; PostgreSQL 17.11 with ICU from the installed task environment. Commands use
`CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_BUILD_JOBS=2`.
No user `.env`, existing worktree/target or shared database was altered. Task-owned scratch
App/PostgreSQL processes were stopped. Only this worktree's completed WASM compile intermediates
were removed after bindgen to recover space; `target/site` was retained. No source/test was
skipped or weakened to claim a full lane PASS.

The first SSR test attempt was 3/4: the removed-state error component needs a Router; the test
harness now provides the real SSR RequestUrl/Router. The final command above passes all four.
An extra package-only build was interrupted before tests to reuse the existing workspace
feature graph under disk pressure; it is not reported as PASS. Identity fixture equality to the
base is recorded in `inputs.json`; this task does not modify Identity.

Direct Dot-browser fallback attempted the live task server at
`http://127.0.0.1:52663/?q=no-match-258`: **FAIL**, `ERR_CONNECTION_REFUSED` from that browser.
The subsequent `getAXState()` on tab 1's internal error document was **DENIED** by the browser
URL policy (only HTTP/HTTPS protocols are allowed); no content was observed, and it was not
retried. User history was checked after the denial; it supplies no authority to change this
browser policy. This is an environment boundary, not a bot block or product test failure.
No app screenshots were captured/opened. Matched-source before/after captures remain **NOT_RUN**.

| AC | Evidence now | Verdict / remaining boundary |
| --- | --- | --- |
| AC-01 | Exact ZIP SHA-256, all asset hashes/container/full decode, real build/static HTTP MIME+bytes | Partial; real browser one-current-state/zero-unrelated request ledger remains NOT_RUN. |
| AC-02 | Real SSR corpus/miss and nonempty page: one image + one HTML status; no H2/H3/0 stack, native recovery; shared tag consumer inspected | Partial; tag/Explore browser visual and pagination qualification remain NOT_RUN. |
| AC-03 | Four SSR regressions cover corpus, miss, cursor/continuation, unknown/retired, retained failure and removed; HTTP checks unknown/invalid | Partial; loading/stale/authority/race browser scenarios remain NOT_RUN. |
| AC-04 | URL/GET/Apply/input owner retained; real SSR links; named landmark and reader fallback source updated | Partial; keyboard/focus/IME/reader-return/no-JS submit browser regression remains NOT_RUN. |
| AC-05 | Decorative alt/intrinsic HTML and single SSR status verified; blocked/corrupt-before-hydration regressions prepared | Partial; runtime focus, failure/reflow/announcement checks remain NOT_RUN. |
| AC-06 | Scoped CSS and unchanged token/font inputs; 320/390/834/1440 × light/dark × text100/200 checks prepared | NOT_RUN for app pixels/geometry/contrast/responsive, including bottom-nav clearance. |
| AC-07 | Asset tests, final compile/lint, 4 SSR regressions and 8 real HTTP checks executed | Partial; browser request/layout/decode checks and matched-source opened before/after captures remain NOT_RUN. |
| AC-08 | LayoutSpec, provenance/pipeline, choices, source map and per-AC evidence agree; #259 has one composition/API and all three assets | PASS for this documentation/handoff scope. No extra asset PR or domain/native scope. |

The live checklist belongs to issue #258. Only AC-08 is supported for ticking; keep all other
ACs unchecked and the issue open while this PR is unmerged. No other issue's checklist changes.

| UX dimension | Result / limits |
| --- | --- |
| Functional | PASS within 4 composition SSR tests and 8 actual HTTP checks; browser task completion NOT_RUN. |
| Geometry | NOT_RUN on rendered app; prepared ratio/size/order/overflow assertions are not measurements. |
| Visual | NOT_RUN on app; supplied board and exact asset pixels opened. `NO_APPROVED_REFERENCE`; no golden claim. |
| Accessibility / Responsive | SSR semantics verified within the stated scope; keyboard/focus, contrast, full viewport/text matrix and named screen reader NOT_RUN. |
| Interaction | NOT_RUN in browser; GET form/link source remains intact. Real OS IME/device keyboards/native NOT_RUN. |

Domain/Quint verification is **NOT_APPLICABLE**: no domain, durable read lifecycle, API, DB or
model contract changed. Separate PostgreSQL/full-model/native lanes were not run. The real SQL
here supplies the existing app's public reads only; it does not qualify new persistence or auth.
GitHub CI was not inspected, polled, rerun or awaited. No merge/deploy/issue closure.

Resume on the Mac or another host that permits Chrome/Identity Unix sockets. Run the same
`verify quick` and `verify web --scenario main --scenario discovery` commands (Mac heavy commands
through `./tools/local-budget run -- …`), then open same-scenario before/after captures, run the
32-case Explore/tag empty matrix and review request count/focus/image-failure evidence before ticking ACs.
The existing browser module also retains #225 Apply/no-JS/IME and discovery-corpus/reader-return
assertions. Full `verify web` remains a separate full-lane claim.

The prepared empty matrix now covers 16 viewport/theme/text combinations per surface (Explore and tag).
The tag matrix was expanded after the launcher failure and syntax-checked; it remains NOT_RUN,
with no change to the tested Rust/CSS/asset inputs.
