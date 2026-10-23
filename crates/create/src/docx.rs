//! Extract plain text from Office and document formats: DOCX, ODT, RTF, DOC.

use std::io::Read;
use flate2::read::DeflateDecoder;
use crate::CreateError;

/// Extract plain text from Word (.docx, .doc), OpenDocument (.odt), or RTF (.rtf).
pub fn extract_document_text(name: &str, bytes: &[u8]) -> Result<String, CreateError> {
    let lower = name.to_ascii_lowercase();

    if lower.ends_with(".rtf") || bytes.starts_with(b"{\\rtf") {
        return Ok(extract_rtf_text(bytes));
    }

    if lower.ends_with(".odt") {
        if let Ok(xml) = extract_zip_entry(bytes, "content.xml") {
            let text = extract_xml_text(&xml);
            if !text.trim().is_empty() {
                return Ok(text);
            }
        }
    }

    // DOCX: search for word/document.xml in the ZIP container
    if lower.ends_with(".docx") || bytes.starts_with(b"PK\x03\x04") {
        if let Ok(xml) = extract_zip_entry(bytes, "word/document.xml") {
            let text = extract_docx_xml_text(&xml);
            if !text.trim().is_empty() {
                return Ok(text);
            }
        }
    }

    // Binary DOC or fallback: extract printable text streams
    let fallback = extract_raw_printable_text(bytes);
    if !fallback.trim().is_empty() {
        return Ok(fallback);
    }

    Err(CreateError::Invalid(format!("{name}: no readable text found in document")))
}

/// Find a file in a ZIP byte buffer and decompress it.
fn extract_zip_entry(bytes: &[u8], target_name: &str) -> Result<String, ()> {
    // 1. Try scanning local file headers
    let mut cursor = 0;
    while cursor + 30 <= bytes.len() {
        if &bytes[cursor..cursor + 4] != b"PK\x03\x04" {
            // Scan forward for next PK\x03\x04
            match bytes[cursor + 1..].windows(4).position(|w| w == b"PK\x03\x04") {
                Some(pos) => {
                    cursor += pos + 1;
                }
                None => break,
            }
        }

        if cursor + 30 > bytes.len() {
            break;
        }

        let method = u16::from_le_bytes([bytes[cursor + 8], bytes[cursor + 9]]);
        let comp_size = u32::from_le_bytes([bytes[cursor + 18], bytes[cursor + 19], bytes[cursor + 20], bytes[cursor + 21]]) as usize;
        let name_len = u16::from_le_bytes([bytes[cursor + 26], bytes[cursor + 27]]) as usize;
        let extra_len = u16::from_le_bytes([bytes[cursor + 28], bytes[cursor + 29]]) as usize;

        let name_start = cursor + 30;
        let name_end = name_start + name_len;
        if name_end > bytes.len() {
            break;
        }

        let entry_name = String::from_utf8_lossy(&bytes[name_start..name_end]);
        let data_start = name_end + extra_len;

        if entry_name == target_name {
            if data_start > bytes.len() {
                return Err(());
            }

            // If compressed size is 0 or data descriptor flag is set, search for next PK
            let data_slice = if comp_size > 0 && data_start + comp_size <= bytes.len() {
                &bytes[data_start..data_start + comp_size]
            } else {
                // Find next PK header or end
                let end = bytes[data_start..].windows(4).position(|w| w.starts_with(b"PK")).map(|p| data_start + p).unwrap_or(bytes.len());
                &bytes[data_start..end]
            };

            if method == 0 {
                return Ok(String::from_utf8_lossy(data_slice).into_owned());
            } else if method == 8 {
                let mut decoder = DeflateDecoder::new(data_slice);
                let mut out = String::new();
                if decoder.read_to_string(&mut out).is_ok() && !out.is_empty() {
                    return Ok(out);
                }
            }
        }

        cursor = if comp_size > 0 { data_start + comp_size } else { data_start + 1 };
    }

    // 2. Try Central Directory if local header scan missed it (e.g. data descriptor was used)
    if let Some(cd_offset) = find_central_directory(bytes) {
        let mut cur = cd_offset;
        while cur + 46 <= bytes.len() && &bytes[cur..cur + 4] == b"PK\x01\x02" {
            let method = u16::from_le_bytes([bytes[cur + 10], bytes[cur + 11]]);
            let comp_size = u32::from_le_bytes([bytes[cur + 20], bytes[cur + 21], bytes[cur + 22], bytes[cur + 23]]) as usize;
            let name_len = u16::from_le_bytes([bytes[cur + 28], bytes[cur + 29]]) as usize;
            let extra_len = u16::from_le_bytes([bytes[cur + 30], bytes[cur + 31]]) as usize;
            let comment_len = u16::from_le_bytes([bytes[cur + 32], bytes[cur + 33]]) as usize;
            let local_offset = u32::from_le_bytes([bytes[cur + 42], bytes[cur + 43], bytes[cur + 44], bytes[cur + 45]]) as usize;

            let name_start = cur + 46;
            let name_end = name_start + name_len;
            if name_end <= bytes.len() {
                let entry_name = String::from_utf8_lossy(&bytes[name_start..name_end]);
                if entry_name == target_name && local_offset + 30 <= bytes.len() {
                    let local_name_len = u16::from_le_bytes([bytes[local_offset + 26], bytes[local_offset + 27]]) as usize;
                    let local_extra_len = u16::from_le_bytes([bytes[local_offset + 28], bytes[local_offset + 29]]) as usize;
                    let data_start = local_offset + 30 + local_name_len + local_extra_len;

                    if data_start + comp_size <= bytes.len() {
                        let data_slice = &bytes[data_start..data_start + comp_size];
                        if method == 0 {
                            return Ok(String::from_utf8_lossy(data_slice).into_owned());
                        } else if method == 8 {
                            let mut decoder = DeflateDecoder::new(data_slice);
                            let mut out = String::new();
                            if decoder.read_to_string(&mut out).is_ok() && !out.is_empty() {
                                return Ok(out);
                            }
                        }
                    }
                }
            }

            cur += 46 + name_len + extra_len + comment_len;
        }
    }

    Err(())
}

