/// Compacts a WebVTT transcript (as exported by Teams) into
/// `Speaker: text` lines, merging consecutive cues from the same speaker.
pub fn compact(raw: &str) -> String {
    let mut utterances: Vec<(Option<String>, String)> = Vec::new();
    let mut cue_text: Vec<String> = Vec::new();

    fn flush(cue_text: &mut Vec<String>, utterances: &mut Vec<(Option<String>, String)>) {
        if cue_text.is_empty() {
            return;
        }
        let joined = cue_text.join(" ");
        cue_text.clear();
        let (speaker, text) = split_voice(&joined);
        let text = text.trim().to_string();
        if text.is_empty() {
            return;
        }
        if let Some(last) = utterances.last_mut()
            && speaker.is_some()
            && last.0 == speaker
        {
            last.1.push(' ');
            last.1.push_str(&text);
            return;
        }
        utterances.push((speaker, text));
    }

    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            flush(&mut cue_text, &mut utterances);
            continue;
        }
        if trimmed.starts_with("WEBVTT") || trimmed.starts_with("NOTE") || trimmed.contains("-->") {
            continue;
        }
        if is_cue_id(trimmed) {
            continue;
        }
        cue_text.push(trimmed.to_string());
    }
    flush(&mut cue_text, &mut utterances);

    utterances
        .into_iter()
        .map(|(speaker, text)| match speaker {
            Some(s) => format!("{s}: {text}"),
            None => text,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn is_cue_id(line: &str) -> bool {
    // Teams cue ids look like "<uuid>/5-0"; generic ids are digits only.
    line.chars().all(|c| c.is_ascii_digit())
        || (line.contains('/')
            && !line.contains(' ')
            && line.chars().all(|c| c.is_ascii_hexdigit() || c == '-' || c == '/'))
}

fn split_voice(s: &str) -> (Option<String>, String) {
    if let Some(rest) = s.strip_prefix("<v ")
        && let Some(end) = rest.find('>')
    {
        let speaker = rest[..end].trim().to_string();
        let text = rest[end + 1..].replace("</v>", "");
        return (Some(speaker), text);
    }
    (None, s.replace("</v>", ""))
}

#[cfg(test)]
mod tests {
    use super::compact;

    const SAMPLE: &str = "WEBVTT\n\n12acf6b1/5-0\n00:00:03.322 --> 00:00:05.922\n<v Alex Morgan>And I share my screen.</v>\n\n12acf6b1/6-0\n00:00:13.002 --> 00:00:14.882\n<v Alex Morgan>And we'll start up a new page here.</v>\n\n12acf6b1/9-0\n00:00:23.762 --> 00:00:26.273\n<v Sam Rivera>Let's just call it like cloud\ninfrastructure,</v>\n";

    #[test]
    fn drops_timing_and_ids() {
        let out = compact(SAMPLE);
        assert!(!out.contains("WEBVTT"));
        assert!(!out.contains("-->"));
        assert!(!out.contains("12acf6b1"));
        assert!(!out.contains("<v "));
    }

    #[test]
    fn joins_wrapped_cue_lines() {
        let out = compact(SAMPLE);
        assert!(out.contains("Sam Rivera: Let's just call it like cloud infrastructure,"), "{out}");
    }

    #[test]
    fn merges_consecutive_same_speaker() {
        let out = compact(SAMPLE);
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines.len(), 2, "{out}");
        assert_eq!(lines[0], "Alex Morgan: And I share my screen. And we'll start up a new page here.");
    }

    #[test]
    fn cue_without_voice_tag_is_kept() {
        let out = compact("WEBVTT\n\n00:00:01.000 --> 00:00:02.000\nplain line\n");
        assert_eq!(out, "plain line");
    }
}
