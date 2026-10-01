//! Public macOS Metadata C API. NSString/NSArray are toll-free CF bridges.
use super::*;
use objc2::rc::{Retained, autoreleasepool};
use objc2_foundation::{NSArray, NSString};
use std::ffi::{CStr, c_void};
use std::ptr::{NonNull, null};

type Ref = *const c_void;
const UTF8: u32 = 0x08000100;

#[link(name = "CoreServices", kind = "framework")]
unsafe extern "C" {
    fn MDQueryCreate(allocator: Ref, query: Ref, values: Ref, sorting: Ref) -> Ref;
    fn MDQuerySetSearchScope(query: Ref, scope: Ref, options: u32);
    fn MDQuerySetMaxCount(query: Ref, count: isize);
    fn MDQueryExecute(query: Ref, options: usize) -> u8;
    fn MDQueryGetResultCount(query: Ref) -> isize;
    fn MDQueryGetResultAtIndex(query: Ref, index: isize) -> Ref;
    fn MDItemCopyAttribute(item: Ref, key: Ref) -> Ref;
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFRelease(value: Ref);
    fn CFGetTypeID(value: Ref) -> usize;
    fn CFStringGetTypeID() -> usize;
    fn CFStringGetLength(value: Ref) -> isize;
    fn CFStringGetCString(
        value: Ref,
        buffer: *mut std::ffi::c_char,
        size: isize,
        encoding: u32,
    ) -> u8;
}

struct Owned(NonNull<c_void>);
impl Owned {
    fn new(pointer: Ref, message: &str) -> Result<Self, FileError> {
        NonNull::new(pointer.cast_mut())
            .map(Self)
            .ok_or_else(|| unavailable(message))
    }
    fn pointer(&self) -> Ref {
        self.0.as_ptr()
    }
}
impl Drop for Owned {
    fn drop(&mut self) {
        // SAFETY: Only +1 Create/Copy CF objects enter this wrapper; release once.
        unsafe { CFRelease(self.pointer()) };
    }
}

/// Gather bounded candidate paths from one canonical directory scope.
pub(super) fn search(
    root: &Path,
    expression: &str,
    max_candidates: usize,
) -> Result<Vec<PathBuf>, FileError> {
    autoreleasepool(|_| {
        let root = root.to_str().ok_or_else(|| {
            FileError::new(
                FileErrorCode::UnsupportedPathEncoding,
                "Spotlight root is not UTF-8",
            )
        })?;
        let expression = NSString::from_str(expression);
        let scope = NSString::from_str(root);
        let scopes = NSArray::from_slice(&[&*scope]);
        // SAFETY: NSString/NSArray bridge to CFString/CFArray. All borrowed
        // objects outlive the query, which is released before this pool drains.
        let query = unsafe {
            Owned::new(
                MDQueryCreate(null(), Retained::as_ptr(&expression).cast(), null(), null()),
                "Spotlight could not create the query",
            )?
        };
        unsafe {
            MDQuerySetSearchScope(query.pointer(), Retained::as_ptr(&scopes).cast(), 0);
            // One sentinel result makes overflow detectable without unbounded results.
            MDQuerySetMaxCount(query.pointer(), (max_candidates + 1) as isize);
            // kMDQuerySynchronous = 1: static gathering, no ongoing live updates.
            if MDQueryExecute(query.pointer(), 1) == 0 {
                return Err(unavailable("Spotlight could not execute the query"));
            }
        }
        paths(&query, max_candidates)
    })
}

