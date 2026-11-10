//! Vink Identity: a separate process that owns authentication facts and credential/security state
//! (accounts, verified addresses, one-time challenges, sessions, the outbox of codes) in its own
//! database, serving a private contract on a Unix socket. Without configuration it answers
//! `authority_not_configured`. The native email-code authority (`authority`, `email_code`,
//! `accounts`) mails codes through a durable outbox and worker (`delivery`) over a transport
//! (`mail`: SMTP, or a local outbox for development). D-05 is still Provisional: a working SMTP
//! path is not an accepted production authority (see `docs/architecture/auth-authority.md`).
//!
//! Identity grants no Vink permission (admin, editor, publish) and never reads App data. It issues
//! and verifies session credentials; the App owns principals, the subject → principal binding,
//! suspension, authorization and works. Neither crate imports the other.

pub mod accounts;
pub mod authority;
pub mod config;
pub mod contract;
pub mod delivery;
pub mod email_code;
pub mod http;
pub mod mail;
pub mod persistence;
pub mod runtime;
pub mod server;
