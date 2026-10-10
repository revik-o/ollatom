use crate::command::command_windows::environment::quote_windows_argument;

#[test]
fn quotes_windows_arguments_with_spaces_and_quotes() {
    assert_eq!(quote_windows_argument("plain"), "\"plain\"");
    assert_eq!(quote_windows_argument("two words"), "\"two words\"");
    assert_eq!(
        quote_windows_argument("quote\"inside"),
        "\"quote\\\"inside\""
    );
    assert_eq!(
        quote_windows_argument("ending slash\\"),
        "\"ending slash\\\\\""
    );
}
