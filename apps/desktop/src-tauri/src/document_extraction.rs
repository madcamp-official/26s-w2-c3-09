use std::fs::{self, File};
use std::io::Read;
use std::path::Path;

use file_engine_cli::path_guard::PathGuard;
use quick_xml::events::Event;
use quick_xml::Reader;
use serde::Serialize;
use sha2::{Digest, Sha256};
use zip::ZipArchive;

const MAX_DOCUMENT_BYTES: u64 = 20 * 1024 * 1024;
const MAX_EXTRACTED_CHARS: usize = 100_000;
const CHUNK_CHARS: usize = 4_000;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentExtraction {
    pub relative_path: String,
    pub sha256: String,
    pub modified_unix_ms: i64,
    pub chunks: Vec<String>,
    pub truncated: bool,
}

pub fn extract_document(
    root: &str,
    relative_path: &str,
    consent: bool,
    expected_sha256: Option<&str>,
) -> Result<DocumentExtraction, String> {
    if !consent {
        return Err("CONSENT_REQUIRED".to_string());
    }
    let guard = PathGuard::new(root).map_err(|_| "OUTSIDE_MANAGED_ROOT".to_string())?;
    let path = guard
        .resolve_existing(relative_path)
        .map_err(|_| "OUTSIDE_MANAGED_ROOT".to_string())?;
    let metadata = fs::metadata(&path).map_err(|_| "SOURCE_CHANGED".to_string())?;
    if !metadata.is_file() {
        return Err("UNSUPPORTED_FORMAT".to_string());
    }
    if metadata.len() > MAX_DOCUMENT_BYTES {
        return Err("TOO_LARGE".to_string());
    }
    let bytes = fs::read(&path).map_err(|_| "SOURCE_CHANGED".to_string())?;
    let sha256 = format!("{:x}", Sha256::digest(&bytes));
    if expected_sha256.is_some_and(|expected| !sha256.eq_ignore_ascii_case(expected)) {
        return Err("SOURCE_CHANGED".to_string());
    }
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let text = match extension.as_str() {
        "txt" | "md" | "markdown" | "json" | "csv" | "tsv" | "log" => {
            String::from_utf8(bytes.clone()).map_err(|_| "UNSUPPORTED_FORMAT".to_string())?
        }
        "pdf" => pdf_extract::extract_text_from_mem(&bytes).map_err(|error| {
            if error.to_string().to_lowercase().contains("password") {
                "ENCRYPTED_DOCUMENT".to_string()
            } else {
                "UNSUPPORTED_FORMAT".to_string()
            }
        })?,
        "docx" | "pptx" | "xlsx" => extract_ooxml(&path, &extension)?,
        _ => return Err("UNSUPPORTED_FORMAT".to_string()),
    };
    let normalized: String = text.chars().take(MAX_EXTRACTED_CHARS).collect();
    let truncated = text.chars().count() > MAX_EXTRACTED_CHARS;
    let chunks = chunk_text(&normalized);
    let modified_unix_ms = metadata
        .modified()
        .ok()
        .and_then(|value| value.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|value| value.as_millis() as i64)
        .unwrap_or(0);
    Ok(DocumentExtraction {
        relative_path: relative_path.replace('\\', "/"),
        sha256,
        modified_unix_ms,
        chunks,
        truncated,
    })
}

fn extract_ooxml(path: &Path, extension: &str) -> Result<String, String> {
    let file = File::open(path).map_err(|_| "SOURCE_CHANGED".to_string())?;
    let mut archive = ZipArchive::new(file).map_err(|error| {
        if error.to_string().to_lowercase().contains("password") {
            "ENCRYPTED_DOCUMENT".to_string()
        } else {
            "UNSUPPORTED_FORMAT".to_string()
        }
    })?;
    let mut output = String::new();
    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|_| "UNSUPPORTED_FORMAT".to_string())?;
        let name = entry.name().to_string();
        let selected = match extension {
            "docx" => name == "word/document.xml",
            "pptx" => name.starts_with("ppt/slides/slide") && name.ends_with(".xml"),
            "xlsx" => {
                name == "xl/sharedStrings.xml"
                    || (name.starts_with("xl/worksheets/sheet") && name.ends_with(".xml"))
            }
            _ => false,
        };
        if !selected {
            continue;
        }
        let mut xml = String::new();
        entry
            .read_to_string(&mut xml)
            .map_err(|_| "UNSUPPORTED_FORMAT".to_string())?;
        output.push_str(&xml_text(&xml));
        output.push('\n');
    }
    if output.trim().is_empty() {
        return Err("UNSUPPORTED_FORMAT".to_string());
    }
    Ok(output)
}

fn xml_text(xml: &str) -> String {
    let mut reader = Reader::from_str(xml);
    let mut output = String::new();
    loop {
        match reader.read_event() {
            Ok(Event::Text(text)) => {
                if let Ok(value) = text.decode() {
                    if !output.is_empty() {
                        output.push(' ');
                    }
                    output.push_str(&value);
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    output
}

fn chunk_text(text: &str) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    chars
        .chunks(CHUNK_CHARS)
        .map(|chunk| chunk.iter().collect::<String>())
        .filter(|chunk| !chunk.trim().is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requires_consent_and_stays_inside_root() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(temp.path().join("notes.txt"), "hello mousekeeper").unwrap();
        assert_eq!(
            extract_document(temp.path().to_str().unwrap(), "notes.txt", false, None).unwrap_err(),
            "CONSENT_REQUIRED"
        );
        assert_eq!(
            extract_document(temp.path().to_str().unwrap(), "../notes.txt", true, None)
                .unwrap_err(),
            "OUTSIDE_MANAGED_ROOT"
        );
    }

    #[test]
    fn extracts_bounded_utf8_text_chunks() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(temp.path().join("notes.md"), "안녕 MouseKeeper").unwrap();
        let result =
            extract_document(temp.path().to_str().unwrap(), "notes.md", true, None).unwrap();
        assert_eq!(result.chunks, vec!["안녕 MouseKeeper"]);
        assert!(!result.sha256.is_empty());
        assert_eq!(
            extract_document(
                temp.path().to_str().unwrap(),
                "notes.md",
                true,
                Some(&"0".repeat(64)),
            )
            .unwrap_err(),
            "SOURCE_CHANGED"
        );
    }
}
