//! Separates "the database could not serve this" from statement and data errors.

/// Connection-level failures: I/O, TLS, pool timeout/closure, a crashed connection worker, and
/// SQLSTATE classes 08 (connection exception), 53 (insufficient resources) and 57 (operator
/// intervention). Retrying later may succeed. Everything else — constraints, privileges,
/// invariants — is for the owning adapter to classify.
pub fn is_unavailable(error: &sqlx::Error) -> bool {
    match error {
        sqlx::Error::Database(db) => {
            let code = db.code().unwrap_or_default();
            code.starts_with("08") || code.starts_with("53") || code.starts_with("57")
        }
        sqlx::Error::Io(_)
        | sqlx::Error::Tls(_)
        | sqlx::Error::PoolTimedOut
        | sqlx::Error::PoolClosed
        | sqlx::Error::WorkerCrashed => true,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transport_failures_are_unavailable_and_data_errors_are_not() {
        assert!(is_unavailable(&sqlx::Error::PoolTimedOut));
        assert!(is_unavailable(&sqlx::Error::PoolClosed));
        assert!(is_unavailable(&sqlx::Error::Io(std::io::Error::from(
            std::io::ErrorKind::ConnectionRefused
        ))));
        assert!(!is_unavailable(&sqlx::Error::RowNotFound));
        assert!(!is_unavailable(&sqlx::Error::Protocol("unexpected".into())));
    }
}
