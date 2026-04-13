use super::tree_ops::{
    BookmarkEntry, BookmarkFolder, BookmarkNode, BookmarkTree, parse_folder_path,
};
use super::{SafariBookmarkItem, SafariBookmarkItemKind};
use crate::MacosError;
use plist::{Dictionary, Value};
use std::path::Path;
use uuid::Uuid;

pub(super) fn load_tree_from_path(path: &Path) -> Result<BookmarkTree, MacosError> {
    let value = Value::from_file(path)
        .map_err(|err| MacosError::Other(format!("plist decode failed: {err}")))?;
    tree_from_value(&value)
}

#[cfg(test)]
pub(super) fn save_tree_to_path(tree: &BookmarkTree, path: &Path) -> Result<(), MacosError> {
    tree_to_value(tree)
        .to_file_xml(path)
        .map_err(|err| MacosError::Other(format!("plist write failed: {err}")))
}

pub(super) fn add_bookmark_to_path(
    path: &Path,
    folder: Option<&str>,
    title: &str,
    url: &str,
) -> Result<SafariBookmarkItem, MacosError> {
    let mut root = load_root_value(path)?;
    let children = resolve_children_mut_value(&mut root, folder)?;
    let duplicate = children
        .iter()
        .any(|value| bookmark_matches(value, title, url));
    if duplicate {
        return Err(MacosError::Other("duplicate bookmark".to_string()));
    }

    children.push(make_leaf_value(title, url));
    save_root_value(path, &root)?;

    Ok(SafariBookmarkItem {
        kind: SafariBookmarkItemKind::Bookmark,
        title: Some(title.to_string()),
        url: Some(url.to_string()),
        folder_path: folder.unwrap_or_default().to_string(),
    })
}

pub(super) fn delete_bookmark_from_path(
    path: &Path,
    folder: Option<&str>,
    title: &str,
    url: &str,
) -> Result<SafariBookmarkItem, MacosError> {
    let mut root = load_root_value(path)?;
    let children = resolve_children_mut_value(&mut root, folder)?;
    let matches: Vec<usize> = children
        .iter()
        .enumerate()
        .filter_map(|(index, value)| bookmark_matches(value, title, url).then_some(index))
        .collect();

    let result = match matches.len() {
        0 => Err(MacosError::Other("bookmark not found".to_string())),
        1 => {
            let value = children.remove(matches[0]);
            bookmark_item_from_value(&value, folder.unwrap_or_default())
        }
        _ => Err(MacosError::Other("bookmark data conflict".to_string())),
    }?;

    save_root_value(path, &root)?;
    Ok(result)
}

fn tree_from_value(value: &Value) -> Result<BookmarkTree, MacosError> {
    let dict = value.as_dictionary().ok_or_else(|| {
        MacosError::Other("plist decode failed: root is not a dictionary".to_string())
    })?;
    let children = dict
        .get("Children")
        .and_then(Value::as_array)
        .ok_or_else(|| MacosError::Other("plist decode failed: missing Children".to_string()))?;

    let mut parsed_children = Vec::new();
    for child in children {
        if let Some(node) = node_from_value(child)? {
            parsed_children.push(node);
        }
    }

    Ok(BookmarkTree {
        children: parsed_children,
    })
}

fn node_from_value(value: &Value) -> Result<Option<BookmarkNode>, MacosError> {
    let dict = value.as_dictionary().ok_or_else(|| {
        MacosError::Other("plist decode failed: bookmark node is not a dictionary".to_string())
    })?;
    let Some(bookmark_type) = dict.get("WebBookmarkType").and_then(Value::as_string) else {
        return Ok(None);
    };

    match bookmark_type {
        "WebBookmarkTypeList" => {
            let title = dict
                .get("Title")
                .and_then(Value::as_string)
                .unwrap_or_default()
                .to_string();
            let mut children = Vec::new();
            if let Some(values) = dict.get("Children").and_then(Value::as_array) {
                for child in values {
                    if let Some(node) = node_from_value(child)? {
                        children.push(node);
                    }
                }
            }
            Ok(Some(BookmarkNode::Folder(BookmarkFolder {
                title,
                children,
            })))
        }
        "WebBookmarkTypeLeaf" => {
            let title = dict
                .get("URIDictionary")
                .and_then(Value::as_dictionary)
                .and_then(|uri| uri.get("title"))
                .and_then(Value::as_string)
                .unwrap_or_default()
                .to_string();
            let Some(url) = dict.get("URLString").and_then(Value::as_string) else {
                return Ok(None);
            };
            Ok(Some(BookmarkNode::Bookmark(BookmarkEntry {
                title,
                url: url.to_string(),
            })))
        }
        _ => Ok(None),
    }
}

