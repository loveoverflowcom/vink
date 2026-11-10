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
Manifest hiện hành: 21 discussion harness (giữ #79, thêm 3 admission #120, 1 overlay riêng tư
DISC-02/03 và 2 outbox DISC-04 và 2 operator post-review) + 6 endorsement + 6 taxonomy = 33.

DISC-POST-REVIEW-RUNTIME thêm hai harness trong `discussion/proofs.rs`:
`operator_review_never_certifies_a_foreign_or_newer_state` (binding/digest, mọi i64 deadline/now,
exhaustion và finding; cover keep/hold/superseded) và
`every_accepted_human_reaffirmation_fences_the_previous_review` (approved/rejected cùng trạng thái tăng
version và làm ticket cũ superseded). Chúng compile source production; SHA-256, authority ticket,
SQL và worker là giả định, không nằm trong proof. Kết quả thực thi được báo riêng trong PR.
