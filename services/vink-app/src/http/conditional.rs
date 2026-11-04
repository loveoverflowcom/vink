//! Conditional public reads (READ-01 pilot, option A).
//!
//! The validator is a **strong** entity tag computed from the exact JSON bytes of the selected
//! representation, after the use case has answered. Body and tag therefore come from one
//! serialization of one query result, so no request can pair a validator with another body, and
//! every displayed field (title, byline, document, order, membership) is covered without a
//! dependency list to keep in sync. A `304` still runs the same SQL and serialization; it only
//! saves the body bytes. No content coding is applied, so the bytes hashed are the bytes sent;
//! adding compression requires a per-coding tag (or a weak one) first.
//!
//! Only the two bounded public reads use this. Drafts, discussion and errors never do.

use axum::body::Body;
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use sha2::{Digest, Sha256};

use super::ApiError;

/// Successful public reads: a cache (the browser's) may store the response but must revalidate
/// with the server before each reuse; shared caches are not part of the pilot.
pub(super) const PUBLIC_READ_CACHE_CONTROL: &str = "private, no-cache";
/// Public read errors (`404`, `503`, `500`) are never stored, so recovery is not masked.
pub(super) const PUBLIC_ERROR_CACHE_CONTROL: &str = "no-store";

/// Larger `If-None-Match` fields are ignored (answered with the full `200`), never trusted.
const MAX_FIELD_BYTES: usize = 4096;
const MAX_TAGS: usize = 32;
/// 128 bits of SHA-256 as 32 hex digits.
const TAG_BYTES: usize = 16;

/// The client's `If-None-Match`, parsed per RFC 9110 §13.1.2 / §8.8.3.
#[derive(Debug, PartialEq, Eq)]
enum IfNoneMatch {
    Absent,
    /// `*`: any current representation.
    Any,
    /// Opaque tags without quotes or `W/` (If-None-Match uses weak comparison).
    Tags(Vec<Vec<u8>>),
    /// Malformed or oversized: evaluated as if absent, so the answer is a full `200`.
    Ignored,
}

impl IfNoneMatch {
    fn from_headers(headers: &HeaderMap) -> Self {
        let mut lines = headers.get_all(header::IF_NONE_MATCH).iter().peekable();
        if lines.peek().is_none() {
            return Self::Absent;
        }
        // Field lines combine into one comma-separated list (RFC 9110 §5.3).
        let mut field = Vec::new();
        for (index, line) in lines.enumerate() {
            if index > 0 {
                field.push(b',');
            }
            field.extend_from_slice(line.as_bytes());
            if field.len() > MAX_FIELD_BYTES {
                return Self::Ignored;
            }
        }
        Self::parse(&field)
    }

    fn parse(field: &[u8]) -> Self {
        if trim_ows(field) == b"*" {
            return Self::Any;
        }
        let mut tags = Vec::new();
        let mut rest = field;
        loop {
            rest = trim_start_ows(rest);
            match rest.first() {
                None => break,
                // Empty list elements are allowed and ignored (RFC 9110 §5.6.1).
                Some(b',') => {
                    rest = &rest[1..];
                    continue;
                }
                Some(_) => {}
            }
            let Some((opaque, after)) = entity_tag(rest) else {
                return Self::Ignored;
            };
            tags.push(opaque.to_vec());
            if tags.len() > MAX_TAGS {
                return Self::Ignored;
            }
            rest = trim_start_ows(after);
            match rest.first() {
                None => break,
                Some(b',') => rest = &rest[1..],
                Some(_) => return Self::Ignored,
            }
        }
        if tags.is_empty() {
            Self::Ignored
        } else {
            Self::Tags(tags)
        }
    }

    /// Called only once the resource is known to have a current representation.
    fn matches(&self, current: &[u8]) -> bool {
        match self {
            Self::Any => true,
            Self::Tags(tags) => tags.iter().any(|tag| tag == current),
            Self::Absent | Self::Ignored => false,
        }
    }
}

/// `entity-tag = [ "W/" ] DQUOTE *etagc DQUOTE`; returns the opaque bytes and the remainder.
fn entity_tag(input: &[u8]) -> Option<(&[u8], &[u8])> {
    let input = input.strip_prefix(b"W/").unwrap_or(input);
    let input = input.strip_prefix(b"\"")?;
    let end = input.iter().position(|&byte| byte == b'"')?;
    let opaque = &input[..end];
    // etagc = %x21 / %x23-7E / obs-text; a comma is legal inside the quotes.
    opaque
        .iter()
        .all(|&byte| byte == 0x21 || (0x23..=0x7e).contains(&byte) || byte >= 0x80)
        .then_some((opaque, &input[end + 1..]))
}

