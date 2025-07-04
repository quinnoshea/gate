//! SNI (Server Name Indication) extraction from TLS ClientHello

use crate::common::error::{Result, TlsForwardError};
use bytes::Buf;
use std::io::Cursor;

/// Extract the SNI hostname from a TLS ClientHello message
pub fn extract_sni(data: &[u8]) -> Result<String> {
    let mut cursor = Cursor::new(data);

    // Check minimum size for TLS record header
    if data.len() < 5 {
        return Err(TlsForwardError::InvalidSni(
            "Data too short for TLS record".into(),
        ));
    }

    // Read TLS record header
    let content_type = cursor.get_u8();
    let version_major = cursor.get_u8();
    let version_minor = cursor.get_u8();
    let record_length = cursor.get_u16();

    // Validate TLS handshake record (content_type = 22)
    if content_type != 22 {
        return Err(TlsForwardError::InvalidSni(format!(
            "Not a handshake record: content_type = {content_type}"
        )));
    }

    // Validate TLS version (TLS 1.0-1.3)
    if version_major != 3 || version_minor > 4 {
        return Err(TlsForwardError::InvalidSni(format!(
            "Invalid TLS version: {version_major}.{version_minor}"
        )));
    }

    // Check if we have enough data for the record
    if cursor.remaining() < record_length as usize {
        return Err(TlsForwardError::InvalidSni("Incomplete TLS record".into()));
    }

    // Read handshake header
    if cursor.remaining() < 4 {
        return Err(TlsForwardError::InvalidSni("No handshake header".into()));
    }

    let handshake_type = cursor.get_u8();
    let handshake_length = cursor.get_uint(3) as usize;

    // Validate ClientHello (handshake_type = 1)
    if handshake_type != 1 {
        return Err(TlsForwardError::InvalidSni(format!(
            "Not a ClientHello: handshake_type = {handshake_type}"
        )));
    }

    // Check handshake message length
    if cursor.remaining() < handshake_length {
        return Err(TlsForwardError::InvalidSni(
            "Incomplete handshake message".into(),
        ));
    }

    // Skip client version (2 bytes)
    cursor.advance(2);

    // Skip random (32 bytes)
    cursor.advance(32);

    // Skip session ID
    let session_id_len = cursor.get_u8() as usize;
    cursor.advance(session_id_len);

    // Skip cipher suites
    let cipher_suites_len = cursor.get_u16() as usize;
    cursor.advance(cipher_suites_len);

    // Skip compression methods
    let compression_methods_len = cursor.get_u8() as usize;
    cursor.advance(compression_methods_len);

    // Check if extensions are present
    if cursor.remaining() < 2 {
        return Err(TlsForwardError::InvalidSni("No extensions present".into()));
    }

    let extensions_len = cursor.get_u16() as usize;
    if cursor.remaining() < extensions_len {
        return Err(TlsForwardError::InvalidSni("Incomplete extensions".into()));
    }

    let extensions_end = cursor.position() + extensions_len as u64;

    // Parse extensions
    while cursor.position() < extensions_end {
        if cursor.remaining() < 4 {
            break;
        }

        let extension_type = cursor.get_u16();
        let extension_len = cursor.get_u16() as usize;

        if cursor.remaining() < extension_len {
            return Err(TlsForwardError::InvalidSni("Incomplete extension".into()));
        }

        // Server Name extension (type = 0)
        if extension_type == 0 {
            return parse_sni_extension(&mut cursor, extension_len);
        }

        // Skip this extension
        cursor.advance(extension_len);
    }

    Err(TlsForwardError::InvalidSni("No SNI extension found".into()))
}

/// Parse the SNI extension to extract the hostname
fn parse_sni_extension(cursor: &mut Cursor<&[u8]>, extension_len: usize) -> Result<String> {
    let start_pos = cursor.position();

    // Read server name list length
    if cursor.remaining() < 2 {
        return Err(TlsForwardError::InvalidSni("Invalid SNI extension".into()));
    }
    let _list_len = cursor.get_u16() as usize;

    // Parse server names
    while cursor.position() < start_pos + extension_len as u64 {
        if cursor.remaining() < 3 {
            break;
        }

        let name_type = cursor.get_u8();
        let name_len = cursor.get_u16() as usize;

        if cursor.remaining() < name_len {
            return Err(TlsForwardError::InvalidSni("Incomplete server name".into()));
        }

        // Host name type (0)
        if name_type == 0 {
            let mut hostname_bytes = vec![0u8; name_len];
            cursor.copy_to_slice(&mut hostname_bytes);

            let hostname = String::from_utf8(hostname_bytes)
                .map_err(|_| TlsForwardError::InvalidSni("Invalid UTF-8 in hostname".into()))?;

            // Basic validation
            if hostname.is_empty() {
                return Err(TlsForwardError::InvalidSni("Empty hostname".into()));
            }

            return Ok(hostname);
        }

        // Skip other name types
        cursor.advance(name_len);
    }

    Err(TlsForwardError::InvalidSni(
        "No hostname in SNI extension".into(),
    ))
}
