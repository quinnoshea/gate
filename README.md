# Gate - P2P AI Inference Network

Secure peer-to-peer AI inference with end-to-end encryption and zero data exposure.

## Quick Start

### Installation
```bash
cargo build --release
```

### Basic Usage

**Start Gate daemon** (the full P2P AI inference network):
```bash
gate daemon
```

**Connect to a peer** for inference:
```bash
gate p2p --peer <node_id>@<ip>:<port> inference --model gpt-4 --message "Hello"
```

**Generate default config**:
```bash
gate config --output gate.json
```

**For Relay**: See `crates/relay/README.md`

### CLI Options
- `-d, --data-dir <PATH>` - Data directory (configs, logs, identity)
- `-l, --log-level <LEVEL>` - Logging level: error, warn, info, debug, trace
- `-t, --timeout <SECS>` - Command timeout (0 = no timeout)

### Logs
Logs are stored in `.state/{daemon,relay,cli}/logs/YYYY-MM-DD`

So, e.g .state/daemon/logs/2025-06-11

### Development Setup

```bash
nix develop  # or direnv allow, or use rustup with rust-toolchain.toml
cargo build
cargo test
cargo clippy
```

## Documentation

- `docs/META.md` - **Start here** - Development workflow and contribution guidelines
- `docs/OVERVIEW.md` - Project overview and business context
- `docs/DESIGN.md` - Technical architecture
- `docs/PLAN.md` - Implementation roadmap with current tasks
