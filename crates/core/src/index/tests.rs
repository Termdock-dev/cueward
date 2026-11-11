use super::*;
use serde_json::json;

fn memory_index() -> CueIndex {
    let schema = build_schema();
    let index = Index::create_in_ram(schema.clone());
    configure(&index);
    CueIndex { index, schema }
}

fn cue(content: &str) -> Cue {
    serde_json::from_value(json!({"source":"notes", "timestamp":"2026-11-11T12:00:00Z",
        "title":"fixture", "content":content}))
    .unwrap()
}

#[test]
fn chinese_substrings_long_sentences_and_english_queries_are_searchable() {
    let index = memory_index();
    let long = "這是一段超過四十位元組的中文會議紀錄內容包含產品規劃與客戶訪談重點";
    index
        .add_cues(&[
            cue("今天討論客戶回饋"),
            cue(long),
            cue("Rust concurrency"),
            cue("客製化使用戶"),
        ])
        .unwrap();
    assert_eq!(index.search("客戶", 10).unwrap().len(), 2);
    assert_eq!(index.search("回饋", 10).unwrap().len(), 1);
    assert_eq!(index.search(long, 10).unwrap().len(), 1);
    assert_eq!(index.search("RUST", 10).unwrap().len(), 1);
    assert_eq!(index.search("content:concurrency", 10).unwrap().len(), 1);
    assert_eq!(index.search("\"Rust concurrency\"", 10).unwrap().len(), 1);
    assert!(index.search("query", 0).is_err());
}

#[test]
fn upserts_ignore_tag_changes_but_preserve_distinct_source_records() {
    let index = memory_index();
    let mut first = cue("uniquemarker");
    first.metadata.insert("source_id".into(), "1".into());
    index.add_cues(&[first.clone(), first.clone()]).unwrap();
    first.tags = vec!["newtag".into()];
    index.add_cues(&[first.clone()]).unwrap();
    let rows = index.search("uniquemarker", 10).unwrap();
    assert_eq!(rows.len(), 1);
    assert!(rows[0].contains("newtag"));
    first.metadata.insert("source_id".into(), "2".into());
    index.add_cues(&[first]).unwrap();
    assert_eq!(index.search("uniquemarker", 10).unwrap().len(), 2);
}

#[test]
fn legacy_import_preserves_duplicate_rows_and_original_files_and_can_resume() {
    let directory = tempfile::tempdir().unwrap();
    let legacy = directory.path().join("index");
    let destination = directory.path().join("index-v2");
    fs::create_dir(&legacy).unwrap();
    fs::create_dir(&destination).unwrap();
    let mut builder = Schema::builder();
    for name in FIELDS {
        builder.add_text_field(name, TEXT | STORED);
    }
    let schema = builder.build();
    let original = Index::create_in_dir(&legacy, schema.clone()).unwrap();
    let mut writer = original
        .writer::<tantivy::TantivyDocument>(50_000_000)
        .unwrap();
    for _ in 0..2 {
        let mut doc = tantivy::TantivyDocument::default();
        for name in FIELDS {
            doc.add_text(
                schema.get_field(name).unwrap(),
                if name == "content" {
                    "保留中文產品規劃紀錄"
                } else {
                    "legacy"
                },
            );
        }
        writer.add_document(doc).unwrap();
    }
    writer.commit().unwrap();
    drop(writer);
    let before = fs::read(legacy.join("meta.json")).unwrap();
    let index = memory_index();
    migration::import_legacy(&index.index, &legacy, &destination).unwrap();
    assert_eq!(index.search("產品規劃", 10).unwrap().len(), 2);
    // Simulate interruption after verified commit but before the durable completion marker.
    fs::remove_file(destination.join("legacy-import-complete.json")).unwrap();
    migration::import_legacy(&index.index, &legacy, &destination).unwrap();
    assert_eq!(index.search("產品規劃", 10).unwrap().len(), 2);
    assert_eq!(fs::read(legacy.join("meta.json")).unwrap(), before);
    assert_eq!(
        Index::open_in_dir(legacy)
            .unwrap()
            .reader()
            .unwrap()
            .searcher()
            .num_docs(),
        2
    );
}
