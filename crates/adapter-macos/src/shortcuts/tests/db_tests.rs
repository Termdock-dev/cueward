use std::path::Path;

use cueward_core::ShortcutInputPolicy;
use rusqlite::{Connection, params};
use tempfile::TempDir;

use crate::shortcuts::{ShortcutSelector, find_shortcut, write_shortcut_payload};

use super::fixture_db;

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
fn find_shortcut_rejects_ambiguous_name_matches() {
    let dir = TempDir::new().unwrap();
    let db_path = fixture_db(&dir);
    let conn = Connection::open(&db_path).unwrap();
    conn.execute(
        "INSERT INTO ZSHORTCUT (Z_PK, ZNAME, ZWORKFLOWID, ZACTIONCOUNT) VALUES (2, ?1, ?2, 0)",
        params!["Clean URL Share", "WF-2"],
    )
    .unwrap();

    let err = find_shortcut(Path::new(&db_path), &ShortcutSelector::Name("Clean URL Share".into()))
        .unwrap_err();

    let message = err.to_string();
    assert!(message.contains("multiple shortcuts matched"));
    assert!(message.contains("Clean URL Share"));
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

    super::super::db::rename_shortcut_name_by_workflow_id(
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
fn load_shortcut_payload_treats_null_blob_as_empty_action_array() {
    let dir = TempDir::new().unwrap();
    let db_path = fixture_db(&dir);
    let conn = Connection::open(&db_path).unwrap();
    conn.execute("UPDATE ZSHORTCUTACTIONS SET ZDATA = NULL WHERE ZSHORTCUT = 1", [])
        .unwrap();

    let payload = super::super::db::load_shortcut_payload(Path::new(&db_path), 1).unwrap();
    let actions = plist::from_bytes::<Vec<plist::Value>>(&payload).unwrap();

    assert!(actions.is_empty());
}

#[test]
fn encode_input_classes_for_url_round_trips_to_url_content_item() {
    let blob = super::super::db::encode_input_classes(&ShortcutInputPolicy::Url).unwrap();
    let classes = plist::from_bytes::<Vec<String>>(&blob).unwrap();

    assert_eq!(classes, vec!["WFURLContentItem"]);
}

#[test]
fn sync_shortcut_surfaces_replaces_named_folder_and_sets_share_sheet() {
    let dir = TempDir::new().unwrap();
    let db_path = fixture_db(&dir);

    super::super::db::sync_shortcut_surfaces(
        Path::new(&db_path),
        1,
        &[
            cueward_core::ShortcutSurface::ShareSheet,
            cueward_core::ShortcutSurface::Folder("Work".into()),
        ],
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

#[test]
fn sync_shortcut_surfaces_can_replace_folder_without_removing_existing_share_sheet() {
    let dir = TempDir::new().unwrap();
    let db_path = fixture_db(&dir);
    let conn = Connection::open(&db_path).unwrap();
    conn.execute(
        "INSERT INTO Z_4SHORTCUTS (Z_4PARENTS1, Z_7SHORTCUTS, Z_FOK_7SHORTCUTS) VALUES (2, 1, 2)",
        [],
    )
    .unwrap();

    super::super::db::sync_shortcut_surfaces(
        Path::new(&db_path),
        1,
        &[
            cueward_core::ShortcutSurface::ShareSheet,
            cueward_core::ShortcutSurface::Folder("Work".into()),
        ],
    )
    .unwrap();

    let rows: Vec<(i64, i64)> = conn
        .prepare("SELECT Z_4PARENTS1, Z_7SHORTCUTS FROM Z_4SHORTCUTS WHERE Z_7SHORTCUTS = 1 ORDER BY Z_4PARENTS1")
        .unwrap()
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();

    assert_eq!(rows, vec![(2, 1), (6, 1), (7, 1)]);
}

#[test]
fn ensure_shortcut_folder_relation_adds_folder_without_dropping_existing_relations() {
    let dir = TempDir::new().unwrap();
    let db_path = fixture_db(&dir);
    let conn = Connection::open(&db_path).unwrap();
    conn.execute(
        "INSERT INTO Z_4SHORTCUTS (Z_4PARENTS1, Z_7SHORTCUTS, Z_FOK_7SHORTCUTS) VALUES (2, 1, 0)",
        [],
    )
    .unwrap();

    super::super::db::ensure_shortcut_folder_relation(Path::new(&db_path), 1, "Work").unwrap();

    let rows: Vec<(i64, i64)> = conn
        .prepare("SELECT Z_4PARENTS1, Z_7SHORTCUTS FROM Z_4SHORTCUTS WHERE Z_7SHORTCUTS = 1 ORDER BY Z_4PARENTS1")
        .unwrap()
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();

    assert_eq!(rows, vec![(2, 1), (6, 1), (7, 1), (8, 1)]);
}

#[test]
fn ensure_shortcut_relation_assigns_next_collection_order_index() {
    let dir = TempDir::new().unwrap();
    let db_path = fixture_db(&dir);

    super::super::db::ensure_shortcut_relation(Path::new(&db_path), 1, 2).unwrap();

    let conn = Connection::open(&db_path).unwrap();
    let order_index: i64 = conn
        .query_row(
            "SELECT Z_FOK_7SHORTCUTS FROM Z_4SHORTCUTS WHERE Z_4PARENTS1 = 2 AND Z_7SHORTCUTS = 1",
            [],
            |row| row.get(0),
        )
        .unwrap();

    assert_eq!(order_index, 0);
}

#[test]
fn find_latest_shortcut_after_pk_returns_newer_row() {
    let dir = TempDir::new().unwrap();
    let db_path = fixture_db(&dir);
    let conn = Connection::open(&db_path).unwrap();
    conn.execute(
        "INSERT INTO ZSHORTCUT (Z_PK, ZNAME, ZWORKFLOWID, ZACTIONCOUNT) VALUES (5, 'New Shortcut', 'WF-5', 0)",
        [],
    )
    .unwrap();

    let record = super::super::db::find_latest_shortcut_after_pk(Path::new(&db_path), 1)
        .unwrap()
        .unwrap();

    assert_eq!(record.pk, 5);
    assert_eq!(record.workflow_id, "WF-5");
}
