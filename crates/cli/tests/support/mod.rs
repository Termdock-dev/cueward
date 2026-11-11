//! Shared test-only HOME and osascript boundary; never uses real application data.
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::{Command, Output};

use serde_json::{Value, json};

pub struct Home {
    directory: tempfile::TempDir,
}

impl Home {
    pub fn new() -> Self {
        let directory = tempfile::tempdir().unwrap();
        let bin = directory.path().join("bin");
        fs::create_dir(&bin).unwrap();
        let script = bin.join("osascript");
        fs::write(
            &script,
            "#!/bin/sh\nprintf '%s' \"$CUEWARD_TEST_SCRIPT_OUTPUT\"\n",
        )
        .unwrap();
        fs::set_permissions(&script, fs::Permissions::from_mode(0o700)).unwrap();
        Self { directory }
    }

    pub fn path(&self, relative: &str) -> PathBuf {
        self.directory.path().join(relative)
    }

    pub fn command(&self) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_cueward"));
        command
            .env("HOME", self.directory.path())
            .env("PATH", self.path("bin"))
            .env("CUEWARD_TEST_SCRIPT_OUTPUT", "");
        command
    }

    pub fn run(&self, args: &[&str]) -> Output {
        self.command().args(args).output().unwrap()
    }

    pub fn inbox(&self, cues: Value) {
        fs::create_dir_all(self.path(".cueward/inbox")).unwrap();
        fs::write(self.path(".cueward/inbox/batch.json"), cues.to_string()).unwrap();
    }
}

pub fn cue(content: &str) -> Value {
    json!({"source":"notes", "timestamp":"2026-11-11T12:00:00Z",
        "title":"synthetic fixture", "content":content})
}

pub fn search_results(output: &Output) -> Vec<Value> {
    let text = std::str::from_utf8(&output.stdout).unwrap();
    if let Some(body) = text.strip_prefix("<external source=\"cueward/search\">\n") {
        serde_json::from_str(body.strip_suffix("\n</external>\n").unwrap()).unwrap()
    } else {
        text.lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect()
    }
}
