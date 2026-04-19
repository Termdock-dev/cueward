use std::path::Path;

use cueward_core::{
    ShortcutAction, ShortcutInputPolicy, ShortcutReference, ShortcutSpec, ShortcutSurface,
};
use rusqlite::{Connection, params};
use tempfile::TempDir;

use super::{ShortcutSelector, append_action, compile_actions, find_shortcut, write_shortcut_payload};

fn fixture_db(dir: &TempDir) -> String {
    let path = dir.path().join("Shortcuts.sqlite");
    let conn = Connection::open(&path).unwrap();
    conn.execute_batch(
        r#"
        CREATE TABLE ZSHORTCUT (
            Z_PK INTEGER PRIMARY KEY,
            ZNAME TEXT NOT NULL,
            ZWORKFLOWID TEXT NOT NULL,
            ZACTIONCOUNT INTEGER NOT NULL DEFAULT 0,
            ZACTIONSDESCRIPTION TEXT,
            ZWORKFLOWSUBTITLE TEXT,
            ZINPUTCLASSESDATA BLOB,
            ZHASSHORTCUTINPUTVARIABLES INTEGER NOT NULL DEFAULT 0
        );
        CREATE TABLE ZSHORTCUTACTIONS (
            Z_PK INTEGER PRIMARY KEY,
            ZSHORTCUT INTEGER NOT NULL,
            ZDATA BLOB
        );
        CREATE TABLE ZCOLLECTION (
            Z_PK INTEGER PRIMARY KEY,
            ZNAME TEXT,
            ZIDENTIFIER TEXT
        );
        CREATE TABLE Z_4SHORTCUTS (
            Z_4PARENTS1 INTEGER NOT NULL,
            Z_7SHORTCUTS INTEGER NOT NULL,
            Z_FOK_7SHORTCUTS INTEGER
        );
        "#,
    )
    .unwrap();

    conn.execute(
        "INSERT INTO ZSHORTCUT (Z_PK, ZNAME, ZWORKFLOWID, ZACTIONCOUNT) VALUES (1, ?1, ?2, 0)",
        params!["Clean URL Share", "WF-1"],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO ZSHORTCUTACTIONS (Z_PK, ZSHORTCUT, ZDATA) VALUES (1, 1, X'626C6F62')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO ZCOLLECTION (Z_PK, ZNAME, ZIDENTIFIER) VALUES (2, NULL, 'ShareSheet')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO ZCOLLECTION (Z_PK, ZNAME, ZIDENTIFIER) VALUES (6, NULL, 'Root')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO ZCOLLECTION (Z_PK, ZNAME, ZIDENTIFIER) VALUES (7, 'Work', 'work-folder')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO ZCOLLECTION (Z_PK, ZNAME, ZIDENTIFIER) VALUES (8, 'Old', 'old-folder')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO Z_4SHORTCUTS (Z_4PARENTS1, Z_7SHORTCUTS, Z_FOK_7SHORTCUTS) VALUES (6, 1, 6)",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO Z_4SHORTCUTS (Z_4PARENTS1, Z_7SHORTCUTS, Z_FOK_7SHORTCUTS) VALUES (8, 1, 8)",
        [],
    )
    .unwrap();

    path.display().to_string()
}

#[test]
fn find_shortcut_returns_exact_match_by_name() {
    let dir = TempDir::new().unwrap();
    let db_path = fixture_db(&dir);

    let record = find_shortcut(Path::new(&db_path), &ShortcutSelector::Name("Clean URL Share".into()))
        .unwrap();

    assert_eq!(record.pk, 1);
    assert_eq!(record.workflow_id, "WF-1");
}

#[test]
fn write_shortcut_payload_updates_blob_and_counts() {
    let dir = TempDir::new().unwrap();
    let db_path = fixture_db(&dir);
    let payload = b"new-payload";
    let input_classes = b"input-classes";

    write_shortcut_payload(Path::new(&db_path), 1, payload, 2, Some(input_classes), true).unwrap();

    let conn = Connection::open(&db_path).unwrap();
    let (count, description, subtitle, has_input_vars, blob, classes): (
        i64,
        Option<String>,
        Option<String>,
        i64,
        Vec<u8>,
        Vec<u8>,
    ) = conn
        .query_row(
            r#"
            SELECT
                s.ZACTIONCOUNT,
                s.ZACTIONSDESCRIPTION,
                s.ZWORKFLOWSUBTITLE,
                s.ZHASSHORTCUTINPUTVARIABLES,
                a.ZDATA,
                s.ZINPUTCLASSESDATA
            FROM ZSHORTCUT s
            JOIN ZSHORTCUTACTIONS a ON a.ZSHORTCUT = s.Z_PK
            WHERE s.Z_PK = 1
            "#,
            [],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                ))
            },
        )
        .unwrap();

    assert_eq!(count, 2);
    assert_eq!(description.as_deref(), Some("2 actions"));
    assert_eq!(subtitle.as_deref(), Some("2 actions"));
    assert_eq!(has_input_vars, 1);
    assert_eq!(blob, payload);
    assert_eq!(classes, input_classes);
}

