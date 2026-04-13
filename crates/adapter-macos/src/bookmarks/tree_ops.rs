use super::{SafariBookmarkItem, SafariBookmarkItemKind};
use crate::MacosError;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct BookmarkTree {
    pub(super) children: Vec<BookmarkNode>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum BookmarkNode {
    Folder(BookmarkFolder),
    Bookmark(BookmarkEntry),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct BookmarkFolder {
    pub(super) title: String,
    pub(super) children: Vec<BookmarkNode>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct BookmarkEntry {
    pub(super) title: String,
    pub(super) url: String,
}

fn invalid_folder_path() -> MacosError {
    MacosError::Other("invalid folder path".to_string())
}

pub(super) fn parse_folder_path(path: &str) -> Result<Vec<String>, MacosError> {
    if path.trim().is_empty() {
        return Err(invalid_folder_path());
    }

    let parts: Vec<String> = path
        .split('/')
        .map(str::trim)
        .map(ToOwned::to_owned)
        .collect();

    if parts.iter().any(|part| part.is_empty()) {
        return Err(invalid_folder_path());
    }

    Ok(parts)
}

fn join_folder_path(parent: &str, title: &str) -> String {
    if parent.is_empty() {
        title.to_string()
    } else {
        format!("{parent}/{title}")
    }
}

fn resolve_children<'a>(
    tree: &'a BookmarkTree,
    folder: Option<&str>,
) -> Result<(&'a [BookmarkNode], String), MacosError> {
    let Some(folder) = folder else {
        return Ok((&tree.children, String::new()));
    };

    let parts = parse_folder_path(folder)?;
    let mut children = tree.children.as_slice();
    let mut current_path = String::new();

    for part in parts {
        let next = children.iter().find_map(|node| match node {
            BookmarkNode::Folder(folder) if folder.title == part => Some(folder),
            _ => None,
        });
        let Some(folder) = next else {
            return Err(invalid_folder_path());
        };
        current_path = join_folder_path(&current_path, &folder.title);
        children = folder.children.as_slice();
    }

    Ok((children, current_path))
}

fn map_child_to_item(node: &BookmarkNode, current_folder_path: &str) -> SafariBookmarkItem {
    match node {
        BookmarkNode::Folder(folder) => SafariBookmarkItem {
            kind: SafariBookmarkItemKind::Folder,
            title: Some(folder.title.clone()),
            url: None,
            folder_path: join_folder_path(current_folder_path, &folder.title),
        },
        BookmarkNode::Bookmark(entry) => SafariBookmarkItem {
            kind: SafariBookmarkItemKind::Bookmark,
            title: Some(entry.title.clone()),
            url: Some(entry.url.clone()),
            folder_path: current_folder_path.to_string(),
        },
    }
}

pub(super) fn list_items_in_folder(
    tree: &BookmarkTree,
    folder: Option<&str>,
) -> Result<Vec<SafariBookmarkItem>, MacosError> {
    let (children, current_path) = resolve_children(tree, folder)?;
    Ok(children
        .iter()
        .map(|node| map_child_to_item(node, &current_path))
        .collect())
}

pub(super) fn search_items(
    tree: &BookmarkTree,
    query: &str,
    folder: Option<&str>,
) -> Result<Vec<SafariBookmarkItem>, MacosError> {
    let (children, current_path) = resolve_children(tree, folder)?;
    let normalized_query = query.trim().to_ascii_lowercase();
    let mut results = Vec::new();
    collect_search_matches(children, &current_path, &normalized_query, &mut results);
    Ok(results)
}

fn collect_search_matches(
    children: &[BookmarkNode],
    current_folder_path: &str,
    normalized_query: &str,
    results: &mut Vec<SafariBookmarkItem>,
) {
    for node in children {
        match node {
            BookmarkNode::Folder(folder) => {
                let next_path = join_folder_path(current_folder_path, &folder.title);
                collect_search_matches(
                    folder.children.as_slice(),
                    &next_path,
                    normalized_query,
                    results,
                );
            }
            BookmarkNode::Bookmark(entry) => {
                let title = entry.title.to_ascii_lowercase();
                let url = entry.url.to_ascii_lowercase();
                if title.contains(normalized_query) || url.contains(normalized_query) {
                    results.push(SafariBookmarkItem {
                        kind: SafariBookmarkItemKind::Bookmark,
                        title: Some(entry.title.clone()),
                        url: Some(entry.url.clone()),
                        folder_path: current_folder_path.to_string(),
                    });
                }
            }
        }
    }
}

#[cfg(test)]
fn resolve_children_mut_tree<'a>(
    tree: &'a mut BookmarkTree,
    folder: Option<&str>,
) -> Result<&'a mut Vec<BookmarkNode>, MacosError> {
    let Some(folder) = folder else {
        return Ok(&mut tree.children);
    };
    let parts = parse_folder_path(folder)?;
    resolve_children_mut_nodes(&mut tree.children, &parts)
}

#[cfg(test)]
fn resolve_children_mut_nodes<'a>(
    children: &'a mut Vec<BookmarkNode>,
    parts: &[String],
) -> Result<&'a mut Vec<BookmarkNode>, MacosError> {
    let Some((segment, rest)) = parts.split_first() else {
        return Ok(children);
    };

    let Some(index) = children.iter().position(|node| match node {
        BookmarkNode::Folder(folder) => folder.title == *segment,
        BookmarkNode::Bookmark(_) => false,
    }) else {
        return Err(invalid_folder_path());
    };

    match children.get_mut(index) {
        Some(BookmarkNode::Folder(folder)) => {
            resolve_children_mut_nodes(&mut folder.children, rest)
        }
        _ => Err(invalid_folder_path()),
    }
}

#[cfg(test)]
pub(super) fn add_bookmark(
    tree: &mut BookmarkTree,
    folder: Option<&str>,
    title: &str,
    url: &str,
) -> Result<(), MacosError> {
    let children = resolve_children_mut_tree(tree, folder)?;
    let duplicate = children.iter().any(|node| match node {
        BookmarkNode::Bookmark(entry) => entry.title == title && entry.url == url,
        BookmarkNode::Folder(_) => false,
    });
    if duplicate {
        return Err(MacosError::Other("duplicate bookmark".to_string()));
    }

    children.push(BookmarkNode::Bookmark(BookmarkEntry {
        title: title.to_string(),
        url: url.to_string(),
    }));
    Ok(())
}

#[cfg(test)]
pub(super) fn delete_bookmark(
    tree: &mut BookmarkTree,
    folder: Option<&str>,
    title: &str,
    url: &str,
) -> Result<SafariBookmarkItem, MacosError> {
    let current_folder_path = folder.unwrap_or_default().to_string();
    let children = resolve_children_mut_tree(tree, folder)?;
    let matches: Vec<usize> = children
        .iter()
        .enumerate()
        .filter_map(|(index, node)| match node {
            BookmarkNode::Bookmark(entry) if entry.title == title && entry.url == url => {
                Some(index)
            }
            _ => None,
        })
        .collect();

    match matches.len() {
        0 => Err(MacosError::Other("bookmark not found".to_string())),
        1 => {
            let index = matches[0];
            let BookmarkNode::Bookmark(entry) = children.remove(index) else {
                return Err(MacosError::Other("bookmark not found".to_string()));
            };
            Ok(SafariBookmarkItem {
                kind: SafariBookmarkItemKind::Bookmark,
                title: Some(entry.title),
                url: Some(entry.url),
                folder_path: current_folder_path,
            })
        }
        _ => Err(MacosError::Other("bookmark data conflict".to_string())),
    }
}
