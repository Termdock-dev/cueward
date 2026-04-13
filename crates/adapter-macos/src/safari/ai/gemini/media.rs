use std::thread;
use std::time::Duration;

use serde_json::Value;

use crate::MacosError;
use crate::safari_guard::with_safari_session;

use super::super::super::core::execute_js_for_profile;
use super::super::super::script::escape_js_string;
use super::super::SafariAiResponseResult;

pub fn gemini_save_images(
    conversation_url: &str,
    output_dir: &str,
    profile_filter: Option<&str>,
) -> Result<Vec<String>, MacosError> {
    with_safari_session(|| {
        let nav_js = format!(
            r#"(function() {{ window.location.href = "{url}"; return "true"; }})()"#,
            url = escape_js_string(conversation_url),
        );
        let _ = execute_js_for_profile(&nav_js, profile_filter, "safari_gemini_save_img_navigate")?;
        thread::sleep(Duration::from_millis(5000));

        let count_js = r#"(() => {
        const imgs = document.querySelectorAll('img[alt*="AI"], img[alt*="生成"]');
        return String(imgs.length);
    })()"#;
        let count: usize =
            execute_js_for_profile(count_js, profile_filter, "safari_gemini_img_count")?
                .trim()
                .parse()
                .unwrap_or(0);

        if count == 0 {
            return Ok(Vec::new());
        }

        let out_path = std::path::Path::new(output_dir);
        if !out_path.exists() {
            std::fs::create_dir_all(out_path)
                .map_err(|e| MacosError::Other(format!("failed to create output dir: {e}")))?;
        }

        use base64::Engine;
        let engine = base64::engine::general_purpose::STANDARD;
        let mut saved = Vec::new();

        for i in 0..count {
            let extract_js = format!(
                r#"(() => {{
                const imgs = document.querySelectorAll('img[alt*="AI"], img[alt*="生成"]');
                const img = imgs[{i}];
                if (!img) return "";
                const canvas = document.createElement("canvas");
                canvas.width = img.naturalWidth || img.width;
                canvas.height = img.naturalHeight || img.height;
                canvas.getContext("2d").drawImage(img, 0, 0);
                const dataUrl = canvas.toDataURL("image/png");
                const idx = dataUrl.indexOf(",");
                return idx >= 0 ? dataUrl.substring(idx + 1) : "";
            }})()"#,
            );
            let b64 =
                execute_js_for_profile(&extract_js, profile_filter, "safari_gemini_img_extract")?;
            let b64 = b64.trim();
            if b64.is_empty() {
                continue;
            }

            let bytes = engine.decode(b64).map_err(|e| {
                MacosError::Other(format!("base64 decode failed for image {i}: {e}"))
            })?;

            let filename = format!("gemini_image_{i}.png");
            let filepath = out_path.join(&filename);
            std::fs::write(&filepath, &bytes).map_err(|e| {
                MacosError::Other(format!("failed to write {}: {e}", filepath.display()))
            })?;
            saved.push(filepath.to_string_lossy().into_owned());
        }

        Ok(saved)
    })
}

pub fn gemini_save_media(
    conversation_url: &str,
    profile_filter: Option<&str>,
) -> Result<SafariAiResponseResult, MacosError> {
    with_safari_session(|| {
        let nav_js = format!(
            r#"(function() {{ window.location.href = "{url}"; return "true"; }})()"#,
            url = escape_js_string(conversation_url),
        );
        let _ = execute_js_for_profile(&nav_js, profile_filter, "safari_gemini_media_navigate")?;
        thread::sleep(Duration::from_millis(5000));

        let js = r#"(() => {
        const video = document.querySelector("video");
        if (!video) return JSON.stringify({ status: "error", response: "no media found" });
        const src = video.src || video.currentSrc || "";
        if (!src) return JSON.stringify({ status: "error", response: "no media source" });
        const a = document.createElement("a");
        a.href = src;
        a.download = "gemini_media.mp4";
        document.body.appendChild(a);
        a.click();
        document.body.removeChild(a);
        return JSON.stringify({ status: "downloading", response: src });
    })()"#;
        let raw = execute_js_for_profile(js, profile_filter, "safari_gemini_media_download")?;
        let value: Value = serde_json::from_str(&raw)
            .map_err(|e| MacosError::Other(format!("failed to parse media result: {e}")))?;
        let status = value
            .get("status")
            .and_then(|v| v.as_str())
            .unwrap_or("error");
        let response = value.get("response").and_then(|v| v.as_str()).unwrap_or("");
        Ok(SafariAiResponseResult {
            provider: "gemini".to_string(),
            status: status.to_string(),
            response: response.to_string(),
            conversation_url: None,
        })
    })
}