#[test]
fn rename_shortcut_name_by_workflow_id_updates_row() {
    let dir = TempDir::new().unwrap();
    let db_path = fixture_db(&dir);

    super::db::rename_shortcut_name_by_workflow_id(
        Path::new(&db_path),
        "WF-1",
        "Renamed Shortcut",
    )
    .unwrap();

    let conn = Connection::open(&db_path).unwrap();
    let renamed: String = conn
        .query_row(
            "SELECT ZNAME FROM ZSHORTCUT WHERE ZWORKFLOWID = 'WF-1'",
            [],
            |row| row.get(0),
        )
        .unwrap();

    assert_eq!(renamed, "Renamed Shortcut");
}

#[test]
fn compile_actions_builds_text_and_clipboard_chain() {
    let spec = ShortcutSpec {
        name: "Plan Smoke".into(),
        surfaces: vec![],
        input: ShortcutInputPolicy::Any,
        actions: vec![
            ShortcutAction::Text {
                value: "hello".into(),
                output: Some("greeting".into()),
            },
            ShortcutAction::CopyToClipboard {
                from: ShortcutReference::Output("greeting".into()),
            },
        ],
    };

    let payload = compile_actions(&spec).unwrap();
    let actions = plist::from_bytes::<Vec<plist::Value>>(&payload).unwrap();

    assert_eq!(actions.len(), 2);

    let first = actions[0].as_dictionary().unwrap();
    assert_eq!(
        first.get("WFWorkflowActionIdentifier").unwrap().as_string(),
        Some("is.workflow.actions.gettext")
    );
    let first_params = first.get("WFWorkflowActionParameters").unwrap().as_dictionary().unwrap();
    assert_eq!(
        first_params.get("CustomOutputName").and_then(plist::Value::as_string),
        Some("greeting")
    );

    let second = actions[1].as_dictionary().unwrap();
    assert_eq!(
        second.get("WFWorkflowActionIdentifier").unwrap().as_string(),
        Some("is.workflow.actions.setclipboard")
    );
    let second_params = second.get("WFWorkflowActionParameters").unwrap().as_dictionary().unwrap();
    let input = second_params.get("WFInput").unwrap().as_dictionary().unwrap();
    let value = input.get("Value").unwrap().as_dictionary().unwrap();
    assert_eq!(
        value.get("OutputName").and_then(plist::Value::as_string),
        Some("greeting")
    );
}

#[test]
fn append_action_uses_existing_custom_output_name_as_reference() {
    let spec = ShortcutSpec {
        name: "Plan Smoke".into(),
        surfaces: vec![],
        input: ShortcutInputPolicy::Any,
        actions: vec![ShortcutAction::Text {
            value: "hello".into(),
            output: Some("greeting".into()),
        }],
    };

    let existing = compile_actions(&spec).unwrap();
    let appended = append_action(
        &existing,
        &ShortcutAction::CopyToClipboard {
            from: ShortcutReference::Output("greeting".into()),
        },
    )
    .unwrap();

    let actions = plist::from_bytes::<Vec<plist::Value>>(&appended).unwrap();
    assert_eq!(actions.len(), 2);
    let second = actions[1].as_dictionary().unwrap();
    let second_params = second.get("WFWorkflowActionParameters").unwrap().as_dictionary().unwrap();
    let input = second_params.get("WFInput").unwrap().as_dictionary().unwrap();
    let value = input.get("Value").unwrap().as_dictionary().unwrap();
    assert_eq!(
        value.get("OutputName").and_then(plist::Value::as_string),
        Some("greeting")
    );
}

#[test]
fn load_shortcut_payload_treats_null_blob_as_empty_action_array() {
    let dir = TempDir::new().unwrap();
    let db_path = fixture_db(&dir);
    let conn = Connection::open(&db_path).unwrap();
    conn.execute("UPDATE ZSHORTCUTACTIONS SET ZDATA = NULL WHERE ZSHORTCUT = 1", [])
        .unwrap();

    let payload = super::db::load_shortcut_payload(Path::new(&db_path), 1).unwrap();
    let actions = plist::from_bytes::<Vec<plist::Value>>(&payload).unwrap();

    assert!(actions.is_empty());
}

#[test]
fn encode_input_classes_for_url_round_trips_to_url_content_item() {
    let blob = super::db::encode_input_classes(&ShortcutInputPolicy::Url).unwrap();
    let classes = plist::from_bytes::<Vec<String>>(&blob).unwrap();

    assert_eq!(classes, vec!["WFURLContentItem"]);
}

#[test]
fn sync_shortcut_surfaces_replaces_named_folder_and_sets_share_sheet() {
    let dir = TempDir::new().unwrap();
    let db_path = fixture_db(&dir);

    super::db::sync_shortcut_surfaces(
        Path::new(&db_path),
        1,
        &[ShortcutSurface::ShareSheet, ShortcutSurface::Folder("Work".into())],
    )
    .unwrap();

    let conn = Connection::open(&db_path).unwrap();
    let rows: Vec<(i64, i64)> = conn
        .prepare("SELECT Z_4PARENTS1, Z_7SHORTCUTS FROM Z_4SHORTCUTS WHERE Z_7SHORTCUTS = 1 ORDER BY Z_4PARENTS1")
        .unwrap()
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();

    assert_eq!(rows, vec![(2, 1), (6, 1), (7, 1)]);
}
