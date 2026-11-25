//! Owner-scoped use cases. Only the resolved session selects the membership key.
use time::OffsetDateTime;
use uuid::Uuid;

use super::{Vink, ports::StoreError, profile::AuthorAttribution};
use crate::domain::{
    ids::{PrincipalId, WorkId},
    saved_work::{SaveError, SaveIntent, SavedGeneration, SavedWork, SavedWorkKey, SetSaved},
};

#[derive(Debug)]
pub enum SavedWorksError {
    Store(StoreError),
    Decision(SaveError),
    WrongPrincipal,
    InvalidCursor,
}

impl From<StoreError> for SavedWorksError {
    fn from(error: StoreError) -> Self {
        Self::Store(error)
    }
}

pub enum ListItem {
    Readable {
        work_id: Uuid,
        generation: i64,
        saved_at: OffsetDateTime,
        title: String,
        form: Option<String>,
        author: AuthorAttribution,
    },
    Hidden {
        work_id: Uuid,
        generation: i64,
    },
    AuthorDeleted {
        work_id: Uuid,
        generation: i64,
    },
}

pub struct SavedPage {
    pub items: Vec<ListItem>,
    pub next_cursor: Option<String>,
}

pub trait SavedWorksStore {
    fn membership(
        &self,
        key: SavedWorkKey,
    ) -> impl Future<Output = Result<SavedWork, StoreError>> + Send;
    fn set_membership(
        &self,
        command: SetSaved,
    ) -> impl Future<Output = Result<SavedWork, SavedWorksError>> + Send;
    fn list_memberships(
        &self,
        owner: PrincipalId,
        cursor: Option<&str>,
    ) -> impl Future<Output = Result<SavedPage, SavedWorksError>> + Send;
}

impl<S: SavedWorksStore> Vink<S> {
    pub async fn saved_work(
        &self,
        actor: PrincipalId,
        work: WorkId,
    ) -> Result<SavedWork, StoreError> {
        self.store
            .membership(SavedWorkKey {
                principal: actor,
                work,
            })
            .await
    }

    pub async fn set_saved_work(
        &self,
        actor: PrincipalId,
        expected_principal: PrincipalId,
        work: WorkId,
        desired: SaveIntent,
        expected_generation: SavedGeneration,
    ) -> Result<SavedWork, SavedWorksError> {
        if actor != expected_principal {
            return Err(SavedWorksError::WrongPrincipal);
        }
        self.store
            .set_membership(SetSaved {
                key: SavedWorkKey {
                    principal: actor,
                    work,
                },
                desired,
                expected_generation,
            })
            .await
    }

    pub async fn saved_works(
        &self,
        actor: PrincipalId,
        cursor: Option<&str>,
    ) -> Result<SavedPage, SavedWorksError> {
        self.store.list_memberships(actor, cursor).await
    }
}
