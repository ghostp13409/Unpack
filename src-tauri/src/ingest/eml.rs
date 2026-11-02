use super::html;
use super::IngestError;
use mailparse::{MailHeaderMap, ParsedMail};

/// Renders an .eml as headers (From/To/Cc/Date/Subject), a blank line, and
/// the decoded body. Prefers text/plain; falls back to stripped text/html.
pub fn extract(bytes: &[u8]) -> Result<String, IngestError> {
    let mail = mailparse::parse_mail(bytes)
        .map_err(|e| IngestError::Parse(format!("could not parse email: {e}")))?;
    let mut out = String::new();
    for name in ["From", "To", "Cc", "Date", "Subject"] {
        if let Some(v) = mail.headers.get_first_value(name) {
            out.push_str(name);
            out.push_str(": ");
            out.push_str(v.trim());
            out.push('\n');
        }
    }
    out.push('\n');
    let body = find_part(&mail, "text/plain")
        .map(|t| t.trim().to_string())
        .or_else(|| find_part(&mail, "text/html").map(|h| html::strip(&h)))
        .unwrap_or_default();
    out.push_str(&body);
    Ok(out)
}

fn find_part(part: &ParsedMail, mime: &str) -> Option<String> {
    if part.ctype.mimetype.eq_ignore_ascii_case(mime) {
        return part.get_body().ok();
    }
    for sub in &part.subparts {
        if let Some(found) = find_part(sub, mime) {
            return Some(found);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::extract;

    const PLAIN_AND_HTML: &str = "From: Alex Morgan <alex@example.com>\r\nTo: Sam Rivera <sam@example.com>\r\nCc: Nate <nate@example.com>\r\nDate: Mon, 5 Oct 2026 19:27:52 +0000\r\nSubject: Re: Pricing Portal\r\nMIME-Version: 1.0\r\nContent-Type: multipart/alternative; boundary=\"b1\"\r\n\r\n--b1\r\nContent-Type: text/plain; charset=utf-8\r\n\r\nPlease send the pricing by Friday.\r\n--b1\r\nContent-Type: text/html; charset=utf-8\r\n\r\n<p>Please send the <b>pricing</b> by Friday.</p>\r\n--b1--\r\n";

    const HTML_ONLY: &str = "From: a@example.com\r\nTo: b@example.com\r\nSubject: Hello\r\nContent-Type: text/html; charset=utf-8\r\nContent-Transfer-Encoding: quoted-printable\r\n\r\n<div>Deadline is <b>Nov=C2=A05</b>.</div><div>Thanks</div>\r\n";

    #[test]
    fn plain_text_part_preferred() {
        let out = extract(PLAIN_AND_HTML.as_bytes()).unwrap();
        assert!(out.contains("Please send the pricing by Friday."));
        assert!(!out.contains("<b>"));
    }

    #[test]
    fn html_only_mail_is_stripped() {
        let out = extract(HTML_ONLY.as_bytes()).unwrap();
        assert!(out.contains("Deadline is Nov\u{a0}5."), "{out}");
        assert!(out.contains("Thanks"));
        assert!(!out.contains("<div>"));
    }

    #[test]
    fn headers_listed_first() {
        let out = extract(PLAIN_AND_HTML.as_bytes()).unwrap();
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines[0], "From: Alex Morgan <alex@example.com>");
        assert_eq!(lines[1], "To: Sam Rivera <sam@example.com>");
        assert_eq!(lines[2], "Cc: Nate <nate@example.com>");
        assert_eq!(lines[3], "Date: Mon, 5 Oct 2026 19:27:52 +0000");
        assert_eq!(lines[4], "Subject: Re: Pricing Portal");
        assert_eq!(lines[5], "");
    }
}
