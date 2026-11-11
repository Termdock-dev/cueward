//! Versioned BM25 index with retry-safe cue upserts and non-destructive legacy import.
use std::fs;

use sha2::{Digest, Sha256};
use tantivy::collector::TopDocs;
use tantivy::directory::MmapDirectory;
use tantivy::query::QueryParser;
use tantivy::schema::{
    IndexRecordOption, STORED, STRING, Schema, TEXT, TextFieldIndexing, TextOptions, Value,
};
use tantivy::tokenizer::{LowerCaser, RemoveLongFilter, TextAnalyzer};
use tantivy::{Index, IndexWriter, Term, doc};

use crate::Cue;
mod migration;
#[cfg(test)]
mod tests;
mod tokenizer;

const FIELDS: [&str; 6] = ["source", "timestamp", "title", "content", "url", "tags"];

fn build_schema() -> Schema {
    let mut builder = Schema::builder();
    let text = TextOptions::default().set_stored().set_indexing_options(
        TextFieldIndexing::default()
            .set_tokenizer("cueward-mixed-v1")
            .set_index_option(IndexRecordOption::WithFreqsAndPositions),
    );
    for name in FIELDS {
        builder.add_text_field(
            name,
            if matches!(name, "title" | "content" | "tags") {
                text.clone()
            } else {
                TEXT | STORED
            },
        );
    }
    builder.add_text_field("cue_id", STRING | STORED);
    builder.build()
}

fn configure(index: &Index) {
    index.tokenizers().register(
        "cueward-mixed-v1",
        TextAnalyzer::builder(tokenizer::MixedTokenizer)
            .filter(RemoveLongFilter::limit(40))
            .filter(LowerCaser)
            .build(),
    );
}

fn invalid(error: impl ToString) -> tantivy::TantivyError {
    tantivy::TantivyError::InvalidArgument(error.to_string())
}

/// Searchable cue index; the legacy index remains intact alongside index-v2.
pub struct CueIndex {
    index: Index,
    schema: Schema,
}

impl CueIndex {
    /// Open v2, importing and verifying the old stored fields once without modifying the old index.
    pub fn open_or_create() -> tantivy::Result<Self> {
        let _lock = crate::storage::lock("index")?;
        let root = crate::storage::data_dir()?;
        let dir = root.join("index-v2");
        fs::create_dir_all(&dir)?;
        let schema = build_schema();
        let index = Index::open_or_create(MmapDirectory::open(&dir)?, schema.clone())?;
        configure(&index);
        migration::import_legacy(&index, &root.join("index"), &dir)?;
        Ok(Self { index, schema })
    }

    /// Upsert complete cue versions in one commit; repeated batches and changed tags do not append duplicates.
    pub fn add_cues(&self, cues: &[Cue]) -> tantivy::Result<usize> {
        let mut writer: IndexWriter = self.index.writer(50_000_000)?;
        for cue in cues {
            let row = serde_json::json!({
                "source": cue.source, "timestamp":cue.timestamp.to_rfc3339(),
                "title":cue.title.as_deref().unwrap_or(""), "content":cue.content,
                "url":cue.url.as_deref().unwrap_or(""), "tags":cue.tags.join(", ")
            });
            let mut identity = serde_json::to_value(cue).map_err(invalid)?;
            if let Some(map) = identity.as_object_mut() {
                map.remove("tags");
                map.remove("attachment_segments");
            }
            let key = format!(
                "cue:{:x}",
                Sha256::digest(serde_json::to_vec(&identity).map_err(invalid)?)
            );
            upsert(&self.schema, &mut writer, &key, &row)?;
        }
        writer.commit()?;
        Ok(cues.len())
    }

    /// Search stored fields using the same mixed tokenizer as indexing.
    pub fn search(&self, query_str: &str, limit: usize) -> tantivy::Result<Vec<String>> {
        if limit == 0 {
            return Err(invalid("search limit must be greater than zero"));
        }
        let reader = self.index.reader()?;
        let searcher = reader.searcher();
        let fields = ["content", "title", "tags"]
            .iter()
            .map(|name| self.schema.get_field(name))
            .collect::<tantivy::Result<Vec<_>>>()?;
        let parser = QueryParser::for_index(&self.index, fields);
        let query = parser.parse_query(query_str).map_err(invalid)?;
        let docs = searcher.search(&query, &TopDocs::with_limit(limit))?;
        docs.into_iter()
            .map(|(_, address)| {
                let document = searcher.doc::<tantivy::TantivyDocument>(address)?;
                serde_json::to_string(&stored_row(&self.schema, &document)?).map_err(invalid)
            })
            .collect()
    }
}

fn upsert(
    schema: &Schema,
    writer: &mut IndexWriter,
    key: &str,
    row: &serde_json::Value,
) -> tantivy::Result<()> {
    let id = schema.get_field("cue_id")?;
    writer.delete_term(Term::from_field_text(id, key));
    let mut document = doc!(id => key);
    for name in FIELDS {
        document.add_text(schema.get_field(name)?, row[name].as_str().unwrap_or(""));
    }
    writer.add_document(document)?;
    Ok(())
}

fn stored_row(
    schema: &Schema,
    document: &tantivy::TantivyDocument,
) -> tantivy::Result<serde_json::Value> {
    let mut row = serde_json::Map::new();
    for name in FIELDS {
        let field = schema.get_field(name)?;
        let text = document
            .get_first(field)
            .and_then(|value| value.as_str())
            .unwrap_or("");
        row.insert(name.to_owned(), text.into());
    }
    Ok(row.into())
}
