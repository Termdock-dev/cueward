use super::*;
use plist::Dictionary;
use std::str::FromStr;

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
fn parse_frame_round_trips_cocoa_frame_strings() {
    let frame = parse_frame("{{200, 900}, {300, 200}}").expect("frame");

    assert_eq!(
        frame,
        StickyFrame {
            x: 200,
            y: 900,
            width: 300,
            height: 200,
        }
    );
    assert_eq!(frame.to_state_value(), "{{200, 900}, {300, 200}}");
}

#[test]
fn parse_expanded_size_round_trips_state_strings() {
    let size = parse_expanded_size("{420, 260}").expect("size");

    assert_eq!(
        size,
        StickySize {
            width: 420,
            height: 260,
        }
    );
    assert_eq!(size.to_state_value(), "{420, 260}");
}

#[test]
fn blue_preset_emits_four_color_dictionaries() {
    let scheme = StickyColorPreset::Blue.scheme();

    assert_eq!(
        scheme.sticky,
        color_dictionary(0.6784313725490196, 0.9568627450980393, 1.0, 1.0)
    );
    assert_eq!(
        scheme.control,
        color_dictionary(0.1411764705882353, 0.8156862745098039, 0.9137254901960784, 1.0)
    );
    assert_eq!(
        scheme.highlight,
        color_dictionary(0.00784313725490196, 0.7372549019607844, 0.8431372549019608, 1.0)
    );
    assert_eq!(
        scheme.spine,
        color_dictionary(0.5372549019607843, 0.9411764705882353, 1.0, 1.0)
    );
}

#[test]
fn create_sticky_with_geometry_writes_frame_and_expanded_size() {
    let temp = tempfile::tempdir().expect("tempdir");
    let sticky = create_sticky_in_root_with_options(
        temp.path(),
        "幾何測試",
        "內容",
        &StickyMutationOptions {
            width: Some(420),
            height: Some(260),
            ..Default::default()
        },
        &[],
    )
    .expect("create sticky");

    let entries = parse_saved_state(&saved_state_path(temp.path())).expect("state");
    let created = entries
        .iter()
        .find(|entry| entry.id == sticky.id)
        .expect("created entry");

    assert_eq!(
        created.raw.get("Frame").and_then(Value::as_string),
        Some("{{200, 900}, {420, 260}}")
    );
    assert_eq!(
        created.raw.get("ExpandedSize").and_then(Value::as_string),
        Some("{420, 260}")
    );
}

#[test]
fn update_sticky_with_geometry_rewrites_frame_and_expanded_size() {
    let temp = tempfile::tempdir().expect("tempdir");
    let sticky = create_sticky_in_root(temp.path(), "幾何測試", "內容").expect("create");

    update_sticky_in_root_with_options(
        temp.path(),
        &sticky.id,
        None,
        None,
        &StickyMutationOptions {
            x: Some(10),
            y: Some(20),
            width: Some(360),
            height: Some(220),
            ..Default::default()
        },
        &[],
    )
    .expect("update");

    let entries = parse_saved_state(&saved_state_path(temp.path())).expect("state");
    let updated = entries
        .iter()
        .find(|entry| entry.id == sticky.id)
        .expect("updated entry");

    assert_eq!(
        updated.raw.get("Frame").and_then(Value::as_string),
        Some("{{10, 20}, {360, 220}}")
    );
    assert_eq!(
        updated.raw.get("ExpandedSize").and_then(Value::as_string),
        Some("{360, 220}")
    );
}

#[test]
fn display_relative_geometry_resolves_to_global_frame() {
    let frame = resolve_frame_for_display(
        StickyDisplayBounds {
            frame: StickyFrame {
                x: -2910,
                y: 1169,
                width: 2560,
                height: 1440,
            },
            visible_frame: StickyFrame {
                x: -2910,
                y: 1262,
                width: 2560,
                height: 1347,
            },
        },
        StickySize {
            width: 420,
            height: 260,
        },
        40,
        80,
    );

    assert_eq!(
        frame,
        StickyFrame {
            x: -2870,
            y: 2269,
            width: 420,
            height: 260,
        }
    );
}