fn find_central_directory(bytes: &[u8]) -> Option<usize> {
    // EOCD signature PK\x05\x06 is near end of file (at least 22 bytes from end)
    let search_start = bytes.len().saturating_sub(65536 + 22);
    let end_slice = &bytes[search_start..];
    let pos = end_slice.windows(4).rposition(|w| w == b"PK\x05\x06")?;
    let eocd = search_start + pos;
    if eocd + 20 <= bytes.len() {
        let cd_offset = u32::from_le_bytes([bytes[eocd + 16], bytes[eocd + 17], bytes[eocd + 18], bytes[eocd + 19]]) as usize;
        Some(cd_offset)
    } else {
        None
    }
}

/// Extract text from Word XML (word/document.xml).
fn extract_docx_xml_text(xml: &str) -> String {
    let mut out = String::with_capacity(xml.len() / 4);
    let mut in_text = false;
    let mut chars = xml.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '<' {
            let mut tag = String::new();
            for next_c in chars.by_ref() {
                if next_c == '>' {
                    break;
                }
                tag.push(next_c);
            }
            let tag_trimmed = tag.trim();

            if tag_trimmed.starts_with("w:p ") || tag_trimmed == "w:p" || tag_trimmed.starts_with("/w:p") {
                if !out.ends_with('\n') && !out.is_empty() {
                    out.push('\n');
                }
            } else if tag_trimmed.starts_with("w:br") || tag_trimmed.starts_with("w:cr") {
                out.push('\n');
            } else if tag_trimmed.starts_with("w:tab") {
                out.push('\t');
            } else if tag_trimmed.starts_with("w:t ") || tag_trimmed == "w:t" {
                in_text = true;
            } else if tag_trimmed == "/w:t" {
                in_text = false;
            }
        } else if in_text {
            if c == '&' {
                let mut entity = String::new();
                for next_c in chars.by_ref() {
                    if next_c == ';' {
                        break;
                    }
                    entity.push(next_c);
                    if entity.len() > 10 {
                        break;
                    }
                }
                match entity.as_str() {
                    "amp" => out.push('&'),
                    "lt" => out.push('<'),
                    "gt" => out.push('>'),
                    "quot" => out.push('"'),
                    "apos" => out.push('\''),
                    _ => {
                        if entity.starts_with("#x") {
                            if let Ok(val) = u32::from_str_radix(&entity[2..], 16) {
                                if let Some(ch) = char::from_u32(val) {
                                    out.push(ch);
                                }
                            }
                        } else if entity.starts_with('#') {
                            if let Ok(val) = entity[1..].parse::<u32>() {
                                if let Some(ch) = char::from_u32(val) {
                                    out.push(ch);
                                }
                            }
                        }
                    }
                }
            } else {
                out.push(c);
            }
        }
    }

    clean_text(&out)
}

/// Generic XML text extraction (e.g. for ODT content.xml).
fn extract_xml_text(xml: &str) -> String {
    let mut out = String::with_capacity(xml.len() / 4);
    let mut in_tag = false;

    for c in xml.chars() {
        if c == '<' {
            in_tag = true;
        } else if c == '>' {
            in_tag = false;
        } else if !in_tag {
            out.push(c);
        }
    }

    clean_text(&out)
}

