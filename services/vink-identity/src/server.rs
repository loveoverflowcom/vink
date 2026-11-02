//! The Unix socket and its accept loop.
//!
//! Each connection is HTTP/1.1 through hyper with a header-read timeout, so a peer that stalls
//! before finishing its request head is closed instead of holding the server open. The peer's OS
//! user is attached to every request as [`ConnectInfo<Peer>`]. Shutdown first removes the socket
//! file while the listener still holds it (so its inode cannot have been reused by a successor),
//! then stops accepting, lets open requests finish for a bounded grace period and closes the rest.

use std::io;
use std::os::unix::fs::{FileTypeExt, MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::time::Duration;

use axum::Router;
use axum::extract::connect_info::ConnectInfo;
use hyper::server::conn::http1;
use hyper_util::rt::{TokioIo, TokioTimer};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::watch;
use tokio::task::JoinSet;
use tower::ServiceExt;

use crate::http::Peer;

/// A peer must send its whole request head within this time.
pub const HEADER_TIMEOUT: Duration = Duration::from_secs(2);
/// How long shutdown waits for open requests before closing them.
pub const SHUTDOWN_GRACE: Duration = Duration::from_secs(5);

#[derive(Debug, PartialEq, Eq)]
pub enum SocketError {
    /// The path or its directory is unsuitable; fixing configuration is required.
    Unsuitable(String),
    /// Another process serves the path, or the filesystem refused an operation.
    Unavailable(String),
}

/// The socket file this process created, identified by device and inode.
#[derive(Debug)]
pub struct BoundSocket {
    path: PathBuf,
    device: u64,
    inode: u64,
}

impl BoundSocket {
    /// Removes the socket only if the path still holds the file this process bound; a successor
    /// that replaced it keeps its socket. Call it while the listener is open: once it is closed the
    /// filesystem may give a successor's socket the same inode.
    pub fn remove(&self) {
        match std::fs::symlink_metadata(&self.path) {
            Ok(meta)
                if meta.file_type().is_socket()
                    && meta.dev() == self.device
                    && meta.ino() == self.inode =>
            {
                if let Err(error) = std::fs::remove_file(&self.path) {
                    tracing::warn!(%error, "cannot remove the socket");
                }
            }
            Ok(_) => tracing::info!("socket path now belongs to another instance; left in place"),
            Err(_) => {}
        }
    }
}

/// Checks the directory, replaces only a socket nobody listens on, binds with mode `0660`.
/// The directory may grant group search (`x`) and read, never group write or anything to others.
pub fn bind(path: &Path) -> Result<(UnixListener, BoundSocket), SocketError> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let directory = std::fs::metadata(parent).map_err(|_| {
        SocketError::Unsuitable(format!("directory {} does not exist", parent.display()))
    })?;
    if !directory.is_dir() || directory.permissions().mode() & 0o027 != 0 {
        return Err(SocketError::Unsuitable(format!(
            "{} must be a directory without access for other users and without group write (chmod g-w,o-rwx)",
            parent.display()
        )));
    }
    match std::fs::symlink_metadata(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Ok(existing) if existing.file_type().is_socket() => {
            match std::os::unix::net::UnixStream::connect(path) {
                Err(error) if error.kind() == io::ErrorKind::ConnectionRefused => {
                    tracing::warn!(socket = %path.display(), "replacing a stale socket");
                    std::fs::remove_file(path).map_err(|error| {
                        SocketError::Unavailable(format!("remove stale socket: {error}"))
                    })?;
                }
                Ok(_) => {
                    return Err(SocketError::Unavailable(format!(
                        "another process is already serving {}",
                        path.display()
                    )));
                }
                Err(error) => {
                    return Err(SocketError::Unavailable(format!(
                        "cannot tell whether {} is in use ({error}); refusing to replace it",
                        path.display()
                    )));
                }
            }
        }
        Ok(_) => {
            return Err(SocketError::Unsuitable(format!(
                "{} exists and is not a socket; refusing to replace it",
                path.display()
            )));
        }
        Err(error) => {
            return Err(SocketError::Unavailable(format!(
                "inspect socket path: {error}"
            )));
        }
    }
    let listener = UnixListener::bind(path).map_err(|error| {
        SocketError::Unavailable(format!("cannot bind {}: {error}", path.display()))
    })?;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o660))
        .map_err(|error| SocketError::Unavailable(format!("set socket permissions: {error}")))?;
    let meta = std::fs::symlink_metadata(path)
        .map_err(|error| SocketError::Unavailable(format!("inspect bound socket: {error}")))?;
    Ok((
        listener,
        BoundSocket {
            path: path.to_path_buf(),
            device: meta.dev(),
            inode: meta.ino(),
        },
    ))
}

