use cueward_core::files::{FileError, FileErrorCode};
use std::marker::PhantomData;
use std::rc::Rc;

// Public Darwin resource.h constants: VFS materialize dataless files, thread, OFF.
const POLICY: i32 = 3;
const THREAD: i32 = 1;
const OFF: i32 = 1;

unsafe extern "C" {
    fn getiopolicy_np(policy: i32, scope: i32) -> i32;
    fn setiopolicy_np(policy: i32, scope: i32, value: i32) -> i32;
}

pub(super) struct NoMaterialization {
    previous: i32,
    // Thread-local policy must be restored by the thread that changed it.
    _same_thread: PhantomData<Rc<()>>,
}

impl NoMaterialization {
    pub(super) fn enter() -> Result<Self, FileError> {
        // SAFETY: these APIs take integer policy identifiers and retain no pointers.
        let previous = unsafe { getiopolicy_np(POLICY, THREAD) };
        if previous < 0 || unsafe { setiopolicy_np(POLICY, THREAD, OFF) } != 0 {
            return Err(FileError::new(
                FileErrorCode::Unavailable,
                "cannot deny dataless file downloads on this thread; operation was not started",
            ));
        }
        Ok(Self {
            previous,
            _same_thread: PhantomData,
        })
    }
}

impl Drop for NoMaterialization {
    fn drop(&mut self) {
        // SAFETY: restoration runs on the same thread; production worker exits next.
        unsafe {
            setiopolicy_np(POLICY, THREAD, self.previous);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn policy_is_thread_scoped_and_restored_after_error() {
        let before = unsafe { getiopolicy_np(POLICY, THREAD) };
        let operation = || -> Result<(), FileError> {
            let _guard = NoMaterialization::enter()?;
            assert_eq!(unsafe { getiopolicy_np(POLICY, THREAD) }, OFF);
            Err(FileError::new(FileErrorCode::NotFound, "test failure"))
        };
        assert_eq!(operation().unwrap_err().code, FileErrorCode::NotFound);
        assert_eq!(unsafe { getiopolicy_np(POLICY, THREAD) }, before);
    }
}
