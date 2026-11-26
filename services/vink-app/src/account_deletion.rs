//! Replay Identity's durable deletion receipts using the App's existing Identity connection.
use std::time::Duration;

use crate::application::social_retirement::SocialRetirementProgress;
use crate::identity::IdentityClient;
use crate::persistence::PgStore;

pub async fn reconcile(store: PgStore, identity: IdentityClient) {
    let mut timer = tokio::time::interval(Duration::from_secs(10));
    timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut after = None;
    loop {
        timer.tick().await;
        let Ok(operations) = identity.pending_account_deletions(after).await else {
            continue;
        };
        // Walk the queue even when individual operations fail. At the end, next tick starts
        // over and retries unfinished work rather than starving better-behaved receipts.
        after = (operations.len() == 32).then(|| operations[31].operation_id);
        for operation in operations {
            if overdue(operation.accepted_at, time::OffsetDateTime::now_utc()) {
                tracing::error!(operation_id = %operation.operation_id, "account deletion cleanup exceeded 30 days");
            }
            let result = async {
                let command = store.apply_account_deletion(operation).await?;
                store.purge_account_deletion(operation).await?;
                if let Some(command) = command
                    && store.step_social_retirement(command).await?
                        != SocialRetirementProgress::Complete
                {
                    return Ok(false);
                }
                Ok::<_, crate::application::social_retirement::SocialRetirementError>(true)
            }
            .await;
            match result {
                Ok(true) => {
                    if identity
                        .finish_account_deletion(operation.operation_id)
                        .await
                        .is_err()
                    {
                        tracing::warn!(operation_id = %operation.operation_id, "account deletion completion will retry");
                    }
                }
                Ok(false) => {}
                Err(error) => {
                    tracing::warn!(operation_id = %operation.operation_id, %error, "account deletion cleanup will retry")
                }
            }
        }
    }
}

fn overdue(accepted: time::OffsetDateTime, now: time::OffsetDateTime) -> bool {
    now - accepted >= time::Duration::days(30)
}

#[cfg(test)]
mod tests {
    use super::overdue;
    use time::{Duration, macros::datetime};

    #[test]
    fn account_deletion_deadline_uses_the_original_acceptance_instant() {
        let accepted = datetime!(2026-11-26 04:40 UTC);
        let deadline = accepted + Duration::days(30);
        assert!(!overdue(accepted, deadline - Duration::microseconds(1)));
        assert!(overdue(accepted, deadline));
        assert!(overdue(accepted, deadline + Duration::days(1)));
    }
}
