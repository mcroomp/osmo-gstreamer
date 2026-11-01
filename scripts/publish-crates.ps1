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
foreach ($name in @('osmo-camera', 'gst-plugin-osmo')) {
    if ($alreadyPublished[$name]) {
        continue
    }
    $checksum = (Get-FileHash (Join-Path 'dist' "$name-$Version.crate") -Algorithm SHA256).Hash.ToLowerInvariant()
    & cargo publish --dry-run --locked --registry crates-io -p $name --all-features
    if ($LASTEXITCODE -ne 0) {
        throw "Pre-publication verification of $name failed."
    }
    $publishedArchive = Join-Path 'target' (Join-Path 'package' "$name-$Version.crate")
    if ((Get-FileHash $publishedArchive -Algorithm SHA256).Hash.ToLowerInvariant() -ne $checksum) {
        throw "$name archive differs from the audited build artifact; refusing to publish."
    }
    & cargo publish --locked --registry crates-io -p $name --all-features
    if ($LASTEXITCODE -ne 0) {
        throw "Publishing $name failed. Check crates.io before retrying; an upload may have succeeded."
    }
}
