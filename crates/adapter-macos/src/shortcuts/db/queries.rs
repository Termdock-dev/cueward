use std::path::Path;

use rusqlite::{OptionalExtension, params};

use cueward_core::{ShortcutInputPolicy, ShortcutSurface};

use crate::MacosError;

use super::{default_db_path, open_db};
use crate::shortcuts::types::{ShortcutRecord, ShortcutSelector};

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

pub fn list_shortcuts_live() -> Result<Vec<ShortcutRecord>, MacosError> {
    let db_path = default_db_path()?;
    list_shortcuts(&db_path)
}

pub fn latest_shortcut_pk(db_path: &Path) -> Result<Option<i64>, MacosError> {
    let conn = open_db(db_path)?;
    conn.query_row("SELECT MAX(Z_PK) FROM ZSHORTCUT", [], |row| row.get(0))
        .map_err(MacosError::from)
}

pub fn latest_shortcut_pk_live() -> Result<Option<i64>, MacosError> {
    let db_path = default_db_path()?;
    latest_shortcut_pk(&db_path)
}

pub fn find_latest_shortcut_after_pk(
    db_path: &Path,
    min_pk: i64,
) -> Result<Option<ShortcutRecord>, MacosError> {
    let conn = open_db(db_path)?;
    conn.query_row(
        r#"
        SELECT Z_PK, ZNAME, ZWORKFLOWID, ZACTIONCOUNT
        FROM ZSHORTCUT
        WHERE Z_PK > ?1
        ORDER BY Z_PK DESC
        LIMIT 1
        "#,
        params![min_pk],
        |row| {
            Ok(ShortcutRecord {
                pk: row.get(0)?,
                name: row.get(1)?,
                workflow_id: row.get(2)?,
                action_count: row.get::<_, Option<i64>>(3)?.unwrap_or_default(),
            })
        },
    )
    .optional()
    .map_err(MacosError::from)
}

pub fn find_latest_shortcut_after_pk_live(min_pk: i64) -> Result<Option<ShortcutRecord>, MacosError> {
    let db_path = default_db_path()?;
    find_latest_shortcut_after_pk(&db_path, min_pk)
}

pub fn find_shortcut(
    db_path: &Path,
    selector: &ShortcutSelector,
) -> Result<ShortcutRecord, MacosError> {
    match selector {
        ShortcutSelector::Id(id) => find_shortcut_by_id(db_path, id),
        ShortcutSelector::Name(name) => find_shortcut_by_name(db_path, name),
    }
}

pub fn find_shortcut_live(selector: &ShortcutSelector) -> Result<ShortcutRecord, MacosError> {
    let db_path = default_db_path()?;
    find_shortcut(&db_path, selector)
}

fn find_shortcut_by_id(db_path: &Path, workflow_id: &str) -> Result<ShortcutRecord, MacosError> {
    let conn = open_db(db_path)?;
    conn.query_row(
        r#"
        SELECT Z_PK, ZNAME, ZWORKFLOWID, ZACTIONCOUNT
        FROM ZSHORTCUT
        WHERE ZWORKFLOWID = ?1
        "#,
        params![workflow_id],
        |row| {
            Ok(ShortcutRecord {
                pk: row.get(0)?,
                name: row.get(1)?,
                workflow_id: row.get(2)?,
                action_count: row.get::<_, Option<i64>>(3)?.unwrap_or_default(),
            })
        },
    )
    .optional()?
    .ok_or_else(|| MacosError::NotFound(format!("shortcut not found: {workflow_id}")))
}

fn find_shortcut_by_name(db_path: &Path, name: &str) -> Result<ShortcutRecord, MacosError> {
    let conn = open_db(db_path)?;
    let mut stmt = conn.prepare(
        r#"
        SELECT Z_PK, ZNAME, ZWORKFLOWID, ZACTIONCOUNT
        FROM ZSHORTCUT
        WHERE ZNAME = ?1
        ORDER BY Z_PK
        "#,
    )?;
    let rows = stmt.query_map(params![name], |row| {
        Ok(ShortcutRecord {
            pk: row.get(0)?,
            name: row.get(1)?,
            workflow_id: row.get(2)?,
            action_count: row.get::<_, Option<i64>>(3)?.unwrap_or_default(),
        })
    })?;
    let records = rows.collect::<Result<Vec<_>, _>>()?;

    match records.as_slice() {
        [] => Err(MacosError::NotFound(format!("shortcut not found: {name}"))),
        [record] => Ok(record.clone()),
        _ => Err(MacosError::Other(format!("multiple shortcuts matched: {name}"))),
    }
}

