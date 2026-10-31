---
name: vink-maintain-verification
description: >-
  Maintain Vink's feature map and verification guidance against current routes,
  code, tests and command owners. Use when requested to refresh verification,
  investigate stale coverage or improve a repeated checking workflow; routine
  feature implementation uses vink-engineering's verification guide.
---

# Vink maintain verification

Keep the path from a feature claim to a reproducible check accurate. Read
[AGENTS.md](../../../AGENTS.md) and inspect current source before updating descriptions.
This is task-driven maintenance; invoking it does not create a daily schedule, monitor or
require a full-stack run for documentation changes.

## Keep one owner for each kind of information

| Information | Owner |
| --- | --- |
| User journey, route, preconditions, observable states, check coverage | [Feature map](../../../docs/development/feature-map.md) |
| Setup, supported commands, prerequisites and lane limits | [Development guide](../../../docs/development/README.md) |
| Executable commands and process setup | [xtask](../../../xtask/src/main.rs), [web runner](../../../xtask/src/web.rs), [CI](../../../.github/workflows/ci.yml) |
| Which evidence a claim requires; result vocabulary | [Verification guide](../vink-engineering/references/verification.md) |
| Product acceptance and open decisions | [Requirements](../../../prd/requirements.md), [decision log](../../../docs/architecture/decisions.md) |

The map is an index into source/tests, not another command manual or a permanent green status
board. A test's presence is coverage intent; only a run on identified inputs is run evidence.

## Reconcile a bounded slice

1. Identify the changed feature or unreliable check and pin branch/SHA plus relevant local
   changes. For a general refresh, enumerate current routes, HTTP contracts and command owners
   first; do not derive implemented features from requirements alone.
2. Trace the affected route/component, application/store and test assertions. Match the map's
   entrypoints, actor/data preconditions, visible states and failure/recovery steps to code.
   Distinguish public SSR, private API loads and hydrated interaction. Verify fixture IDs,
   labels and selectors before documenting them.
3. Classify each mismatch: stale description, missing assertion, unsupported environment,
   flaky tooling, or product regression. A failed check does not justify changing the expected
   behavior to match the bug. Fix the owning layer within scope; preserve the intended claim.
4. Update only the affected owner and its links. A map-only change needs diff/link/YAML review;
   a test/runner change needs an actual affected run and relevant failure-path validation.
   Use the verification guide for missing prerequisites and independent work.

Track useful negative cases: private text absent from public/SSR payloads, stale saves do not
write, failed saves retain local text, and implemented service errors remain visible. Distinguish
requirements to check from assertions the current test actually makes. The local principal
fixture, simulated browser composition and lifecycle-only worker leave explicit coverage gaps.

## Improve tooling only for a concrete repeated need

Inspect existing `xtask` and browser helpers before creating anything. Prefer a focused
extension when repeatable automation is necessary and authorized; documentation can often
resolve a discovery problem. Do not add a new runner or parallel CLI merely to validate skills.

For an actual tooling change, keep commands composable with actionable errors, stable exit
status and bounded waits. Add structured output when there is a real machine consumer; use
preview/dry-run for an applicable mutation rather than inventing one for read-only checks.
Make fixture/resource ownership and cleanup explicit, and keep credentials and manuscripts
out of diagnostics. Update the development guide with any changed command contract.

Browser smoke currently checks interactions via CDP; it has no screenshot/trace output. Use
available browser tools for requested visual evidence or implement that capability within
scope, then inspect the captures. Do not describe absent tooling as already available.

## Finish with scoped evidence

Check paths, metadata and routing with the repository's existing checks or a focused local
review; no new test framework is required. Record what changed, executed checks and their
inputs, and unresolved claims with their next check. A successful docs check does not refresh
old runtime evidence. Keep historical results tied to their original revision/environment,
and never mark a product requirement or D/S decision complete just because the map is current.
