use std::path::Path;

use rusqlite::{Connection, OptionalExtension, params};

use crate::MacosError;

use super::types::{ShortcutRecord, ShortcutSelector};

fn open_db(db_path: &Path) -> Result<Connection, MacosError> {
    Connection::open(db_path).map_err(MacosError::from)
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
        Ok(ShortcutRecord {
            pk: row.get(0)?,
            name: row.get(1)?,
            workflow_id: row.get(2)?,
            action_count: row.get(3)?,
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
        Ok(ShortcutRecord {
            pk: row.get(0)?,
            name: row.get(1)?,
            workflow_id: row.get(2)?,
            action_count: row.get(3)?,
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
