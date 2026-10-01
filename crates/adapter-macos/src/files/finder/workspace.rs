//! AppKit workspace boundaries; no Accessibility or Swift toolchain dependency.
use cueward_core::files::{FileError, FileErrorCode};
use objc2::{
    msg_send,
    rc::{Retained, autoreleasepool},
    runtime::{AnyClass, AnyObject},
};
use objc2_foundation::{NSArray, NSString, NSURL};
use std::path::Path;

#[link(name = "AppKit", kind = "framework")]
unsafe extern "C" {}

fn class(name: &'static std::ffi::CStr) -> Result<&'static AnyClass, FileError> {
    AnyClass::get(name).ok_or_else(|| {
        FileError::new(
            FileErrorCode::Unavailable,
            "AppKit class is unavailable; action was not submitted",
        )
    })
}

fn workspace() -> Result<Retained<AnyObject>, FileError> {
    let class = class(c"NSWorkspace")?;
    // SAFETY: AppKit NSWorkspace.sharedWorkspace returns a live NSWorkspace object.
    Ok(unsafe { msg_send![class, sharedWorkspace] })
}

/// Observe Finder instances without launching them.
pub(super) fn finder_pid() -> Result<Option<i32>, FileError> {
    autoreleasepool(|_| {
        let class = class(c"NSRunningApplication")?;
        let id = NSString::from_str("com.apple.finder");
        // SAFETY: Documented class selector returns an NSArray of NSRunningApplication.
        let apps: Retained<NSArray<AnyObject>> =
            unsafe { msg_send![class, runningApplicationsWithBundleIdentifier: &*id] };
        match apps.count() {
            0 => Ok(None),
            1 => {
                let app = apps.objectAtIndex(0);
                // SAFETY: The array's documented element type provides these getters.
                let terminated: bool = unsafe { msg_send![&*app, isTerminated] };
                let pid: i32 = unsafe { msg_send![&*app, processIdentifier] };
                Ok((!terminated && pid > 0).then_some(pid))
            }
            _ => Err(FileError::new(
                FileErrorCode::Unavailable,
                "multiple Finder processes are running; observe before retrying",
            )),
        }
    })
}

/// Observe the foreground owner, preserving unavailable state.
pub(super) fn foreground_pid() -> Option<i32> {
    autoreleasepool(|_| {
        let workspace = workspace().ok()?;
        // SAFETY: NSWorkspace.frontmostApplication is nullable NSRunningApplication.
        let app: Option<Retained<AnyObject>> =
            unsafe { msg_send![&*workspace, frontmostApplication] };
        let app = app?;
        let pid: i32 = unsafe { msg_send![&*app, processIdentifier] };
        (pid > 0).then_some(pid)
    })
}

/// Submit the already-scoped path to AppKit's explicit Finder activation API.
pub(super) fn reveal(path: &Path) -> Result<(), FileError> {
    let text = path.to_str().ok_or_else(|| {
        FileError::new(
            FileErrorCode::UnsupportedPathEncoding,
            "reveal path must be UTF-8",
        )
    })?;
    autoreleasepool(|_| {
        let workspace = workspace()?;
        let url = NSURL::fileURLWithPath(&NSString::from_str(text));
        let urls = NSArray::from_retained_slice(&[url]);
        // SAFETY: AppKit documents a void method accepting NSArray<NSURL>. It
        // requests Finder activation/selection and provides no completion callback.
        unsafe {
            let _: () = msg_send![&*workspace, activateFileViewerSelectingURLs: &*urls];
        }
        Ok(())
    })
}
