use std::path::Path;

use rusqlite::{OptionalExtension, params};

use cueward_core::ShortcutSurface;

use crate::MacosError;

use super::{default_db_path, open_db};

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
