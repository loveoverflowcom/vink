# Kani compatibility surface

Pinned verifier: `cargo-kani 0.67.0`, bundled compiler `nightly-2025-11-21`, the same pin as
[`packages/expressions/kani`](../../../packages/expressions/kani/README.md).
`lib.rs` mounts exactly `../src/domain` from production; the App's HTTP, SQL and Leptos
dependencies are not compiled and no rule is copied. Harnesses live in
[`src/domain/discussion/proofs.rs`](../src/domain/discussion/proofs.rs) and
[`src/domain/endorsement/proofs.rs`](../src/domain/endorsement/proofs.rs); both files are registered
for this manifest in `xtask/src/kani.rs`, which requires every declared harness to be verified.

Commands, harnesses, bounds and what they do not prove:
[development guide](../../../docs/development/README.md#kani).
Do not upgrade the verifier silently; source/compiler compatibility needs a real rerun.
