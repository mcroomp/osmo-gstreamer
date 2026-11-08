param(
    [Parameter(Mandatory)]
    [string] $Version,
    [Parameter(Mandatory)]
    [ValidateSet('x86_64-pc-windows-msvc')]
    [string] $Target
)

$ErrorActionPreference = 'Stop'
$name = "osmo-gstreamer-$Version-$Target"
$stage = Join-Path 'dist' $name
if (Test-Path $stage) {
    throw "Release staging directory already exists: $stage"
}
New-Item -ItemType Directory -Path (Join-Path $stage 'lib'), (Join-Path $stage 'bin') -Force | Out-Null
$build = Join-Path (Join-Path 'target' $Target) 'release'
$library = 'gstosmo.dll'
$executable = 'osmo-pair.exe'
Copy-Item (Join-Path $build $library) (Join-Path $stage 'lib')
Copy-Item (Join-Path $build $executable) (Join-Path $stage 'bin')
Copy-Item 'LICENSE', 'README.md', 'PROTOCOL.md' $stage
Copy-Item (Join-Path 'dist' 'THIRD-PARTY-LICENSES.txt') $stage

$env:GST_PLUGIN_PATH = (Resolve-Path (Join-Path $stage 'lib')).Path
$env:GST_REGISTRY = Join-Path ([IO.Path]::GetTempPath()) "$name-$([guid]::NewGuid())-registry.bin"
$inspection = & gst-inspect-1.0 osmosrc 2>&1
if ($LASTEXITCODE -ne 0) {
    throw "Packaged plugin inspection failed: $($inspection -join "`n")"
}
$expectedLibrary = (Resolve-Path (Join-Path (Join-Path $stage 'lib') $library)).Path
if (-not (($inspection -join "`n").Contains($expectedLibrary))) {
    throw 'Inspection did not load the staged plugin.'
}
Write-Host ($inspection -join "`n")

$runtime = & gst-inspect-1.0 --version
if ($LASTEXITCODE -ne 0) {
    throw 'Could not identify the GStreamer runtime.'
}
$rustVersion = & rustc --version
if ($LASTEXITCODE -ne 0) {
    throw 'Could not identify the Rust toolchain.'
}
@(
    "Version: $Version"
    "Commit: $env:GITHUB_SHA"
    "Target: $Target"
    "Runner: $env:ImageOS $env:ImageVersion"
    "Rust: $rustVersion"
    ($runtime -join "`n")
    'GStreamer and system libraries are not bundled. Use the recorded runtime version or a separately validated compatible runtime.'
    'Hardware compatibility is not certified. See README.md for authorization and model limitations.'
) | Set-Content (Join-Path $stage 'BUILD-INFO.txt') -Encoding utf8
Compress-Archive -Path $stage -DestinationPath (Join-Path 'dist' "$name.zip")
Remove-Item -LiteralPath $env:GST_REGISTRY -ErrorAction Stop
