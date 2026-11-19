//! Shared comparison keys for already-loaded cards. Never rewrite authored text.

use unicode_normalization::{UnicodeNormalization, char::is_combining_mark};

/// Accept Vietnamese accents in NFC/NFD, Đ/đ, case and repeated whitespace.
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

/// Split `text` into contiguous `(slice, matched)` spans matching whitespace-separated folded
/// terms of `query` using the same Vietnamese NFD/Đ/đ/case rules as [`search_key`].
/// Never rewrites `text` and never splits a base character from its combining diacritic marks.
pub fn highlight_spans(text: &str, query: &str) -> Vec<(String, bool)> {
    if text.is_empty() {
        return Vec::new();
    }
    let normalized_query = search_key(query);
    if normalized_query.is_empty() {
        return vec![(text.to_string(), false)];
    }

    let mut folded = String::new();
    // Each entry is `(folded_byte_start, src_byte_start, src_byte_end)` for one char in `folded`.
    let mut entries: Vec<(usize, usize, usize)> = Vec::new();
    let mut last_cluster_entry_start: Option<usize> = None;

    for (src_start, ch) in text.char_indices() {
        let src_end = src_start + ch.len_utf8();
        if is_combining_mark(ch) {
            if let Some(start_idx) = last_cluster_entry_start {
                for entry in &mut entries[start_idx..] {
                    entry.2 = src_end;
                }
            }
            continue;
        }
        let cluster_start = entries.len();
        let mut pushed_any = false;
        for base in text[src_start..src_end]
            .nfd()
            .filter(|c| !is_combining_mark(*c))
        {
            for lower in base.to_lowercase().filter(|c| !is_combining_mark(*c)) {
                let mapped = if lower == 'đ' { 'd' } else { lower };
                let folded_byte_start = folded.len();
                folded.push(mapped);
                entries.push((folded_byte_start, src_start, src_end));
                pushed_any = true;
            }
        }
        if pushed_any {
            last_cluster_entry_start = Some(cluster_start);
        }
    }

    let mut intervals: Vec<(usize, usize)> = Vec::new();
    for term in normalized_query.split_whitespace() {
        for (match_start, _) in folded.match_indices(term) {
            let match_end = match_start + term.len();
            let first = entries.partition_point(|&(fb, _, _)| fb < match_start);
            let past_last = entries.partition_point(|&(fb, _, _)| fb < match_end);
            if first < past_last {
                intervals.push((entries[first].1, entries[past_last - 1].2));
            }
        }
    }

    if intervals.is_empty() {
        return vec![(text.to_string(), false)];
    }

    intervals.sort_unstable();
    let mut merged: Vec<(usize, usize)> = Vec::with_capacity(intervals.len());
    for (start, end) in intervals {
        if let Some(last) = merged.last_mut()
            && start <= last.1
        {
            last.1 = last.1.max(end);
        } else {
            merged.push((start, end));
        }
    }

    let mut spans = Vec::new();
    let mut cursor = 0usize;
    for (start, end) in merged {
        if cursor < start {
            spans.push((text[cursor..start].to_string(), false));
        }
        spans.push((text[start..end].to_string(), true));
        cursor = end;
    }
    if cursor < text.len() {
        spans.push((text[cursor..].to_string(), false));
    }
    spans
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn highlights_diacriticless_case_and_d_stroked_matches_verbatim() {
        assert_eq!(
            highlight_spans("Mưa trên phố cũ", "mua"),
            vec![("Mưa".into(), true), (" trên phố cũ".into(), false)]
        );
        assert_eq!(
            highlight_spans("Đường về nhà", "duong ve"),
            vec![
                ("Đường".into(), true),
                (" ".into(), false),
                ("về".into(), true),
                (" nhà".into(), false),
            ]
        );
    }

    #[test]
    fn preserves_nfd_combining_marks_and_merges_overlaps() {
        let nfd = "Mu\u{031b}a pho\u{0302}\u{0301}";
        assert_eq!(
            highlight_spans(nfd, "mua ua pho"),
            vec![
                ("Mu\u{031b}a".into(), true),
                (" ".into(), false),
                ("pho\u{0302}\u{0301}".into(), true),
            ]
        );
    }

    #[test]
    fn handles_empty_and_unmatched_queries_cleanly() {
        assert_eq!(highlight_spans("", "mua"), Vec::<(String, bool)>::new());
        assert_eq!(
            highlight_spans("Chiều mưa", "   "),
            vec![("Chiều mưa".into(), false)]
        );
        assert_eq!(
            highlight_spans("Chiều mưa", "nắng"),
            vec![("Chiều mưa".into(), false)]
        );
    }
}
