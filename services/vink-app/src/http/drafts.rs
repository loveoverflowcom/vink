//! Strict, bounded private-list query decoding. Errors never echo a title/query/cursor.
use super::ApiError;
use crate::application::drafts::SearchDrafts;

fn invalid() -> ApiError {
    ApiError::Validation {
        field: "query",
        message: "unknown, repeated, malformed or oversized draft query".into(),
    }
}

fn decode(raw: &str) -> Result<String, ApiError> {
    let mut decoded = Vec::with_capacity(raw.len());
    let mut bytes = raw.bytes();
    while let Some(c) = bytes.next() {
        match c {
            b'+' => decoded.push(b' '),
            b'%' => {
                let digit = |v: Option<u8>| {
                    v.and_then(|v| (v as char).to_digit(16))
                        .map(|v| v as u8)
                        .ok_or_else(invalid)
                };
                decoded.push((digit(bytes.next())? << 4) | digit(bytes.next())?);
            }
            _ => decoded.push(c),
        }
    }
    String::from_utf8(decoded).map_err(|_| invalid())
}

pub(super) fn query(raw: Option<&str>) -> Result<SearchDrafts, ApiError> {
    let raw = raw.unwrap_or_default();
    if raw.len() > 32_768 {
        return Err(invalid());
    }
    let mut search = SearchDrafts::default();
    if raw.is_empty() {
        return Ok(search);
    }
    for part in raw.split('&') {
        let (key, value) = part.split_once('=').ok_or_else(invalid)?;
        let key = decode(key)?;
        let slot = match key.as_str() {
            "q" => &mut search.query,
            "sort" => &mut search.sort,
            "limit" => &mut search.limit,
            "cursor" => &mut search.cursor,
            _ => return Err(invalid()),
        };
        if slot.is_some() {
            return Err(invalid());
        }
        *slot = Some(decode(value)?);
    }
    Ok(search)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn strict_bounded_query_does_not_silently_widen_search() {
        for raw in [
            "q=a&q=b",
            "sort=recent&sort=title",
            "limit=1&limit=1",
            "cursor=x&cursor=x",
            "owner_id=x",
            "q=%",
            "q=%xx",
            "q=%ff",
            "q",
            "&",
            "q=x&",
        ] {
            assert!(query(Some(raw)).is_err(), "{raw}");
        }
        assert!(query(Some(&"x".repeat(32_769))).is_err());
        assert_eq!(
            query(Some(
                "q=%C4%90%C6%B0%E1%BB%9Dng+v%E1%BB%81&sort=title&limit=24"
            ))
            .unwrap()
            .query
            .as_deref(),
            Some("Đường về")
        );
        assert_eq!(query(None).unwrap(), SearchDrafts::default());
    }
}
