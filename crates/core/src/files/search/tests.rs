use super::*;

#[test]
fn unknown_or_invalid_modification_time_cannot_satisfy_a_time_filter() {
    let mut entry = SearchEntry {
        relative_path: "a".into(),
        file: FileInfo {
            requested_path: "/root/a".into(),
            path: "/root/a".into(),
            resolved_path: Some("/root/a".into()),
            name: "a".into(),
            kind: FileKind::File,
            identity: "identity".into(),
            version: "revision".into(),
            size: 0,
            modified: None,
            created: None,
            readonly: false,
            mode: None,
            data_state: DataState::Unknown,
            link_target: None,
        },
    };
    let time = chrono::DateTime::parse_from_rfc3339("2026-10-01T00:00:00Z")
        .unwrap()
        .to_utc();
    let after = SearchOptions {
        modified_after: Some(time),
        ..SearchOptions::default()
    };
    let before = SearchOptions {
        modified_before: Some(time),
        ..SearchOptions::default()
    };
    for modified in [None, Some("not a date".into())] {
        entry.file.modified = modified;
        assert!(matches(&entry, &SearchOptions::default()));
        assert!(!matches(&entry, &after));
        assert!(!matches(&entry, &before));
    }
    entry.file.modified = Some("2026-10-01T08:00:00+08:00".into());
    assert!(matches(&entry, &after));
    assert!(matches(&entry, &before));
}
