//! Process lifecycle shared by the app and worker binaries.

use tracing_subscriber::EnvFilter;

use crate::config::LogFormat;

const DEFAULT_FILTER: &str = "info,sqlx=warn,tower_http=info";

/// Structured logs on stderr. `RUST_LOG` overrides the filter; `VINK_LOG_FORMAT=json` selects
/// one JSON object per line. Request bodies (manuscripts) are never logged.
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

/// Resolves on SIGINT (Ctrl-C) or SIGTERM.
pub async fn shutdown_signal() {
    let ctrl_c = async {
        if let Err(error) = tokio::signal::ctrl_c().await {
            tracing::error!(%error, "cannot listen for Ctrl-C");
            std::future::pending::<()>().await;
        }
    };
    #[cfg(unix)]
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
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! {
        () = ctrl_c => tracing::info!(signal = "SIGINT", "shutdown requested"),
        () = terminate => tracing::info!(signal = "SIGTERM", "shutdown requested"),
    }
}
