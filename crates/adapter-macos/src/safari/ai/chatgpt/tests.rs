use super::{
    SafariAiImage, SafariAiImageResult, build_chatgpt_click_send_js,
    build_chatgpt_fill_input_js, build_chatgpt_go_home_js, chatgpt_image_extract_js,
    chatgpt_image_list_js, chatgpt_image_result_is_ready, chatgpt_list_conversations_js,
    chatgpt_response_extract_js, chatgpt_should_return_empty_complete_result,
    parse_chatgpt_image_payload,
};
use std::time::Duration;

#[test]
fn chatgpt_go_home_script_targets_new_chat() {
    let script = build_chatgpt_go_home_js();

    assert!(script.contains("chatgpt.com"));
    assert!(script.contains("window.location.href"));
}

#[test]
fn chatgpt_fill_input_script_targets_prompt_textarea() {
    let script = build_chatgpt_fill_input_js("hello from chatgpt");

    assert!(script.contains("#prompt-textarea"));
    assert!(script.contains("hello from chatgpt"));
    assert!(script.contains("execCommand"));
}

#[test]
fn chatgpt_click_send_script_checks_accessible_labels() {
    let script = build_chatgpt_click_send_js();

    assert!(script.contains("Send prompt"));
    assert!(script.contains("Send message"));
    assert!(script.contains("button.disabled"));
}

#[test]
fn chatgpt_response_extract_script_returns_response_and_conversation_url() {
    let script = chatgpt_response_extract_js();

    assert!(script.contains("article"));
    assert!(script.contains("conversation_url"));
    assert!(script.contains("window.location.href"));
    assert!(script.contains("complete"));
}

#[test]
fn chatgpt_list_script_targets_history_nav_links() {
    let script = chatgpt_list_conversations_js();

    assert!(script.contains("Chat history"));
    assert!(script.contains("nav[role=\"navigation\"]"));
    assert!(script.contains("a[href^=\"/c/\"]"));
    assert!(script.contains("https://chatgpt.com"));
    assert!(script.contains("Recent"));
}

#[test]
fn chatgpt_image_list_script_reads_generated_images() {
    let script = chatgpt_image_list_js();

    assert!(script.contains("chatgpt\\.com\\/backend-api\\/estuary\\/content"));
    assert!(script.contains("conversation_url"));
    assert!(script.contains("img.width < 256"));
}

#[test]
fn chatgpt_image_list_script_includes_loaded_signal() {
    let script = chatgpt_image_list_js();

    assert!(script.contains("img.complete"));
    assert!(script.contains("loaded"));
}

#[test]
fn chatgpt_image_extract_script_targets_url() {
    let script = chatgpt_image_extract_js("https://example.com/img.png");

    assert!(script.contains("https://example.com/img.png"));
    assert!(script.contains("drawImage"));
    assert!(!script.contains("new Set"));
}

#[test]
fn parse_chatgpt_image_payload_reads_images() {
    let payload = r#"{"status":"complete","conversation_url":"https://chatgpt.com/c/test","images":[{"url":"https://chatgpt.com/backend-api/estuary/content?id=file_123"}]}"#;
    let result = parse_chatgpt_image_payload(payload).expect("parse payload");

    assert_eq!(result.status, "complete");
    assert_eq!(
        result.conversation_url.as_deref(),
        Some("https://chatgpt.com/c/test")
    );
    assert_eq!(result.images.len(), 1);
    assert_eq!(
        result.images[0].url,
        "https://chatgpt.com/backend-api/estuary/content?id=file_123"
    );
}

#[test]
fn parse_chatgpt_image_payload_reads_loaded_state() {
    let payload = r#"{"status":"complete","conversation_url":"https://chatgpt.com/c/test","images":[{"url":"https://chatgpt.com/backend-api/estuary/content?id=file_123","loaded":true},{"url":"https://chatgpt.com/backend-api/estuary/content?id=file_456","loaded":false}]}"#;
    let result = parse_chatgpt_image_payload(payload).expect("parse payload");

    assert_eq!(result.images.len(), 2);
    assert!(result.images[0].loaded);
    assert!(!result.images[1].loaded);
}

#[test]
fn chatgpt_image_result_requires_all_images_to_be_ready() {
    let unloaded = SafariAiImageResult {
        provider: "chatgpt".to_string(),
        mode: "image".to_string(),
        status: "complete".to_string(),
        conversation_url: Some("https://chatgpt.com/c/test".to_string()),
        images: vec![SafariAiImage {
            url: "https://chatgpt.com/backend-api/estuary/content?id=file_123".to_string(),
            loaded: false,
        }],
    };
    let mixed = SafariAiImageResult {
        images: vec![
            SafariAiImage {
                url: "https://chatgpt.com/backend-api/estuary/content?id=file_123".to_string(),
                loaded: true,
            },
            SafariAiImage {
                url: "https://chatgpt.com/backend-api/estuary/content?id=file_456".to_string(),
                loaded: false,
            },
        ],
        ..unloaded.clone()
    };
    let loaded = SafariAiImageResult {
        images: vec![
            SafariAiImage {
                url: "https://chatgpt.com/backend-api/estuary/content?id=file_123".to_string(),
                loaded: true,
            },
            SafariAiImage {
                url: "https://chatgpt.com/backend-api/estuary/content?id=file_456".to_string(),
                loaded: true,
            },
        ],
        ..unloaded.clone()
    };

    assert!(!chatgpt_image_result_is_ready(&unloaded));
    assert!(!chatgpt_image_result_is_ready(&mixed));
    assert!(chatgpt_image_result_is_ready(&loaded));
}

#[test]
fn chatgpt_empty_complete_grace_only_applies_without_images() {
    let complete_without_images = SafariAiImageResult {
        provider: "chatgpt".to_string(),
        mode: "image".to_string(),
        status: "complete".to_string(),
        conversation_url: Some("https://chatgpt.com/c/test".to_string()),
        images: Vec::new(),
    };
    let complete_with_unloaded_images = SafariAiImageResult {
        images: vec![SafariAiImage {
            url: "https://chatgpt.com/backend-api/estuary/content?id=file_123".to_string(),
            loaded: false,
        }],
        ..complete_without_images.clone()
    };

    assert!(chatgpt_should_return_empty_complete_result(
        &complete_without_images,
        Duration::from_secs(8),
        3
    ));
    assert!(!chatgpt_should_return_empty_complete_result(
        &complete_with_unloaded_images,
        Duration::from_secs(8),
        3
    ));
}
