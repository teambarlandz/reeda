# Builds a portable Windows package for Reeda (M7 packaging decision:
# bundle pdfium.dll next to the exe - no runtime download, works with the
# plain system library search path; Windows DLL search order finds the
# application directory first, so `Pdfium::bind_to_system_library` in
# reeda-pdf picks it up with zero configuration).
#
# Usage: powershell -ExecutionPolicy Bypass -File scripts/package.ps1
# Output:
#   target/release/reeda-ui.exe
#   target/release/pdfium.dll                 (bundled binary)
#   dist/reeda-<version>-win-x64.zip          (portable zip, exe + DLL)

$ErrorActionPreference = "Stop"

$root = Join-Path $PSScriptRoot ".."
$version = (Select-String -Path (Join-Path $root "Cargo.toml") -Pattern '^version\s*=\s*"([^"]+)"' | Select-Object -First 1).Matches.Groups[1].Value
if (-not $version) { throw "could not read workspace version from Cargo.toml" }

Write-Host "Building reeda-ui ($version) in release mode..."
Push-Location $root
try {
    cargo build -p reeda-ui --release
    if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }
} finally {
    Pop-Location
}

$lib = Join-Path $root "third_party\pdfium\win-x64\pdfium.dll"
if (-not (Test-Path $lib)) {
    throw "PDFium binary not found: $lib. Run scripts/fetch_pdfium.ps1 first."
}

$outDir = Join-Path $root "target\release"
$exe = Join-Path $outDir "reeda-ui.exe"
if (-not (Test-Path $exe)) { throw "release binary not found: $exe" }

Copy-Item $lib (Join-Path $outDir "pdfium.dll") -Force
Write-Host "Bundled: $outDir\pdfium.dll"

$distDir = Join-Path $root "dist"
New-Item -ItemType Directory -Force -Path $distDir | Out-Null
$zip = Join-Path $distDir "reeda-$version-win-x64.zip"
Remove-Item $zip -Force -ErrorAction SilentlyContinue
Compress-Archive -Path $exe, (Join-Path $outDir "pdfium.dll") -DestinationPath $zip
Write-Host "Package: $zip"
Write-Host ""
Write-Host "Verify (no PDFIUM_LIBRARY_PATH needed - the DLL sits next to the exe):"
Write-Host "  & '$exe'"