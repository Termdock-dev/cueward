//! Foundation resource values, queried separately to retain per-field failures.
use cueward_core::files::*;
use objc2::msg_send;
use objc2::rc::{Retained, autoreleasepool};
use objc2::runtime::{AnyClass, AnyObject};
use objc2_foundation::{
    NSArray, NSError, NSNumber, NSString, NSURL, NSURLIsAliasFileKey, NSURLIsPackageKey,
    NSURLResourceKey, NSURLTagNamesKey,
};
use std::fs::Metadata;
use std::path::Path;

// Resolve the macOS 11 key at runtime so older systems can still launch other
// commands. Foundation loads the UTType class when it returns a content type.
#[link(name = "System")]
unsafe extern "C" {
    fn dlsym(
        handle: *mut std::ffi::c_void,
        symbol: *const std::ffi::c_char,
    ) -> *mut std::ffi::c_void;
}

const MAX_TAGS: usize = 256;
const MAX_TEXT_BYTES: usize = 65536;

pub(super) fn inspect(path: &Path, metadata: &Metadata) -> Result<ResourceMetadata, FileError> {
    let text = path.to_str().ok_or_else(|| {
        FileError::new(
            FileErrorCode::UnsupportedPathEncoding,
            "resource path must be UTF-8",
        )
    })?;
    Ok(autoreleasepool(|_| {
        let url = NSURL::fileURLWithPath_isDirectory(&NSString::from_str(text), metadata.is_dir());
        // SAFETY: These Foundation constants are linked NSString resource keys.
        unsafe {
            ResourceMetadata {
                content_type: content_type_key().map_or(ResourceValue::Unsupported, |key| {
                    read(&url, key, content_type)
                }),
                finder_tags: read(&url, NSURLTagNamesKey, tags),
                is_package: read(&url, NSURLIsPackageKey, boolean),
                is_alias_file: read(&url, NSURLIsAliasFileKey, boolean),
            }
        }
    }))
}

fn content_type_key() -> Option<&'static NSURLResourceKey> {
    // SAFETY: Darwin's RTLD_DEFAULT is -2. The symbol is a process-lifetime
    // Foundation NSString* constant; check both symbol and object for null.
    unsafe {
        let symbol = dlsym((-2isize) as *mut _, c"NSURLContentTypeKey".as_ptr());
        let address = (symbol as *const *const NSURLResourceKey).as_ref()?;
        address.as_ref()
    }
}

fn read<T>(
    url: &NSURL,
    key: &NSURLResourceKey,
    decode: impl Fn(&AnyObject) -> Result<T, String>,
) -> ResourceValue<T> {
    let mut value: Option<Retained<AnyObject>> = None;
    // SAFETY: Output is retained as AnyObject, then checked against its expected class.
    let result = unsafe { url.getResourceValue_forKey_error(&mut value, key) }.map(|()| value);
    decode_resource(result, decode)
}

fn decode_resource<T>(
    result: Result<Option<Retained<AnyObject>>, Retained<NSError>>,
    decode: impl Fn(&AnyObject) -> Result<T, String>,
) -> ResourceValue<T> {
    match result {
        Err(error) => ResourceValue::Error {
            error: native_error(&error),
        },
        Ok(value) => match value {
            None => ResourceValue::Unavailable,
            Some(value) => match decode(&value) {
                Ok(value) => ResourceValue::Available { value },
                Err(message) => ResourceValue::Error {
                    error: MetadataError {
                        domain: "cueward.files.metadata".into(),
                        code: 1,
                        message,
                    },
                },
            },
        },
    }
}

#[cfg(test)]
#[path = "metadata_decode_tests.rs"]
mod tests;

fn native_error(error: &NSError) -> MetadataError {
    MetadataError {
        domain: error.domain().to_string(),
        code: error.code() as i64,
        message: error.localizedDescription().to_string(),
    }
}

fn content_type(value: &AnyObject) -> Result<String, String> {
    let class = AnyClass::get(c"UTType").ok_or("UTType class unavailable")?;
    // SAFETY: The class exists and Objective-C's isKindOfClass: accepts every object.
    let valid: bool = unsafe { msg_send![value, isKindOfClass: class] };
    if !valid {
        return Err("content type resource is not a UTType".into());
    }
    // SAFETY: UTType.identifier is documented as an NSString; retain and validate it.
    let identifier: Option<Retained<AnyObject>> = unsafe { msg_send![value, identifier] };
    let identifier = identifier
        .as_ref()
        .and_then(|object| object.downcast_ref::<NSString>())
        .ok_or("UTType identifier is unavailable or not a string")?;
    string(identifier)
}

fn boolean(value: &AnyObject) -> Result<bool, String> {
    value
        .downcast_ref::<NSNumber>()
        .map(NSNumber::boolValue)
        .ok_or_else(|| "boolean resource is not an NSNumber".into())
}

fn tags(value: &AnyObject) -> Result<Vec<String>, String> {
    let array = value
        .downcast_ref::<NSArray<AnyObject>>()
        .ok_or("tags resource is not an NSArray")?;
    if array.count() > MAX_TAGS {
        return Err("tags exceed the 256-item metadata limit".into());
    }
    let mut total = 0;
    let mut tags = Vec::new();
    for index in 0..array.count() {
        let object = array.objectAtIndex(index);
        let value = object
            .downcast_ref::<NSString>()
            .ok_or("tag value is not an NSString")?;
        let value = string(value)?;
        total += value.len();
        if total > MAX_TEXT_BYTES {
            return Err("tags exceed the 64 KiB metadata limit".into());
        }
        tags.push(value);
    }
    Ok(tags)
}

fn string(value: &NSString) -> Result<String, String> {
    let value = value.to_string();
    if value.len() > MAX_TEXT_BYTES {
        return Err("resource string exceeds the 64 KiB metadata limit".into());
    }
    Ok(value)
}
