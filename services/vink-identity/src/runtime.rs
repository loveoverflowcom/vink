//! Logging and shutdown for the Identity process. Mirrors `vink-app`'s runtime on purpose:
//! services do not import each other, and these few lines have no business rule to share.

use tracing_subscriber::EnvFilter;

use crate::config::LogFormat;

const DEFAULT_FILTER: &str = "info,sqlx=warn";

/// Structured logs on stderr. Request bodies (credentials) are never logged.
pub fn init_tracing(format: LogFormat) {
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(DEFAULT_FILTER));
    let builder = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr);
    match format {
        LogFormat::Text => builder.init(),
        LogFormat::Json => builder.json().init(),
    }
}

/// Resolves on SIGINT or SIGTERM.
pub async fn shutdown_signal() {
    let ctrl_c = async {
        if let Err(error) = tokio::signal::ctrl_c().await {
            tracing::error!(%error, "cannot listen for Ctrl-C");
            std::future::pending::<()>().await;
        }
    };
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut signal) => {
                signal.recv().await;
            }
            Err(error) => {
                tracing::error!(%error, "cannot listen for SIGTERM");
                std::future::pending::<()>().await;
            }
        }
    };
    tokio::select! {
        () = ctrl_c => tracing::info!(signal = "SIGINT", "shutdown requested"),
        () = terminate => tracing::info!(signal = "SIGTERM", "shutdown requested"),
    }
}
