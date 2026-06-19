$ErrorActionPreference = 'Stop'
$Root = $PSScriptRoot

Write-Host "[wisp] Building Rust binary..."
Set-Location "$Root\wisp"
cargo build --release

Write-Host "[wisp] Deploying to npm\dist\..."
$Src  = "target\release\wisp.exe"
$Dest = "$Root\npm\dist\wisp.exe"

# On Windows, the running exe is locked — remove first, then copy.
if (Test-Path $Dest) {
    try {
        Remove-Item $Dest -Force
    } catch {
        Write-Host ""
        Write-Host "[wisp] ERROR: Cannot replace npm\dist\wisp.exe — the file is locked."
        Write-Host "       Close any running 'wisp' session and try again."
        exit 1
    }
}
Copy-Item $Src $Dest

Write-Host "[wisp] Done — npm\dist\wisp.exe updated."
