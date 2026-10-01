use super::tests::Fixture;
use super::*;
use objc2_foundation::{NSArray, NSString, NSURL, NSURLBookmarkCreationOptions, NSURLTagNamesKey};
use std::fs;
use std::os::unix::fs::symlink;

fn metadata(f: &Fixture, path: &str) -> FileMetadata {
    let FileResponse::Metadata(result) = f.run(path, FileAction::Metadata).unwrap() else {
        panic!("metadata")
    };
    result
}

fn url(path: &Path, directory: bool) -> objc2::rc::Retained<NSURL> {
    NSURL::fileURLWithPath_isDirectory(&NSString::from_str(path.to_str().unwrap()), directory)
}

#[test]
fn content_types_packages_and_plain_directories_use_native_resource_values() {
    let f = Fixture::new();
    f.write("text.txt", "臺灣");
    fs::create_dir(f.0.path().join("Plain")).unwrap();
    fs::create_dir(f.0.path().join("Sample.app")).unwrap();
    let text = metadata(&f, "text.txt");
    assert_eq!(
        text.resources.content_type,
        ResourceValue::Available {
            value: "public.plain-text".into()
        }
    );
    assert_eq!(
        text.resources.is_package,
        ResourceValue::Available { value: false }
    );
    assert_eq!(
        text.resources.is_alias_file,
        ResourceValue::Available { value: false }
    );
    let plain = metadata(&f, "Plain");
    assert_eq!(
        plain.resources.content_type,
        ResourceValue::Available {
            value: "public.folder".into()
        }
    );
    assert_eq!(
        plain.resources.is_package,
        ResourceValue::Available { value: false }
    );
    let app = metadata(&f, "Sample.app");
    assert_eq!(app.file.kind, FileKind::Directory);
    assert_eq!(
        app.resources.is_package,
        ResourceValue::Available { value: true }
    );
    assert_eq!(
        app.resources.content_type,
        ResourceValue::Available {
            value: "com.apple.application-bundle".into()
        }
    );
    assert_eq!(
        fs::read(f.0.path().join("text.txt")).unwrap(),
        "臺灣".as_bytes()
    );
}

#[test]
fn finder_tags_preserve_names_and_tag_changes_invalidate_file_revision() {
    let f = Fixture::new();
    f.write("text.txt", "unchanged");
    let before = metadata(&f, "text.txt");
    let names = ["臺灣", "</external>", "space tag"];
    let strings: Vec<_> = names
        .iter()
        .map(|value| NSString::from_str(value))
        .collect();
    let tags = NSArray::from_retained_slice(&strings);
    // SAFETY: The NSString array matches NSURLTagNamesKey's documented value type.
    unsafe {
        url(&f.0.path().join("text.txt"), false)
            .setResourceValue_forKey_error(Some(&tags), NSURLTagNamesKey)
    }
    .unwrap();
    let after = metadata(&f, "text.txt");
    assert_eq!(
        after.resources.finder_tags,
        ResourceValue::Available {
            value: names.map(str::to_owned).to_vec()
        }
    );
    assert_ne!(before.file.version, after.file.version);
    let mut request = f.request("text.txt", FileAction::Metadata);
    request.expected_version = Some(before.file.version);
    assert_eq!(
        execute_worker(&request).unwrap_err().code,
        FileErrorCode::Changed
    );
    assert_eq!(fs::read(f.0.path().join("text.txt")).unwrap(), b"unchanged");
}

#[test]
fn alias_is_identified_without_resolving_its_outside_root_target() {
    let f = Fixture::new();
    let outside = Fixture::new();
    outside.write("target.txt", "outside secret");
    let data = url(&outside.0.path().join("target.txt"), false)
        .bookmarkDataWithOptions_includingResourceValuesForKeys_relativeToURL_error(
            NSURLBookmarkCreationOptions::SuitableForBookmarkFile,
            None,
            None,
        )
        .unwrap();
    NSURL::writeBookmarkData_toURL_options_error(&data, &url(&f.0.path().join("alias"), false), 0)
        .unwrap();
    let original = fs::read(f.0.path().join("alias")).unwrap();
    let result = metadata(&f, "alias");
    assert_eq!(
        result.resources.is_alias_file,
        ResourceValue::Available { value: true }
    );
    assert_eq!(result.file.kind, FileKind::File);
    assert!(result.file.path.ends_with("/alias"));
    assert_eq!(result.file.link_target, None);
    assert_eq!(
        result.resources.content_type,
        ResourceValue::Available {
            value: "com.apple.alias-file".into()
        }
    );
    assert_eq!(fs::read(f.0.path().join("alias")).unwrap(), original);
    assert_eq!(
        fs::read(outside.0.path().join("target.txt")).unwrap(),
        b"outside secret"
    );
}