/// Serves `router` until `shutdown` resolves, then removes the socket, stops accepting and drains
/// open connections for at most [`SHUTDOWN_GRACE`].
pub async fn serve(
    listener: UnixListener,
    bound: BoundSocket,
    router: Router,
    shutdown: impl Future<Output = ()>,
) {
    let (closing, _) = watch::channel(false);
    let mut connections = JoinSet::new();
    let mut shutdown = std::pin::pin!(shutdown);
    loop {
        tokio::select! {
            () = &mut shutdown => break,
            accepted = listener.accept() => match accepted {
                Ok((stream, _)) => {
                    connections.spawn(connection(stream, router.clone(), closing.subscribe()));
                }
                Err(error) => {
                    tracing::warn!(%error, "accept failed");
                    tokio::time::sleep(Duration::from_millis(50)).await;
                }
            },
            Some(_) = connections.join_next(), if !connections.is_empty() => {}
        }
    }
    bound.remove();
    drop(listener);
    let _ = closing.send(true);
    let drained = tokio::time::timeout(SHUTDOWN_GRACE, async {
        while connections.join_next().await.is_some() {}
    })
    .await;
    if drained.is_err() {
        tracing::warn!(
            open = connections.len(),
            "closing connections still open after the shutdown grace period"
        );
        connections.abort_all();
        while connections.join_next().await.is_some() {}
    }
}

async fn connection(stream: UnixStream, router: Router, mut closing: watch::Receiver<bool>) {
    let peer = Peer {
        uid: stream.peer_cred().ok().map(|cred| cred.uid()),
    };
    let service = hyper::service::service_fn(move |mut request: hyper::Request<_>| {
        request.extensions_mut().insert(ConnectInfo(peer));
        router.clone().oneshot(request)
    });
    let mut builder = http1::Builder::new();
    builder
        .timer(TokioTimer::new())
        .header_read_timeout(HEADER_TIMEOUT);
    let connection = builder.serve_connection(TokioIo::new(stream), service);
    let mut connection = std::pin::pin!(connection);
    tokio::select! {
        result = connection.as_mut() => {
            if let Err(error) = result {
                tracing::debug!(%error, "connection closed with an error");
            }
        }
        _ = closing.changed() => {
            connection.as_mut().graceful_shutdown();
            let _ = connection.await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A private temporary directory with a short path (Unix socket paths are limited).
    fn private_dir(name: &str, mode: u32) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("vink-id-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(mode)).unwrap();
        dir
    }

    #[tokio::test]
    async fn directory_must_not_be_open_to_others_or_group_writable() {
        for mode in [0o755, 0o770, 0o701] {
            let dir = private_dir(&format!("mode{mode:o}"), mode);
            let result = bind(&dir.join("s.sock"));
            assert!(
                matches!(result, Err(SocketError::Unsuitable(_))),
                "{mode:o}"
            );
            std::fs::remove_dir_all(&dir).unwrap();
        }
        let dir = private_dir("mode750", 0o750);
        let (_listener, bound) = bind(&dir.join("s.sock")).unwrap();
        let mode = std::fs::metadata(dir.join("s.sock"))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o660);
        bound.remove();
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[tokio::test]
    async fn only_a_stale_socket_is_replaced() {
        let dir = private_dir("stale", 0o700);
        let path = dir.join("s.sock");

        let (live, _bound) = bind(&path).unwrap();
        assert!(
            matches!(bind(&path), Err(SocketError::Unavailable(m)) if m.contains("already serving"))
        );
        drop(live); // the file stays, nobody listens: stale
        let (_again, bound) = bind(&path).unwrap();
        bound.remove();
        assert!(!path.exists());

        std::fs::write(&path, "data").unwrap();
        assert!(matches!(bind(&path), Err(SocketError::Unsuitable(_))));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "data");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[tokio::test]
    async fn removal_spares_a_successor_socket() {
        let dir = private_dir("successor", 0o700);
        let path = dir.join("s.sock");
        let (_first, first_bound) = bind(&path).unwrap();
        // While the first listener is still open, something else replaces the path.
        std::fs::remove_file(&path).unwrap();
        let (_second, second_bound) = bind(&path).unwrap();
        first_bound.remove();
        assert!(
            path.exists(),
            "the first instance removed its successor's socket"
        );
        second_bound.remove();
        assert!(!path.exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