fn trim_start_ows(input: &[u8]) -> &[u8] {
    let start = input
        .iter()
        .position(|&byte| byte != b' ' && byte != b'\t')
        .unwrap_or(input.len());
    &input[start..]
}

fn trim_ows(input: &[u8]) -> &[u8] {
    let input = trim_start_ows(input);
    let end = input
        .iter()
        .rposition(|&byte| byte != b' ' && byte != b'\t')
        .map_or(0, |index| index + 1);
    &input[..end]
}

/// The opaque part (without quotes) of the strong tag of `body`.
fn opaque_tag(body: &[u8]) -> String {
    let digest = Sha256::digest(body);
    digest[..TAG_BYTES]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// Serializes `value` once and answers `304` when the request's `If-None-Match` matches the tag
/// of those bytes, otherwise `200` with them. The caller has already resolved visibility and
/// every error, so a `304` is only ever the confirmation of a representation that exists now.
pub(super) fn respond<T: Serialize>(request: &HeaderMap, value: &T) -> Result<Response, ApiError> {
    let body = serde_json::to_vec(value).map_err(|error| {
        tracing::error!(%error, "public read serialization failed");
        ApiError::Internal
    })?;
    let opaque = opaque_tag(&body);
    let etag = HeaderValue::try_from(format!("\"{opaque}\"")).map_err(|_| ApiError::Internal)?;
    let mut response = if IfNoneMatch::from_headers(request).matches(opaque.as_bytes()) {
        // RFC 9110 §15.4.5: no content and no representation metadata beyond the validator and
        // cache policy that the matching 200 carries.
        StatusCode::NOT_MODIFIED.into_response()
    } else {
        let mut response = Body::from(body).into_response();
        response.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        );
        response
    };
    let headers = response.headers_mut();
    headers.insert(header::ETAG, etag);
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static(PUBLIC_READ_CACHE_CONTROL),
    );
    Ok(response)
}

#[cfg(test)]
mod tests {
    use http_body_util::BodyExt;
    use serde_json::json;

    use super::*;

    fn headers(lines: &[&[u8]]) -> HeaderMap {
        let mut headers = HeaderMap::new();
        for line in lines {
            headers.append(
                header::IF_NONE_MATCH,
                HeaderValue::from_bytes(line).unwrap(),
            );
        }
        headers
    }

    fn parsed(lines: &[&[u8]]) -> IfNoneMatch {
        IfNoneMatch::from_headers(&headers(lines))
    }

    fn tags(tags: &[&str]) -> IfNoneMatch {
        IfNoneMatch::Tags(tags.iter().map(|tag| tag.as_bytes().to_vec()).collect())
    }

