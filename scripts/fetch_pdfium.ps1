# Fetches the prebuilt PDFium binary for the host platform (PDF_SPEC.md §7).
#
# Usage:   powershell -ExecutionPolicy Bypass -File scripts/fetch_pdfium.ps1
# Output:  third_party/pdfium/<platform>/pdfium.dll|libpdfium.so
#          (Windows: bundled next to the app via scripts/package.ps1)
#
# The pinned PDFium release is bblanchon/pdfium-binaries (tag
# `chromium/<sha>`). Re-pin deliberately: bump the tag AND update the
# sha256 for the host asset below.

param(
    [string]$Version = "chromium/7881"
)

$ErrorActionPreference = "Stop"

$root = Join-Path $PSScriptRoot ".."
$outDir = Join-Path $root "third_party\pdfium"
New-Item -ItemType Directory -Force -Path $outDir | Out-Null

$os = if ($IsWindows -or -not $IsLinux -and -not $IsMacOS) { "win" }
       elseif ($IsLinux) { "linux" } else { "mac" }
$arch = if ($env:PROCESSOR_ARCHITECTURE -eq "ARM64" -or $env:PROCESSOR_ARCHITECTURE -eq "ARM") { "arm64" } else { "x64" }

# Only the host dev platform is needed locally; Android .so files are
# handled in build-apk.yml (cargo-apk jniLibs).
switch ("$os-$arch") {
    "win-x64" {
        $libName = "pdfium.dll"
        $url = "https://github.com/bblanchon/pdfium-binaries/releases/download/$Version/pdfium-win-x64.tgz"
        # sha256 of the x64 Windows asset for the pinned release above.
        $expectedSha256 = ""
    }
    "linux-x64" {
        $libName = "libpdfium.so"
        $url = "https://github.com/bblanchon/pdfium-binaries/releases/download/$Version/pdfium-linux-x64.tgz"
        # sha256 of the x64 Linux asset for the pinned release above.
        $expectedSha256 = "1470e21b8b4a3b4ad7f85684e2da11d94f3b69a86d81dee11b9b6709d927ac1d"
    }
    "mac-x64" {
        $libName = "libpdfium.dylib"
        $url = "https://github.com/bblanchon/pdfium-binaries/releases/download/$Version/pdfium-mac-x64.tgz"
        $expectedSha256 = ""
    }
    "mac-arm64" {
        $libName = "libpdfium.dylib"
        $url = "https://github.com/bblanchon/pdfium-binaries/releases/download/$Version/pdfium-mac-arm64.tgz"
        $expectedSha256 = ""
    }
    default { throw "Unsupported host platform: $os-$arch" }
}

$destDir = Join-Path $outDir "$os-$arch"
New-Item -ItemType Directory -Force -Path $destDir | Out-Null
$libPath = Join-Path $destDir $libName

if (Test-Path $libPath) {
    Write-Host "PDFium already present: $libPath"
    exit 0
}

# $env:TEMP is null on Linux (PS Core); GetTempPath works everywhere.
$tmpDir = [System.IO.Path]::GetTempPath()
$tgzPath = Join-Path $tmpDir "pdfium-$os-$arch.tgz"
Write-Host "Downloading $url"
Invoke-WebRequest -Uri $url -OutFile $tgzPath -UseBasicParsing

if ($expectedSha256) {
    $hash = (Get-FileHash -Algorithm SHA256 -Path $tgzPath).Hash.ToLowerInvariant()
    if ($hash -ne $expectedSha256) {
        Remove-Item $tgzPath -Force
        throw "sha256 mismatch: expected $expectedSha256, got $hash"
    }
} else {
    Write-Host "WARNING: no pinned sha256 for this asset; skipping integrity check."
}

# The tgz contains build/<platform>/pdfium.dll etc. tar.gz has no built-in
# Windows support; use tar (available since Windows 10 1803).
$extractDir = Join-Path $tmpDir "pdfium-extract"
Remove-Item -Recurse -Force $extractDir -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force -Path $extractDir | Out-Null
tar -xzf $tgzPath -C $extractDir
if ($LASTEXITCODE -ne 0) { throw "tar extraction failed" }

$found = Get-ChildItem -Path $extractDir -Recurse -Filter $libName | Select-Object -First 1
if (-not $found) { throw "PDFium binary not found in archive" }

Copy-Item $found.FullName $libPath
Remove-Item -Recurse -Force $extractDir -ErrorAction SilentlyContinue
Remove-Item $tgzPath -Force -ErrorAction SilentlyContinue

Write-Host "Installed PDFium: $libPath"
Write-Host ""
Write-Host "Next: add $destDir to the library search path, e.g."
Write-Host "  PowerShell: `$env:PATH = `"$destDir;`$env:PATH`""
Write-Host "  or set PDFIUM_LIBRARY_PATH=$libPath (pdfium-render respects it)"