#[test]
fn create_on_display_without_explicit_coordinates_cascades_within_visible_frame() {
    let temp = tempfile::tempdir().expect("tempdir");
    let displays = [StickyDisplayBounds {
        frame: StickyFrame {
            x: -2910,
            y: 1169,
            width: 2560,
            height: 1440,
        },
        visible_frame: StickyFrame {
            x: -2910,
            y: 1262,
            width: 2560,
            height: 1347,
        },
    }];

    let first = create_sticky_in_root_with_options(
        temp.path(),
        "第一張",
        "內容",
        &StickyMutationOptions {
            display: Some(1),
            ..Default::default()
        },
        &displays,
    )
    .expect("first");
    let second = create_sticky_in_root_with_options(
        temp.path(),
        "第二張",
        "內容",
        &StickyMutationOptions {
            display: Some(1),
            ..Default::default()
        },
        &displays,
    )
    .expect("second");

    let entries = parse_saved_state(&saved_state_path(temp.path())).expect("state");
    let first_entry = entries.iter().find(|entry| entry.id == first.id).expect("first entry");
    let second_entry = entries.iter().find(|entry| entry.id == second.id).expect("second entry");

    assert_eq!(
        first_entry.raw.get("Frame").and_then(Value::as_string),
        Some("{{-2870, 2369}, {300, 200}}")
    );
    assert_eq!(
        second_entry.raw.get("Frame").and_then(Value::as_string),
        Some("{{-2846, 2345}, {300, 200}}")
    );
}

#[test]
fn create_sticky_with_color_writes_all_color_fields() {
    let temp = tempfile::tempdir().expect("tempdir");
    let sticky = create_sticky_in_root_with_options(
        temp.path(),
        "顏色測試",
        "內容",
        &StickyMutationOptions {
            color: Some(StickyColorPreset::Green),
            ..Default::default()
        },
        &[],
    )
    .expect("create sticky");

    let entries = parse_saved_state(&saved_state_path(temp.path())).expect("state");
    let created = entries
        .iter()
        .find(|entry| entry.id == sticky.id)
        .expect("created entry");

    assert_eq!(
        created.raw.get("StickyColor").and_then(Value::as_dictionary),
        Some(&color_dictionary(0.6980392156862745, 1.0, 0.6313725490196078, 1.0))
    );
    assert_eq!(
        created.raw.get("ControlColor").and_then(Value::as_dictionary),
        Some(&color_dictionary(0.3176470588235294, 0.7333333333333333, 0.3176470588235294, 1.0))
    );
}

#[test]
fn update_sticky_with_color_rewrites_all_color_fields() {
    let temp = tempfile::tempdir().expect("tempdir");
    let sticky = create_sticky_in_root(temp.path(), "顏色測試", "內容").expect("create");

    update_sticky_in_root_with_options(
        temp.path(),
        &sticky.id,
        Some("粉紅"),
        None,
        &StickyMutationOptions {
            color: Some(StickyColorPreset::Pink),
            ..Default::default()
        },
        &[],
    )
    .expect("update");

    let entries = parse_saved_state(&saved_state_path(temp.path())).expect("state");
    let updated = entries
        .iter()
        .find(|entry| entry.id == sticky.id)
        .expect("updated entry");

    assert_eq!(
        updated.raw.get("HighlightColor").and_then(Value::as_dictionary),
        Some(&color_dictionary(0.8862745098039215, 0.4588235294117647, 0.4588235294117647, 1.0))
    );
    assert_eq!(
        updated.raw.get("SpineColor").and_then(Value::as_dictionary),
        Some(&color_dictionary(1.0, 0.6980392156862745, 0.6980392156862745, 1.0))
    );
}

#[test]
fn update_sticky_title_only_preserves_existing_frame_and_expanded_size() {
    let temp = tempfile::tempdir().expect("tempdir");
    let sticky = create_sticky_in_root(temp.path(), "顏色測試", "內容").expect("create");

    update_sticky_in_root_with_options(
        temp.path(),
        &sticky.id,
        Some("只改標題"),
        None,
        &StickyMutationOptions::default(),
        &[],
    )
    .expect("update");

    let entries = parse_saved_state(&saved_state_path(temp.path())).expect("state");
    let updated = entries
        .iter()
        .find(|entry| entry.id == sticky.id)
        .expect("updated entry");

    assert_eq!(
        updated.raw.get("Frame").and_then(Value::as_string),
        Some("{{200, 900}, {300, 200}}")
    );
    assert_eq!(
        updated.raw.get("ExpandedSize").and_then(Value::as_string),
        Some("{300, 200}")
    );
}

#[test]
fn sticky_color_preset_rejects_unsupported_values() {
    let err = StickyColorPreset::from_str("orange").expect_err("unsupported color should fail");

    assert_eq!(err.to_string(), "unsupported sticky color: orange");
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
