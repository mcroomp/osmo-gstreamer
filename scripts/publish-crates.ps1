param(
    [Parameter(Mandatory)]
    [string] $Version
)

$ErrorActionPreference = 'Stop'
if ([string]::IsNullOrWhiteSpace($env:CARGO_REGISTRY_TOKEN)) {
    throw 'Configure the CARGO_REGISTRY_TOKEN GitHub Actions secret before publishing.'
}
$alreadyPublished = @{}
foreach ($name in @('osmo-camera', 'gst-plugin-osmo')) {
    $archive = Join-Path 'dist' "$name-$Version.crate"
    $checksum = (Get-FileHash $archive -Algorithm SHA256).Hash.ToLowerInvariant()
    $existing = $null
    try {
        $existing = Invoke-RestMethod -Uri "https://crates.io/api/v1/crates/$name/$Version" -UserAgent 'osmo-gstreamer-release'
    } catch {
        if ($_.Exception.Response.StatusCode -ne [System.Net.HttpStatusCode]::NotFound) {
            throw
        }
    }
    if ($existing) {
        if ($existing.version.checksum -ne $checksum) {
            throw "$name $Version already exists with different contents. Crate versions cannot be overwritten."
        }
        Write-Host "$name $Version is already published with the expected checksum."
        $alreadyPublished[$name] = $true
    }
}
if ($alreadyPublished.Count -eq 2) {
    return
}
# Rebuild with the same command as the audited source artifact; cargo package
# writes to the documented target/package/<name>-<version>.crate location.
& cargo package --locked --workspace --all-features
if ($LASTEXITCODE -ne 0) {
    throw 'Pre-publication packaging and verification failed.'
}
foreach ($name in @('osmo-camera', 'gst-plugin-osmo')) {
    $audited = (Get-FileHash (Join-Path 'dist' "$name-$Version.crate") -Algorithm SHA256).Hash.ToLowerInvariant()
    $rebuilt = Join-Path 'target' (Join-Path 'package' "$name-$Version.crate")
    if (-not (Test-Path -LiteralPath $rebuilt -PathType Leaf)) {
        throw "Packaging did not produce $name archive; refusing to publish."
    }
    if ((Get-FileHash $rebuilt -Algorithm SHA256).Hash.ToLowerInvariant() -ne $audited) {
        throw "$name archive differs from the audited build artifact; refusing to publish."
    }
}
foreach ($name in @('osmo-camera', 'gst-plugin-osmo')) {
    if ($alreadyPublished[$name]) {
        continue
    }
    & cargo publish --locked --registry crates-io -p $name --all-features
    if ($LASTEXITCODE -ne 0) {
        throw "Publishing $name failed. Check crates.io before retrying; an upload may have succeeded."
    }
}
