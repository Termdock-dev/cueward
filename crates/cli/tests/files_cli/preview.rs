use super::*;
fn digest(bytes: &[u8]) -> String {
    let mut child = Command::new("/usr/bin/shasum")
        .args(["-a", "256"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(bytes).unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    String::from_utf8(output.stdout)
        .unwrap()
        .split_whitespace()
        .next()
        .unwrap()
        .to_owned()
}

fn preview(root: &Path, kind: &str, args: &[&str], success: bool) -> Value {
    let output = command()
        .args(["files", "preview", kind, "--root"])
        .arg(root)
        .args(["--timeout-ms", "30000"])
        .args(args)
        .output()
        .unwrap();
    assert_eq!(
        output.status.success(),
        success,
        "stdout: {}, stderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    envelope(&output.stdout, "files")
}
fn fixtures(directory: &Path) {
    let swift = directory.join("fixture.swift");
    fs::write(
        &swift,
        include_str!("../../../adapter-macos/src/files/preview/fixture.swift"),
    )
    .unwrap();
    let output = Command::new("swift")
        .arg(swift)
        .arg(directory)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
fn result(value: &Value) -> &Value {
    &value["Ok"]["result"]
}
fn verify_artifact(page: &Value, maximum: u64) {
    let artifact = &page["preview"]["value"];
    let path = Path::new(artifact["path"].as_str().unwrap());
    let bytes = fs::read(path).unwrap();
    assert!(bytes.starts_with(b"\x89PNG\r\n\x1a\n"));
    assert_eq!(artifact["sha256"], digest(&bytes));
    assert_eq!(artifact["bytes"], bytes.len());
    assert!(
        artifact["width"].as_u64().unwrap() <= maximum
            && artifact["height"].as_u64().unwrap() <= maximum
    );
    // Delete only the per-operation cache that produced this exact verified artifact.
    let cache = path.parent().unwrap();
    assert!(
        cache
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .starts_with("preview-")
    );
    fs::remove_dir_all(cache).unwrap();
}

#[test]
fn preview_native_pdf_ranges_ocr_images_errors_and_source_preservation() {
    let dir = tempfile::tempdir().unwrap();
    fixtures(dir.path());
    let original = fs::read(dir.path().join("mixed.pdf")).unwrap();
    verify_pdf_native(dir.path());
    verify_pdf_ocr(dir.path());
    verify_rotated(dir.path());
    verify_annotation(dir.path());
    verify_legacy_ocr(dir.path());
    verify_blank_page(dir.path());
    verify_image(dir.path());
    verify_failures(dir.path());
    let v = preview(
        dir.path(),
        "pdf",
        &[
            "--path",
            "mixed.pdf",
            "--max-text-bytes",
            "5",
            "--no-render",
        ],
        true,
    );
    assert_eq!(result(&v)["text_truncated"], true);
    assert!(
        result(&v)["pages"][0]["text"]["value"]
            .as_str()
            .unwrap()
            .len()
            <= 5
    );
    assert_eq!(fs::read(dir.path().join("mixed.pdf")).unwrap(), original);
}
fn verify_image(root: &Path) {
    let original = fs::read(root.join("scan.png")).unwrap();
    let v = preview(
        root,
        "image",
        &["--path", "scan.png", "--ocr", "--max-dimension", "800"],
        true,
    );
    let v = result(&v);
    assert_eq!(v["image"]["width"], 800);
    assert_eq!(v["image"]["height"], 300);
    assert_eq!(v["image"]["frame_count"], 1);
    assert_eq!(v["pages"][0]["text_source"], "vision_ocr");
    assert!(
        v["pages"][0]["text"]["value"]
            .as_str()
            .unwrap()
            .contains("SCAN EXAMPLE 123")
    );
    verify_artifact(&v["pages"][0], 800);
    assert_eq!(fs::read(root.join("scan.png")).unwrap(), original);
    let v = preview(root, "image", &["--path", "scan.png", "--no-render"], true);
    assert_eq!(result(&v)["pages"][0]["text_source"], "not_requested");
    assert_eq!(result(&v)["pages"][0]["text"]["status"], "not_applicable");
}
fn verify_failures(root: &Path) {
    for (path, code) in [
        ("locked.pdf", "encrypted"),
        ("damaged.pdf", "corrupt_data"),
        ("unknown.bin", "unsupported_type"),
    ] {
        assert_eq!(
            preview(root, "pdf", &["--path", path], false)["Err"]["code"],
            code
        );
    }
    assert_eq!(
        preview(
            root,
            "pdf",
            &["--path", "mixed.pdf", "--start-page", "4"],
            false
        )["Err"]["code"],
        "invalid_options"
    );
    assert_eq!(
        preview(
            root,
            "pdf",
            &["--path", "mixed.pdf", "--expected-version", "stale"],
            false
        )["Err"]["code"],
        "changed"
    );
    assert_eq!(
        preview(
            root,
            "pdf",
            &["--path", "mixed.pdf", "--max-input-bytes", "1"],
            false
        )["Err"]["code"],
        "scan_limit"
    );
    verify_partial_preview(root);
}

#[test]
fn preview_scope_link_and_raw_worker_rejections_preserve_data() {
    use std::os::unix::fs::symlink;
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("a"), "unchanged").unwrap();
    symlink("/outside/missing", dir.path().join("link")).unwrap();
    for (path, code) in [
        ("../outside", "outside_root"),
        ("link", "unsupported_type"),
        (".", "unsupported_type"),
    ] {
        assert_eq!(
            preview(dir.path(), "image", &["--path", path], false)["Err"]["code"],
            code
        );
    }
    let mut child = command()
        .arg("files-preview-worker")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(b"{").unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(!output.status.success());
    assert_eq!(
        envelope(&output.stdout, "files/worker")["Err"]["code"],
        "invalid_options"
    );
    assert_eq!(fs::read(dir.path().join("a")).unwrap(), b"unchanged");
}
#[test]
#[ignore = "requires a running macOS Quick Look thumbnail service; uses only owned fixtures"]
fn preview_quick_look_is_visual_only_and_rejects_icons() {
    let dir = tempfile::tempdir().unwrap();
    fixtures(dir.path());
    let v = preview(
        dir.path(),
        "thumbnail",
        &["--path", "mixed.pdf", "--max-dimension", "256"],
        true,
    );
    let v = result(&v);
    assert_eq!(v["pages"][0]["text_source"], "not_requested");
    assert_eq!(v["pages"][0]["text"]["status"], "not_applicable");
    verify_artifact(&v["pages"][0], 256);
}

fn verify_pdf_native(root: &Path) {
    let v = preview(
        root,
        "pdf",
        &[
            "--path",
            "mixed.pdf",
            "--page-count",
            "1",
            "--max-dimension",
            "256",
        ],
        true,
    );
    let v = result(&v);
    assert_eq!(v["total_pages"], 3);
    assert_eq!(v["selection_truncated"], true);
    assert_eq!(v["pages"][0]["text_source"], "native_pdf");
    assert!(
        v["pages"][0]["text"]["value"]
            .as_str()
            .unwrap()
            .contains("NATIVE EXAMPLE 456")
    );
    assert!(v["pages"][0]["confidence"].is_null());
    verify_artifact(&v["pages"][0], 256);
}
fn verify_pdf_ocr(root: &Path) {
    let v = preview(
        root,
        "pdf",
        &[
            "--path",
            "mixed.pdf",
            "--start-page",
            "2",
            "--page-count",
            "1",
            "--ocr",
            "--no-render",
        ],
        true,
    );
    let page = &result(&v)["pages"][0];
    assert_eq!(page["page"], 2);
    assert_eq!(page["text_source"], "vision_ocr");
    assert!(
        page["text"]["value"]
            .as_str()
            .unwrap()
            .contains("SCAN EXAMPLE 123")
    );
    assert!((0.0..=1.0).contains(&page["confidence"].as_f64().unwrap()));
    assert_eq!(page["preview"]["status"], "not_applicable");
}

fn verify_partial_preview(root: &Path) {
    let v = preview(
        root,
        "pdf",
        &[
            "--path",
            "mixed.pdf",
            "--page-count",
            "1",
            "--max-preview-bytes",
            "1",
        ],
        true,
    );
    let page = &result(&v)["pages"][0];
    assert_eq!(page["text"]["status"], "available");
    assert_eq!(page["preview"]["status"], "error");
    assert!(
        page["preview"]["error"]["message"]
            .as_str()
            .unwrap()
            .contains("scan_limit")
    );
}

fn verify_rotated(root: &Path) {
    let v = preview(
        root,
        "pdf",
        &[
            "--path",
            "rotated.pdf",
            "--page-count",
            "1",
            "--max-dimension",
            "256",
        ],
        true,
    );
    let page = &result(&v)["pages"][0];
    assert!(
        page["preview"]["value"]["height"].as_u64().unwrap()
            > page["preview"]["value"]["width"].as_u64().unwrap()
    );
    verify_artifact(page, 256);
}
fn verify_legacy_ocr(root: &Path) {
    let output = command()
        .arg("ocr")
        .arg(root.join("native-only.pdf"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    let prefix = "<external source=\"cueward/ocr\">\n";
    let value: Value = serde_json::from_str(
        stdout
            .strip_prefix(prefix)
            .unwrap()
            .strip_suffix("\n</external>\n")
            .unwrap(),
    )
    .unwrap();
    assert_eq!(value[0]["source"], "ocr");
    assert!(
        value[0]["content"]
            .as_str()
            .unwrap()
            .contains("NATIVE EXAMPLE 456")
    );
}
fn verify_blank_page(root: &Path) {
    let v = preview(
        root,
        "pdf",
        &[
            "--path",
            "mixed.pdf",
            "--start-page",
            "3",
            "--page-count",
            "1",
            "--no-render",
        ],
        true,
    );
    let page = &result(&v)["pages"][0];
    assert_eq!(page["text_source"], "native_pdf");
    if page["text"]["status"] == "available" {
        assert_eq!(page["text"]["value"], "");
    } else {
        assert_eq!(page["text"]["status"], "unavailable");
    }
    assert_eq!(page["preview"]["status"], "not_applicable");
}

fn verify_annotation(root: &Path) {
    let v = preview(
        root,
        "pdf",
        &[
            "--path",
            "annotated.pdf",
            "--page-count",
            "1",
            "--max-dimension",
            "256",
        ],
        true,
    );
    let page = &result(&v)["pages"][0];
    let script = root.join("pixels.swift");
    fs::write(
        &script,
        include_str!("../../../adapter-macos/src/files/preview/pixel_check.swift"),
    )
    .unwrap();
    let output = Command::new("swift")
        .arg(script)
        .arg(page["preview"]["value"]["path"].as_str().unwrap())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    verify_artifact(page, 256);
}
