use tempfile::TempDir;

mod compiler_tests;
mod db_tests;

pub(super) fn fixture_db(dir: &TempDir) -> String {
    let path = dir.path().join("Shortcuts.sqlite");
    let conn = rusqlite::Connection::open(&path).unwrap();
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
        rusqlite::params!["Clean URL Share", "WF-1"],
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
