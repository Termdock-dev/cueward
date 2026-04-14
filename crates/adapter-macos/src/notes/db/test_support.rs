use std::ffi::OsString;
use std::path::Path;
use std::sync::Mutex;

static HOME_LOCK: Mutex<()> = Mutex::new(());

struct HomeGuard(Option<OsString>);

impl Drop for HomeGuard {
    fn drop(&mut self) {
        match self.0.take() {
            Some(previous_home) => unsafe {
                std::env::set_var("HOME", previous_home);
            },
            None => unsafe {
                std::env::remove_var("HOME");
            },
        }
    }
}

pub(crate) fn with_temp_home<T>(test: impl FnOnce(&Path) -> T) -> T {
    let _guard = HOME_LOCK.lock().expect("home lock");
    let temp = tempfile::tempdir().expect("tempdir");
    let previous_home = std::env::var_os("HOME");
    let _home_guard = HomeGuard(previous_home);
    unsafe {
        std::env::set_var("HOME", temp.path());
    }
    test(temp.path())
}
