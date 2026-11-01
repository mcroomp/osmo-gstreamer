param(
    [Parameter(Mandatory)]
    [string] $Tag
)

$ErrorActionPreference = 'Stop'
if ($Tag -notmatch '^v(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$') {
    throw "Release tags must have the form vX.Y.Z, without leading zeroes: '$Tag'."
}
$version = $Tag.Substring(1)
$metadataJson = & cargo metadata --locked --no-deps --format-version 1
if ($LASTEXITCODE -ne 0) {
    throw 'Could not read locked Cargo metadata.'
}
$metadata = ($metadataJson -join "`n") | ConvertFrom-Json
foreach ($name in @('osmo-camera', 'gst-plugin-osmo')) {
    $package = @($metadata.packages | Where-Object name -EQ $name)
    if ($package.Count -ne 1 -or $package[0].version -ne $version) {
        throw "Tag '$Tag' must match the $name package version."
    }
    if ($package[0].license -ne 'MIT') {
        throw "$name must declare the MIT SPDX license."
    }
    foreach ($field in @('description', 'readme', 'homepage', 'repository', 'documentation')) {
        if ([string]::IsNullOrWhiteSpace($package[0].$field)) {
            throw "$name is missing publication metadata: $field."
        }
    }
    foreach ($field in @('authors', 'keywords', 'categories')) {
        if (@($package[0].$field).Count -eq 0) {
            throw "$name is missing publication metadata: $field."
        }
    }
    $crateLicense = Join-Path (Split-Path $package[0].manifest_path) 'LICENSE'
    if ((Get-Content $crateLicense -Raw).Replace("`r`n", "`n").TrimEnd() -ne (Get-Content 'LICENSE' -Raw).Replace("`r`n", "`n").TrimEnd()) {
        throw "$name LICENSE must match the repository MIT license."
    }
}
$plugin = $metadata.packages | Where-Object name -EQ 'gst-plugin-osmo'
$dependency = $plugin.dependencies | Where-Object name -EQ 'osmo-camera'
if ($dependency.req -ne "=$version") {
    throw "gst-plugin-osmo must depend on osmo-camera with version '=$version'."
}
if ($env:GITHUB_OUTPUT) {
    "version=$version" | Out-File -FilePath $env:GITHUB_OUTPUT -Append -Encoding utf8
}
Write-Host "Validated release $Tag."
