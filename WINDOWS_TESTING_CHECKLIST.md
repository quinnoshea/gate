# Windows Testing Checklist

## Pre-Testing Setup on Windows

### 1. Install Prerequisites
- [ ] Install Rust via rustup-init.exe from https://rustup.rs/
- [ ] Install Visual Studio Build Tools or Visual Studio Community
  - [ ] Ensure "C++ build tools" workload is selected
- [ ] Install Node.js from https://nodejs.org/
- [ ] Install Git for Windows

### 2. Verify Installation
```powershell
# Check Rust installation
rustc --version
cargo --version

# Check if Windows targets are available
rustup target list --installed

# Should show nightly-2025-06-10 toolchain
rustup toolchain list
```

## Build Testing

### 3. Clone and Build
```powershell
# Clone the repository
git clone https://github.com/hellas-ai/gate
cd gate

# Install additional tools
cargo install trunk wasm-bindgen-cli cargo-watch

# Install TailwindCSS
npm install -g tailwindcss

# Test core library build
cargo build --release --package gate-core

# Test main daemon build  
cargo build --release --bin gate

# Test TLS forward service
cargo build --release --bin gate-tlsforward
```

### 4. Frontend Testing
```powershell
# Test frontend builds
cd crates\frontend-daemon
trunk build --release
cd ..\..

cd crates\frontend-tauri  
trunk build --release
cd ..\..
```

### 5. Desktop GUI Testing (if applicable)
```powershell
# Install Tauri CLI
cargo install cargo-tauri

# Test GUI build
cd crates\gui
cargo tauri build
cd ..\..
```

## Runtime Testing

### 6. Basic Functionality
```powershell
# Run the daemon (should start without crashing)
.\target\release\gate.exe --help

# Test basic API endpoints (if daemon starts)
# Check logs for Windows-specific issues
```

### 7. Windows-Specific Features
- [ ] JWT secret file creation (check file permissions)
- [ ] Configuration file handling in Windows paths
- [ ] Service/daemon behavior on Windows
- [ ] File path handling (backslashes vs forward slashes)

## Expected Results

### ✅ Success Criteria
- [ ] All binaries compile without errors
- [ ] `gate.exe` starts and shows help
- [ ] `gate-tlsforward.exe` starts and shows help  
- [ ] JWT secret file is created with proper Windows permissions
- [ ] No Windows-specific crashes or path issues
- [ ] Frontend builds complete successfully

### ❌ Common Issues to Watch For
- [ ] **"lib.exe not found"** - Need Visual Studio Build Tools
- [ ] **Ring crate build failures** - MSVC toolchain issues
- [ ] **Path separator problems** - Backslash vs forward slash
- [ ] **Permission denied** - Windows UAC or antivirus interference
- [ ] **WebView2 missing** - Required for Tauri GUI

## Debugging Commands

If issues occur:
```powershell
# Verbose build output
cargo build --release --bin gate --verbose

# Check specific target compilation
cargo check --target x86_64-pc-windows-msvc

# Environment debugging
set RUST_BACKTRACE=1
set RUST_LOG=debug

# Check Windows-specific dependencies
where lib.exe
where cl.exe
```

## Test Report Template

After testing, document results:

```
## Windows Build Test Results

**Environment:**
- Windows Version: 
- Rust Version: 
- Visual Studio: 
- Hardware: 

**Build Results:**
- [ ] gate-core: ✅/❌
- [ ] gate-daemon: ✅/❌  
- [ ] gate-tlsforward: ✅/❌
- [ ] frontend-daemon: ✅/❌
- [ ] GUI (if tested): ✅/❌

**Runtime Results:**
- [ ] Daemon starts: ✅/❌
- [ ] JWT secret creation: ✅/❌
- [ ] Basic functionality: ✅/❌

**Issues Found:**
- Issue 1: [Description and fix]
- Issue 2: [Description and fix]

**Conclusion:**
Ready for PR: ✅/❌
Needs fixes: [List needed fixes]
```