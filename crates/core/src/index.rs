use std::fs;
use std::path::PathBuf;

use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;
use tantivy::schema::Value;
use tantivy::schema::{STORED, Schema, TEXT};
use tantivy::{Index, IndexWriter, ReloadPolicy, doc};

use crate::Cue;

fn index_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_default();
    PathBuf::from(home).join(".cueward/index")
}

fn build_schema() -> Schema {
    let mut builder = Schema::builder();
    builder.add_text_field("source", TEXT | STORED);
    builder.add_text_field("timestamp", TEXT | STORED);
    builder.add_text_field("title", TEXT | STORED);
    builder.add_text_field("content", TEXT | STORED);
    builder.add_text_field("url", TEXT | STORED);
    builder.add_text_field("tags", TEXT | STORED);
    builder.build()
}

pub struct CueIndex {
    index: Index,
    schema: Schema,
}

impl CueIndex {
    pub fn open_or_create() -> tantivy::Result<Self> {
        let dir = index_dir();
        fs::create_dir_all(&dir).ok();

        let schema = build_schema();
        let index = if dir.join("meta.json").exists() {
            Index::open_in_dir(&dir)?
        } else {
            Index::create_in_dir(&dir, schema.clone())?
        };

        Ok(Self { index, schema })
    }

    pub fn add_cues(&self, cues: &[Cue]) -> tantivy::Result<usize> {
        let mut writer: IndexWriter = self.index.writer(50_000_000)?;

        let source = self.schema.get_field("source").unwrap();
        let timestamp = self.schema.get_field("timestamp").unwrap();
        let title = self.schema.get_field("title").unwrap();
        let content = self.schema.get_field("content").unwrap();
        let url = self.schema.get_field("url").unwrap();
        let tags = self.schema.get_field("tags").unwrap();

        let mut count = 0;
        for cue in cues {
            let source_str = match cue.source {
                crate::CueSource::Safari => "safari",
                crate::CueSource::Notes => "notes",
                crate::CueSource::Messages => "messages",
                crate::CueSource::Ocr => "ocr",
            };
            writer.add_document(doc!(
                source => source_str,
                timestamp => cue.timestamp.to_rfc3339(),
                title => cue.title.as_deref().unwrap_or(""),
                content => cue.content.as_str(),
                url => cue.url.as_deref().unwrap_or(""),
                tags => cue.tags.join(", "),
            ))?;
            count += 1;
        }

        writer.commit()?;
        Ok(count)
    }

    pub fn search(&self, query_str: &str, limit: usize) -> tantivy::Result<Vec<String>> {
        let reader = self
            .index
            .reader_builder()
            .reload_policy(ReloadPolicy::OnCommitWithDelay)
            .try_into()?;
        let searcher = reader.searcher();

        let content = self.schema.get_field("content").unwrap();
        let title = self.schema.get_field("title").unwrap();
        let tags = self.schema.get_field("tags").unwrap();

        let query_parser = QueryParser::for_index(&self.index, vec![content, title, tags]);
        let query = query_parser
            .parse_query(query_str)
            .map_err(|e| tantivy::TantivyError::InvalidArgument(e.to_string()))?;

        let top_docs = searcher.search(&query, &TopDocs::with_limit(limit))?;

        let mut results = Vec::new();
        let field_names: Vec<&str> = vec!["source", "timestamp", "title", "content", "url", "tags"];
        let fields: Vec<_> = field_names
            .iter()
            .map(|n| self.schema.get_field(n).unwrap())
            .collect();

        for (_score, doc_address) in top_docs {
            let doc = searcher.doc::<tantivy::TantivyDocument>(doc_address)?;
            let mut map = serde_json::Map::new();
            for (i, field) in fields.iter().enumerate() {
                let val = doc.get_first(*field).and_then(|v| v.as_str()).unwrap_or("");
                map.insert(
                    field_names[i].to_string(),
                    serde_json::Value::String(val.to_string()),
                );
            }
            results.push(serde_json::to_string(&map).unwrap_or_default());
        }

        Ok(results)
    }
}
