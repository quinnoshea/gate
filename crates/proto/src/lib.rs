//! Gate protocol buffer definitions and type conversions
//! 
//! This crate contains the protobuf schema definitions for the Gate P2P network
//! and provides type conversions between protobuf types and core Gate types.

// Generated protobuf code
pub mod pb;

// Type conversions
pub mod convert;

// Protocol version constants
pub const CONTROL_PROTOCOL_V1: &[u8] = b"/gate/control/v1";
pub const RELAY_PROTOCOL_V1: &[u8] = b"/gate/relay/v1";
pub const INFERENCE_PROTOCOL_V1: &[u8] = b"/gate/inference/v1";