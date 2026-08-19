# Generates Reeda launcher/store artwork (PNG raster fallbacks + store assets).
#   - android/res/mipmap-{mdpi,hdpi,xhdpi,xxhdpi,xxxhdpi}/ic_launcher.png
#   - docs/store/icon-512.png (Play Store icon)
#   - docs/store/feature_graphic.png (Play feature graphic, 1024x500)
# The vector source of truth is android/res/drawable/ic_launcher_foreground.xml
# (adaptive icon, API 26+); these PNGs cover pre-adaptive fallbacks and the
# Play Store. Regenerate with: powershell -File scripts/make_icons.ps1
param(
    [switch]$SkipFeatureGraphic
)
$ErrorActionPreference = "Stop"
Add-Type -AssemblyName System.Drawing

$root = Split-Path -Parent $PSScriptRoot
$bgColor = [System.Drawing.Color]::FromArgb(255, 31, 107, 64)   # ic_launcher_background #1F6B40
$fgColor = [System.Drawing.Color]::White

function New-RoundedRectPath([float]$x, [float]$y, [float]$w, [float]$h, [float]$r) {
    $d = $r * 2
    $p = New-Object System.Drawing.Drawing2D.GraphicsPath
    $p.AddArc($x, $y, $d, $d, 180, 90)
    $p.AddArc($x + $w - $d, $y, $d, $d, 270, 90)
    $p.AddArc($x + $w - $d, $y + $h - $d, $d, $d, 0, 90)
    $p.AddArc($x, $y + $h - $d, $d, $d, 90, 90)
    $p.CloseFigure()
    return $p
}

# Open-book glyph, page geometry relative to a 108dp canvas (mirrors the
# vector drawable): pages slope up toward the spine (gap x 0.407..0.593).
function Add-BookGlyph($g, [float]$size, [float]$cx, [float]$cy, [float]$w) {
    $brush = New-Object System.Drawing.SolidBrush($fgColor)
    $pageW = $w * 0.3333
    $left = $cx - $w / 2
    $right = $cx + $w / 2
    $topOuter = $cy - $w * 0.156
    $topInner = $cy - $w * 0.25
    $botInner = $cy + $w * 0.067
    $botOuter = $cy + $w * 0.132
    $ptsL = [System.Drawing.PointF[]]@(
        [System.Drawing.PointF]::new($left, $topOuter),
        [System.Drawing.PointF]::new($left + $pageW, $topInner),
        [System.Drawing.PointF]::new($left + $pageW, $botInner),
        [System.Drawing.PointF]::new($left, $botOuter)
    )
    $ptsR = [System.Drawing.PointF[]]@(
        [System.Drawing.PointF]::new($right - $pageW, $topInner),
        [System.Drawing.PointF]::new($right, $topOuter),
        [System.Drawing.PointF]::new($right, $botOuter),
        [System.Drawing.PointF]::new($right - $pageW, $botInner)
    )
    $g.FillPolygon($brush, $ptsL)
    $g.FillPolygon($brush, $ptsR)
    $brush.Dispose()
}

function New-IconPng([string]$path, [int]$size) {
    $bmp = New-Object System.Drawing.Bitmap($size, $size)
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias
    $g.Clear([System.Drawing.Color]::Transparent)
    $bg = New-Object System.Drawing.SolidBrush($bgColor)
    $radius = [float]$size * 0.22
    $g.FillPath($bg, (New-RoundedRectPath 0 0 $size $size $radius))
    $bg.Dispose()
    Add-BookGlyph $g $size ($size / 2) ($size / 2) ([float]$size * 0.44)
    $g.Dispose()
    $bmp.Save($path, [System.Drawing.Imaging.ImageFormat]::Png)
    $bmp.Dispose()
    Write-Output "wrote $path"
}

$sizes = @{ "mdpi" = 48; "hdpi" = 72; "xhdpi" = 96; "xxhdpi" = 144; "xxxhdpi" = 192 }
foreach ($k in $sizes.Keys) {
    $dir = Join-Path $root "android\res\mipmap-$k"
    New-Item -ItemType Directory -Force -Path $dir | Out-Null
    New-IconPng (Join-Path $dir "ic_launcher.png") $sizes[$k]
}

$storeDir = Join-Path $root "docs\store"
New-Item -ItemType Directory -Force -Path $storeDir | Out-Null
New-IconPng (Join-Path $storeDir "icon-512.png") 512

if (-not $SkipFeatureGraphic) {
    $bmp = New-Object System.Drawing.Bitmap(1024, 500)
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias
    $g.Clear($bgColor)
    Add-BookGlyph $g 1024 280 250 220
    $font = New-Object System.Drawing.Font("Segoe UI", 96, [System.Drawing.FontStyle]::Bold)
    $brush = New-Object System.Drawing.SolidBrush($fgColor)
    $g.DrawString("Reeda", $font, $brush, 430, 175)
    $font2 = New-Object System.Drawing.Font("Segoe UI", 28, [System.Drawing.FontStyle]::Regular)
    $g.DrawString("Read aloud. Highlight. Search.", $font2, $brush, 435, 300)
    $g.Dispose()
    $bmp.Save((Join-Path $storeDir "feature_graphic.png"), [System.Drawing.Imaging.ImageFormat]::Png)
    $bmp.Dispose()
    $font.Dispose(); $font2.Dispose(); $brush.Dispose()
    Write-Output "wrote docs\store\feature_graphic.png"
}