#[cfg(test)]
fn tree_to_value(tree: &BookmarkTree) -> Value {
    let mut dict = Dictionary::new();
    dict.insert("Title".to_string(), Value::String(String::new()));
    dict.insert(
        "WebBookmarkType".to_string(),
        Value::String("WebBookmarkTypeList".to_string()),
    );
    dict.insert(
        "WebBookmarkFileVersion".to_string(),
        Value::Integer(1_i64.into()),
    );
    dict.insert(
        "Children".to_string(),
        Value::Array(tree.children.iter().map(node_to_value).collect()),
    );
    Value::Dictionary(dict)
}

#[cfg(test)]
fn node_to_value(node: &BookmarkNode) -> Value {
    match node {
        BookmarkNode::Folder(folder) => {
            let mut dict = Dictionary::new();
            dict.insert("Title".to_string(), Value::String(folder.title.clone()));
            dict.insert(
                "WebBookmarkType".to_string(),
                Value::String("WebBookmarkTypeList".to_string()),
            );
            dict.insert(
                "Children".to_string(),
                Value::Array(folder.children.iter().map(node_to_value).collect()),
            );
            Value::Dictionary(dict)
        }
        BookmarkNode::Bookmark(entry) => {
            let mut uri = Dictionary::new();
            uri.insert("title".to_string(), Value::String(entry.title.clone()));

            let mut dict = Dictionary::new();
            dict.insert(
                "WebBookmarkType".to_string(),
                Value::String("WebBookmarkTypeLeaf".to_string()),
            );
            dict.insert("URIDictionary".to_string(), Value::Dictionary(uri));
            dict.insert("URLString".to_string(), Value::String(entry.url.clone()));
            Value::Dictionary(dict)
        }
    }
}

fn load_root_value(path: &Path) -> Result<Value, MacosError> {
    Value::from_file(path).map_err(|err| MacosError::Other(format!("plist decode failed: {err}")))
}

fn save_root_value(path: &Path, root: &Value) -> Result<(), MacosError> {
    let parent = path
        .parent()
        .ok_or_else(|| MacosError::Other("invalid path".to_string()))?;
    let mut temp = tempfile::NamedTempFile::new_in(parent)
        .map_err(|err| MacosError::Other(format!("failed to create temp file: {err}")))?;
    root.to_writer_binary(&mut temp)
        .map_err(|err| MacosError::Other(format!("plist write failed: {err}")))?;
    temp.persist(path)
        .map_err(|err| MacosError::Other(format!("failed to persist plist: {err}")))?;
    Ok(())
}

fn resolve_children_mut_value<'a>(
    root: &'a mut Value,
    folder: Option<&str>,
) -> Result<&'a mut Vec<Value>, MacosError> {
    let dict = root.as_dictionary_mut().ok_or_else(|| {
        MacosError::Other("plist decode failed: root is not a dictionary".to_string())
    })?;
    let mut children = ensure_children_array(dict)?;

    let Some(folder) = folder else {
        return Ok(children);
    };

    let parts = parse_folder_path(folder)?;
    for part in parts {
        let Some(index) = children
            .iter()
            .position(|value| folder_matches(value, &part))
        else {
            return Err(MacosError::Other("invalid folder path".to_string()));
        };
        let next = children
            .get_mut(index)
            .and_then(Value::as_dictionary_mut)
            .map(ensure_children_array)
            .transpose()?
            .ok_or_else(|| MacosError::Other("invalid folder path".to_string()))?;
        children = next;
    }

    Ok(children)
}

fn ensure_children_array(dict: &mut Dictionary) -> Result<&mut Vec<Value>, MacosError> {
    if !dict.contains_key("Children") {
        dict.insert("Children".to_string(), Value::Array(Vec::new()));
    }

    dict.get_mut("Children")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| MacosError::Other("invalid folder path".to_string()))
}

