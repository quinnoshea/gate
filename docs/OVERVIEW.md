# Private Gate - Project Overview

## Introduction

Private Gate is a core component of the Hellas platform, an AI company building a decentralized web3 protocol for trustless AI compute. While the broader Hellas vision includes a peer-to-peer blockchain network where participants can post solutions to compute graphs defined in our custom array language (catgrad), Private Gate serves as a critical intermediate piece that enables secure, private access to distributed AI inference today.

## What is Private Gate?

Private Gate is a lightweight daemon that runs locally on users' machines, providing secure, peer-to-peer access to AI compute resources. It acts as a bridge between standard AI applications and a distributed network of compute providers, enabling users to access remote GPU resources while maintaining complete control over their data and communications.

### Core Value Proposition

The key selling point of Private Gate is **truly confidential, end-to-end access to your own (or trusted) compute resources**. Unlike traditional cloud AI services where your data passes through third-party servers, Private Gate ensures that:

- Your requests are encrypted end-to-end
- Only you control who can access your compute resources  
- Your private keys never leave your machine
- No intermediary can inspect or log your AI interactions

## Architecture Overview

### Local Daemon
The Private Gate daemon runs locally and provides:
- **OpenAI-compatible API** on `localhost:31145` (supporting endpoints like `/v1/chat/completions`, `/v1/models`)
- **Management interface** on `localhost:8145` for configuration and monitoring via JSON-RPC
- **Peer-to-peer networking** on port 41145 using the Rust Iroh library for secure node communication
- **Identity management** via public/private key pairs (public key serves as network address)
- **Outbound HTTP client** to connect to local inference engines (Ollama, LM Studio, etc.)

### Network Communication
Nodes communicate directly via peer-to-peer connections:
1. Client sends request to local Private Gate daemon (`localhost:31145/v1/chat/completions`)
2. Daemon forwards request over p2p network to trusted remote node
3. Remote node processes request via its local inference engine (e.g., Ollama)
4. Response travels back through the encrypted p2p channel
5. Local daemon returns OpenAI-compatible response to client

### Trust Model
Private Gate operates on an explicit trust model:
- **Permission-based access**: Users maintain local configuration files specifying which public keys are allowed to make requests
- **Signature verification**: All network messages are cryptographically signed
- **No automatic trust**: Unlike blockchain-based systems, trust relationships are explicitly configured by users
- **Future-proof**: Architecture designed to integrate with blockchain-based trust and payment systems

## Public HTTPS Access

To provide standard browser compatibility without compromising security, Private Gate supports public HTTPS endpoints through an innovative relay system:

### Relay Architecture
1. **Subdomain provisioning**: When a node joins the network, it requests a unique subdomain (e.g., `{iroh-address}.private.hellas.ai`)
2. **DNS management**: Relay servers automatically provision DNS records via Cloudflare API
3. **Certificate generation**: Each node generates its own SSL certificate using Let's Encrypt DNS challenges
4. **SNI proxying**: Relay servers perform SNI-based proxying to route HTTPS traffic as raw TCP streams to the appropriate node via the p2p network
5. **Multiple identities**: Nodes can support multiple certificates and subdomains simultaneously

### Security Guarantees
- **Private keys stay local**: SSL certificates are generated on the user's machine
- **No relay MITM**: Relays cannot decrypt traffic as they never possess private keys
- **Browser compatibility**: Standard HTTPS works without custom certificates or application-level encryption
- **Self-hostable**: Users can run their own relay infrastructure if desired

## Software Components

Private Gate consists of several integrated components:

### Core Daemon
- Main p2p node with HTTP API server
- Built in Rust for security, performance, and correctness
- Manages network connections, request routing, and local inference integration

### Management Tools
- **CLI tool**: Command-line interface for configuration and node management
- **GUI application**: User-friendly interface for non-technical users
- **Control protocol**: JSON-RPC interface for programmatic management

### Supporting Infrastructure
- **Relay servers**: DNS and HTTPS proxying infrastructure  
- **Client libraries**: Reusable components for integration with other applications
- **Configuration management**: JSON-based configuration with automatic updates

## Technology Stack

- **Networking**: Iroh library for peer-to-peer communication
- **HTTP server**: Hyper with Tokio async runtime
- **Database**: SQLx with SQLite for local data storage  
- **RPC**: jsonrpsee for control port communication
- **Error handling**: thiserror for libraries, anyhow for applications
- **WASM compatibility**: Key components designed to work in browser/serverless environments

## Open Source and Self-Hosting

Private Gate is fully open source, enabling:
- **Transparency**: Users can audit all code for security and correctness
- **Self-compilation**: Technical users can build from source
- **Self-hosting**: Complete relay infrastructure can be self-hosted
- **Community contributions**: Open development model encouraging community participation

We also provide pre-built binaries and user-friendly installers for non-technical users, ensuring the benefits of decentralized AI compute are accessible to everyone.

## Current Status and Roadmap

### Current Implementation
- Trust-based permission system using local configuration
- OpenAI API compatibility for chat completions
- Peer-to-peer networking with encrypted communications
- Integration with local inference engines (Ollama, LM Studio)

### Future Enhancements
- **Catgrad integration**: Native support for our custom array language compute graphs
- **Blockchain trust**: Integration with Hellas blockchain for automated trust and payments
- **Validity bonding**: Cryptographic proofs of correct computation
- **Advanced routing**: Load balancing and capability-based request routing
- **Expanded API support**: Full OpenAI API compatibility including embeddings, fine-tuning, etc.

## Getting Started

Private Gate is designed to be simple to use while providing powerful capabilities for advanced users. Whether you're looking to:
- Share compute resources between your own devices
- Access trusted friends' GPU resources
- Participate in a commercial compute marketplace
- Build applications requiring private AI inference

Private Gate provides the secure, decentralized infrastructure to make it possible.

For installation instructions and usage examples, see the [README](../README.md).
For technical implementation details, see [IMPLEMENTATION.md](IMPLEMENTATION.md).