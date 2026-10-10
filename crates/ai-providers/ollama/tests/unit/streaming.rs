use crate::streaming::NativeChatFrame;

#[test]
fn native_chat_frame_rejects_malformed_consumed_fields() {
    for frame in [
        r#"{"message":null,"done":true}"#,
        r#"{"message":{"content":null},"done":true}"#,
        r#"{"message":{"thinking":4},"done":true}"#,
        r#"{"message":{"tool_calls":{}},"done":true}"#,
        r#"{"message":{"tool_calls":[{"function":{"name":"run","arguments":3}}]},"done":true}"#,
        r#"{"done":"true"}"#,
        r#"{"done":true,"done_reason":4}"#,
        r#"{"done":true,"eval_count":"5"}"#,
        r#"{"error":null}"#,
    ] {
        assert!(
            serde_json::from_str::<NativeChatFrame>(frame).is_err(),
            "{frame}"
        );
    }
}

#[test]
fn native_chat_frame_accepts_complete_and_error_frames() {
    let complete_frame = serde_json::from_str::<NativeChatFrame>(
            r#"{"message":{"content":"done","thinking":"reasoning"},"done":true,"done_reason":"stop","prompt_eval_count":7,"eval_count":2}"#,
        )
        .unwrap();
    assert!(complete_frame.done);
    assert_eq!(complete_frame.message.content, "done");
    assert_eq!(complete_frame.prompt_eval_count, Some(7));

    let error_frame =
        serde_json::from_str::<NativeChatFrame>(r#"{"error":{"message":"daemon error"}}"#).unwrap();
    assert!(error_frame.error.is_some());
}
