# Builds a signed release APK for one Android ABI (docs/BUILD_CI.md §3,
# PDF_SPEC.md §7). Uses the NDK clang toolchain for the Rust cdylib, the
# JDK for the Java shim (javac + d8), and the SDK build-tools (aapt2,
# zipalign, apksigner) for packaging with the real android/AndroidManifest.xml.
#
# Usage:  powershell -File scripts/build_apk.ps1 [-Abi arm64-v8a|x86_64]
# Output: dist/reeda-<version>-<abi>.apk (debug-signed)
#
# Requires env: ANDROID_NDK_HOME, ANDROID_SDK_ROOT, JAVA_HOME.
param(
    [ValidateSet("arm64-v8a", "x86_64")]
    [string]$Abi = "arm64-v8a"
)

# Note: NOT "Stop" — cargo-ndk/aapt2 write progress to stderr, which
# PowerShell 5.1 turns into a terminating NativeCommandError under "Stop".
# `$LASTEXITCODE` checks below handle real failures.
$ErrorActionPreference = "Continue"

if (-not $env:ANDROID_NDK_HOME) { throw "ANDROID_NDK_HOME not set" }
if (-not $env:ANDROID_SDK_ROOT) { throw "ANDROID_SDK_ROOT not set" }
if (-not $env:JAVA_HOME) { throw "JAVA_HOME not set" }

# The slint android backend's build script resolves android.jar from these
# (build.rs: "No Android platforms found" otherwise).
if (-not $env:ANDROID_PLATFORM) { $env:ANDROID_PLATFORM = "android-35" }
if (-not $env:ANDROID_JAR) { $env:ANDROID_JAR = "$env:ANDROID_SDK_ROOT\platforms\android-35\android.jar" }

$root = Split-Path -Parent $PSScriptRoot
$buildTools = "$env:ANDROID_SDK_ROOT\build-tools\35.0.0"
$androidJar = "$env:ANDROID_SDK_ROOT\platforms\android-35\android.jar"
$manifest = "$root\android\AndroidManifest.xml"
$resDir = "$root\android\res"
$jniLibs = "$root\android\src\main\jniLibs"
$rustTarget = if ($Abi -eq "arm64-v8a") { "aarch64-linux-android" } else { "x86_64-linux-android" }

$version = (Select-String -Path "$root\Cargo.toml" -Pattern '^version = "([^"]+)"' | Select-Object -First 1).Matches[0].Groups[1].Value
New-Item -ItemType Directory -Force -Path "$root\dist", "$env:TEMP\reeda-apk" | Out-Null
$work = "$env:TEMP\reeda-apk"

Write-Host "== Building Rust cdylib ($rustTarget) =="
cargo ndk -t $Abi -o "$work\ndk" build --release -p reeda-ui --no-default-features --features platform-android
if ($LASTEXITCODE -ne 0) { throw "cargo ndk build failed" }
$so = "$work\ndk\$Abi\libreeda_ui.so"
if (-not (Test-Path $so)) { throw "cdylib not produced: $so" }

Write-Host "== Compiling Java shim (javac + d8) =="
$javaOut = "$work\classes"
Remove-Item -Recurse -Force $javaOut -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force -Path $javaOut | Out-Null
& "$env:JAVA_HOME\bin\javac.exe" --release 11 -classpath $androidJar -d $javaOut "$root\android\src\io\reeda\app\*.java"
if ($LASTEXITCODE -ne 0) { throw "javac failed" }
$dexOut = "$work\dex"
Remove-Item -Recurse -Force $dexOut -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force -Path $dexOut | Out-Null
& cmd /c "`"$buildTools\d8.bat`" --release --lib $androidJar --min-api 26 --output $dexOut $javaOut\io\reeda\app\*.class"
if ($LASTEXITCODE -ne 0) { throw "d8 failed" }

Write-Host "== Packaging with aapt2 =="
$compiledRes = "$work\res.zip"
Remove-Item -Force $compiledRes -ErrorAction SilentlyContinue
& "$buildTools\aapt2.exe" compile --dir $resDir -o $compiledRes
if ($LASTEXITCODE -ne 0) { throw "aapt2 compile failed" }
$unaligned = "$work\reeda-unaligned.apk"
Remove-Item -Force $unaligned -ErrorAction SilentlyContinue
& "$buildTools\aapt2.exe" link -o $unaligned --manifest $manifest -I $androidJar --min-sdk-version 26 --target-sdk-version 35 --version-code 1 --version-name $version --auto-add-overlay $compiledRes
if ($LASTEXITCODE -ne 0) { throw "aapt2 link failed" }

Write-Host "== Adding native libs + dex =="
$stage = "$work\stage"
Remove-Item -Recurse -Force $stage -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force -Path "$stage\lib\$Abi" | Out-Null
Copy-Item $so "$stage\lib\$Abi\libreeda_ui.so" -Force
Copy-Item "$jniLibs\$Abi\libpdfium.so" "$stage\lib\$Abi\libpdfium.so" -Force
Copy-Item "$dexOut\classes.dex" "$stage\classes.dex" -Force
Push-Location $stage
# `aapt add` (v1, still shipped with build-tools) — aapt2 has no `add`.
& "$buildTools\aapt.exe" add $unaligned "lib\$Abi\libreeda_ui.so" "lib\$Abi\libpdfium.so" "classes.dex"
if ($LASTEXITCODE -ne 0) { throw "aapt add failed" }
Pop-Location

Write-Host "== Aligning + signing =="
$aligned = "$work\reeda-aligned.apk"
Remove-Item -Force $aligned -ErrorAction SilentlyContinue
& "$buildTools\zipalign.exe" -f 4 $unaligned $aligned
if ($LASTEXITCODE -ne 0) { throw "zipalign failed" }

$ks = "$work\debug.keystore"
if (-not (Test-Path $ks)) {
    & "$env:JAVA_HOME\bin\keytool.exe" -genkeypair -keystore $ks -alias reeda -keyalg RSA -keysize 2048 -validity 10000 -storepass reeda-debug -keypass reeda-debug -dname "CN=Reeda Debug, OU=Dev, O=Reeda, L=Unknown, ST=Unknown, C=ZZ"
    if ($LASTEXITCODE -ne 0) { throw "keytool failed" }
}
$out = "$root\dist\reeda-$version-$Abi.apk"
Remove-Item -Force $out -ErrorAction SilentlyContinue
& cmd /c "`"$buildTools\apksigner.bat`" sign --ks $ks --ks-key-alias reeda --ks-pass pass:reeda-debug --key-pass pass:reeda-debug --out $out $aligned"
if ($LASTEXITCODE -ne 0) { throw "apksigner failed" }

Write-Host ""
Write-Host "Signed APK: $out"
Get-FileHash $out -Algorithm SHA256 | ForEach-Object { Write-Host "sha256: $($_.Hash.ToLowerInvariant())" }