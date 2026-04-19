use std::path::Path;
use std::path::PathBuf;

use rusqlite::{Connection, OptionalExtension, params};

use cueward_core::{ShortcutInputPolicy, ShortcutSurface};

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

pub fn encode_input_classes(policy: &ShortcutInputPolicy) -> Result<Vec<u8>, MacosError> {
    let classes: Vec<&str> = match policy {
        ShortcutInputPolicy::Any => vec![
            "WFAppContentItem",
            "WFAppStoreAppContentItem",
            "WFArticleContentItem",
            "WFContactContentItem",
            "WFDateContentItem",
            "WFEmailAddressContentItem",
            "WFFolderContentItem",
            "WFGenericFileContentItem",
            "WFImageContentItem",
            "WFiTunesProductContentItem",
            "WFLocationContentItem",
            "WFDCMapsLinkContentItem",
            "WFAVAssetContentItem",
            "WFPDFContentItem",
            "WFPhoneNumberContentItem",
            "WFRichTextContentItem",
            "WFSafariWebPageContentItem",
            "WFStringContentItem",
            "WFURLContentItem",
        ],
        ShortcutInputPolicy::Url | ShortcutInputPolicy::Urls => vec!["WFURLContentItem"],
        ShortcutInputPolicy::Text => vec!["WFStringContentItem"],
        ShortcutInputPolicy::Image => vec!["WFImageContentItem"],
        ShortcutInputPolicy::File => vec!["WFGenericFileContentItem"],
    };

    let mut buffer = Vec::new();
    plist::to_writer_binary(&mut buffer, &classes)
        .map_err(|error| MacosError::Other(format!("failed to encode shortcut input classes: {error}")))?;
    Ok(buffer)
}

pub fn sync_shortcut_surfaces(
    db_path: &Path,
    shortcut_pk: i64,
    surfaces: &[ShortcutSurface],
) -> Result<(), MacosError> {
    let mut conn = open_db(db_path)?;
    let tx = conn.transaction()?;

    let want_share = surfaces.iter().any(|surface| matches!(surface, ShortcutSurface::ShareSheet));
    let wanted_folders: Vec<&str> = surfaces
        .iter()
        .filter_map(|surface| match surface {
            ShortcutSurface::Folder(name) => Some(name.as_str()),
            _ => None,
        })
        .collect();

    tx.execute(
        r#"
        DELETE FROM Z_4SHORTCUTS
        WHERE Z_7SHORTCUTS = ?1
          AND Z_4PARENTS1 = 2
        "#,
        params![shortcut_pk],
    )?;

    tx.execute(
        r#"
        DELETE FROM Z_4SHORTCUTS
        WHERE Z_7SHORTCUTS = ?1
          AND Z_4PARENTS1 IN (
              SELECT Z_PK
              FROM ZCOLLECTION
              WHERE ZNAME IS NOT NULL
          )
        "#,
        params![shortcut_pk],
    )?;

    tx.execute(
        r#"
        DELETE FROM Z_4SHORTCUTS
        WHERE Z_7SHORTCUTS = ?1
          AND Z_4PARENTS1 = 6
        "#,
        params![shortcut_pk],
    )?;

    tx.execute(
        r#"
        INSERT OR IGNORE INTO Z_4SHORTCUTS (Z_4PARENTS1, Z_7SHORTCUTS, Z_FOK_7SHORTCUTS)
        VALUES (6, ?1, 6)
        "#,
        params![shortcut_pk],
    )?;

    if want_share {
        tx.execute(
            r#"
            INSERT OR IGNORE INTO Z_4SHORTCUTS (Z_4PARENTS1, Z_7SHORTCUTS, Z_FOK_7SHORTCUTS)
            VALUES (2, ?1, 2)
            "#,
            params![shortcut_pk],
        )?;
    }

    for folder_name in wanted_folders {
        let collection_pk: i64 = tx
            .query_row(
                "SELECT Z_PK FROM ZCOLLECTION WHERE ZNAME = ?1",
                params![folder_name],
                |row| row.get(0),
            )
            .optional()?
            .ok_or_else(|| MacosError::Other(format!("shortcut folder not found: {folder_name}")))?;

        tx.execute(
            r#"
            INSERT OR IGNORE INTO Z_4SHORTCUTS (Z_4PARENTS1, Z_7SHORTCUTS, Z_FOK_7SHORTCUTS)
            VALUES (?1, ?2, ?1)
            "#,
            params![collection_pk, shortcut_pk],
        )?;
    }

    tx.commit()?;
    Ok(())
}

