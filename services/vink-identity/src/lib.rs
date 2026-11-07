//! Vink Identity: a separate process that owns authentication facts and credential/security state
//! (accounts, verified addresses, one-time challenges, sessions) in its own database, serving a
//! private contract on a Unix socket. Without configuration it answers `authority_not_configured`;
//! the D-05/S-03 spike adds the native email-code authority (`authority`, `email_code`,
//! `accounts`, `mail`), whose local outbox keeps production authentication NOT_IMPLEMENTED.
//!
//! Identity grants no Vink permission (admin, editor, publish) and never reads App data. It issues
//! and verifies session credentials; the App owns principals, the subject → principal binding,
//! suspension, authorization and works. Neither crate imports the other.

pub mod accounts;
pub mod authority;
pub mod config;
pub mod contract;
pub mod email_code;
pub mod http;
pub mod mail;
pub mod persistence;
pub mod runtime;
pub mod server;
