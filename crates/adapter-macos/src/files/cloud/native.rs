use super::*;
use crate::files::metadata::{boolean, native_error, read, string};
use objc2::rc::autoreleasepool;
use objc2::runtime::AnyObject;
use objc2_foundation::*;

/// Read native resource keys without opening contents or requesting downloads.
pub(super) fn inspect(file: &FileInfo) -> Result<CloudResources, FileError> {
    Ok(autoreleasepool(|_| {
        let url = NSURL::fileURLWithPath_isDirectory(
            &NSString::from_str(&file.path),
            file.kind == FileKind::Directory,
        );
        // SAFETY: these process-lifetime Foundation symbols are NSURLResourceKeys.
        let ubiquitous = unsafe { read(&url, NSURLIsUbiquitousItemKey, boolean) };
        if !matches!(ubiquitous, ResourceValue::Available { value: true }) {
            let mut resources = skipped(
                if matches!(ubiquitous, ResourceValue::Available { value: false }) {
                    Skip::NotApplicable
                } else {
                    Skip::Unavailable
                },
            );
            resources.is_ubiquitous = ubiquitous;
            return resources;
        }
        unsafe {
            CloudResources {
                is_ubiquitous: ubiquitous,
                downloading_status: read(
                    &url,
                    NSURLUbiquitousItemDownloadingStatusKey,
                    downloading_status,
                ),
                is_downloading: read(&url, NSURLUbiquitousItemIsDownloadingKey, boolean),
                download_requested: read(&url, NSURLUbiquitousItemDownloadRequestedKey, boolean),
                is_uploaded: read(&url, NSURLUbiquitousItemIsUploadedKey, boolean),
                is_uploading: read(&url, NSURLUbiquitousItemIsUploadingKey, boolean),
                has_unresolved_conflicts: read(
                    &url,
                    NSURLUbiquitousItemHasUnresolvedConflictsKey,
                    boolean,
                ),
                downloading_error: read(&url, NSURLUbiquitousItemDownloadingErrorKey, error_value),
                uploading_error: read(&url, NSURLUbiquitousItemUploadingErrorKey, error_value),
            }
        }
    }))
}

#[derive(Clone, Copy)]
pub(super) enum Skip {
    NotApplicable,
    Unavailable,
}

/// Preserve unavailable membership separately from inapplicable item types.
pub(super) fn skipped(reason: Skip) -> CloudResources {
    fn field<T>(reason: Skip) -> ResourceValue<T> {
        match reason {
            Skip::NotApplicable => ResourceValue::NotApplicable,
            Skip::Unavailable => ResourceValue::Unavailable,
        }
    }
    CloudResources {
        is_ubiquitous: field(reason),
        downloading_status: field(reason),
        is_downloading: field(reason),
        download_requested: field(reason),
        is_uploaded: field(reason),
        is_uploading: field(reason),
        has_unresolved_conflicts: field(reason),
        downloading_error: field(reason),
        uploading_error: field(reason),
    }
}

fn downloading_status(value: &AnyObject) -> Result<DownloadingStatus, String> {
    let value = value
        .downcast_ref::<NSString>()
        .ok_or("download status is not an NSString")?;
    let text = string(value)?;
    // SAFETY: constants are NSString values; compare values rather than addresses.
    Ok(unsafe {
        if value == NSURLUbiquitousItemDownloadingStatusNotDownloaded {
            DownloadingStatus::NotDownloaded
        } else if value == NSURLUbiquitousItemDownloadingStatusDownloaded {
            DownloadingStatus::Downloaded
        } else if value == NSURLUbiquitousItemDownloadingStatusCurrent {
            DownloadingStatus::Current
        } else {
            DownloadingStatus::Other { value: text }
        }
    })
}

fn error_value(value: &AnyObject) -> Result<MetadataError, String> {
    let error = value
        .downcast_ref::<NSError>()
        .ok_or("download/upload error is not an NSError")?;
    let result = native_error(error);
    if result.domain.len() + result.message.len() > 65536 {
        return Err("native error exceeds 64 KiB".into());
    }
    Ok(result)
}

/// Submit one explicit iCloud download; success is asynchronous submission only.
pub(super) fn download(file: &FileInfo) -> Result<(), FileError> {
    autoreleasepool(|_| {
        let url = NSURL::fileURLWithPath_isDirectory(&NSString::from_str(&file.path), false);
        NSFileManager::defaultManager().startDownloadingUbiquitousItemAtURL_error(&url).map_err(|error| {
            let error = native_error(&error);
            let code = if error.domain == "NSCocoaErrorDomain" {
                match error.code { 4 | 260 => FileErrorCode::NotFound, 257 | 513 => FileErrorCode::PermissionDenied, _ => FileErrorCode::Unavailable }
            } else { FileErrorCode::Unavailable };
            FileError::new(code, format!("download submission failed [{}:{}]: {}; reobserve provider state before retrying", error.domain, error.code, error.message))
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_cloud_decoders_keep_states_unknown_values_and_errors() {
        unsafe {
            assert_eq!(
                downloading_status(NSURLUbiquitousItemDownloadingStatusCurrent).unwrap(),
                DownloadingStatus::Current
            );
            assert_eq!(
                downloading_status(NSURLUbiquitousItemDownloadingStatusDownloaded).unwrap(),
                DownloadingStatus::Downloaded
            );
            assert_eq!(
                downloading_status(NSURLUbiquitousItemDownloadingStatusNotDownloaded).unwrap(),
                DownloadingStatus::NotDownloaded
            );
        }
        let future = NSString::from_str("future value");
        assert_eq!(
            downloading_status(&future).unwrap(),
            DownloadingStatus::Other {
                value: "future value".into()
            }
        );
        assert!(downloading_status(&NSNumber::numberWithBool(true)).is_err());
        assert!(error_value(&future).is_err());
        let error = unsafe {
            NSError::errorWithDomain_code_userInfo(&NSString::from_str("test.cloud"), 7, None)
        };
        let decoded = error_value(&error).unwrap();
        assert_eq!(decoded.domain, "test.cloud");
        assert_eq!(decoded.code, 7);
        assert!(!decoded.message.is_empty());
    }
}
