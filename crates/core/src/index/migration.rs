use std::fs;
use std::path::Path;

use tantivy::Index;
use tantivy::schema::Value;

use super::{invalid, stored_row, upsert};

/// Resume an interrupted import by upserting stable legacy document addresses, then compare all rows.
pub(super) fn import_legacy(
    index: &Index,
    legacy: &Path,
    destination: &Path,
) -> tantivy::Result<()> {
    let marker = destination.join("legacy-import-complete.json");
    match fs::read(&marker) {
        Ok(bytes) => {
            let receipt: serde_json::Value = serde_json::from_slice(&bytes).map_err(invalid)?;
            if receipt["schema_version"] != 2 || receipt["legacy_retained"] != true {
                return Err(invalid("invalid legacy import receipt; original retained"));
            }
            return Ok(());
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(e.into()),
    }
    if legacy.exists() && !legacy.join("meta.json").exists() {
        return Err(invalid(
            "legacy index has no metadata; original retained; inspect before retrying",
        ));
    }
    let mut expected = Vec::new();
    if legacy.join("meta.json").exists() {
        let source = Index::open_in_dir(legacy)?;
        let reader = source.reader()?;
        let searcher = reader.searcher();
        let mut writer = index.writer(50_000_000)?;
        for (ordinal, segment) in searcher.segment_readers().iter().enumerate() {
            for doc_id in segment.doc_ids_alive() {
                let document = searcher.doc::<tantivy::TantivyDocument>(
                    tantivy::DocAddress::new(ordinal as u32, doc_id),
                )?;
                let row = stored_row(&source.schema(), &document)?;
                let id = format!("legacy:{}:{doc_id}", segment.segment_id().uuid_string());
                upsert(&index.schema(), &mut writer, &id, &row)?;
                expected.push(serde_json::to_string(&row).map_err(invalid)?);
            }
        }
        writer.commit()?;
        verify_import(index, &mut expected)?;
        eprintln!(
            "imported {} stored rows into index-v2; original index retained",
            expected.len()
        );
    }
    crate::storage::write_atomic(
        &marker,
        &serde_json::to_vec(
            &serde_json::json!({"schema_version":2,"rows":expected.len(),"legacy_retained":true}),
        )
        .map_err(invalid)?,
        false,
    )?;
    Ok(())
}

fn verify_import(index: &Index, expected: &mut [String]) -> tantivy::Result<()> {
    let reader = index.reader()?;
    let searcher = reader.searcher();
    let schema = index.schema();
    let id_field = schema.get_field("cue_id")?;
    let mut actual = Vec::new();
    for (ordinal, segment) in searcher.segment_readers().iter().enumerate() {
        for doc_id in segment.doc_ids_alive() {
            let document = searcher.doc::<tantivy::TantivyDocument>(tantivy::DocAddress::new(
                ordinal as u32,
                doc_id,
            ))?;
            if document
                .get_first(id_field)
                .and_then(|v| v.as_str())
                .is_some_and(|id| id.starts_with("legacy:"))
            {
                actual.push(
                    serde_json::to_string(&stored_row(&schema, &document)?).map_err(invalid)?,
                );
            }
        }
    }
    expected.sort();
    actual.sort();
    if expected != actual {
        return Err(invalid(
            "legacy index copy verification failed; original retained",
        ));
    }
    Ok(())
}
