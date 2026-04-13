use crate::MacosError;
use crate::safari_guard::with_safari_session;
use serde::Serialize;
use std::path::PathBuf;

#[path = "bookmarks/plist_io.rs"]
mod plist_io;
#[path = "bookmarks/tree_ops.rs"]
mod tree_ops;

use self::plist_io::{add_bookmark_to_path, delete_bookmark_from_path, load_tree_from_path};
use self::tree_ops::{list_items_in_folder, search_items};

#[cfg(test)]
use self::plist_io::save_tree_to_path;
#[cfg(test)]
use self::tree_ops::{
    BookmarkEntry, BookmarkFolder, BookmarkNode, BookmarkTree, add_bookmark, delete_bookmark,
    parse_folder_path,
};

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SafariBookmarkItemKind {
    Folder,
    Bookmark,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct SafariBookmarkItem {
    pub kind: SafariBookmarkItemKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    pub folder_path: String,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct SafariBookmarksListResult {
    pub folder_path: String,
    pub items: Vec<SafariBookmarkItem>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct SafariBookmarksSearchResult {
    pub query: String,
    pub items: Vec<SafariBookmarkItem>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct SafariBookmarkMutationResult {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deleted: Option<bool>,
    pub bookmark: SafariBookmarkItem,
}

/// List direct bookmark or folder items under the root or a specific folder path.
pub fn list_bookmarks(folder: Option<&str>) -> Result<SafariBookmarksListResult, MacosError> {
    with_safari_session(|| {
        let tree = load_tree_from_path(&safari_bookmarks_path()?)?;
        Ok(SafariBookmarksListResult {
            folder_path: folder.unwrap_or_default().to_string(),
            items: list_items_in_folder(&tree, folder)?,
        })
    })
}

/// Search bookmarks recursively from the root or a specific folder path.
pub fn search_bookmarks(
    query: &str,
    folder: Option<&str>,
) -> Result<SafariBookmarksSearchResult, MacosError> {
    with_safari_session(|| {
        let tree = load_tree_from_path(&safari_bookmarks_path()?)?;
        Ok(SafariBookmarksSearchResult {
            query: query.to_string(),
            items: search_items(&tree, query, folder)?,
        })
    })
}

/// Add a bookmark to the root or a specific folder path.
pub fn add_bookmark_cli(
    title: &str,
    url: &str,
    folder: Option<&str>,
) -> Result<SafariBookmarkMutationResult, MacosError> {
    with_safari_session(|| {
        let bookmark = add_bookmark_to_path(&safari_bookmarks_path()?, folder, title, url)?;
        Ok(SafariBookmarkMutationResult {
            created: Some(true),
            deleted: None,
            bookmark,
        })
    })
}

/// Delete a bookmark from the root or a specific folder path by exact title + URL.
pub fn delete_bookmark_cli(
    title: &str,
    url: &str,
    folder: Option<&str>,
) -> Result<SafariBookmarkMutationResult, MacosError> {
    with_safari_session(|| {
        let bookmark = delete_bookmark_from_path(&safari_bookmarks_path()?, folder, title, url)?;
        Ok(SafariBookmarkMutationResult {
            created: None,
            deleted: Some(true),
            bookmark,
        })
    })
}

fn safari_bookmarks_path() -> Result<PathBuf, MacosError> {
    let home = std::env::var("HOME")
        .map_err(|_| MacosError::Other("HOME environment variable must be set".to_string()))?;
    let path = PathBuf::from(home).join("Library/Safari/Bookmarks.plist");
    if !path.exists() {
        return Err(MacosError::Other("bookmarks plist not found".to_string()));
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::{
        BookmarkEntry, BookmarkFolder, BookmarkNode, BookmarkTree, add_bookmark, delete_bookmark,
        list_items_in_folder, load_tree_from_path, parse_folder_path, save_tree_to_path,
        search_items,
    };
    use tempfile::tempdir;

    fn sample_bookmarks_tree() -> BookmarkTree {
        BookmarkTree {
            children: vec![
                BookmarkNode::Folder(BookmarkFolder {
                    title: "Work".to_string(),
                    children: vec![
                        BookmarkNode::Folder(BookmarkFolder {
                            title: "AI Tools".to_string(),
                            children: vec![
                                BookmarkNode::Bookmark(BookmarkEntry {
                                    title: "Claude".to_string(),
                                    url: "https://claude.ai".to_string(),
                                }),
                                BookmarkNode::Bookmark(BookmarkEntry {
                                    title: "Grok".to_string(),
                                    url: "https://grok.com".to_string(),
                                }),
                            ],
                        }),
                        BookmarkNode::Folder(BookmarkFolder {
                            title: "Docs".to_string(),
                            children: vec![BookmarkNode::Bookmark(BookmarkEntry {
                                title: "Rust Book".to_string(),
                                url: "https://doc.rust-lang.org/book/".to_string(),
                            })],
                        }),
                    ],
                }),
                BookmarkNode::Folder(BookmarkFolder {
                    title: "Personal".to_string(),
                    children: vec![BookmarkNode::Bookmark(BookmarkEntry {
                        title: "Blog".to_string(),
                        url: "https://example.com/blog".to_string(),
                    })],
                }),
            ],
        }
    }

    #[test]
    fn bookmarks_lists_direct_children_for_folder_path() {
        let tree = sample_bookmarks_tree();
        let items = list_items_in_folder(&tree, Some("Work/AI Tools")).expect("list folder");

        assert_eq!(items.len(), 2);
        assert_eq!(items[0].title.as_deref(), Some("Claude"));
        assert_eq!(items[1].title.as_deref(), Some("Grok"));
    }

    #[test]
    fn bookmarks_searches_recursively_from_folder_path() {
        let tree = sample_bookmarks_tree();
        let items = search_items(&tree, "claude", Some("Work")).expect("search");

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].folder_path, "Work/AI Tools");
        assert_eq!(items[0].url.as_deref(), Some("https://claude.ai"));
    }

    #[test]
    fn bookmarks_folder_lookup_is_case_insensitive() {
        let tree = sample_bookmarks_tree();
        let items = list_items_in_folder(&tree, Some("work/ai tools")).expect("list folder");

        assert_eq!(items.len(), 2);
        assert_eq!(items[0].title.as_deref(), Some("Claude"));
    }

    #[test]
    fn bookmarks_reject_invalid_folder_path_segments() {
        let err = parse_folder_path("Work//AI").expect_err("empty segment should fail");

        assert!(err.to_string().contains("invalid folder path"));
    }

    #[test]
    fn bookmarks_parses_folder_path_segments() {
        let parts = parse_folder_path("Work/AI Tools").expect("path");

        assert_eq!(parts, vec!["Work".to_string(), "AI Tools".to_string()]);
    }

    #[test]
    fn bookmarks_lists_root_direct_children_only() {
        let tree = sample_bookmarks_tree();
        let items = list_items_in_folder(&tree, None).expect("list root");

        assert_eq!(items.len(), 2);
        assert_eq!(items[0].title.as_deref(), Some("Work"));
        assert_eq!(items[1].title.as_deref(), Some("Personal"));
    }

    #[test]
    fn bookmarks_add_rejects_duplicate_title_and_url_in_same_folder() {
        let mut tree = sample_bookmarks_tree();

        let err = add_bookmark(
            &mut tree,
            Some("Work/AI Tools"),
            "Claude",
            "https://claude.ai",
        )
        .expect_err("duplicate should fail");

        assert!(err.to_string().contains("duplicate bookmark"));
    }

    #[test]
    fn bookmarks_add_allows_same_title_with_different_url() {
        let mut tree = sample_bookmarks_tree();

        add_bookmark(
            &mut tree,
            Some("Work/AI Tools"),
            "Claude",
            "https://example.com/claude-alt",
        )
        .expect("same title, different url is allowed");

        let items = list_items_in_folder(&tree, Some("Work/AI Tools")).expect("list folder");
        assert_eq!(items.len(), 3);
    }

    #[test]
    fn bookmarks_delete_matches_title_and_url() {
        let mut tree = sample_bookmarks_tree();

        let deleted = delete_bookmark(
            &mut tree,
            Some("Work/AI Tools"),
            "Claude",
            "https://claude.ai",
        )
        .expect("delete");

        assert_eq!(deleted.title.as_deref(), Some("Claude"));
        assert_eq!(deleted.url.as_deref(), Some("https://claude.ai"));
    }

    #[test]
    fn bookmarks_delete_returns_not_found_for_missing_item() {
        let mut tree = sample_bookmarks_tree();

        let err = delete_bookmark(
            &mut tree,
            Some("Work/AI Tools"),
            "Claude",
            "https://example.com/missing",
        )
        .expect_err("missing bookmark should fail");

        assert!(err.to_string().contains("bookmark not found"));
    }

    #[test]
    fn bookmarks_delete_reports_conflict_for_duplicate_fingerprint() {
        let mut tree = sample_bookmarks_tree();
        add_bookmark(
            &mut tree,
            Some("Work/AI Tools"),
            "Claude",
            "https://mirror.example/claude",
        )
        .expect("same title different url allowed");
        add_bookmark(
            &mut tree,
            Some("Work/AI Tools"),
            "Claude",
            "https://mirror.example/claude",
        )
        .expect_err("exact duplicate add stays blocked");

        tree.children = vec![BookmarkNode::Folder(BookmarkFolder {
            title: "Work".to_string(),
            children: vec![BookmarkNode::Folder(BookmarkFolder {
                title: "AI Tools".to_string(),
                children: vec![
                    BookmarkNode::Bookmark(BookmarkEntry {
                        title: "Claude".to_string(),
                        url: "https://claude.ai".to_string(),
                    }),
                    BookmarkNode::Bookmark(BookmarkEntry {
                        title: "Claude".to_string(),
                        url: "https://claude.ai".to_string(),
                    }),
                ],
            })],
        })];

        let err = delete_bookmark(
            &mut tree,
            Some("Work/AI Tools"),
            "Claude",
            "https://claude.ai",
        )
        .expect_err("duplicate fingerprint should conflict");

        assert!(err.to_string().contains("bookmark data conflict"));
    }

    #[test]
    fn bookmarks_round_trip_through_plist_file() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("Bookmarks.plist");
        let tree = sample_bookmarks_tree();

        save_tree_to_path(&tree, &path).expect("write plist");
        let loaded = load_tree_from_path(&path).expect("read plist");
        let items = list_items_in_folder(&loaded, Some("Work/AI Tools")).expect("list folder");

        assert_eq!(items.len(), 2);
        assert_eq!(items[0].title.as_deref(), Some("Claude"));
        assert_eq!(items[1].title.as_deref(), Some("Grok"));
    }
}
