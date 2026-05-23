//! Layered decoding strategy: BOM sniff → UTF-8 strict → Windows-1252 fallback.

use crate::error::{ProviderError, ProviderResult};

/// Result of sniffing the first bytes of a file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DetectedEncoding {
    Utf8,           // No BOM, decodes as UTF-8.
    Utf8Bom,        // EF BB BF prefix.
    Utf16Le,        // FF FE prefix.
    Utf16Be,        // FE FF prefix.
    Windows1252,    // No BOM, didn't decode as UTF-8; fallback.
}

/// Decode the entire byte buffer to a String using the layered strategy.
/// Returns (decoded_text, detected_encoding) so callers can surface the
/// "Decoded as Windows-1252" note in the preview header.
pub fn decode_layered(bytes: &[u8]) -> ProviderResult<(String, DetectedEncoding)> {
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        let body = &bytes[3..];
        let (cow, _, had_errors) = encoding_rs::UTF_8.decode(body);
        if had_errors {
            return Err(ProviderError::UndecodableEncoding);
        }
        return Ok((cow.into_owned(), DetectedEncoding::Utf8Bom));
    }
    if bytes.starts_with(&[0xFF, 0xFE]) {
        let body = &bytes[2..];
        let (cow, _, had_errors) = encoding_rs::UTF_16LE.decode(body);
        if had_errors {
            return Err(ProviderError::UndecodableEncoding);
        }
        return Ok((cow.into_owned(), DetectedEncoding::Utf16Le));
    }
    if bytes.starts_with(&[0xFE, 0xFF]) {
        let body = &bytes[2..];
        let (cow, _, had_errors) = encoding_rs::UTF_16BE.decode(body);
        if had_errors {
            return Err(ProviderError::UndecodableEncoding);
        }
        return Ok((cow.into_owned(), DetectedEncoding::Utf16Be));
    }

    // No BOM — try UTF-8 strict on the whole buffer.
    if let Ok(s) = std::str::from_utf8(bytes) {
        return Ok((s.to_owned(), DetectedEncoding::Utf8));
    }

    // Fall back to Windows-1252; encoding_rs guarantees no errors for 1252.
    let (cow, _, _) = encoding_rs::WINDOWS_1252.decode(bytes);
    tracing::warn!("CSV decoded as Windows-1252 (no UTF-8 BOM and not valid UTF-8)");
    Ok((cow.into_owned(), DetectedEncoding::Windows1252))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_utf8_bom() {
        let bytes = b"\xEF\xBB\xBFhello";
        let (s, enc) = decode_layered(bytes).unwrap();
        assert_eq!(s, "hello");
        assert_eq!(enc, DetectedEncoding::Utf8Bom);
    }

    #[test]
    fn decodes_utf16_le_with_bom() {
        // "hi" in UTF-16 LE with BOM
        let bytes = b"\xFF\xFEh\x00i\x00";
        let (s, enc) = decode_layered(bytes).unwrap();
        assert_eq!(s, "hi");
        assert_eq!(enc, DetectedEncoding::Utf16Le);
    }

    #[test]
    fn decodes_utf16_be_with_bom() {
        let bytes = b"\xFE\xFF\x00h\x00i";
        let (s, enc) = decode_layered(bytes).unwrap();
        assert_eq!(s, "hi");
        assert_eq!(enc, DetectedEncoding::Utf16Be);
    }

    #[test]
    fn plain_utf8_no_bom_is_utf8() {
        let bytes = b"name,amount\nSafeway,-8.42\n";
        let (s, enc) = decode_layered(bytes).unwrap();
        assert!(s.starts_with("name,amount"));
        assert_eq!(enc, DetectedEncoding::Utf8);
    }

    #[test]
    fn invalid_utf8_falls_back_to_windows_1252() {
        // 0xE9 is "é" in Windows-1252 but invalid as a UTF-8 lead byte standalone.
        let bytes = b"caf\xE9";
        let (s, enc) = decode_layered(bytes).unwrap();
        assert_eq!(s, "café");
        assert_eq!(enc, DetectedEncoding::Windows1252);
    }
}
