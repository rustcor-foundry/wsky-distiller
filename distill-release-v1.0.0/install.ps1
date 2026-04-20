Write-Host "Installing distill v1.0.0 ecosystem..." -ForegroundColor Green

# Build
Write-Host "Building distill..." -ForegroundColor Cyan
cargo build --release --manifest-path ..\distill\Cargo.toml

Write-Host "Building distill-render..." -ForegroundColor Cyan
cargo build --release --manifest-path ..\distill-render\Cargo.toml

Write-Host "Building distill-gui..." -ForegroundColor Cyan
cargo build --release --manifest-path ..\distill-gui\Cargo.toml

# Install
$InstallDir = "$env:LOCALAPPDATA\distill\bin"
New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null

Copy-Item ..\distill\target\release\distill.exe $InstallDir -Force
Copy-Item ..\distill-render\target\release\distill-render.exe $InstallDir -Force
Copy-Item ..\distill-gui\target\release\distill-gui.exe $InstallDir -Force

Write-Host "✅ Installed to $InstallDir" -ForegroundColor Green
Write-Host ""
Write-Host "Add $InstallDir to your PATH to use the tools from anywhere." -ForegroundColor Yellow
Write-Host "Run 'distill-gui' to start the GUI." -ForegroundColor Yellow
