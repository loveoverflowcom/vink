# Kani compatibility surface

Pinned verifier: `cargo-kani 0.67.0`, bundled compiler `nightly-2025-11-21`, the same pin as
[`packages/expressions/kani`](../../../packages/expressions/kani/README.md).
`lib.rs` mounts exactly `../src/domain` from production; the App's HTTP, SQL and Leptos
dependencies are not compiled and no rule is copied. Harnesses live in
[`src/domain/discussion/proofs.rs`](../src/domain/discussion/proofs.rs),
[`src/domain/endorsement/proofs.rs`](../src/domain/endorsement/proofs.rs) and
[`src/domain/taxonomy/proofs.rs`](../src/domain/taxonomy/proofs.rs); all three files are registered
for this manifest in `xtask/src/kani.rs`, which requires every declared harness to be verified.

Commands, harnesses, bounds và evidence: [registry LVE](../../../docs/development/endorsement-core.md),
[registry taxonomy](../../../docs/development/taxonomy-core.md#kani)
và [development guide](../../../docs/development/README.md#kani).
Manifest hiện hành: 16 discussion harness (giữ #79, thêm 3 admission #120) + 6 endorsement + 6
taxonomy = 28.
