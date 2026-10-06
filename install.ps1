$ErrorActionPreference = 'Stop'

$Repo = if ($env:GITHUB_REPO) { $env:GITHUB_REPO } else { "cismuc/tx" }
$InstallDir = if ($env:TX_INSTALL_DIR) { $env:TX_INSTALL_DIR } else { "$HOME\.local\bin" }

# Detect architecture
$Arch = "x86_64"
try {
    if ([System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture -eq [System.Runtime.InteropServices.Architecture]::Arm64) {
        $Arch = "arm64"
    }
} catch {
    # Fallback for older PowerShell versions
    if ($env:PROCESSOR_ARCHITECTURE -eq "ARM64") {
        $Arch = "arm64"
    }
}

$AssetName = "tx-windows-$Arch.exe"
$DownloadUrl = "https://github.com/$Repo/releases/latest/download/$AssetName"
$TargetFile = Join-Path $InstallDir "tx.exe"

Write-Host "Installing tx (windows-$Arch)..."

if (-not (Test-Path $InstallDir)) {
    New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
}

$TempFile = [System.IO.Path]::GetTempFileName()
try {
    Invoke-WebRequest -Uri $DownloadUrl -OutFile $TempFile -UseBasicParsing
    Move-Item -Path $TempFile -Destination $TargetFile -Force
} finally {
    if (Test-Path $TempFile) {
        Remove-Item $TempFile -Force -ErrorAction SilentlyContinue
    }
}

Write-Host "Installed tx to $TargetFile"

# Ensure InstallDir is in User PATH
$UserPath = [Environment]::GetEnvironmentVariable("PATH", [EnvironmentVariableTarget]::User)
$PathEntries = if ($UserPath) { $UserPath -split ';' } else { @() }

if ($PathEntries -notcontains $InstallDir) {
    Write-Host "Adding $InstallDir to User PATH..."
    $NewPath = if ($UserPath) { "$UserPath;$InstallDir" } else { $InstallDir }
    [Environment]::SetEnvironmentVariable("PATH", $NewPath, [EnvironmentVariableTarget]::User)
    $env:PATH = "$env:PATH;$InstallDir"
    Write-Host "PATH updated successfully."
}

# Verify execution
try {
    & $TargetFile --version
} catch {
    Write-Host "Installation complete. Restart your terminal to use 'tx'."
}
