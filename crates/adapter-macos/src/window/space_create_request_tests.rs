use super::*;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Stdio};
use std::time::Instant;

const CHILD_DIRECTORY: &str = "CUEWARD_TEST_CREATE_REQUEST";
const CHILD_NAME: &str = "CUEWARD_TEST_CREATE_NAME";
const TEST: &str = "window::spaces::create::request_tests::creation_request_rejects_overlap_before_compilation_and_through_readback";

struct Request {
    child: Child,
    directory: PathBuf,
    name: String,
}

impl Request {
    fn start(directory: &Path, name: &str, released: bool) -> Self {
        if released {
            for phase in ["compile", "readback"] {
                fs::write(directory.join(format!("release-{phase}-{name}")), b"").unwrap();
            }
        }
        let child = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", TEST])
            .env(CHILD_DIRECTORY, directory)
            .env(CHILD_NAME, name)
            .env("PATH", directory)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        Self {
            child,
            directory: directory.to_owned(),
            name: name.into(),
        }
    }

    fn release(&self, phase: &str) {
        fs::write(
            self.directory
                .join(format!("release-{phase}-{}", self.name)),
            b"",
        )
        .unwrap();
    }

    fn wait_for(&self, phase: &str) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while !self
            .directory
            .join(format!("{phase}-{}", self.name))
            .exists()
        {
            assert!(
                Instant::now() < deadline,
                "missing {phase} for {}",
                self.name
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    fn finish(&mut self) -> Value {
        self.wait_for("result");
        assert!(self.child.wait().unwrap().success());
        serde_json::from_slice(
            &fs::read(self.directory.join(format!("result-{}", self.name))).unwrap(),
        )
        .unwrap()
    }
}

impl Drop for Request {
    fn drop(&mut self) {
        self.release("compile");
        self.release("readback");
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn child_request(directory: &Path) {
    let name = std::env::var(CHILD_NAME).unwrap();
    let result = match create_space() {
        Ok(result) => json!({"ok": result}),
        Err(error) => json!({"error": error.to_string()}),
    };
    let temporary = directory.join(format!("pending-{name}"));
    fs::write(&temporary, serde_json::to_vec(&result).unwrap()).unwrap();
    fs::rename(temporary, directory.join(format!("result-{name}"))).unwrap();
}

fn assert_rejected_before_compilation(directory: &Path, name: &str) {
    let result = Request::start(directory, name, true).finish();
    assert!(
        result["error"]
            .as_str()
            .is_some_and(|s| s.contains("another")),
        "overlapping request was accepted: {result}"
    );
    assert!(
        !directory.join(format!("compiler-{name}")).exists(),
        "rejected request reached compiler"
    );
}

#[test]
fn creation_request_rejects_overlap_before_compilation_and_through_readback() {
    if let Some(directory) = std::env::var_os(CHILD_DIRECTORY) {
        child_request(Path::new(&directory));
        return;
    }
    let directory = tempfile::tempdir().unwrap();
    let compiler = directory.path().join("clang");
    fs::write(&compiler, include_str!("space_create_request_fixture.sh")).unwrap();
    fs::set_permissions(compiler, fs::Permissions::from_mode(0o700)).unwrap();
    let mut first = Request::start(directory.path(), "first", false);
    first.wait_for("compiler");
    assert_rejected_before_compilation(directory.path(), "during-compilation");
    first.release("compile");
    first.wait_for("helper");
    assert_rejected_before_compilation(directory.path(), "during-readback");
    first.release("readback");
    assert_eq!(first.finish()["ok"]["status"], "sent_unverified");
    for failure in ["compile-failure", "helper-failure"] {
        let result = Request::start(directory.path(), failure, true).finish();
        assert!(result["error"].as_str().is_some());
        assert!(
            directory
                .path()
                .join(format!("compiler-{failure}"))
                .exists()
        );
        let result = Request::start(directory.path(), &format!("after-{failure}"), true).finish();
        assert_eq!(result["ok"]["status"], "sent_unverified");
    }
}
