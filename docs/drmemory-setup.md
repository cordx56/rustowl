# DrMemory Setup for Windows

DrMemory is a memory debugging tool for Windows that can detect memory errors, leaks, and other issues in native applications, including Rust programs.

## Quick Setup

### Option 1: Automatic Installation (Recommended)

Run the security script with automatic installation:

```bash
./scripts/security.sh --install
```

Or use the dedicated PowerShell script:

```powershell
# From PowerShell (as Administrator or with appropriate permissions)
Set-ExecutionPolicy -ExecutionPolicy RemoteSigned -Scope CurrentUser
.\scripts\setup-drmemory-windows.ps1 -AddToPath
```

### Option 2: Manual Installation

1. **Download DrMemory**
   - Go to [DrMemory Releases](https://github.com/DynamoRIO/drmemory/releases)
   - Download `DrMemory-Windows-2.6.0.zip` (or latest version)

2. **Extract and Install**
   ```powershell
   # Extract to a directory, e.g., C:\Tools\DrMemory
   Expand-Archive -Path DrMemory-Windows-2.6.0.zip -DestinationPath C:\Tools\
   ```

3. **Add to PATH**
   ```powershell
   # Add to user PATH environment variable
   $env:PATH += ";C:\Tools\DrMemory-Windows-2.6.0\bin"
   
   # Or add permanently through System Properties > Environment Variables
   ```

## Using DrMemory with RustOwl

### Basic Usage

```bash
# Test RustOwl with DrMemory
drmemory -- ./target/security/rustowl.exe check ./perf-tests/dummy-package
```

### With the Security Script

```bash
# Run all security tests including DrMemory
./scripts/security.sh

# Run only DrMemory tests
./scripts/security.sh --no-miri --no-sanitizers --no-audit
```

## DrMemory Options for Rust Programs

DrMemory has several options that are particularly useful for Rust programs:

```bash
# Basic memory error detection
drmemory -- your_program.exe

# More detailed tracking
drmemory -track_heap -count_leaks -- your_program.exe

# Light mode (faster but less comprehensive)
drmemory -light -- your_program.exe

# Generate reports
drmemory -logdir results -- your_program.exe
```

## Common Issues and Solutions

### Issue: "DrMemory not found"

**Solution:** Ensure DrMemory is in your PATH or use the full path:
```bash
C:\Tools\DrMemory-Windows-2.6.0\bin\drmemory.exe -- your_program.exe
```

### Issue: "Access denied" when installing

**Solution:** Run PowerShell as Administrator or install to user directory:
```powershell
.\setup-drmemory-windows.ps1 -InstallPath "$env:USERPROFILE\DrMemory" -AddToPath
```

### Issue: Performance is very slow

**Solution:** Use light mode for faster analysis:
```bash
drmemory -light -- your_program.exe
```

### Issue: Too much output

**Solution:** Use specific options to focus on errors:
```bash
drmemory -quiet -brief -- your_program.exe
```

## Understanding DrMemory Output

DrMemory will report several types of issues:

- **UNADDRESSABLE ACCESS**: Reading/writing memory that wasn't allocated
- **UNINITIALIZED READ**: Reading uninitialized memory
- **INVALID HEAP ARGUMENT**: Invalid arguments to heap functions
- **LEAK**: Memory that was allocated but never freed
- **POSSIBLE LEAK**: Memory that might be leaked

For Rust programs, you're most likely to see:
- Issues in unsafe code blocks
- Problems with FFI (Foreign Function Interface) calls
- Memory leaks in long-running programs

## Integration with CI/CD

You can integrate DrMemory testing into your CI pipeline:

```yaml
# Example for GitHub Actions
- name: Install DrMemory
  run: |
    powershell -ExecutionPolicy Bypass -File scripts/setup-drmemory-windows.ps1 -AddToPath

- name: Run Security Tests
  run: |
    ./scripts/security.sh
```

## Advanced Configuration

### Custom Suppressions

Create a suppressions file to ignore known false positives:

```
# drmemory_suppressions.txt
UNADDRESSABLE ACCESS
name=known_issue_in_library
...
```

Use with:
```bash
drmemory -suppress drmemory_suppressions.txt -- your_program.exe
```

### Detailed Logging

For debugging DrMemory itself:
```bash
drmemory -verbose 2 -logdir detailed_logs -- your_program.exe
```

## Troubleshooting

1. **Verify Installation**
   ```bash
   drmemory -version
   ```

2. **Test with Simple Program**
   ```bash
   drmemory -- rustowl.exe --help
   ```

3. **Check Logs**
   DrMemory creates log files in the current directory or specified logdir.

## Resources

- [DrMemory Documentation](https://drmemory.org/docs/)
- [DrMemory GitHub Repository](https://github.com/DynamoRIO/drmemory)
- [DynamoRIO Project](https://dynamorio.org/)