use super::html::decode_entities;
use super::IngestError;
use std::io::{Cursor, Read};

/// Pulls paragraph text out of a .docx (word/document.xml).
pub fn extract(bytes: &[u8]) -> Result<String, IngestError> {
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes))
        .map_err(|e| IngestError::Parse(format!("not a .docx file: {e}")))?;
    let file = archive
        .by_name("word/document.xml")
        .map_err(|_| IngestError::Parse("not a .docx file: missing word/document.xml".to_string()))?;
    let mut xml = String::new();
    // Bound the decompressed size so a malformed archive cannot exhaust memory.
    file.take(super::MAX_FILE_BYTES).read_to_string(&mut xml)
        .map_err(|e| IngestError::Parse(format!("could not read document.xml: {e}")))?;
    let xml = xml
        .replace("</w:p>", "\n")
        .replace("<w:tab/>", "\t")
        .replace("<w:br/>", "\n");
    let mut out = String::with_capacity(xml.len());
    let mut in_tag = false;
    for ch in xml.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            c if !in_tag => out.push(c),
            _ => {}
        }
    }
    Ok(decode_entities(&out).trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::extract;
    use std::io::Write;

    fn docx_with(xml: &str) -> Vec<u8> {
        let buf = std::io::Cursor::new(Vec::new());
        let mut w = zip::ZipWriter::new(buf);
        let opts = zip::write::SimpleFileOptions::default();
        w.start_file("word/document.xml", opts).unwrap();
        w.write_all(xml.as_bytes()).unwrap();
        w.finish().unwrap().into_inner()
    }

    #[test]
    fn extracts_paragraphs() {
        let xml = r#"<?xml version="1.0"?><w:document><w:body><w:p><w:r><w:t>First</w:t></w:r><w:r><w:t xml:space="preserve"> para &amp; more</w:t></w:r></w:p><w:p><w:r><w:t>Col</w:t></w:r><w:tab/><w:r><w:t>B</w:t></w:r></w:p></w:body></w:document>"#;
        let out = extract(&docx_with(xml)).unwrap();
        assert_eq!(out, "First para & more\nCol\tB");
    }

    #[test]
    fn not_a_docx_is_parse_error() {
        assert!(extract(b"not a zip").is_err());
    }
}
