//! Pure literary rules. This module must not import HTTP, SQL, Leptos or provider crates;
//! `cargo xtask verify quick` checks that boundary.

pub mod discussion;
pub mod document;
pub mod draft;
pub mod endorsement;
pub mod ids;
pub mod principal;
pub mod revision;
