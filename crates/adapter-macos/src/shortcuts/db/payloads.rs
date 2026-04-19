use std::path::Path;

use rusqlite::params;

use cueward_core::ShortcutInputPolicy;

use crate::MacosError;

use super::{default_db_path, open_db};

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
