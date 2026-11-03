use super::patterns::{Author, DraftSummary};
use crate::contract::{DraftCard, Publication, WorkCard};

/// The current provider exposes a display name, not a stable author ID or profile URL.
/// Keep the avatar neutral rather than treating a name or the current account as that ID.
pub fn card_author(card: &WorkCard) -> Author {
    Author {
        name: card.author_display_name.clone(),
        key: None,
        published_at: card.published_at.clone(),
    }
}

pub fn publication_author(publication: &Publication) -> Author {
    Author {
        name: publication.author_display_name.clone(),
        key: None,
        published_at: publication.published_at.clone(),
    }
}

pub fn draft_summary(draft: &DraftCard) -> DraftSummary {
    DraftSummary {
        title: crate::draft_list::display_title(&draft.title),
        version: draft.version,
        updated_at: draft.updated_at.clone(),
        href: format!("/ban-viet/{}", draft.work_id),
    }
}