    #[test]
    fn if_none_match_follows_the_list_grammar() {
        assert_eq!(parsed(&[]), IfNoneMatch::Absent);
        assert_eq!(parsed(&[b"*"]), IfNoneMatch::Any);
        assert_eq!(parsed(&[b" \t* "]), IfNoneMatch::Any);
        assert_eq!(parsed(&[br#""a""#]), tags(&["a"]));
        assert_eq!(parsed(&[br#"W/"a""#]), tags(&["a"]));
        assert_eq!(parsed(&[br#""""#]), tags(&[""]));
        assert_eq!(
            parsed(&[br#""a" , W/"b",, "c""#]),
            tags(&["a", "b", "c"]),
            "OWS and empty elements"
        );
        assert_eq!(parsed(&[br#", "a""#]), tags(&["a"]));
        assert_eq!(
            parsed(&[br#""a,b""#]),
            tags(&["a,b"]),
            "a comma inside quotes is part of the tag, not a separator"
        );
        assert_eq!(
            parsed(&[br#""a""#, br#"W/"b""#]),
            tags(&["a", "b"]),
            "field lines combine into one list"
        );
        assert_eq!(
            parsed(&[b"\"\xe1\xbb\xb9\""]),
            tags(&["\u{1ef9}"]),
            "obs-text"
        );
    }

    #[test]
    fn malformed_or_oversized_fields_are_ignored_not_matched() {
        for field in [
            &b"a"[..],
            b"\"a",
            b"a\"",
            b"w/\"a\"",
            b"W/a",
            b"W/ \"a\"",
            b"\"a\" \"b\"",
            b"\"a\";q=1",
            b"\"a b\"",
            b"\"a\tb\"",
            b"*, \"a\"",
            b"\"a\", *",
            b"**",
            b",",
            b" , ,",
            b"",
        ] {
            assert_eq!(
                parsed(&[field]),
                IfNoneMatch::Ignored,
                "{}",
                String::from_utf8_lossy(field)
            );
        }
        assert_eq!(
            parsed(&[b"*", b"\"a\""]),
            IfNoneMatch::Ignored,
            "* in a list"
        );
        let many = vec!["\"x\""; MAX_TAGS + 1].join(",");
        assert_eq!(parsed(&[many.as_bytes()]), IfNoneMatch::Ignored);
        let exact = vec!["\"x\""; MAX_TAGS].join(",");
        assert!(matches!(parsed(&[exact.as_bytes()]), IfNoneMatch::Tags(t) if t.len() == MAX_TAGS));
        let long = format!("\"{}\"", "a".repeat(MAX_FIELD_BYTES));
        assert_eq!(parsed(&[long.as_bytes()]), IfNoneMatch::Ignored);
    }

    #[test]
    fn matching_is_exact_on_the_opaque_tag() {
        let current = b"abcd";
        assert!(tags(&["x", "abcd"]).matches(current));
        assert!(IfNoneMatch::Any.matches(current));
        for other in [
            tags(&["abc"]),
            tags(&["abcde"]),
            tags(&["ABCD"]),
            tags(&["bcd"]),
        ] {
            assert!(!other.matches(current), "{other:?}");
        }
        assert!(!IfNoneMatch::Absent.matches(current));
        assert!(!IfNoneMatch::Ignored.matches(current));
    }

    #[test]
    fn tag_is_a_digest_of_every_byte() {
        let a = opaque_tag(r#"{"title":"Mưa"}"#.as_bytes());
        assert_eq!(a.len(), 2 * TAG_BYTES);
        assert!(
            a.bytes()
                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
        );
        assert_eq!(a, opaque_tag(r#"{"title":"Mưa"}"#.as_bytes()));
        assert_ne!(a, opaque_tag(br#"{"title":"Mua"}"#));
        assert_ne!(a, opaque_tag(r#"{"title":"Mưa" }"#.as_bytes()));
    }

    async fn parts(response: Response) -> (StatusCode, HeaderMap, Vec<u8>) {
        let status = response.status();
        let headers = response.headers().clone();
        let body = response.into_body().collect().await.unwrap().to_bytes();
        (status, headers, body.to_vec())
    }

    #[tokio::test]
    async fn not_modified_has_the_same_validator_and_no_content() {
        let value = json!({ "works": [{ "title": "Mưa trên phố cũ" }] });
        let (status, full, body) = parts(respond(&HeaderMap::new(), &value).unwrap()).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, serde_json::to_vec(&value).unwrap());
        assert_eq!(full[header::CONTENT_TYPE], "application/json");
        assert_eq!(full[header::CACHE_CONTROL], PUBLIC_READ_CACHE_CONTROL);
        assert!(full.get(header::VARY).is_none());
        let etag = full[header::ETAG].to_str().unwrap().to_string();
        assert_eq!(etag, format!("\"{}\"", opaque_tag(&body)));

        let weak = format!("W/{etag}");
        for field in [etag.as_str(), weak.as_str(), "*"] {
            let request = headers(&[field.as_bytes()]);
            let (status, short, body) = parts(respond(&request, &value).unwrap()).await;
            assert_eq!(status, StatusCode::NOT_MODIFIED, "{field}");
            assert!(body.is_empty());
            assert_eq!(short[header::ETAG], full[header::ETAG]);
            assert_eq!(short[header::CACHE_CONTROL], full[header::CACHE_CONTROL]);
            assert!(short.get(header::CONTENT_TYPE).is_none());
        }

        let changed = json!({ "works": [{ "title": "Mưa trên phố mới" }] });
        let request = headers(&[etag.as_bytes()]);
        let (status, headers, body) = parts(respond(&request, &changed).unwrap()).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, serde_json::to_vec(&changed).unwrap());
        assert_ne!(headers[header::ETAG], full[header::ETAG]);
    }
}
