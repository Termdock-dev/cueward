use super::*;

fn sample_saved_state() -> Value {
    Value::Array(vec![Value::Dictionary({
        let mut raw = Dictionary::new();
        raw.insert(
            "UUID".into(),
            Value::String("DF260009-9714-421B-BB65-D2B413C55F46".into()),
        );
        raw
    })])
}

#[test]
fn parse_saved_state_reads_uuid_entries() {
    let entries = parse_saved_state_value(sample_saved_state()).expect("state");

    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].id, "DF260009-9714-421B-BB65-D2B413C55F46");
}

#[test]
fn sticky_title_falls_back_from_body_when_missing() {
    let title = derive_sticky_title(None, "第一行\n第二行", "sticky-1");

    assert_eq!(title, "第一行");
}

#[test]
fn sticky_title_falls_back_to_id_when_body_empty() {
    let title = derive_sticky_title(None, "", "DF260009-9714-421B-BB65-D2B413C55F46");

    assert_eq!(title, "Sticky DF260009");
}

#[test]
fn update_requires_title_or_body() {
    let err = ensure_update_fields(None, None).unwrap_err();

    assert_eq!(err.to_string(), "no sticky updates specified");
}

#[test]
fn read_sticky_body_decodes_rtf_via_textutil() {
    let temp = tempfile::tempdir().expect("tempdir");
    let path = temp.path().join("TXT.rtf");
    write_sticky_body(&path, "第一行\n第二行").expect("write rtf");

    let body = read_sticky_body(&path).expect("read rtf");

    assert_eq!(body, "第一行\n第二行");
}

#[test]
fn read_sticky_body_preserves_trailing_blank_lines() {
    let temp = tempfile::tempdir().expect("tempdir");
    let path = temp.path().join("TXT.rtf");
    write_sticky_body(&path, "第一行\n第二行\n\n").expect("write rtf");

    let body = read_sticky_body(&path).expect("read rtf");

    assert_eq!(body, "第一行\n第二行\n\n");
}

#[test]
fn create_sticky_writes_rtfd_and_updates_state() {
    let temp = tempfile::tempdir().expect("tempdir");
    let sticky = create_sticky_in_root(temp.path(), "臨時待辦", "記得回覆客戶")
        .expect("create sticky");

    assert!(saved_state_path(temp.path()).exists());
    assert!(sticky_rtf_path(temp.path(), &sticky.id).exists());

    let notes = list_stickies_from_root(temp.path()).expect("list");
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0].title, "臨時待辦");
    assert_eq!(notes[0].body, "記得回覆客戶");
}

#[test]
fn create_sticky_clones_existing_state_shape() {
    let temp = tempfile::tempdir().expect("tempdir");
    let template = StickyStateEntry {
        id: "EXISTING".into(),
        raw: {
            let mut raw = Dictionary::new();
            raw.insert("UUID".into(), Value::String("EXISTING".into()));
            raw.insert(TITLE_KEY.into(), Value::String("舊標題".into()));
            raw.insert("Frame".into(), Value::String("{{200, 900}, {300, 200}}".into()));
            raw.insert("ZOrder".into(), Value::Integer(5.into()));
            raw.insert("StickyColor".into(), Value::String("blue".into()));
            raw
        },
    };
    write_saved_state(&saved_state_path(temp.path()), &[template]).expect("seed state");

    let sticky = create_sticky_in_root(temp.path(), "新標題", "新內容").expect("create");
    let entries = parse_saved_state(&saved_state_path(temp.path())).expect("state");
    let created = entries
        .iter()
        .find(|entry| entry.id == sticky.id)
        .expect("created entry");

    assert_eq!(
        created.raw.get(TITLE_KEY).and_then(Value::as_string),
        Some("新標題")
    );
    assert_eq!(
        created.raw.get("StickyColor").and_then(Value::as_string),
        Some("blue")
    );
    assert_eq!(
        created.raw.get("Frame").and_then(Value::as_string),
        Some("{{224, 876}, {300, 200}}")
    );
    assert_eq!(
        created.raw.get("ZOrder").and_then(Value::as_signed_integer),
        Some(6)
    );
}

#[test]
fn update_sticky_rewrites_title_and_body_for_id() {
    let temp = tempfile::tempdir().expect("tempdir");
    let sticky = create_sticky_in_root(temp.path(), "舊標題", "舊內容")
        .expect("create sticky");

    let updated = update_sticky_in_root(
        temp.path(),
        &sticky.id,
        Some("新標題"),
        Some("新內容"),
    )
    .expect("update sticky");

    assert_eq!(updated.title, "新標題");
    assert_eq!(updated.body, "新內容");

    let notes = list_stickies_from_root(temp.path()).expect("list");
    assert_eq!(notes[0].title, "新標題");
    assert_eq!(notes[0].body, "新內容");
}

#[test]
fn title_only_update_preserves_trailing_blank_lines() {
    let temp = tempfile::tempdir().expect("tempdir");
    let sticky = create_sticky_in_root(temp.path(), "舊標題", "第一行\n第二行\n\n")
        .expect("create sticky");

    let updated = update_sticky_in_root(temp.path(), &sticky.id, Some("新標題"), None)
        .expect("update sticky");

    assert_eq!(updated.title, "新標題");
    assert_eq!(updated.body, "第一行\n第二行\n\n");
}

#[test]
fn delete_sticky_removes_state_and_rtfd() {
    let temp = tempfile::tempdir().expect("tempdir");
    let sticky = create_sticky_in_root(temp.path(), "標題", "內容")
        .expect("create sticky");

    delete_sticky_in_root(temp.path(), &sticky.id).expect("delete sticky");

    assert!(!sticky_dir(temp.path(), &sticky.id).exists());
    let notes = list_stickies_from_root(temp.path()).expect("list");
    assert!(notes.is_empty());
}
