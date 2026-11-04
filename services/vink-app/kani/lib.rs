//! Kani entry point. Compiles the production pure core in place; harnesses live next to the
//! code they check (`src/domain/discussion/proofs.rs`), behind `cfg(kani)`.
#[path = "../src/domain/mod.rs"]
pub mod domain;
