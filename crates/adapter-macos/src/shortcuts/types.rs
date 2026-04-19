use serde::Serialize;

#[derive(Debug, Clone)]
pub enum ShortcutSelector {
    Id(String),
    Name(String),
}

#[derive(Debug, Clone, Serialize)]
pub struct ShortcutRecord {
    pub pk: i64,
    pub name: String,
    pub workflow_id: String,
    pub action_count: i64,
}
