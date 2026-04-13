use serde::Serialize;

mod threads;
mod x;

pub use threads::threads_extract_feed;
pub use x::{x_extract_feed, x_read_post, x_search};

#[derive(Clone, Debug, Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct SocialFeedPost {
    pub author: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub handle: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time: Option<String>,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub metrics: Vec<String>,
}
