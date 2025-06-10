# Gate - Local LLM Gateway

Secure peer-to-peer LLM inference with end-to-end encryption and zero data exposure.

## Quick Start

### Development Setup

1. Enter the development environment:
```bash
nix develop  # or direnv allow
```

Otherwise use rustup - the `rust-toolchain.toml` file will configure the correct toolchain.

### Development Commands

```bash
cargo build                    # Build the project
cargo test                     # Run tests
cargo clippy                   # Check code quality
cargo fmt                      # Format code
cargo check --workspace       # Check all crates compile
```

## Project Status

**Phase 1: Foundation** - In Progress
- ✅ **P2P Networking**: Basic peer-to-peer communication using Iroh (see `crates/p2p/`)
- 🚧 **Core Types**: Message protocols and data structures
- ⏳ **HTTP API**: OpenAI-compatible endpoints
- ⏳ **Web Frontend**: Management interface

See `docs/META.md` for development workflow and `docs/PLAN.md` for detailed roadmap.

## Documentation

- `docs/META.md` - **Start here** - Development workflow and contribution guidelines
- `docs/OVERVIEW.md` - Project overview and business context
- `docs/DESIGN.md` - Technical architecture
- `docs/PLAN.md` - Implementation roadmap with current tasks
