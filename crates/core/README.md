# hellas-gate-core

Core types and utilities shared across all Gate components.

## Contents

This crate will contain:

- **Identity and Cryptography**: Ed25519 keypairs, node IDs, signing/verification
- **Protocol Messages**: P2P message types for node-to-node and node-to-relay communication
- **Configuration Schema**: Structured configuration types with validation
- **Core Data Types**: Node information, capabilities, connection status
- **Error Types**: Shared error definitions for the Gate ecosystem

## Usage

This crate is designed to be used by all other Gate crates as a foundation for shared types and utilities.

```rust
use hellas_gate_core::{Identity, NodeId, Configuration};
```