fn paths(query: &Owned, max_candidates: usize) -> Result<Vec<PathBuf>, FileError> {
    // SAFETY: query is a live, completed synchronous MDQuery.
    let count = unsafe { MDQueryGetResultCount(query.pointer()) };
    if count < 0 {
        return Err(unavailable("Spotlight returned an invalid candidate count"));
    }
    if count as usize > max_candidates {
        return Err(FileError::new(
            FileErrorCode::ScanLimit,
            "Spotlight candidates exceed max-candidates; narrow root or text",
        ));
    }
    let key = NSString::from_str("kMDItemPath");
    let mut paths = Vec::with_capacity(count as usize);
    let mut path_bytes = 0;
    for index in 0..count {
        // SAFETY: index is within the completed query's count; default results
        // are borrowed MDItemRefs. No custom result callback is installed.
        let item = unsafe { MDQueryGetResultAtIndex(query.pointer(), index) };
        if item.is_null() {
            return Err(unavailable("Spotlight candidate is unavailable"));
        }
        let value = unsafe {
            Owned::new(
                MDItemCopyAttribute(item, Retained::as_ptr(&key).cast()),
                "Spotlight candidate path is unavailable",
            )?
        };
        let path = PathBuf::from(string(&value)?);
        budget_path(&mut path_bytes, &path)?;
        paths.push(path);
    }
    Ok(paths)
}

fn string(value: &Owned) -> Result<String, FileError> {
    // SAFETY: value is a live CF object; type-check before using string methods.
    if unsafe { CFGetTypeID(value.pointer()) != CFStringGetTypeID() } {
        return Err(unavailable("Spotlight candidate path is not a string"));
    }
    let length = unsafe { CFStringGetLength(value.pointer()) };
    if !(0..=16384).contains(&length) {
        return Err(FileError::new(
            FileErrorCode::ScanLimit,
            "Spotlight path exceeds 16 KiB",
        ));
    }
    decode_string(value, length)
}

fn decode_string(value: &Owned, length: isize) -> Result<String, FileError> {
    // UTF-8 fits in four bytes per UTF-16 unit plus NUL. Check actual byte size too.
    let mut buffer = vec![0u8; length as usize * 4 + 1];
    if unsafe {
        CFStringGetCString(
            value.pointer(),
            buffer.as_mut_ptr().cast(),
            buffer.len() as isize,
            UTF8,
        )
    } == 0
    {
        return Err(FileError::new(
            FileErrorCode::UnsupportedPathEncoding,
            "Spotlight path cannot be decoded as UTF-8",
        ));
    }
    let text = CStr::from_bytes_until_nul(&buffer)
        .map_err(|e| unavailable(e.to_string()))?
        .to_str()
        .map_err(|e| unavailable(e.to_string()))?;
    // Embedded NUL would silently shorten C strings; compare UTF-16 unit counts.
    if text.encode_utf16().count() != length as usize {
        return Err(FileError::new(
            FileErrorCode::UnsupportedPathEncoding,
            "Spotlight path contains NUL",
        ));
    }
    if text.len() > 16384 {
        return Err(FileError::new(
            FileErrorCode::ScanLimit,
            "Spotlight path exceeds 16 KiB",
        ));
    }
    Ok(text.to_owned())
}

fn unavailable(message: impl Into<String>) -> FileError {
    FileError::new(FileErrorCode::Unavailable, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn value(text: &str) -> Owned {
        // Transfer NSString's +1 ownership into the toll-free CF wrapper.
        Owned::new(Retained::into_raw(NSString::from_str(text)).cast(), "test").unwrap()
    }

    #[test]
    fn native_path_decoding_preserves_unicode_and_rejects_nul_and_oversize() {
        let text = "/tmp/臺灣\n space é e\u{301}";
        assert_eq!(string(&value(text)).unwrap(), text);
        assert_eq!(
            string(&value("/tmp/a\0b")).unwrap_err().code,
            FileErrorCode::UnsupportedPathEncoding
        );
        assert_eq!(
            string(&value(&"a".repeat(16385))).unwrap_err().code,
            FileErrorCode::ScanLimit
        );
        assert_eq!(
            string(&value(&"臺".repeat(6000))).unwrap_err().code,
            FileErrorCode::ScanLimit
        );
    }
}
