//! Type conversions between protobuf types and core Gate types

use crate::pb::gate::common::v1 as proto;
use std::net::SocketAddr;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ConversionError {
    #[error("Invalid GateId length: expected 32 bytes, got {0}")]
    InvalidGateIdLength(usize),
    #[error("Invalid socket address format: {0}")]
    InvalidSocketAddr(String),
    #[error("Missing required field: {0}")]
    MissingField(&'static str),
    #[error("JSON parsing error: {0}")]
    JsonError(#[from] serde_json::Error),
}

// GateId conversions
impl From<hellas_gate_core::GateId> for proto::GateId {
    fn from(id: hellas_gate_core::GateId) -> Self {
        Self {
            id: id.as_bytes().to_vec(),
        }
    }
}

impl TryFrom<proto::GateId> for hellas_gate_core::GateId {
    type Error = ConversionError;

    fn try_from(proto: proto::GateId) -> Result<Self, Self::Error> {
        if proto.id.len() != 32 {
            return Err(ConversionError::InvalidGateIdLength(proto.id.len()));
        }
        let mut bytes = [0u8; 32];
        bytes.copy_from_slice(&proto.id);
        Ok(hellas_gate_core::GateId::from_bytes(bytes))
    }
}

// SocketAddr conversions
impl From<SocketAddr> for proto::SocketAddr {
    fn from(addr: SocketAddr) -> Self {
        match addr {
            SocketAddr::V4(v4) => Self {
                addr: Some(proto::socket_addr::Addr::Ipv4(v4.to_string())),
            },
            SocketAddr::V6(v6) => Self {
                addr: Some(proto::socket_addr::Addr::Ipv6(v6.to_string())),
            },
        }
    }
}

impl TryFrom<proto::SocketAddr> for SocketAddr {
    type Error = ConversionError;

    fn try_from(proto: proto::SocketAddr) -> Result<Self, Self::Error> {
        match proto.addr {
            Some(proto::socket_addr::Addr::Ipv4(addr_str)) => {
                addr_str.parse().map_err(|_| {
                    ConversionError::InvalidSocketAddr(format!("Invalid IPv4 address: {}", addr_str))
                })
            }
            Some(proto::socket_addr::Addr::Ipv6(addr_str)) => {
                addr_str.parse().map_err(|_| {
                    ConversionError::InvalidSocketAddr(format!("Invalid IPv6 address: {}", addr_str))
                })
            }
            None => Err(ConversionError::MissingField("socket_addr.addr")),
        }
    }
}

// GateAddr conversions
impl From<hellas_gate_core::GateAddr> for proto::GateAddr {
    fn from(addr: hellas_gate_core::GateAddr) -> Self {
        Self {
            id: Some(addr.id.into()),
            direct_addresses: addr.direct_addresses.into_iter().map(Into::into).collect(),
        }
    }
}

impl TryFrom<proto::GateAddr> for hellas_gate_core::GateAddr {
    type Error = ConversionError;

    fn try_from(proto: proto::GateAddr) -> Result<Self, Self::Error> {
        let id = proto
            .id
            .ok_or(ConversionError::MissingField("gate_addr.id"))?
            .try_into()?;

        let direct_addresses: Result<Vec<SocketAddr>, _> = proto
            .direct_addresses
            .into_iter()
            .map(TryInto::try_into)
            .collect();

        Ok(hellas_gate_core::GateAddr::new(id, direct_addresses?))
    }
}

// JsonValue conversions
impl From<serde_json::Value> for proto::JsonValue {
    fn from(value: serde_json::Value) -> Self {
        Self {
            json: value.to_string(),
        }
    }
}

impl TryFrom<proto::JsonValue> for serde_json::Value {
    type Error = ConversionError;

    fn try_from(proto: proto::JsonValue) -> Result<Self, Self::Error> {
        serde_json::from_str(&proto.json).map_err(Into::into)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gate_id_roundtrip() {
        let original = hellas_gate_core::GateId::from_bytes([42u8; 32]);
        let proto: proto::GateId = original.into();
        let converted: hellas_gate_core::GateId = proto.try_into().unwrap();
        assert_eq!(original.as_bytes(), converted.as_bytes());
    }

    #[test]
    fn test_socket_addr_roundtrip() {
        let original: SocketAddr = "127.0.0.1:8080".parse().unwrap();
        let proto: proto::SocketAddr = original.into();
        let converted: SocketAddr = proto.try_into().unwrap();
        assert_eq!(original, converted);
    }

    #[test]
    fn test_json_value_roundtrip() {
        let original = serde_json::json!({"test": "value", "number": 42});
        let proto: proto::JsonValue = original.clone().into();
        let converted: serde_json::Value = proto.try_into().unwrap();
        assert_eq!(original, converted);
    }
}