/// Extract text from RTF.
fn extract_rtf_text(bytes: &[u8]) -> String {
    let text = String::from_utf8_lossy(bytes);
    let mut out = String::new();
    let mut chars = text.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '\\' {
            let mut cmd = String::new();
            while let Some(&next) = chars.peek() {
                if next.is_ascii_alphabetic() {
                    cmd.push(chars.next().unwrap());
                } else {
                    break;
                }
            }
            // Skip optional space after RTF command
            if chars.peek() == Some(&' ') {
                chars.next();
            }
            if cmd == "par" || cmd == "line" {
                out.push('\n');
            } else if cmd == "tab" {
                out.push('\t');
            }
        } else if c == '{' || c == '}' {
            // Ignore RTF block braces
        } else if c == '\r' || c == '\n' {
            // RTF linefeeds inside raw stream are formatting whitespace
        } else {
            out.push(c);
        }
    }

    clean_text(&out)
}

/// Fallback for binary .doc or corrupted files: find sequences of ASCII / UTF-8 printable text.
fn extract_raw_printable_text(bytes: &[u8]) -> String {
    let mut out = String::new();
    let mut current_word = Vec::new();

    for &b in bytes {
        if (32..=126).contains(&b) || b == b'\n' || b == b'\t' || b >= 128 {
            current_word.push(b);
        } else {
            if current_word.len() >= 4 {
                if let Ok(s) = std::str::from_utf8(&current_word) {
                    out.push_str(s);
                    out.push(' ');
                }
            }
            current_word.clear();
        }
    }

    if current_word.len() >= 4 {
        if let Ok(s) = std::str::from_utf8(&current_word) {
            out.push_str(s);
        }
    }

    clean_text(&out)
}

fn clean_text(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut consecutive_newlines = 0;

    for line in input.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            consecutive_newlines += 1;
            if consecutive_newlines <= 2 {
                result.push('\n');
            }
        } else {
            consecutive_newlines = 0;
            for c in trimmed.chars() {
                if c == '\t' || pdfcraft_fonts::win_ansi_encodable(c) {
                    result.push(c);
                } else {
                    match c {
                        '“' | '”' => result.push('"'),
                        '‘' | '’' => result.push('\''),
                        '–' | '—' => result.push('-'),
                        '•' | '·' => {
                            result.push('-');
                            result.push(' ');
                        }
                        '…' => result.push_str("..."),
                        '«' | '»' => result.push('"'),
                        '✓' | '✔' => result.push_str("[x]"),
                        '✗' | '✘' => result.push_str("[ ]"),
                        '★' | '☆' => result.push('*'),
                        '→' | '➔' => result.push_str("->"),
                        '←' => result.push_str("<-"),
                        '\u{a0}' => result.push(' '),
                        _ => {
                            // Emojis (e.g. 🛒 U+1F6D2) and symbols outside WinAnsi:
                            // Safely omit emoji/symbols so the standard PDF font renders perfectly
                            if (0x1F000..=0x1FAFF).contains(&(c as u32))
                                || (0x2600..=0x27BF).contains(&(c as u32))
                                || (0x1F300..=0x1F9FF).contains(&(c as u32))
                            {
                                // Omit emoji cleanly
                            } else {
                                // For other characters, replace with space
                                result.push(' ');
                            }
                        }
                    }
                }
            }
            result.push('\n');
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cleans_emojis_and_special_characters() {
        let text_with_emoji = "Line 1: Panettone 2026\nLine 2: 🛒 Carrello spesa — €50 “offerta”\n";
        let cleaned = clean_text(text_with_emoji);
        assert!(!cleaned.contains('🛒'), "Emoji should be removed");
        assert!(cleaned.contains("Panettone 2026"));
        assert!(cleaned.contains("Carrello spesa"));
        // Assert it is 100% win_ansi encodable
        for c in cleaned.chars() {
            assert!(c == '\n' || c == '\r' || pdfcraft_fonts::win_ansi_encodable(c), "Character {c} must be encodable");
        }
    }

    #[test]
    fn extracts_docx_xml() {
        let sample_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
        <w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
            <w:body>
                <w:p>
                    <w:r><w:t>Benvenuto in </w:t></w:r>
                    <w:r><w:t>PdfCraft!</w:t></w:r>
                </w:p>
                <w:p>
                    <w:r><w:t>Conversione Word nativa &amp; veloce su iPadOS.</w:t></w:r>
                </w:p>
            </w:body>
        </w:document>"#;

        let extracted = extract_docx_xml_text(sample_xml);
        assert!(extracted.contains("Benvenuto in PdfCraft!"));
        assert!(extracted.contains("Conversione Word nativa & veloce su iPadOS."));
    }
}
