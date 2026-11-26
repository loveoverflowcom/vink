//! Vink service: the native composition root.
//!
//! Dependency direction: `http` and `persistence` (adapters) → `application` (use cases and
//! ports) → `domain` (pure rules). `http` also hosts `vink-web` SSR and implements its
//! `PublicReads` seam. Both binaries (`vink-app`, `vink-worker`) share `config` and `runtime`.

pub mod account_deletion;
pub mod application;
pub mod config;
pub mod dev_seed;
pub mod domain;
pub mod evaluation_worker;
pub mod http;
pub mod identity;
pub mod notification_inbox;
pub mod notification_relay;
pub mod notification_transport;
pub mod outbox_worker;
pub mod persistence;
pub mod runtime;
