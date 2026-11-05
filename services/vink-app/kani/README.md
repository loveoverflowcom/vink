# Kani compatibility surface

Pinned verifier: `cargo-kani 0.67.0`, bundled compiler `nightly-2025-11-21`, the same pin as
[`packages/expressions/kani`](../../../packages/expressions/kani/README.md).
`lib.rs` mounts exactly `../src/domain` from production; the App's HTTP, SQL and Leptos
dependencies are not compiled and no rule is copied. Harnesses live in
[`src/domain/discussion/proofs.rs`](../src/domain/discussion/proofs.rs) (11) và
[`src/domain/endorsement/proofs.rs`](../src/domain/endorsement/proofs.rs) (6). Cả hai file được
registry `xtask/src/kani.rs` đếm cho cùng manifest: phải verify đủ 17, không tạo runner riêng.
[Endorsement evidence/gap](../../../docs/development/endorsement-core.md) ghi Kani NOT_RUN khi
host thiếu verifier; declaration không là bằng chứng proof PASS.

Commands, harnesses, bounds and what they do not prove:
[development guide](../../../docs/development/README.md#kani).
Do not upgrade the verifier silently; source/compiler compatibility needs a real rerun.
