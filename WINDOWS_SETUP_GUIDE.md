# Windows Setup Guide for Gate Project Testing

This guide will help you set up the Gate project for testing on Windows 11 Enterprise.

## Prerequisites Installation

### Step 1: Copy Project Files
Since the Windows compatibility changes haven't been pushed to GitHub yet, copy the entire `gate` directory from your Linux development machine to your Windows desktop.

**Recommended location:** `C:\Development\gate\`

### Step 2: Install Rust
1. **Download Rust:**
   - Go to https://rustup.rs/
   - Click "Download rustup-init.exe (64-bit)"
   - Save to your Downloads folder

2. **Install Rust:**
   ```powershell
   # Run PowerShell as Administrator (optional but recommended)
   # Navigate to Downloads and run:
   .\rustup-init.exe
   ```
   - When prompted, choose option `1` (default installation)
   - Wait for installation to complete
   - Press Enter when done

3. **Restart PowerShell/Command Prompt**
   - Close and reopen your terminal for PATH changes to take effect

4. **Verify Installation:**
   ```powershell
   rustc --version
   cargo --version
   rustup --version
   ```
   You should see version information for each command.

### Step 3: Install Visual Studio Build Tools
This is **critical** - the project won't build without proper C++ build tools.

1. **Download Build Tools:**
   - Go to https://visualstudio.microsoft.com/downloads/
   - Scroll down to "Tools for Visual Studio 2022"
   - Download "Build Tools for Visual Studio 2022" (it's free)

2. **Install Build Tools:**
   - Run the installer
   - When the Visual Studio Installer opens:
     - Select "C++ build tools" workload
     - Make sure these individual components are checked:
       - MSVC v143 - VS 2022 C++ x64/x86 build tools
       - Windows 11 SDK (latest version)
       - CMake tools for Visual Studio
   - Click "Install" (this will take 10-15 minutes)

3. **Verify Installation:**
   ```powershell
   # Check if cl.exe (MSVC compiler) is available
   where cl.exe
   
   # If not found, you may need to run this first:
   # "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\Common7\Tools\VsDevCmd.bat"
   ```

### Step 4: Install Node.js (for TailwindCSS)
1. **Download Node.js:**
   - Go to https://nodejs.org/
   - Download the LTS version (currently 20.x.x)

2. **Install Node.js:**
   - Run the installer with default settings
   - Check "Automatically install the necessary tools" if prompted

3. **Verify Installation:**
   ```powershell
   node --version
   npm --version
   ```

### Step 5: Install Git (Optional but Recommended)
1. **Download Git:**
   - Go to https://git-scm.com/download/win
   - Download the 64-bit installer

2. **Install Git:**
   - Run installer with default settings
   - Choose "Use Git from the Windows Command Prompt" when asked

## Project Setup

### Step 6: Navigate to Project Directory
```powershell
# Navigate to where you copied the gate directory
cd C:\Development\gate
```

### Step 7: Install Rust Project Tools
```powershell
# Install required Cargo tools (this will take several minutes)
cargo install trunk
cargo install wasm-bindgen-cli
cargo install cargo-watch
```

### Step 8: Install TailwindCSS
```powershell
# Install TailwindCSS globally
npm install -g tailwindcss
```

### Step 9: Add Windows Targets (if needed)
```powershell
# The project should already have Windows targets configured
# But verify they're installed:
rustup target list --installed

# If Windows targets are missing, install them:
rustup target add x86_64-pc-windows-msvc
rustup target add x86_64-pc-windows-gnu
```

## Testing the Build

### Step 10: Test Core Library
```powershell
# Test the core library first (fastest build)
cargo build --release --package gate-core
```

**Expected Result:** Should complete successfully with no errors.

### Step 11: Test Main Daemon
```powershell
# Test the main gate daemon
cargo build --release --bin gate
```

**Expected Result:** Should create `target\release\gate.exe`

### Step 12: Test TLS Forward Service
```powershell
# Test the TLS forwarding service
cargo build --release --package gate-tlsforward
```

**Expected Result:** Should create `target\release\gate-tlsforward.exe`

### Step 13: Test Frontend Build
```powershell
# Navigate to frontend directory
cd crates\frontend-daemon

# Build frontend (this tests WASM compilation)
trunk build --release

# Return to root
cd ..\..
```

**Expected Result:** Should create `crates\frontend-daemon\dist\` directory with built assets.

### Step 14: Basic Runtime Test
```cmd
# Test that the executable runs
.\target\release\gate.exe --help

# Test TLS forward service
.\target\release\gate-tlsforward.exe --help
```

**Expected Result:** Should show help text for both commands.

## Troubleshooting Common Issues

### Issue: "lib.exe not found" or C++ compiler errors
**Solution:** 
- Ensure Visual Studio Build Tools are installed correctly
- Run this command to set up the build environment:
  ```powershell
  "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\Common7\Tools\VsDevCmd.bat"
  ```
- Then retry the build

### Issue: "trunk: command not found"
**Solution:**
- Ensure `%USERPROFILE%\.cargo\bin` is in your PATH
- Restart PowerShell after installing Cargo tools
- Verify installation: `cargo install --list`

### Issue: TailwindCSS not found during frontend build
**Solution:**
- Verify Node.js installation: `node --version`
- Reinstall TailwindCSS: `npm install -g tailwindcss`
- Check if `tailwindcss` command works: `tailwindcss --help`

### Issue: Antivirus interference
**Solution:**
- Add the `gate` project directory to your antivirus exclusions
- Add `%USERPROFILE%\.cargo\` to exclusions (where Rust tools are stored)

### Issue: Permission denied errors
**Solution:**
- Try running PowerShell as Administrator
- Check that Windows Defender isn't blocking execution

## Testing JWT Secret Creation (Windows-specific)

This tests our Windows file permissions fix:

```powershell
# Create a test to verify JWT secret handling works
$env:RUST_LOG="debug"
.\target\release\gate.exe --help
```

Check if JWT secret files are created correctly with Windows permissions.

## Success Criteria

✅ **All builds complete successfully**
✅ **Both executables run and show help**  
✅ **Frontend builds without errors**
✅ **JWT secret creation works**
✅ **No Windows-specific crashes**

## Reporting Results

After testing, let me know:
1. Which steps completed successfully ✅
2. Any errors encountered ❌  
3. Full error messages if builds fail
4. Performance observations (build times, etc.)

## Next Steps After Successful Testing

Once all tests pass on Windows:
1. Document any additional fixes needed
2. Update the main documentation
3. Prepare for pull request submission

---

**Need help?** If you encounter issues at any step, copy the full error message and let me know which step failed.