pub fn shortcut_has_relation(
    db_path: &Path,
    shortcut_pk: i64,
    collection_pk: i64,
) -> Result<bool, MacosError> {
    let conn = open_db(db_path)?;
    let exists = conn
        .query_row(
            "SELECT 1 FROM Z_4SHORTCUTS WHERE Z_7SHORTCUTS = ?1 AND Z_4PARENTS1 = ?2 LIMIT 1",
            params![shortcut_pk, collection_pk],
            |_| Ok(true),
        )
        .optional()?
        .unwrap_or(false);
    Ok(exists)
}

pub fn shortcut_has_relation_live(shortcut_pk: i64, collection_pk: i64) -> Result<bool, MacosError> {
    let db_path = default_db_path()?;
    shortcut_has_relation(&db_path, shortcut_pk, collection_pk)
}

pub fn load_shortcut_surfaces(db_path: &Path, shortcut_pk: i64) -> Result<Vec<ShortcutSurface>, MacosError> {
    let conn = open_db(db_path)?;
    let mut stmt = conn.prepare(
        r#"
        SELECT c.Z_PK, c.ZNAME
        FROM Z_4SHORTCUTS rel
        JOIN ZCOLLECTION c ON c.Z_PK = rel.Z_4PARENTS1
        WHERE rel.Z_7SHORTCUTS = ?1
        ORDER BY c.Z_PK
        "#,
    )?;
    let rows = stmt.query_map(params![shortcut_pk], |row| {
        Ok((row.get::<_, i64>(0)?, row.get::<_, Option<String>>(1)?))
    })?;

    let mut surfaces = Vec::new();
    for row in rows {
        let (pk, name) = row?;
        match pk {
            2 => surfaces.push(ShortcutSurface::ShareSheet),
            6 => surfaces.push(ShortcutSurface::LibraryRoot),
            _ => {
                if let Some(name) = name {
                    surfaces.push(ShortcutSurface::Folder(name));
                }
            }
        }
    }
    Ok(surfaces)
}

pub fn load_shortcut_surfaces_live(shortcut_pk: i64) -> Result<Vec<ShortcutSurface>, MacosError> {
    let db_path = default_db_path()?;
    load_shortcut_surfaces(&db_path, shortcut_pk)
}

pub fn decode_input_policy(input_classes: &[u8]) -> Result<ShortcutInputPolicy, MacosError> {
    let classes = plist::from_bytes::<Vec<String>>(input_classes)
        .map_err(|error| MacosError::Other(format!("failed to decode shortcut input classes: {error}")))?;

    let policy = match classes.as_slice() {
        [] => ShortcutInputPolicy::Any,
        [single] if single == "WFURLContentItem" => ShortcutInputPolicy::Url,
        [single] if single == "WFStringContentItem" => ShortcutInputPolicy::Text,
        [single] if single == "WFImageContentItem" => ShortcutInputPolicy::Image,
        [single] if single == "WFGenericFileContentItem" => ShortcutInputPolicy::File,
        _ => ShortcutInputPolicy::Any,
    };
    Ok(policy)
}

pub fn load_shortcut_input_policy(db_path: &Path, shortcut_pk: i64) -> Result<ShortcutInputPolicy, MacosError> {
    let conn = open_db(db_path)?;
    let blob = conn
        .query_row(
            "SELECT ZINPUTCLASSESDATA FROM ZSHORTCUT WHERE Z_PK = ?1",
            params![shortcut_pk],
            |row| row.get::<_, Option<Vec<u8>>>(0),
        )
        .map_err(MacosError::from)?;

    match blob {
        Some(blob) => decode_input_policy(&blob),
        None => Ok(ShortcutInputPolicy::Any),
    }
}

pub fn load_shortcut_input_policy_live(shortcut_pk: i64) -> Result<ShortcutInputPolicy, MacosError> {
    let db_path = default_db_path()?;
    load_shortcut_input_policy(&db_path, shortcut_pk)
}

pub fn load_shortcut_payload(db_path: &Path, shortcut_pk: i64) -> Result<Vec<u8>, MacosError> {
    let conn = open_db(db_path)?;
    let payload = conn
        .query_row(
            "SELECT ZDATA FROM ZSHORTCUTACTIONS WHERE ZSHORTCUT = ?1",
            params![shortcut_pk],
            |row| row.get::<_, Option<Vec<u8>>>(0),
        )
        .map_err(MacosError::from)?;

    match payload {
        Some(payload) => Ok(payload),
        None => {
            let mut buffer = Vec::new();
            plist::to_writer_binary(&mut buffer, &Vec::<String>::new()).map_err(|error| {
                MacosError::Other(format!("failed to encode empty shortcut actions plist: {error}"))
            })?;
            Ok(buffer)
        }
    }
}

pub fn load_shortcut_payload_live(shortcut_pk: i64) -> Result<Vec<u8>, MacosError> {
    let db_path = default_db_path()?;
    load_shortcut_payload(&db_path, shortcut_pk)
}
