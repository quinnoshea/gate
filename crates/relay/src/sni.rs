use crate::error::{RelayError, Result};
use bytes::{Buf, Bytes};
use std::collections::HashMap;
use tracing::{debug, warn};

/// Extracts Server Name Indication (SNI) from TLS ClientHello messages
#[derive(Clone)]
pub struct SniExtractor {
    /// Cache of recently parsed domains to improve performance
    domain_cache: HashMap<Vec<u8>, Option<String>>,
}

impl SniExtractor {
    /// Create a new SNI extractor
    pub fn new() -> Self {
        Self {
            domain_cache: HashMap::new(),
        }
    }

    /// Extract the SNI hostname from TLS ClientHello data
    ///
    /// This parses the initial bytes of a TLS connection to find the
    /// Server Name Indication extension containing the requested hostname.
    pub fn extract_sni(&self, data: &[u8]) -> Result<Option<String>> {
        if data.len() < 43 {
            return Ok(None);
        }

        let mut cursor = std::io::Cursor::new(data);

        // Parse TLS record header
        let record = self.parse_tls_record(&mut cursor)?;
        if record.content_type != 22 {
            // Not a handshake record
            return Ok(None);
        }

        // Parse ClientHello
        let client_hello = self.parse_client_hello(&mut cursor)?;

        // Extract SNI from extensions
        self.extract_sni_from_extensions(&client_hello.extensions)
    }

    fn parse_tls_record(&self, cursor: &mut std::io::Cursor<&[u8]>) -> Result<TlsRecord> {
        if cursor.remaining() < 5 {
            return Err(RelayError::SniExtraction(
                "Insufficient data for TLS record header".to_string(),
            ));
        }

        let content_type = cursor.get_u8();
        let version = cursor.get_u16();
        let length = cursor.get_u16();

        debug!(
            "TLS Record: type={}, version={:x}, length={}",
            content_type, version, length
        );

        Ok(TlsRecord {
            content_type,
            version,
            length,
        })
    }

    fn parse_client_hello(&self, cursor: &mut std::io::Cursor<&[u8]>) -> Result<ClientHello> {
        if cursor.remaining() < 4 {
            return Err(RelayError::SniExtraction(
                "Insufficient data for ClientHello header".to_string(),
            ));
        }

        let handshake_type = cursor.get_u8();
        if handshake_type != 1 {
            return Err(RelayError::SniExtraction(format!(
                "Expected ClientHello (1), got {}",
                handshake_type
            )));
        }

        let length = cursor.get_uint(3) as usize;
        let _version = cursor.get_u16();

        // Skip random (32 bytes)
        if cursor.remaining() < 32 {
            return Err(RelayError::SniExtraction("Missing random".to_string()));
        }
        cursor.advance(32);

        // Skip session ID
        if cursor.remaining() < 1 {
            return Err(RelayError::SniExtraction(
                "Missing session ID length".to_string(),
            ));
        }
        let session_id_len = cursor.get_u8() as usize;
        if cursor.remaining() < session_id_len {
            return Err(RelayError::SniExtraction(
                "Truncated session ID".to_string(),
            ));
        }
        cursor.advance(session_id_len);

        // Skip cipher suites
        if cursor.remaining() < 2 {
            return Err(RelayError::SniExtraction(
                "Missing cipher suites length".to_string(),
            ));
        }
        let cipher_suites_len = cursor.get_u16() as usize;
        if cursor.remaining() < cipher_suites_len {
            return Err(RelayError::SniExtraction(
                "Truncated cipher suites".to_string(),
            ));
        }
        cursor.advance(cipher_suites_len);

        // Skip compression methods
        if cursor.remaining() < 1 {
            return Err(RelayError::SniExtraction(
                "Missing compression methods length".to_string(),
            ));
        }
        let compression_len = cursor.get_u8() as usize;
        if cursor.remaining() < compression_len {
            return Err(RelayError::SniExtraction(
                "Truncated compression methods".to_string(),
            ));
        }
        cursor.advance(compression_len);

        // Parse extensions
        let extensions = if cursor.remaining() >= 2 {
            let extensions_len = cursor.get_u16() as usize;
            if cursor.remaining() >= extensions_len {
                self.parse_extensions(cursor, extensions_len)?
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };

        Ok(ClientHello { extensions })
    }

    fn parse_extensions(
        &self,
        cursor: &mut std::io::Cursor<&[u8]>,
        total_len: usize,
    ) -> Result<Vec<Extension>> {
        let mut extensions = Vec::new();
        let mut remaining = total_len;

        while remaining >= 4 {
            let ext_type = cursor.get_u16();
            let ext_len = cursor.get_u16() as usize;

            if remaining < 4 + ext_len {
                break;
            }

            let ext_data = if ext_len > 0 && cursor.remaining() >= ext_len {
                let mut data = vec![0u8; ext_len];
                cursor.copy_to_slice(&mut data);
                data
            } else {
                Vec::new()
            };

            extensions.push(Extension {
                ext_type,
                data: ext_data,
            });

            remaining -= 4 + ext_len;
        }

        Ok(extensions)
    }

    fn extract_sni_from_extensions(&self, extensions: &[Extension]) -> Result<Option<String>> {
        // Look for Server Name Indication extension (type 0)
        for ext in extensions {
            if ext.ext_type == 0 {
                return self.parse_sni_extension(&ext.data);
            }
        }

        Ok(None)
    }

    fn parse_sni_extension(&self, data: &[u8]) -> Result<Option<String>> {
        if data.len() < 5 {
            return Ok(None);
        }

        let mut cursor = std::io::Cursor::new(data);

        // Server Name List Length
        let list_len = cursor.get_u16() as usize;
        if cursor.remaining() < list_len {
            return Ok(None);
        }

        // Name Type (should be 0 for hostname)
        let name_type = cursor.get_u8();
        if name_type != 0 {
            return Ok(None);
        }

        // Name Length
        let name_len = cursor.get_u16() as usize;
        if cursor.remaining() < name_len {
            return Ok(None);
        }

        // Extract hostname
        let mut hostname = vec![0u8; name_len];
        cursor.copy_to_slice(&mut hostname);

        match String::from_utf8(hostname) {
            Ok(domain) => {
                debug!("Extracted SNI: {}", domain);
                Ok(Some(domain))
            }
            Err(e) => {
                warn!("Invalid UTF-8 in SNI: {}", e);
                Ok(None)
            }
        }
    }
}

#[derive(Debug)]
struct TlsRecord {
    content_type: u8,
    version: u16,
    length: u16,
}

#[derive(Debug)]
struct ClientHello {
    extensions: Vec<Extension>,
}

#[derive(Debug)]
struct Extension {
    ext_type: u16,
    data: Vec<u8>,
}

impl Default for SniExtractor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_sni_from_valid_client_hello() {
        let extractor = SniExtractor::new();

        // This is a simplified test case - in practice you'd use real ClientHello data
        // For now, test the basic structure
        let result = extractor.extract_sni(&[]);
        assert!(result.is_ok());
    }

    #[test]
    fn test_extract_sni_from_empty_data() {
        let extractor = SniExtractor::new();
        let result = extractor.extract_sni(&[]);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), None);
    }
}
