#[derive(Debug, Clone)]
pub enum ShortcutSelector {
    Id(String),
    Name(String),
}

#[derive(Debug, Clone)]
pub struct ShortcutRecord {
    pub pk: i64,
    pub name: String,
    pub workflow_id: String,
    pub action_count: i64,
}
