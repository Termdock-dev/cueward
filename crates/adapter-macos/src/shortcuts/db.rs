use std::path::Path;
use std::path::PathBuf;

use rusqlite::{Connection, OptionalExtension, params};

use crate::MacosError;

use super::types::{ShortcutRecord, ShortcutSelector};

fn open_db(db_path: &Path) -> Result<Connection, MacosError> {
    Connection::open(db_path).map_err(MacosError::from)
}

fn default_db_path() -> Result<PathBuf, MacosError> {
    let home = std::env::var("HOME")
        .map_err(|err| MacosError::Other(format!("failed to resolve HOME for Shortcuts db: {err}")))?;
    Ok(PathBuf::from(home).join("Library/Shortcuts/Shortcuts.sqlite"))
}

pub fn list_shortcuts_live() -> Result<Vec<ShortcutRecord>, MacosError> {
    let db_path = default_db_path()?;
    list_shortcuts(&db_path)
}

pub fn find_shortcut_live(selector: &ShortcutSelector) -> Result<ShortcutRecord, MacosError> {
    let db_path = default_db_path()?;
    find_shortcut(&db_path, selector)
}

pub fn write_shortcut_payload_live(
    shortcut_pk: i64,
    payload: &[u8],
    action_count: usize,
    input_classes: Option<&[u8]>,
    has_shortcut_input_variables: bool,
) -> Result<(), MacosError> {
    let db_path = default_db_path()?;
    write_shortcut_payload(
        &db_path,
        shortcut_pk,
        payload,
        action_count,
        input_classes,
        has_shortcut_input_variables,
    )
}

pub fn rename_shortcut_name_by_workflow_id(
    db_path: &Path,
    workflow_id: &str,
    new_name: &str,
) -> Result<(), MacosError> {
    let conn = open_db(db_path)?;
    conn.execute(
        r#"
        UPDATE ZSHORTCUT
        SET ZNAME = ?1
        WHERE ZWORKFLOWID = ?2
        "#,
        params![new_name, workflow_id],
    )?;
    Ok(())
}

pub fn rename_shortcut_name_by_workflow_id_live(
    workflow_id: &str,
    new_name: &str,
) -> Result<(), MacosError> {
    let db_path = default_db_path()?;
    rename_shortcut_name_by_workflow_id(&db_path, workflow_id, new_name)
}

pub fn list_shortcuts(db_path: &Path) -> Result<Vec<ShortcutRecord>, MacosError> {
    let conn = open_db(db_path)?;
    let mut stmt = conn.prepare(
        r#"
        SELECT Z_PK, ZNAME, ZWORKFLOWID, ZACTIONCOUNT
        FROM ZSHORTCUT
        ORDER BY Z_PK
        "#,
    )?;

    let rows = stmt.query_map([], |row| {
        let action_count = row.get::<_, Option<i64>>(3)?.unwrap_or_default();
        Ok(ShortcutRecord {
            pk: row.get(0)?,
            name: row.get(1)?,
            workflow_id: row.get(2)?,
            action_count,
        })
    })?;

    rows.collect::<Result<Vec<_>, _>>().map_err(MacosError::from)
}

pub fn find_shortcut(
    db_path: &Path,
    selector: &ShortcutSelector,
) -> Result<ShortcutRecord, MacosError> {
    let conn = open_db(db_path)?;
    let sql = match selector {
        ShortcutSelector::Id(_) => {
            r#"
            SELECT Z_PK, ZNAME, ZWORKFLOWID, ZACTIONCOUNT
            FROM ZSHORTCUT
            WHERE ZWORKFLOWID = ?1
            "#
        }
        ShortcutSelector::Name(_) => {
            r#"
            SELECT Z_PK, ZNAME, ZWORKFLOWID, ZACTIONCOUNT
            FROM ZSHORTCUT
            WHERE ZNAME = ?1
            "#
        }
    };

    let needle = match selector {
        ShortcutSelector::Id(id) => id.as_str(),
        ShortcutSelector::Name(name) => name.as_str(),
    };

    conn.query_row(sql, params![needle], |row| {
        let action_count = row.get::<_, Option<i64>>(3)?.unwrap_or_default();
        Ok(ShortcutRecord {
            pk: row.get(0)?,
            name: row.get(1)?,
            workflow_id: row.get(2)?,
            action_count,
        })
    })
    .optional()?
    .ok_or_else(|| MacosError::Other(format!("shortcut not found: {needle}")))
}

pub fn write_shortcut_payload(
    db_path: &Path,
    shortcut_pk: i64,
    payload: &[u8],
    action_count: usize,
    input_classes: Option<&[u8]>,
    has_shortcut_input_variables: bool,
) -> Result<(), MacosError> {
    let mut conn = open_db(db_path)?;
    let tx = conn.transaction()?;

    tx.execute(
        r#"
        UPDATE ZSHORTCUTACTIONS
        SET ZDATA = ?1
        WHERE ZSHORTCUT = ?2
        "#,
        params![payload, shortcut_pk],
    )?;

    tx.execute(
        r#"
        UPDATE ZSHORTCUT
        SET
            ZACTIONCOUNT = ?1,
            ZACTIONSDESCRIPTION = ?2,
            ZWORKFLOWSUBTITLE = ?3,
            ZINPUTCLASSESDATA = ?4,
            ZHASSHORTCUTINPUTVARIABLES = ?5
        WHERE Z_PK = ?6
        "#,
        params![
            action_count as i64,
            format!("{action_count} actions"),
            format!("{action_count} actions"),
            input_classes,
            if has_shortcut_input_variables { 1 } else { 0 },
            shortcut_pk,
        ],
    )?;

    tx.commit()?;
    Ok(())
}
