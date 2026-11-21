//! Vink Notification Service (Notifications v1, #347; Epic #305 R3, K1–K5): a separate process
//! with its own PostgreSQL database that owns ingestion receipts, the write-time rollup of inbox
//! groups, the Seen watermark and Read marks, and their retention.
//!
//! - `envelope`: strict parsing of the contract §1.1 envelope and its canonical digest.
//! - `rollup`: pure rules of the rollup (group keys, P-02B/P-03A Thấm steps, preview, Read).
//! - `store`: the transactions (receipt + effect atomically, ACK commands, reads, purge).
//! - `http` + `server`: two private Unix sockets (T-07 CHOICE-21 A), one for the App relay
//!   (ingest) and one for the App Gateway (inbox), each answering only its configured peer user.
//!
//! The service never reads the App or Identity databases, owns no content and grants no
//! permission to see one: the App Gateway (#348) checks current visibility at every read (N4).

pub mod config;
pub mod envelope;
pub mod http;
pub mod persistence;
pub mod rollup;
pub mod runtime;
pub mod server;
pub mod store;
