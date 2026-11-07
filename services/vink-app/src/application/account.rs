//! Account use cases (D-05/S-03 spike): find the principal of an authenticated subject and
//! provision one on first sign-in. Authentication is Identity's; this module only decides which
//! Vink principal acts and never grants a role.

use uuid::Uuid;

use super::Vink;
use super::ports::{BoundPrincipal, PrincipalStore, Provisioned, StoreError};
use crate::domain::ids::PrincipalId;
use crate::domain::principal::{PenName, SubjectKey};

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum AccountError {
    #[error("pen name must be 1–80 characters without control characters")]
    InvalidPenName,
    #[error("storage unavailable")]
    Unavailable,
}

impl From<StoreError> for AccountError {
    fn from(_: StoreError) -> Self {
        // Integrity and unknown-principal errors are not expected here; none may be read as
        // "no binding", so every failure is unavailable.
        AccountError::Unavailable
    }
}

impl<S> Vink<S>
where
    S: PrincipalStore,
{
    /// `None`: the subject is authenticated but has no Vink principal yet.
    pub async fn principal_for_subject(
        &self,
        subject: &SubjectKey,
    ) -> Result<Option<BoundPrincipal>, AccountError> {
        Ok(self.store().principal_for_subject(subject).await?)
    }

    /// Creates the subject's principal with `pen_name`, or returns the existing one unchanged (a
    /// retry after a lost answer, or a concurrent first request, never creates a second).
    pub async fn provision_principal(
        &self,
        subject: &SubjectKey,
        pen_name: &str,
    ) -> Result<Provisioned, AccountError> {
        let pen_name = PenName::parse(pen_name).ok_or(AccountError::InvalidPenName)?;
        let candidate = PrincipalId::from_uuid(Uuid::now_v7());
        Ok(self
            .store()
            .provision_principal(subject, candidate, &pen_name)
            .await?)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;
    use crate::domain::principal::Standing;

    #[derive(Default)]
    struct Recording {
        calls: Mutex<Vec<String>>,
    }

    impl PrincipalStore for Recording {
        async fn principal_for_subject(
            &self,
            _subject: &SubjectKey,
        ) -> Result<Option<BoundPrincipal>, StoreError> {
            Err(StoreError::Integrity("unexpected".into()))
        }

        async fn provision_principal(
            &self,
            subject: &SubjectKey,
            candidate: PrincipalId,
            pen_name: &PenName,
        ) -> Result<Provisioned, StoreError> {
            self.calls.lock().unwrap().push(format!(
                "{}/{}:{}",
                subject.authority(),
                subject.id(),
                pen_name.as_str()
            ));
            Ok(Provisioned {
                bound: BoundPrincipal {
                    principal: candidate,
                    standing: Standing::Active,
                },
                created: true,
            })
        }
    }

    #[tokio::test]
    async fn an_invalid_pen_name_never_reaches_storage_and_failures_are_never_no_binding() {
        let vink = Vink::new(Recording::default());
        let subject = SubjectKey::new("vink-identity", "s-1").unwrap();
        assert_eq!(
            vink.provision_principal(&subject, " \n ").await,
            Err(AccountError::InvalidPenName)
        );
        assert!(vink.store().calls.lock().unwrap().is_empty());
        let provisioned = vink
            .provision_principal(&subject, " Thơ Mới ")
            .await
            .unwrap();
        assert!(provisioned.created);
        assert_eq!(
            vink.store().calls.lock().unwrap().as_slice(),
            ["vink-identity/s-1:Thơ Mới"]
        );
        // A storage integrity error is "unavailable", not "unbound".
        assert_eq!(
            vink.principal_for_subject(&subject).await,
            Err(AccountError::Unavailable)
        );
    }
}
