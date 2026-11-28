//! Pure literary rules. This module must not import HTTP, SQL, Leptos or provider crates;
//! `cargo xtask verify quick` checks that boundary.

pub mod abuse;
pub mod discovery;
pub mod discussion;
pub mod document;
pub mod draft;
pub mod draft_query;
pub mod endorsement;
pub mod follow;
pub mod handle;
pub mod ids;
pub mod moderation;
pub mod moderation_queue;
pub mod notification;
pub mod notification_inbox;
pub mod principal;
pub mod profile;
pub mod publication;
pub mod reputation;
pub mod revision;
pub mod saved_work;
pub mod taxonomy;
pub mod work_view;
