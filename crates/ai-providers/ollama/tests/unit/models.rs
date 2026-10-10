use crate::metadata::NativeModelListEntry;
use crate::models::native_model_supports_chat;
use serde_json::json;

#[test]
fn model_listing_excludes_known_non_chat_models() {
    let completion_model = serde_json::from_value::<NativeModelListEntry>(json!({
        "name": "chat-model",
        "capabilities": ["completion", "tools"]
    }))
    .unwrap();
    let embedding_model = serde_json::from_value::<NativeModelListEntry>(json!({
        "name": "embedding-model",
        "capabilities": ["embedding"]
    }))
    .unwrap();
    let model_without_capabilities =
        serde_json::from_value::<NativeModelListEntry>(json!({"name": "legacy-model"})).unwrap();

    assert!(native_model_supports_chat(&completion_model));
    assert!(!native_model_supports_chat(&embedding_model));
    assert!(native_model_supports_chat(&model_without_capabilities));
}
