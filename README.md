# Gate - P2P AI Compute Network

A peer-to-peer AI compute network providing secure, private access to distributed AI inference resources.

## Quick Start

### Prerequisites

- [Nix](https://nixos.org/download.html) with flakes enabled
- Git

### Development Setup

1. Clone and enter the repository:
```bash
git clone <repository-url>
cd private-gate
```

2. Enter the development environment:
```bash
nix develop
```

This provides a consistent Rust toolchain (nightly-2025-06-10) with all required tools.

### Development Commands

```bash
# Build the project
cargo build

# Run tests
cargo test

# Check code quality
cargo clippy

# Format code
cargo fmt

# Check all crates compile
cargo check --workspace
```

### Non-Nix Users

If you prefer using rustup directly, the `rust-toolchain.toml` file will automatically configure the same toolchain version.

## Project Status

Currently in documentation-only phase. See `docs/META.md` for development workflow and contribution guidelines.

## Documentation

- `docs/META.md` - **Start here** - Development workflow and contribution guidelines
- `docs/OVERVIEW.md` - Project overview and business context
- `docs/DESIGN.md` - Technical architecture
- `docs/PLAN.md` - Implementation roadmap with current tasks