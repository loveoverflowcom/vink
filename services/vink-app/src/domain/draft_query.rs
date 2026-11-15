//! Private title search and keyset paging (#201). A cursor is a position, never authority.
//! Unlike public discovery, punctuation is retained and matching is a literal substring.

use sha2::{Digest, Sha256};
use time::OffsetDateTime;
use unicode_normalization::{UnicodeNormalization, char::is_combining_mark};

use super::draft::DraftSummary;
use super::ids::{PrincipalId, WorkId};

pub const MAX_PAGE_SIZE: usize = 100;
pub const MAX_QUERY_CHARS: usize = 200;
pub const MAX_QUERY_BYTES: usize = 16 * MAX_QUERY_CHARS;
pub const MAX_KEY_BYTES: usize = 16 * 200;
const HEADER_BYTES: usize = 1 + 32 + 16 + 1;
pub const MAX_CURSOR_BYTES: usize = 2 * (HEADER_BYTES + MAX_KEY_BYTES);

/// Identical to Web's existing `search::search_key`; never applied to stored authored text.
pub fn search_key(text: &str) -> String {
    text.nfd()
        .filter(|c| !is_combining_mark(*c))
        .flat_map(char::to_lowercase)
        .map(|c| if c == 'đ' { 'd' } else { c })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn title_key(title: &str) -> String {
    search_key(if title.is_empty() {
        "Chưa đặt tên"
    } else {
        title
    })
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum DraftSort {
    #[default]
    Recent,
    Title,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DraftPosition {
    Recent {
        updated_at: OffsetDateTime,
        work_id: WorkId,
    },
    Title {
        key: String,
        work_id: WorkId,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum DraftQueryError {
    #[error("q must contain at most 200 NFC characters, 3200 bytes and no NUL")]
    Query,
    #[error("sort must be recent or title")]
    Sort,
    #[error("limit must be an integer between 1 and 100")]
    Limit,
    #[error("cursor is invalid or belongs to another search; start from the first page")]
    Cursor,
}

impl DraftQueryError {
    pub fn field(self) -> &'static str {
        match self {
            Self::Query => "q",
            Self::Sort => "sort",
            Self::Limit => "limit",
            Self::Cursor => "cursor",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DraftSearch {
    query_key: String,
    sort: DraftSort,
    size: usize,
    scope: [u8; 32],
    after: Option<DraftPosition>,
}

impl DraftSearch {
    pub fn parse(
        owner: PrincipalId,
        query: &str,
        sort: Option<&str>,
        limit: Option<&str>,
        cursor: Option<&str>,
    ) -> Result<Self, DraftQueryError> {
        if query.len() > MAX_QUERY_BYTES
            || query.contains('\0')
            || query.nfc().count() > MAX_QUERY_CHARS
        {
            return Err(DraftQueryError::Query);
        }
        let query_key = search_key(query);
        let sort = match sort {
            None | Some("recent") => DraftSort::Recent,
            Some("title") => DraftSort::Title,
            _ => return Err(DraftQueryError::Sort),
        };
        let size = match limit {
            None => MAX_PAGE_SIZE,
            Some(raw) if !raw.is_empty() && raw.bytes().all(|c| c.is_ascii_digit()) => raw
                .parse::<usize>()
                .ok()
                .filter(|n| (1..=MAX_PAGE_SIZE).contains(n))
                .ok_or(DraftQueryError::Limit)?,
            _ => return Err(DraftQueryError::Limit),
        };
        let mut digest = Sha256::new();
        digest.update(b"vink-private-drafts-v1");
        digest.update(owner.as_uuid().as_bytes());
        digest.update([sort as u8, size as u8]);
        digest.update(query_key.as_bytes());
        let scope = digest.finalize().into();
        let mut search = Self {
            query_key,
            sort,
            size,
            scope,
            after: None,
        };
        if let Some(raw) = cursor {
            search.after = Some(search.decode_cursor(raw)?);
        }
        Ok(search)
    }

    pub fn query_key(&self) -> &str {
        &self.query_key
    }
    pub fn sort(&self) -> DraftSort {
        self.sort
    }
    pub fn size(&self) -> usize {
        self.size
    }
    pub fn fetch(&self) -> i64 {
        (self.size + 1) as i64
    }
    pub fn after(&self) -> Option<&DraftPosition> {
        self.after.as_ref()
    }

    fn decode_cursor(&self, raw: &str) -> Result<DraftPosition, DraftQueryError> {
        let invalid = DraftQueryError::Cursor;
        if raw.len() < 2 * HEADER_BYTES
            || raw.len() > MAX_CURSOR_BYTES
            || !raw.len().is_multiple_of(2)
        {
            return Err(invalid);
        }
        let bytes: Vec<u8> = raw
            .as_bytes()
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| {
                let digit = |v| match v {
                    b'0'..=b'9' => Ok(v - b'0'),
                    b'a'..=b'f' => Ok(v - b'a' + 10),
                    _ => Err(invalid),
                };
                Ok((digit(pair[0])? << 4) | digit(pair[1])?)
            })
            .collect::<Result<_, _>>()?;
        if bytes[0] != 1 || bytes[1..33] != self.scope || bytes[49] != self.sort as u8 {
            return Err(invalid);
        }
        let work_id = WorkId::from_uuid(uuid::Uuid::from_bytes(
            bytes[33..49].try_into().map_err(|_| invalid)?,
        ));
        let position = &bytes[HEADER_BYTES..];
        match self.sort {
            DraftSort::Recent => {
                let micros = i64::from_be_bytes(position.try_into().map_err(|_| invalid)?);
                let updated_at =
                    OffsetDateTime::from_unix_timestamp_nanos(i128::from(micros) * 1000)
                        .map_err(|_| invalid)?;
                Ok(DraftPosition::Recent {
                    updated_at,
                    work_id,
                })
            }
            DraftSort::Title => {
                let key = String::from_utf8(position.to_vec()).map_err(|_| invalid)?;
                if key.contains('\0') || search_key(&key) != key {
                    return Err(invalid);
                }
                Ok(DraftPosition::Title { key, work_id })
            }
        }
    }

    fn cursor(&self, last: &DraftSummary) -> String {
        let mut bytes = vec![1];
        bytes.extend_from_slice(&self.scope);
        bytes.extend_from_slice(last.work_id.as_uuid().as_bytes());
        bytes.push(self.sort as u8);
        match self.sort {
            DraftSort::Recent => bytes.extend_from_slice(
                &((last.updated_at.unix_timestamp_nanos() / 1000) as i64).to_be_bytes(),
            ),
            DraftSort::Title => bytes.extend_from_slice(title_key(last.title.as_str()).as_bytes()),
        }
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }

    /// Input is already filtered and sorted by SQL, with at most `fetch()` summaries.
    pub fn page(&self, mut drafts: Vec<DraftSummary>) -> DraftPage {
        let more = drafts.len() > self.size;
        drafts.truncate(self.size);
        let next_cursor = more
            .then(|| drafts.last().map(|last| self.cursor(last)))
            .flatten();
        DraftPage {
            drafts,
            next_cursor,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DraftPage {
    pub drafts: Vec<DraftSummary>,
    pub next_cursor: Option<String>,
}

#[cfg(test)]
mod tests;
