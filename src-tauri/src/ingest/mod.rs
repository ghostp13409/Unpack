pub mod docx;
pub mod eml;
pub mod html;
pub mod vtt;

use base64::Engine;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Refuse inputs above this many characters of extracted text (PDFs excluded).
pub const MAX_TOTAL_CHARS: usize = 350_000;
/// Largest PDF accepted, checked before the file is read (the API caps PDFs at 32 MB).
pub const MAX_PDF_BYTES: u64 = 32 * 1024 * 1024;
/// Largest text, transcript, email or Word file accepted, checked before reading.
pub const MAX_FILE_BYTES: u64 = 25 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Extracted {
    Text { filename: String, text: String },
    Pdf { filename: String, base64: String, path: String },
}

impl Extracted {
    pub fn filename(&self) -> &str {
        match self {
            Extracted::Text { filename, .. } | Extracted::Pdf { filename, .. } => filename,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum IngestError {
    #[error("Unsupported file type {0:?}")]
    Unsupported(String),
    #[error("Could not read file: {0}")]
    Read(String),
    #[error("Could not parse file: {0}")]
    Parse(String),
    #[error("{0}")]
    TooLarge(String),
}

fn check_size(path: &Path, filename: &str, limit: u64, what: &str) -> Result<(), IngestError> {
    let len = std::fs::metadata(path).map_err(|e| IngestError::Read(format!("{filename}: {e}")))?.len();
    if len > limit {
        let mb = |b: u64| b.div_ceil(1024 * 1024);
        return Err(IngestError::TooLarge(format!(
            "{filename} is too large ({} MB). The limit{what} is {} MB.",
            mb(len),
            mb(limit)
        )));
    }
    Ok(())
}

pub fn extract_file(path: &Path) -> Result<Extracted, IngestError> {
    let filename = path
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string_lossy().to_string());
    let ext = path
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy().to_ascii_lowercase()))
        .unwrap_or_default();
    match ext.as_str() {
        ".pdf" => check_size(path, &filename, MAX_PDF_BYTES, " for PDFs")?,
        ".txt" | ".md" | ".json" | ".csv" | ".log" | ".vtt" | ".eml" | ".docx" => {
            check_size(path, &filename, MAX_FILE_BYTES, "")?
        }
        _ => {}
    }
    let read = || std::fs::read(path).map_err(|e| IngestError::Read(format!("{filename}: {e}")));
    let text = match ext.as_str() {
        ".txt" | ".md" | ".json" | ".csv" | ".log" => String::from_utf8_lossy(&read()?).to_string(),
        ".vtt" => vtt::compact(&String::from_utf8_lossy(&read()?)),
        ".eml" => eml::extract(&read()?)?,
        ".docx" => docx::extract(&read()?)?,
        ".pdf" => {
            let b64 = base64::engine::general_purpose::STANDARD.encode(read()?);
            return Ok(Extracted::Pdf { filename, base64: b64, path: path.to_string_lossy().to_string() });
        }
        _ => return Err(IngestError::Unsupported(ext)),
    };
    Ok(Extracted::Text { filename, text })
}

pub fn total_chars(inputs: &[Extracted]) -> usize {
    inputs
        .iter()
        .map(|i| match i {
            Extracted::Text { text, .. } => text.chars().count(),
            Extracted::Pdf { .. } => 0,
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsupported_extension_is_error() {
        let dir = std::env::temp_dir();
        let p = dir.join("pgtt_test.xlsx");
        std::fs::write(&p, b"x").unwrap();
        let err = extract_file(&p).unwrap_err();
        assert!(matches!(err, IngestError::Unsupported(_)));
        assert_eq!(err.to_string(), "Unsupported file type \".xlsx\"");
    }

    #[test]
    fn text_file_is_read_and_pdf_is_base64() {
        let dir = std::env::temp_dir();
        let t = dir.join("pgtt_test.md");
        std::fs::write(&t, "# hi").unwrap();
        match extract_file(&t).unwrap() {
            Extracted::Text { filename, text } => {
                assert_eq!(filename, "pgtt_test.md");
                assert_eq!(text, "# hi");
            }
            _ => panic!("expected text"),
        }
        let p = dir.join("pgtt_test.pdf");
        std::fs::write(&p, b"%PDF").unwrap();
        match extract_file(&p).unwrap() {
            Extracted::Pdf { filename, base64, path } => {
                assert_eq!(filename, "pgtt_test.pdf");
                assert_eq!(base64, "JVBERg==");
                assert!(path.ends_with("pgtt_test.pdf"));
            }
            _ => panic!("expected pdf"),
        }
    }

    #[test]
    fn total_chars_ignores_pdf() {
        let inputs = vec![
            Extracted::Text { filename: "a".into(), text: "abc".into() },
            Extracted::Pdf { filename: "b".into(), base64: "xxxxxxxx".into(), path: "b".into() },
        ];
        assert_eq!(total_chars(&inputs), 3);
        assert_eq!(MAX_TOTAL_CHARS, 350_000);
    }


    #[test]
    fn oversized_pdf_is_refused_before_reading() {
        let p = std::env::temp_dir().join("pgtt_big.pdf");
        let f = std::fs::File::create(&p).unwrap();
        f.set_len(MAX_PDF_BYTES + 1).unwrap();
        let err = extract_file(&p).unwrap_err();
        assert!(matches!(err, IngestError::TooLarge(_)), "{err:?}");
        assert_eq!(err.to_string(), "pgtt_big.pdf is too large (33 MB). The limit for PDFs is 32 MB.");
        std::fs::remove_file(&p).ok();
    }

    #[test]
    fn oversized_text_file_is_refused_before_reading() {
        let p = std::env::temp_dir().join("pgtt_big.txt");
        let f = std::fs::File::create(&p).unwrap();
        f.set_len(MAX_FILE_BYTES + 1).unwrap();
        let err = extract_file(&p).unwrap_err();
        assert!(matches!(err, IngestError::TooLarge(_)), "{err:?}");
        assert_eq!(err.to_string(), "pgtt_big.txt is too large (26 MB). The limit is 25 MB.");
        std::fs::remove_file(&p).ok();
    }
}
