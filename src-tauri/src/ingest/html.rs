/// Turn HTML into readable plain text: drops scripts/styles, turns block
/// boundaries into newlines, strips the remaining tags and decodes the
/// common entities.
pub fn strip(html: &str) -> String {
    let lower = html.to_ascii_lowercase();
    let mut out = String::with_capacity(html.len());
    let mut i = 0;
    let bytes = html.as_bytes();
    while i < bytes.len() {
        if bytes[i] == b'<' {
            let rest = &lower[i..];
            if rest.starts_with("<script") || rest.starts_with("<style") {
                let close = if rest.starts_with("<script") { "</script>" } else { "</style>" };
                match rest.find(close) {
                    Some(end) => {
                        i += end + close.len();
                        continue;
                    }
                    None => break,
                }
            }
            let end = match html[i..].find('>') {
                Some(e) => i + e + 1,
                None => break,
            };
            let tag = &lower[i + 1..end - 1];
            let closing = tag.starts_with('/');
            let name: String = tag
                .trim_start_matches('/')
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric())
                .collect();
            let block = matches!(
                name.as_str(),
                "p" | "div"
                    | "tr"
                    | "li"
                    | "h1"
                    | "h2"
                    | "h3"
                    | "h4"
                    | "h5"
                    | "h6"
                    | "blockquote"
                    | "pre"
            );
            if name == "br" || (closing && block) {
                out.push('\n');
            } else if closing && (name == "td" || name == "th") {
                out.push('\t');
            }
            i = end;
        } else {
            let ch = html[i..].chars().next().unwrap();
            out.push(ch);
            i += ch.len_utf8();
        }
    }
    let decoded = decode_entities(&out);
    collapse_blank_lines(&decoded)
}

pub fn decode_entities(s: &str) -> String {
    s.replace("&nbsp;", "\u{a0}")
        .replace("&#160;", "\u{a0}")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

/// Trims each line's trailing whitespace and collapses 3+ newlines to 2.
pub fn collapse_blank_lines(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut blank_run = 0;
    for line in s.lines() {
        let line = line.trim_end();
        if line.trim().is_empty() {
            blank_run += 1;
            if blank_run >= 2 {
                continue;
            }
            out.push('\n');
        } else {
            blank_run = 0;
            out.push_str(line);
            out.push('\n');
        }
    }
    out.trim_matches('\n').to_string()
}

#[cfg(test)]
mod tests {
    use super::strip;

    #[test]
    fn strip_turns_blocks_into_newlines_and_decodes_entities() {
        let html = "<html><head><style>p{color:red}</style><script>alert(1)</script></head><body><p>Hi &amp; <b>bye</b></p><div>Line&nbsp;2</div><ul><li>one</li><li>two</li></ul><p>a<br>b</p></body></html>";
        let out = strip(html);
        assert!(!out.contains("alert"));
        assert!(!out.contains("color"));
        assert_eq!(out, "Hi & bye\nLine\u{a0}2\none\ntwo\na\nb");
    }

    #[test]
    fn collapses_runs_of_blank_lines() {
        let out = strip("<p>a</p><p></p><p></p><p></p><p>b</p>");
        assert_eq!(out, "a\n\nb");
    }
}
