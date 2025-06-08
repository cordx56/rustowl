# DrMemory Setup Script for Windows
# This script downloads and installs DrMemory for memory testing on Windows

param(
    [string]$InstallPath = "$env:USERPROFILE\.drmemory",
    [string]$Version = "2.6.0",
    [switch]$AddToPath,
    [switch]$Force
)

$ErrorActionPreference = "Stop"

# Colors for output
$Red = "Red"
$Green = "Green"
$Yellow = "Yellow"
$Blue = "Blue"

function Write-ColorText {
    param($Text, $Color = "White")
    Write-Host $Text -ForegroundColor $Color
}

function Test-DrMemoryInstalled {
    param($Path)
    return Test-Path (Join-Path $Path "bin\drmemory.exe")
}

function Install-DrMemory {
    param($InstallPath, $Version)
    
    Write-ColorText "Installing DrMemory $Version to $InstallPath..." $Blue
    
    # Create installation directory
    if (!(Test-Path $InstallPath)) {
        New-Item -ItemType Directory -Path $InstallPath -Force | Out-Null
    }
    
    # Download URL
    $DownloadUrl = "https://github.com/DynamoRIO/drmemory/releases/download/release_$Version/DrMemory-Windows-$Version.zip"
    $ZipPath = Join-Path $InstallPath "drmemory.zip"
    
    Write-ColorText "  Downloading from $DownloadUrl..." $Yellow
    
    try {
        Invoke-WebRequest -Uri $DownloadUrl -OutFile $ZipPath -UseBasicParsing
        Write-ColorText "  [OK] Downloaded successfully" $Green
    }
    catch {
        Write-ColorText "  [ERROR] Download failed: $($_.Exception.Message)" $Red
        throw
    }
    
    # Extract archive
    Write-ColorText "  Extracting DrMemory..." $Yellow
    
    try {
        Expand-Archive -Path $ZipPath -DestinationPath $InstallPath -Force
        
        # Find extracted directory and move contents up one level
        $ExtractedDir = Get-ChildItem -Path $InstallPath -Directory | Where-Object { $_.Name -like "DrMemory-*" } | Select-Object -First 1
        
        if ($ExtractedDir) {
            $ExtractedPath = $ExtractedDir.FullName
            
            # Move all contents to the root installation path
            Get-ChildItem -Path $ExtractedPath | ForEach-Object {
                $DestPath = Join-Path $InstallPath $_.Name
                if (Test-Path $DestPath) {
                    Remove-Item $DestPath -Recurse -Force
                }
                Move-Item $_.FullName $InstallPath
            }
            
            # Remove empty extracted directory
            Remove-Item $ExtractedPath -Force
        }
        
        Write-ColorText "  [OK] Extracted successfully" $Green
    }
    catch {
        Write-ColorText "  [ERROR] Extraction failed: $($_.Exception.Message)" $Red
        throw
    }
    finally {
        # Clean up zip file
        if (Test-Path $ZipPath) {
            Remove-Item $ZipPath -Force
        }
    }
    
    # Verify installation
    if (Test-DrMemoryInstalled $InstallPath) {
        Write-ColorText "  [OK] DrMemory installation completed successfully" $Green
        return $true
    }
    else {
        Write-ColorText "  [ERROR] Installation verification failed" $Red
        return $false
    }
}

function Add-ToUserPath {
    param($Path)
    
    Write-ColorText "Adding DrMemory to user PATH..." $Yellow
    
    $BinPath = Join-Path $Path "bin"
    $CurrentPath = [Environment]::GetEnvironmentVariable("PATH", "User")
    
    if ($CurrentPath -notlike "*$BinPath*") {
        $NewPath = "$CurrentPath;$BinPath"
        [Environment]::SetEnvironmentVariable("PATH", $NewPath, "User")
        Write-ColorText "  [OK] Added to user PATH (restart terminal to take effect)" $Green
        
        # Also add to current session
        $env:PATH += ";$BinPath"
        Write-ColorText "  [OK] Added to current session PATH" $Green
    }
    else {
        Write-ColorText "  [INFO] DrMemory bin directory already in PATH" $Yellow
    }
}

