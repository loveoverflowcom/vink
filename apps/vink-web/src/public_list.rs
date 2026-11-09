//! Safe fragment identity shared by discovery and reader return links.

pub fn work_anchor(work_id: &str) -> String {
    if !work_id.is_empty()
        && work_id.len() <= 64
        && work_id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-')
    {
        format!("work-{work_id}")
    } else {
        "community-stream".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::work_anchor;
    #[test]
    fn untrusted_ids_never_become_arbitrary_fragments() {
        assert_eq!(work_anchor("a-123"), "work-a-123");
        for id in ["", "//outside/#x", "<script>", "a?b", "tên"] {
            assert_eq!(work_anchor(id), "community-stream");
        }
    }
}
