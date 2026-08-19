#!/usr/bin/env pwsh
# Desktop performance smoke benchmarks (PERFORMANCE.md §9).
#
# Runs the release-mode benchmark tests that can be measured on a developer
# machine (no Android device needed). The strict device budgets (PERFORMANCE.md
# §1) are measured on a Pixel 6a via `scripts/bench_android.ps1` (P2, needs
# hardware) in M7g.
#
# Usage: ./scripts/bench_desktop.ps1
# Exit code 0 = all budgets met; non-zero = at least one budget exceeded.

$ErrorActionPreference = "Continue"

function Invoke-Bench([string] $Label, [string[]] $Args) {
    Write-Host "`n=== $Label ===" -ForegroundColor Cyan
    & cargo test --release @Args 2>&1
    $code = $LASTEXITCODE
    if ($code -ne 0) {
        Write-Host "FAILED: $Label (exit $code)" -ForegroundColor Red
        exit $code
    }
    Write-Host "OK: $Label" -ForegroundColor Green
}

if (-not $env:PDFIUM_LIBRARY_PATH) {
    $env:PDFIUM_LIBRARY_PATH = Join-Path $PSScriptRoot "..\third_party\pdfium\win-x64\pdfium.dll"
}

Invoke-Bench "Search index build + query p95 (<10 s/100 books, <1 s p95)" @("-p", "reeda-search", "--test", "perf_fixture", "--", "--nocapture")
Invoke-Bench "PDF first raster + cached blit p95 (<250 ms, <8 ms)" @("-p", "reeda-pdf", "--test", "perf_bench", "--", "--nocapture")
Invoke-Bench "EPUB pagination avg + long chapter p95 (<150 ms, <400 ms on device)" @("-p", "reeda-epub", "--test", "perf_bench", "--", "--nocapture")

Write-Host "`nAll desktop performance budgets met." -ForegroundColor Green