pub fn sync_shortcut_surfaces_live(
    shortcut_pk: i64,
    surfaces: &[ShortcutSurface],
) -> Result<(), MacosError> {
    let db_path = default_db_path()?;
    sync_shortcut_surfaces(&db_path, shortcut_pk, surfaces)
}

pub fn ensure_shortcut_relation(
    db_path: &Path,
    shortcut_pk: i64,
    collection_pk: i64,
) -> Result<(), MacosError> {
    let conn = open_db(db_path)?;
    conn.execute(
        r#"
        INSERT INTO Z_4SHORTCUTS (Z_4PARENTS1, Z_7SHORTCUTS, Z_FOK_7SHORTCUTS)
        SELECT ?1, ?2, ?1
        WHERE NOT EXISTS (
            SELECT 1
            FROM Z_4SHORTCUTS
            WHERE Z_4PARENTS1 = ?1
              AND Z_7SHORTCUTS = ?2
        )
        "#,
        params![collection_pk, shortcut_pk],
    )?;
    Ok(())
}

pub fn ensure_shortcut_relation_live(shortcut_pk: i64, collection_pk: i64) -> Result<(), MacosError> {
    let db_path = default_db_path()?;
    ensure_shortcut_relation(&db_path, shortcut_pk, collection_pk)
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

pub fn update_shortcut_input_classes(
    db_path: &Path,
    shortcut_pk: i64,
    input_classes: &[u8],
) -> Result<(), MacosError> {
    let conn = open_db(db_path)?;
    conn.execute(
        r#"
        UPDATE ZSHORTCUT
        SET ZINPUTCLASSESDATA = ?1
        WHERE Z_PK = ?2
        "#,
        params![input_classes, shortcut_pk],
    )?;
    Ok(())
}

pub fn update_shortcut_input_classes_live(
    shortcut_pk: i64,
    input_classes: &[u8],
) -> Result<(), MacosError> {
    let db_path = default_db_path()?;
    update_shortcut_input_classes(&db_path, shortcut_pk, input_classes)
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

pub fn update_shortcut_actions_blob(
    db_path: &Path,
    shortcut_pk: i64,
    payload: &[u8],
    action_count: usize,
) -> Result<(), MacosError> {
    let mut conn = open_db(db_path)?;
    let tx = conn.transaction()?;

    tx.execute(
        "UPDATE ZSHORTCUTACTIONS SET ZDATA = ?1 WHERE ZSHORTCUT = ?2",
        params![payload, shortcut_pk],
    )?;
    tx.execute(
        r#"
        UPDATE ZSHORTCUT
        SET
            ZACTIONCOUNT = ?1,
            ZACTIONSDESCRIPTION = ?2,
            ZWORKFLOWSUBTITLE = ?3
        WHERE Z_PK = ?4
        "#,
        params![
            action_count as i64,
            format!("{action_count} actions"),
            format!("{action_count} actions"),
            shortcut_pk,
        ],
    )?;

    tx.commit()?;
    Ok(())
}

pub fn update_shortcut_actions_blob_live(
    shortcut_pk: i64,
    payload: &[u8],
    action_count: usize,
) -> Result<(), MacosError> {
    let db_path = default_db_path()?;
    update_shortcut_actions_blob(&db_path, shortcut_pk, payload, action_count)
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
