//! Sign-in rate limits (D-05): count one request of a client network and decide. The network is
//! attributed by the HTTP adapter (`domain::abuse::client_address`); this use case never sees
//! an address, only its bucket key.

use super::Vink;
use super::ports::{RateLimitStore, StoreError};
use crate::domain::abuse::{Admission, Network, RateLimit, decide};

impl<S> Vink<S>
where
    S: RateLimitStore,
{
    /// Counts this request and decides. A storage failure is an error, never an admission: the
    /// caller answers `503`, so an outage cannot switch the limit off.
    pub async fn admit(
        &self,
        limit: &RateLimit,
        network: &Network,
    ) -> Result<Admission, StoreError> {
        let count = self
            .store()
            .count_request(
                limit.bucket,
                &network.key(limit.bucket),
                limit.window_seconds,
            )
            .await?;
        Ok(decide(limit, count.hits, count.seconds_left))
    }

    pub async fn prune_rate_limits(&self) -> Result<u64, StoreError> {
        self.store().prune_rate_limits().await
    }
}
