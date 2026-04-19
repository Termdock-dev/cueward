use std::path::Path;

use cueward_core::{ShortcutAction, ShortcutInputPolicy, ShortcutReference, ShortcutSpec};
use rusqlite::{Connection, params};
use tempfile::TempDir;

use super::{ShortcutSelector, compile_actions, find_shortcut, write_shortcut_payload};

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
            ZDATA BLOB NOT NULL
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

    let second = actions[1].as_dictionary().unwrap();
    assert_eq!(
        second.get("WFWorkflowActionIdentifier").unwrap().as_string(),
        Some("is.workflow.actions.setclipboard")
    );
}