function Test-DrMemoryWorking {
    param($InstallPath)
    
    Write-ColorText "Testing DrMemory installation..." $Yellow
    
    $DrMemoryExe = Join-Path $InstallPath "bin\drmemory.exe"
    
    try {
        $Output = & $DrMemoryExe -version 2>&1
        if ($LASTEXITCODE -eq 0) {
            Write-ColorText "  [OK] DrMemory is working correctly" $Green
            Write-ColorText "  Version info: $($Output[0])" $Blue
            return $true
        }
        else {
            Write-ColorText "  [ERROR] DrMemory test failed with exit code $LASTEXITCODE" $Red
            return $false
        }
    }
    catch {
        Write-ColorText "  [ERROR] Failed to run DrMemory: $($_.Exception.Message)" $Red
        return $false
    }
}

function Show-Usage {
    Write-ColorText "DrMemory Setup for Windows" $Blue
    Write-ColorText "=========================" $Blue
    Write-Host ""
    Write-Host "This script downloads and installs DrMemory for memory testing."
    Write-Host ""
    Write-Host "Usage: .\setup-drmemory-windows.ps1 [options]"
    Write-Host ""
    Write-Host "Options:"
    Write-Host "  -InstallPath <path>   Installation directory (default: %USERPROFILE%\.drmemory)"
    Write-Host "  -Version <version>    DrMemory version to install (default: 2.6.0)"
    Write-Host "  -AddToPath           Add DrMemory to user PATH environment variable"
    Write-Host "  -Force               Force reinstallation even if already installed"
    Write-Host ""
    Write-Host "Examples:"
    Write-Host "  .\setup-drmemory-windows.ps1 -AddToPath"
    Write-Host "  .\setup-drmemory-windows.ps1 -InstallPath C:\Tools\DrMemory -Force"
    Write-Host ""
}

# Main execution
try {
    Write-ColorText "=====================================" $Blue
    Write-ColorText "  DrMemory Setup for Windows" $Blue
    Write-ColorText "=====================================" $Blue
    Write-Host ""
    
    # Check if already installed and not forcing
    if ((Test-DrMemoryInstalled $InstallPath) -and !$Force) {
        Write-ColorText "DrMemory is already installed at $InstallPath" $Green
        Write-ColorText "Use -Force to reinstall" $Yellow
        
        # Test if it's working
        if (Test-DrMemoryWorking $InstallPath) {
            if ($AddToPath) {
                Add-ToUserPath $InstallPath
            }
            Write-ColorText "Installation verified successfully!" $Green
            exit 0
        }
        else {
            Write-ColorText "Existing installation is not working properly. Use -Force to reinstall." $Red
            exit 1
        }
    }
    
    # Install DrMemory
    if (Install-DrMemory $InstallPath $Version) {
        # Test installation
        if (Test-DrMemoryWorking $InstallPath) {
            if ($AddToPath) {
                Add-ToUserPath $InstallPath
            }
            
            Write-Host ""
            Write-ColorText "[OK] DrMemory setup completed successfully!" $Green
            Write-Host ""
            Write-ColorText "Installation location: $InstallPath" $Blue
            Write-ColorText "Binary location: $(Join-Path $InstallPath 'bin\drmemory.exe')" $Blue
            Write-Host ""
            
            if ($AddToPath) {
                Write-ColorText "DrMemory has been added to your PATH." $Green
                Write-ColorText "Restart your terminal or PowerShell session for PATH changes to take effect." $Yellow
            }
            else {
                Write-ColorText "To use DrMemory from anywhere, add it to your PATH:" $Yellow
                Write-ColorText "  $((Join-Path $InstallPath 'bin'))" $Blue
                Write-ColorText "Or run this script again with -AddToPath" $Yellow
            }
            
            Write-Host ""
            Write-ColorText "Test DrMemory with: drmemory.exe -version" $Blue
            Write-ColorText "Use with RustOwl: drmemory.exe -- rustowl.exe check [target]" $Blue
        }
        else {
            Write-ColorText "[ERROR] Installation completed but DrMemory is not working properly" $Red
            exit 1
        }
    }
    else {
        Write-ColorText "[ERROR] DrMemory installation failed" $Red
        exit 1
    }
}
catch {
    Write-ColorText "[ERROR] Error during setup: $($_.Exception.Message)" $Red
    exit 1
}
