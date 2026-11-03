//! Expression identity is derived from one manifest. Authored text is never rewritten.
//! Playback decisions have no framework, I/O, DOM or persistence dependency.

#[rustfmt::skip]
pub mod catalog;
pub mod policy;

#[cfg(kani)]
mod proofs;
#[cfg(all(test, not(kani)))]
mod tests;
