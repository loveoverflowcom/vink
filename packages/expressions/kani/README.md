# Kani compatibility surface

Pinned verifier: `cargo-kani 0.67.0`, bundled compiler `nightly-2025-11-21`.
The manifest compiles `../src/lib.rs`, exactly the production crate, without the workspace's
Rust 1.99 minimum or dev-only serde fixture dependency. No implementation is copied.

Commands, bounds and results vocabulary: [development guide](../../../docs/development/README.md#kani).
Do not upgrade the verifier silently; source/compiler compatibility needs a real rerun.
