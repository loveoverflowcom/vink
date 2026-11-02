//! Vink Identity: a separate process that will own authentication facts and credential/security
//! state once an authority is selected (D-05, S-03). Today it serves an honest private contract
//! (`authority_not_configured`) on a Unix socket, with its own database, roles and migrations.
//!
//! Identity grants no Vink permission (admin, editor, publish) and never reads App data; the App
//! owns principals, sessions, authorization and works. Neither crate imports the other.

pub mod authority;
pub mod config;
pub mod contract;
pub mod http;
pub mod persistence;
pub mod runtime;
pub mod server;