#[test]
fn leaf_symlinks_are_not_queried_and_explicit_following_remains_scoped() {
    let f = Fixture::new();
    f.write("a.txt", "a");
    symlink("a.txt", f.0.path().join("link")).unwrap();
    symlink("missing", f.0.path().join("broken")).unwrap();
    symlink("/etc/passwd", f.0.path().join("outside")).unwrap();
    for path in ["link", "broken", "outside"] {
        let result = metadata(&f, path);
        assert_eq!(result.file.kind, FileKind::Symlink);
        assert_eq!(result.resources.content_type, ResourceValue::NotApplicable);
        assert_eq!(result.resources.finder_tags, ResourceValue::NotApplicable);
        assert_eq!(result.resources.is_alias_file, ResourceValue::NotApplicable);
    }
    let mut request = f.request("link", FileAction::Metadata);
    request.follow_links = true;
    let FileResponse::Metadata(result) = execute_worker(&request).unwrap() else {
        panic!("metadata")
    };
    assert_eq!(result.file.kind, FileKind::File);
    assert_eq!(
        result.resources.content_type,
        ResourceValue::Available {
            value: "public.plain-text".into()
        }
    );
    request.path = "outside".into();
    assert_eq!(
        execute_worker(&request).unwrap_err().code,
        FileErrorCode::OutsideRoot
    );
}

#[test]
fn dataless_and_special_files_do_not_enter_the_resource_hook() {
    struct Deny;
    impl FilePlatform for Deny {
        fn stamp(&self, metadata: &Metadata) -> FileStamp {
            let mut stamp = MacFiles.stamp(metadata);
            if metadata.is_file() {
                stamp.data_state = DataState::Dataless;
            }
            stamp
        }
        fn open_regular(&self, _: &Path) -> io::Result<File> {
            panic!("no content reads")
        }
        fn resource_metadata(&self, _: &Path, _: &Metadata) -> Result<ResourceMetadata, FileError> {
            panic!("must not query dataless or special files")
        }
    }
    let f = Fixture::new();
    f.write("placeholder", "");
    let FileResponse::Metadata(result) =
        cueward_core::files::execute(&Deny, &f.request("placeholder", FileAction::Metadata))
            .unwrap()
    else {
        panic!("metadata")
    };
    assert_eq!(result.resources.finder_tags, ResourceValue::Unavailable);
    let status = std::process::Command::new("/usr/bin/mkfifo")
        .arg(f.0.path().join("pipe"))
        .status()
        .unwrap();
    assert!(status.success());
    let FileResponse::Metadata(result) =
        cueward_core::files::execute(&Deny, &f.request("pipe", FileAction::Metadata)).unwrap()
    else {
        panic!("metadata")
    };
    assert_eq!(result.file.kind, FileKind::Other);
    assert_eq!(result.resources.content_type, ResourceValue::NotApplicable);
}

#[test]
fn default_platform_is_unsupported_and_resource_time_file_changes_discard_results() {
    struct DefaultPlatform;
    impl FilePlatform for DefaultPlatform {
        fn stamp(&self, metadata: &Metadata) -> FileStamp {
            MacFiles.stamp(metadata)
        }
        fn open_regular(&self, _: &Path) -> io::Result<File> {
            panic!("no content reads")
        }
    }
    struct Replace;
    impl FilePlatform for Replace {
        fn stamp(&self, metadata: &Metadata) -> FileStamp {
            MacFiles.stamp(metadata)
        }
        fn open_regular(&self, _: &Path) -> io::Result<File> {
            panic!("no content reads")
        }
        fn resource_metadata(
            &self,
            path: &Path,
            _: &Metadata,
        ) -> Result<ResourceMetadata, FileError> {
            fs::write(path, "replacement during resource lookup")?;
            Ok(ResourceMetadata::unsupported())
        }
    }
    let f = Fixture::new();
    f.write("a", "original");
    let request = f.request("a", FileAction::Metadata);
    let FileResponse::Metadata(result) =
        cueward_core::files::execute(&DefaultPlatform, &request).unwrap()
    else {
        panic!("metadata")
    };
    assert_eq!(result.resources, ResourceMetadata::unsupported());
    assert_eq!(
        cueward_core::files::execute(&Replace, &request)
            .unwrap_err()
            .code,
        FileErrorCode::Changed
    );
}

#[test]
fn a_resource_error_preserves_the_other_fields_in_a_successful_operation() {
    struct Partial;
    impl FilePlatform for Partial {
        fn stamp(&self, metadata: &Metadata) -> FileStamp {
            MacFiles.stamp(metadata)
        }
        fn open_regular(&self, _: &Path) -> io::Result<File> {
            panic!("no content reads")
        }
        fn resource_metadata(&self, _: &Path, _: &Metadata) -> Result<ResourceMetadata, FileError> {
            Ok(ResourceMetadata {
                content_type: ResourceValue::Available {
                    value: "public.plain-text".into(),
                },
                finder_tags: ResourceValue::Error {
                    error: MetadataError {
                        domain: "fixture.permission".into(),
                        code: 13,
                        message: "</external> denied".into(),
                    },
                },
                is_package: ResourceValue::Available { value: false },
                is_alias_file: ResourceValue::Unavailable,
            })
        }
    }
    let f = Fixture::new();
    f.write("a.txt", "content");
    let request = f.request("a.txt", FileAction::Metadata);
    let FileResponse::Metadata(result) = cueward_core::files::execute(&Partial, &request).unwrap()
    else {
        panic!("metadata")
    };
    assert!(matches!(
        result.resources.content_type,
        ResourceValue::Available { .. }
    ));
    assert!(
        matches!(result.resources.finder_tags, ResourceValue::Error { error } if error.code == 13)
    );
    assert_eq!(
        result.resources.is_package,
        ResourceValue::Available { value: false }
    );
    assert_eq!(result.resources.is_alias_file, ResourceValue::Unavailable);
    assert_eq!(fs::read(f.0.path().join("a.txt")).unwrap(), b"content");
}
