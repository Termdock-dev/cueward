use crate::MacosError;
use crate::safari_guard::with_safari_session;

use super::super::super::core::execute_js_for_profile;
use super::super::super::script::escape_js_string;
use super::{chatgpt_image_extract_js, poll_chatgpt_images};

pub fn chatgpt_save_images(
    conversation_url: &str,
    output_dir: &str,
    profile_filter: Option<&str>,
) -> Result<Vec<String>, MacosError> {
    with_safari_session(|| {
        let nav_js = format!(
            r#"(function() {{ window.location.href = "{url}"; return "true"; }})()"#,
            url = escape_js_string(conversation_url),
        );
        let _ =
            execute_js_for_profile(&nav_js, profile_filter, "safari_chatgpt_save_img_navigate")?;
        let result = poll_chatgpt_images(30, 8, profile_filter)?;
        if result.images.is_empty() {
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
        let filename_prefix = result
            .conversation_url
            .as_deref()
            .and_then(|url| url.rsplit('/').next())
            .filter(|segment| !segment.is_empty())
            .unwrap_or("chatgpt");

        for (i, image) in result.images.iter().enumerate() {
            let b64 = execute_js_for_profile(
                &chatgpt_image_extract_js(&image.url),
                profile_filter,
                "safari_chatgpt_img_extract",
            )?;
            let b64 = b64.trim();
            if b64.is_empty() {
                continue;
            }

            let bytes = engine.decode(b64).map_err(|e| {
                MacosError::Other(format!("base64 decode failed for image {i}: {e}"))
            })?;

            let filename = format!("{filename_prefix}_image_{i}.png");
            let filepath = out_path.join(&filename);
            std::fs::write(&filepath, &bytes).map_err(|e| {
                MacosError::Other(format!("failed to write {}: {e}", filepath.display()))
            })?;
            saved.push(filepath.to_string_lossy().into_owned());
        }

        Ok(saved)
    })
}