fn folder_matches(value: &Value, title: &str) -> bool {
    value
        .as_dictionary()
        .map(|dict| {
            dict.get("WebBookmarkType").and_then(Value::as_string) == Some("WebBookmarkTypeList")
                && dict.get("Title").and_then(Value::as_string) == Some(title)
        })
        .unwrap_or(false)
}

fn bookmark_matches(value: &Value, title: &str, url: &str) -> bool {
    value
        .as_dictionary()
        .map(|dict| {
            dict.get("WebBookmarkType").and_then(Value::as_string) == Some("WebBookmarkTypeLeaf")
                && dict
                    .get("URIDictionary")
                    .and_then(Value::as_dictionary)
                    .and_then(|uri| uri.get("title"))
                    .and_then(Value::as_string)
                    == Some(title)
                && dict.get("URLString").and_then(Value::as_string) == Some(url)
        })
        .unwrap_or(false)
}

fn make_leaf_value(title: &str, url: &str) -> Value {
    let mut uri = Dictionary::new();
    uri.insert("title".to_string(), Value::String(title.to_string()));

    let mut dict = Dictionary::new();
    dict.insert(
        "WebBookmarkType".to_string(),
        Value::String("WebBookmarkTypeLeaf".to_string()),
    );
    dict.insert("URIDictionary".to_string(), Value::Dictionary(uri));
    dict.insert("URLString".to_string(), Value::String(url.to_string()));
    dict.insert(
        "WebBookmarkUUID".to_string(),
        Value::String(Uuid::new_v4().to_string().to_uppercase()),
    );
    Value::Dictionary(dict)
}

fn bookmark_item_from_value(
    value: &Value,
    folder_path: &str,
) -> Result<SafariBookmarkItem, MacosError> {
    let dict = value
        .as_dictionary()
        .ok_or_else(|| MacosError::Other("bookmark not found".to_string()))?;
    let title = dict
        .get("URIDictionary")
        .and_then(Value::as_dictionary)
        .and_then(|uri| uri.get("title"))
        .and_then(Value::as_string)
        .map(ToOwned::to_owned);
    let url = dict
        .get("URLString")
        .and_then(Value::as_string)
        .map(ToOwned::to_owned);

    Ok(SafariBookmarkItem {
        kind: SafariBookmarkItemKind::Bookmark,
        title,
        url,
        folder_path: folder_path.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::{add_bookmark_to_path, delete_bookmark_from_path};
    use plist::{Dictionary, Value};
    use tempfile::tempdir;

    fn empty_folder_root() -> Value {
        let mut folder = Dictionary::new();
        folder.insert("Title".to_string(), Value::String("Ryugu".to_string()));
        folder.insert(
            "WebBookmarkType".to_string(),
            Value::String("WebBookmarkTypeList".to_string()),
        );
        folder.insert(
            "WebBookmarkUUID".to_string(),
            Value::String("F0776FB0-47B8-4DE0-BF4A-9D8ADFA9332A".to_string()),
        );

        let mut root = Dictionary::new();
        root.insert("Title".to_string(), Value::String(String::new()));
        root.insert(
            "WebBookmarkType".to_string(),
            Value::String("WebBookmarkTypeList".to_string()),
        );
        root.insert(
            "WebBookmarkFileVersion".to_string(),
            Value::Integer(1_i64.into()),
        );
        root.insert(
            "Children".to_string(),
            Value::Array(vec![Value::Dictionary(folder)]),
        );
        Value::Dictionary(root)
    }

    #[test]
    fn add_bookmark_to_empty_folder_without_children_key_succeeds() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("Bookmarks.plist");
        empty_folder_root()
            .to_file_binary(&path)
            .expect("write plist root");

        let result = add_bookmark_to_path(
            &path,
            Some("Ryugu"),
            "Smoke Test",
            "https://example.com/smoke",
        )
        .expect("add bookmark");

        assert_eq!(result.title.as_deref(), Some("Smoke Test"));
        assert_eq!(result.url.as_deref(), Some("https://example.com/smoke"));
        assert_eq!(result.folder_path, "Ryugu");
    }

    #[test]
    fn delete_from_empty_folder_without_children_key_returns_not_found() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("Bookmarks.plist");
        empty_folder_root()
            .to_file_binary(&path)
            .expect("write plist root");

        let err = delete_bookmark_from_path(&path, Some("Ryugu"), "Missing", "https://example.com")
            .expect_err("delete should fail");

        assert!(err.to_string().contains("bookmark not found"));
    }
}
