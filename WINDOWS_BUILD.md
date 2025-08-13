# Windows Build Instructions

This document describes how to build Gate for Windows.

## Prerequisites

### Option 1: Building on Windows (Recommended)

1. **Install Rust** (if not already installed):
   ```powershell
   # Using rustup-init.exe from https://rustup.rs/
   rustup-init.exe
   ```

2. **Install Visual Studio Build Tools** or **Visual Studio Community**:
   - Download from: https://visualstudio.microsoft.com/downloads/
   - Ensure "C++ build tools" workload is selected
   - This provides the MSVC compiler toolchain required for the `ring` crate

3. **Install Node.js** (for frontend build tools):
   - Download from: https://nodejs.org/
   - Required for TailwindCSS

4. **Install additional tools**:
   ```powershell
   cargo install trunk wasm-bindgen-cli cargo-watch
   npm install -g tailwindcss
   ```

### Option 2: Cross-compilation from Linux (Advanced)

Cross-compilation requires additional setup:

1. **Install Windows target**:
   ```bash
   rustup target add x86_64-pc-windows-msvc
   ```

2. **Install cross-compilation tools**:
   ```bash
   # Option A: Using xwin (easier)
   cargo install xwin
   xwin --accept-license splat --output ~/.xwin
   
   # Option B: Install mingw-w64 (for GNU target)
   sudo apt install mingw-w64
   rustup target add x86_64-pc-windows-gnu
   ```

3. **Configure cargo for cross-compilation**:
   Create or edit `~/.cargo/config.toml`:
   ```toml
   [target.x86_64-pc-windows-msvc]
   linker = "lld-link"
   
   [target.x86_64-pc-windows-gnu]
   linker = "x86_64-w64-mingw32-gcc"
   ```

## Building

### Native Windows Build

```powershell
# Clone and navigate to project
git clone https://github.com/hellas-ai/gate
cd gate

# Build all components
make build

# Or build specific targets
cargo build --release --bin gate
cargo build --release --bin gate-tlsforward
```

### Cross-compilation Build

```bash
# Build for Windows MSVC (requires xwin setup)
make build-windows

# Build for Windows GNU (requires mingw-w64)
make build-windows-gnu

# Or manually specify target
cargo build --release --target x86_64-pc-windows-msvc --bin gate
```

## Frontend Development

The frontend uses WebAssembly and requires additional setup:

```powershell
# Install frontend dependencies
npm install -g tailwindcss

# Build frontend
cd crates/frontend-daemon
trunk build --release

# Or use make targets
make frontend-daemon-build
```

## Desktop GUI (Tauri)

For the desktop application:

```powershell
# Install Tauri CLI
cargo install cargo-tauri

# Build desktop app
cd crates/gui
cargo tauri build

# Or use make target
make gui-build
```

## Known Issues

### Ring Cryptography Library

The `ring` crate requires MSVC on Windows. If you encounter build errors:

1. Ensure Visual Studio Build Tools are installed
2. Use the MSVC target (`x86_64-pc-windows-msvc`) instead of GNU
3. Set the environment variable: `set VCINSTALLDIR=C:\Program Files (x86)\Microsoft Visual Studio\2019\BuildTools\VC\`

### WebKit Dependencies

The Tauri desktop app requires WebView2, which is included in Windows 11 by default but may need to be installed on Windows 10:

- Download WebView2 Runtime: https://developer.microsoft.com/en-us/microsoft-edge/webview2/

## File Permissions

The application handles file permissions differently on Windows:

- Unix systems: Uses `chmod` with octal permissions
- Windows: Uses read-only attribute for sensitive files like JWT secrets

## Build Artifacts

Windows builds will create:

- `target/x86_64-pc-windows-msvc/release/gate.exe` - Main daemon
- `target/x86_64-pc-windows-msvc/release/gate-tlsforward.exe` - TLS forwarding service
- `target/x86_64-pc-windows-msvc/release/gate-gui.exe` - Desktop application (if built with Tauri)

## Troubleshooting

### Common Build Errors

1. **"failed to find tool lib.exe"**:
   - Install Visual Studio Build Tools
   - Ensure MSVC toolchain is selected during installation

2. **"GNU compiler is not supported for this target"**:
   - Use MSVC target instead of GNU: `--target x86_64-pc-windows-msvc`

3. **WebAssembly build failures**:
   - Ensure `wasm32-unknown-unknown` target is installed: `rustup target add wasm32-unknown-unknown`
   - Install `trunk`: `cargo install trunk`

### Performance Optimization

For faster builds on Windows:

1. **Use SSD storage** for the project directory
2. **Exclude from antivirus scanning**: Add project directory to antivirus exclusions
3. **Use cargo cache**: Consider using `sccache` for faster rebuilds

```powershell
cargo install sccache
set RUSTC_WRAPPER=sccache
```

## Testing

Run tests on Windows:

```powershell
# All tests
make test

# Unit tests only
make test-unit

# Integration tests
make test-integration
```