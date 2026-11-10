//! Sign-in rate-limit counters (D-05): one atomic upsert per request. The window is fixed and
//! derived from the database clock, so every App process agrees on it; the row lock of the
//! upsert serialises concurrent requests of one network, so the count is exact.

use sqlx::Row;

use super::PgStore;
use super::postgres::classify;
use crate::application::ports::{RateLimitStore, StoreError, WindowCount};

impl RateLimitStore for PgStore {
    async fn count_request(
        &self,
        bucket: &str,
        network_key: &[u8; 32],
        window_seconds: i64,
    ) -> Result<WindowCount, StoreError> {
        let row = sqlx::query(
            "WITH w AS (
                 SELECT to_timestamp(floor(extract(epoch FROM now()) / $3) * $3) AS start)
             INSERT INTO auth_rate_limits (bucket, network_key, window_start, window_seconds, hits)
             SELECT $1, $2, w.start, $3, 1 FROM w
             ON CONFLICT (bucket, network_key, window_start)
             DO UPDATE SET hits = auth_rate_limits.hits + 1
             RETURNING hits::bigint AS hits,
                       ceil(extract(epoch FROM window_start + make_interval(secs => $3) - now()))::bigint
                           AS seconds_left",
        )
        .bind(bucket)
        .bind(network_key.as_slice())
        .bind(window_seconds as i32)
        .fetch_one(self.pool())
        .await
        .map_err(classify)?;
        Ok(WindowCount {
            hits: row.try_get("hits").map_err(classify)?,
            seconds_left: row.try_get("seconds_left").map_err(classify)?,
        })
    }

    async fn prune_rate_limits(&self) -> Result<u64, StoreError> {
        Ok(sqlx::query(
            "DELETE FROM auth_rate_limits
              WHERE window_start + make_interval(secs => window_seconds) < now() - interval '1 day'",
        )
        .execute(self.pool())
        .await
        .map_err(classify)?
        .rows_affected())
    }
}
