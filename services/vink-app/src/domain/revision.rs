use sha2::{Digest, Sha256};

use super::document::{Title, TitledDocument};

/// Domain separator of the one canonical content hash. It is not versioned: there is one document
/// contract (`VDR-DOCUMENT-SOURCE-FORMAT@r2`).
const DOMAIN: &[u8] = b"vink-document-content";

/// SHA-256 over a length-prefixed encoding of format, effective title and exact source bytes
/// (independent of JSON formatting). This is a content digest, not an HTTP `ETag`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentHash(String);

impl ContentHash {
    pub fn of(content: &TitledDocument) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(DOMAIN);
        push_str(&mut hasher, content.format().as_str());
        push_str(&mut hasher, content.title().as_str());
        push_str(&mut hasher, content.document().source());
        Self(hex_digest(hasher.finalize()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn parse(raw: &str) -> Option<Self> {
        if raw.len() == 64 && raw.bytes().all(|b| b.is_ascii_hexdigit()) {
            Some(Self(raw.to_ascii_lowercase()))
        } else {
            None
        }
    }
}

impl std::fmt::Display for ContentHash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

fn push_str(hasher: &mut Sha256, value: &str) {
    hasher.update((value.len() as u64).to_be_bytes());
    hasher.update(value.as_bytes());
}

fn hex_digest(digest: impl AsRef<[u8]>) -> String {
    let bytes = digest.as_ref();
    let mut hex = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        hex.push_str(&format!("{byte:02x}"));
    }
    hex
}

/// Publication policy is evaluated only for new submissions, separately from history restoration.
pub fn submission_issues(
    content: &TitledDocument,
) -> vink_markdown_text::submission::SubmissionIssues {
    use super::document::DocumentFormat;
    use vink_markdown_text::submission::{SubmissionBody, check_submission};
    let source = content.document().source();
    let body = match content.format() {
        DocumentFormat::Default => SubmissionBody::Literal(source),
        DocumentFormat::Markdown => SubmissionBody::Markdown(source),
    };
    check_submission(content.title().as_str(), body)
}

/// Content of an immutable revision snapshot. Revisions always carry a title.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RevisionContent {
    content: TitledDocument,
    hash: ContentHash,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RevisionError {
    #[error("a revision needs a title")]
    MissingTitle,
}

impl RevisionContent {
    pub fn new(content: TitledDocument) -> Result<Self, RevisionError> {
        if content.title().is_empty() {
            return Err(RevisionError::MissingTitle);
        }
        let hash = ContentHash::of(&content);
        Ok(Self { content, hash })
    }

    pub fn title(&self) -> &Title {
        self.content.title()
    }

    pub fn content(&self) -> &TitledDocument {
        &self.content
    }

    pub fn hash(&self) -> &ContentHash {
        &self.hash
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::document::{Document, DocumentFormat};

    fn titled(format: DocumentFormat, title: Option<&str>, source: &str) -> TitledDocument {
        TitledDocument::author(Document::new(format, source).unwrap(), title).unwrap()
    }

    fn default(title: &str, source: &str) -> ContentHash {
        ContentHash::of(&titled(DocumentFormat::Default, Some(title), source))
    }

    #[test]
    fn canonical_hash_matches_independent_vectors() {
        // Expected values computed in Python from literal bytes:
        // sha256(b"vink-document-content" + u64be(len) + b"default" + u64be(len) + title + ...).
        const DEFAULT: &str = "f735c19566fdff2931d7a5944bb57882c4a915bd0fecb29f1d3c46119ed2c79b";
        assert_eq!(
            default("Tên riêng", "# Tên trong thân\nĐoạn sau.").as_str(),
            DEFAULT
        );
        // Markdown, NFD source with blank, whitespace-only and CRLF lines; title from the first H1.
        const MARKDOWN: &str = "8ba939c87570af370209535a832b9b86d550eaf5a5f59f3c1f135647c8061769";
        let source = "# Tie\u{0302}\u{0301}ng mưa\r\n\r\n\r\n  \r\n\u{0110}oa\u{0323}n hai\t\u{1f9d1}\u{200d}\u{1f33e}\n";
        let content = titled(DocumentFormat::Markdown, None, source);
        assert_eq!(content.title().as_str(), "Tiếng mưa");
        assert_eq!(ContentHash::of(&content).as_str(), MARKDOWN);
    }

    #[test]
    fn every_byte_of_format_title_and_source_is_significant() {
        let base = default("Mưa", "a\nb");
        assert_eq!(
            base,
            default("  Mưa ", "a\nb"),
            "title is trimmed/NFC first"
        );
        assert_ne!(base, default("Mưa", "a\r\nb"), "line endings are content");
        assert_ne!(base, default("Mưa", "a\n\nb"), "blank lines are content");
        assert_ne!(base, default("Mưa", " a\nb"), "indentation is content");
        assert_ne!(base, default("mưa", "a\nb"), "case is content");
        assert_ne!(
            default("Việt", "Vie\u{0323}\u{0302}t"),
            default("Việt", "Việt"),
            "source keeps its Unicode form"
        );
        assert_eq!(
            default("Vie\u{0323}\u{0302}t", "x"),
            default("Việt", "x"),
            "the title projection is NFC"
        );
        // Length prefixes keep title/source boundaries unambiguous.
        assert_ne!(default("ab", "c"), default("a", "bc"));
        let markdown = ContentHash::of(&titled(DocumentFormat::Markdown, None, "# Mưa\nx"));
        assert_ne!(default("Mưa", "# Mưa\nx"), markdown, "format is content");
    }

    #[test]
    fn revision_requires_a_title_for_every_format() {
        assert_eq!(
            RevisionContent::new(titled(DocumentFormat::Default, Some(" "), "Thân")),
            Err(RevisionError::MissingTitle)
        );
        for source in ["Thân bài.", "#\n\n# Tên sau", "<h1>Tên</h1>"] {
            assert_eq!(
                RevisionContent::new(titled(DocumentFormat::Markdown, None, source)),
                Err(RevisionError::MissingTitle),
                "{source:?}: the UI placeholder never satisfies MissingTitle"
            );
        }
        let revision =
            RevisionContent::new(titled(DocumentFormat::Markdown, None, "# Mưa")).unwrap();
        assert_eq!(revision.title().as_str(), "Mưa");
        assert_eq!(revision.hash(), &ContentHash::of(revision.content()));
    }
}
