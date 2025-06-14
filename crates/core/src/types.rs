//! Core Gate types

use serde::{Deserialize, Serialize};
use std::fmt;

/// Gate node identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct GateId([u8; 32]);

/// Gate node address (ID + connection info)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GateAddr {
    pub id: GateId,
    pub direct_addresses: Vec<std::net::SocketAddr>,
}

impl GateId {
    /// Create a new `GateId` from bytes
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Get the bytes of this `GateId`
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Get the bytes as a slice
    #[must_use]
    pub const fn as_slice(&self) -> &[u8] {
        &self.0
    }
}

impl fmt::Display for GateId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", hex::encode(self.0))
    }
}

impl std::str::FromStr for GateId {
    type Err = hex::FromHexError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let bytes = hex::decode(s)?;
        if bytes.len() != 32 {
            return Err(hex::FromHexError::InvalidStringLength);
        }
        let mut array = [0u8; 32];
        array.copy_from_slice(&bytes);
        Ok(Self(array))
    }
}

impl GateAddr {
    /// Create a new `GateAddr`
    #[must_use]
    pub const fn new(id: GateId, direct_addresses: Vec<std::net::SocketAddr>) -> Self {
        Self {
            id,
            direct_addresses,
        }
    }
}

impl fmt::Display for GateAddr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.direct_addresses.is_empty() {
            write!(f, "{}", self.id)
        } else {
            write!(f, "{}@{}", self.id, self.direct_addresses[0])
        }
    }
}

impl std::str::FromStr for GateAddr {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = s.splitn(2, '@').collect();
        if parts.len() == 1 {
            // Just an ID, no addresses
            let id = parts[0]
                .parse::<GateId>()
                .map_err(|e| format!("Invalid GateId: {e}"))?;
            return Ok(Self {
                id,
                direct_addresses: vec![],
            });
        }

        if parts.len() != 2 {
            return Err("GateAddr must be in format id@addr".to_string());
        }

        let id = parts[0]
            .parse::<GateId>()
            .map_err(|e| format!("Invalid GateId: {e}"))?;

        // Parse as socket address (ip:port format)
        let socket_addr = parts[1]
            .parse::<std::net::SocketAddr>()
            .map_err(|e| format!("Invalid socket address: {e}"))?;

        Ok(Self {
            id,
            direct_addresses: vec![socket_addr],
        })
    }
}
