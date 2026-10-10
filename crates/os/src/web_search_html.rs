use lol_html::{RewriteStrSettings, element, rewrite_str, text};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};

pub(super) fn strip_html(source: &str) -> String {
    let collected_text = Arc::new(Mutex::new(String::new()));
    let element_text = collected_text.clone();
    let content_text = collected_text.clone();
    let excluded_depth = Arc::new(AtomicUsize::new(0));
    let element_excluded_depth = excluded_depth.clone();
    let content_excluded_depth = excluded_depth.clone();
    let rewrite_result = rewrite_str(
        source,
        RewriteStrSettings::new()
            .append_element_content_handler(element!("*", move |element| {
                let tag_name = element.tag_name();

                if matches!(tag_name.as_str(), "script" | "style") {
                    element_excluded_depth.fetch_add(1, Ordering::AcqRel);

                    let end_tag_excluded_depth = element_excluded_depth.clone();
                    let end_tag_handler: lol_html::EndTagHandler<'static> = Box::new(move |_| {
                        end_tag_excluded_depth.fetch_sub(1, Ordering::AcqRel);
                        Ok(())
                    });

                    element.on_end_tag(end_tag_handler)?;
                } else if let Ok(mut output) = element_text.lock() {
                    output.push(' ');
                }

                Ok(())
            }))
            .append_element_content_handler(text!("*", move |text_chunk| {
                if content_excluded_depth.load(Ordering::Acquire) == 0
                    && let Ok(mut output) = content_text.lock()
                {
                    output.push_str(text_chunk.as_str());
                }

                Ok(())
            })),
    );

    if rewrite_result.is_err() {
        return String::new();
    }

    collected_text.lock().map_or_else(
        |poisoned| normalize_whitespace(&poisoned.into_inner()),
        |output| normalize_whitespace(&output),
    )
}

fn normalize_whitespace(source: &str) -> String {
    html_escape::decode_html_entities(source)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}
