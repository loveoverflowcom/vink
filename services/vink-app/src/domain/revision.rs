use sha2::{Digest, Sha256};

use super::document::{Document, Title};

/// SHA-256 over a length-prefixed encoding of title and lines (independent of JSON formatting).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentHash(String);

impl ContentHash {
    pub fn of(title: &Title, document: &Document) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(b"vink-revision-content-v1");
        push_str(&mut hasher, title.as_str());
        hasher.update((document.blocks().len() as u64).to_be_bytes());
        for lines in document.blocks() {
            hasher.update((lines.len() as u64).to_be_bytes());
            for line in lines {
                push_str(&mut hasher, line);
            }
        }
        let digest = hasher.finalize();
        let mut hex = String::with_capacity(64);
        for byte in digest.iter() {
            hex.push_str(&format!("{byte:02x}"));
        }
        Self(hex)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

fn push_str(hasher: &mut Sha256, value: &str) {
    hasher.update((value.len() as u64).to_be_bytes());
    hasher.update(value.as_bytes());
}

/// Content of an immutable revision snapshot. Revisions always carry a title.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RevisionContent {
    title: Title,
    document: Document,
    hash: ContentHash,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RevisionError {
    #[error("a revision needs a title")]
    MissingTitle,
}

impl RevisionContent {
    pub fn new(title: Title, document: Document) -> Result<Self, RevisionError> {
        if title.is_empty() {
            return Err(RevisionError::MissingTitle);
        }
        let hash = ContentHash::of(&title, &document);
        Ok(Self {
            title,
            document,
            hash,
        })
    }

    pub fn title(&self) -> &Title {
        &self.title
    }

    pub fn document(&self) -> &Document {
        &self.document
    }

    pub fn hash(&self) -> &ContentHash {
        &self.hash
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::document::{BlockData, DocumentData, SCHEMA_VERSION};

    fn doc(blocks: &[&[&str]]) -> Document {
        Document::parse(DocumentData {
            schema_version: SCHEMA_VERSION,
            blocks: blocks
                .iter()
                .map(|l| BlockData {
                    lines: l.iter().map(|s| s.to_string()).collect(),
                })
                .collect(),
        })
        .unwrap()
    }

    #[test]
    fn hash_distinguishes_line_and_stanza_boundaries() {
        let title = Title::parse("Mưa").unwrap();
        let one_stanza = ContentHash::of(&title, &doc(&[&["a", "b"]]));
        let two_stanzas = ContentHash::of(&title, &doc(&[&["a"], &["b"]]));
        let joined = ContentHash::of(&title, &doc(&[&["ab"]]));
        assert_ne!(one_stanza, two_stanzas);
        assert_ne!(one_stanza, joined);
        assert_eq!(one_stanza, ContentHash::of(&title, &doc(&[&["a", "b"]])));
        assert_eq!(one_stanza.as_str().len(), 64);
    }

    #[test]
    fn canonical_hash_matches_fixed_v1_vector_for_nfc_and_nfd() {
        // Fixed vector: UTF-8 title length 12; two blocks with 2/1 lines of 14/15/13 bytes.
        // Expected SHA-256 was computed independently from those literal canonical bytes,
        // not by calling this implementation or serializing its output.
        const EXPECTED: &str = "88bbbf2e34c3174e82d6a49388dddb70882a216e7ec3ba6b76eea165a49c36dd";
        let nfc = RevisionContent::new(
            Title::parse("Việt Đ/đ").unwrap(),
            doc(&[&["Đêm xuống,", "  đợi trăng"], &["đường xa."]]),
        )
        .unwrap();
        let nfd = RevisionContent::new(
            Title::parse("  Vie\u{0323}\u{0302}t Đ/đ  ").unwrap(),
            doc(&[&["Đe\u{0302}m xuống,", "  đợi trăng"], &["đường xa."]]),
        )
        .unwrap();
        assert_eq!(nfc.hash().as_str(), EXPECTED);
        assert_eq!(nfd.hash().as_str(), EXPECTED);
        assert_eq!(nfc, nfd);
    }

    #[test]
    fn hash_keeps_title_line_lengths_case_and_indentation_significant() {
        let title = Title::parse("Đêm").unwrap();
        let original = ContentHash::of(&title, &doc(&[&["a", "bc"]]));
        assert_ne!(original, ContentHash::of(&title, &doc(&[&["ab", "c"]])));
        assert_ne!(original, ContentHash::of(&title, &doc(&[&[" a", "bc"]])));
        assert_ne!(
            original,
            ContentHash::of(&Title::parse("đêm").unwrap(), &doc(&[&["a", "bc"]]))
        );
        assert_ne!(
            ContentHash::of(&title, &doc(&[&["Đ"]])),
            ContentHash::of(&title, &doc(&[&["đ"]]))
        );
    }

    #[test]
    fn revision_requires_title() {
        assert_eq!(
            RevisionContent::new(Title::parse(" ").unwrap(), Document::empty()),
            Err(RevisionError::MissingTitle)
        );
    }
}
