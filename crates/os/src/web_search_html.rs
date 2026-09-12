pub(super) fn strip_html(source: &str) -> String {
    let mut plain_text = String::new();
    let mut source_position = 0;
    let source_bytes = source.as_bytes();

    while source_position < source_bytes.len() {
        if source[source_position..].starts_with("<!--") {
            source_position = source[source_position + 4..]
                .find("-->")
                .map_or(source_bytes.len(), |offset| {
                    source_position + 4 + offset + 3
                });
            plain_text.push(' ');
        } else if source_bytes[source_position] == b'<' {
            let tag_end = source[source_position + 1..]
                .find('>')
                .map(|offset| source_position + offset + 2);
            let Some(tag_end) = tag_end else {
                break;
            };
            let tag = source[source_position..tag_end].to_ascii_lowercase();

            if tag.starts_with("<script") || tag.starts_with("<style") {
                let tag_name = if tag.starts_with("<script") {
                    "</script>"
                } else {
                    "</style>"
                };

                source_position = source[tag_end..]
                    .to_ascii_lowercase()
                    .find(tag_name)
                    .map_or(source_bytes.len(), |offset| {
                        tag_end + offset + tag_name.len()
                    });
            } else {
                source_position = tag_end;
                plain_text.push(' ');
            }
        } else {
            let Some(character) = source[source_position..].chars().next() else {
                break;
            };

            plain_text.push(character);
            source_position += character.len_utf8();
        }
    }

    decode_html_entities(&plain_text)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn decode_html_entities(source: &str) -> String {
    source
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
}
