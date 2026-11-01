$ErrorActionPreference = 'Stop'

function Assert-Fails {
    param([scriptblock] $Action, [string] $Message)
    try {
        & $Action
    } catch {
        if ($_.Exception.Message -notlike $Message) {
            throw
        }
        return
    }
    throw "Expected failure: $Message"
}

$validate = Join-Path $PSScriptRoot 'validate-release.ps1'
$publish = Join-Path $PSScriptRoot 'publish-crates.ps1'
$metadata = (& cargo metadata --locked --no-deps --format-version 1 | ConvertFrom-Json)
if ($LASTEXITCODE -ne 0) {
    throw 'Could not read test metadata.'
}
$version = ($metadata.packages | Where-Object name -EQ 'osmo-camera').version
& $validate -Tag "v$version"
foreach ($tag in @('1.2.3', 'v01.2.3', 'v1.2.3-rc.1', 'v1.2.3;echo bad')) {
    Assert-Fails { & $validate -Tag $tag } 'Release tags must have the form*'
}
Assert-Fails { & $validate -Tag 'v999999.0.0' } 'Tag*must match*'

$originalToken = $env:CARGO_REGISTRY_TOKEN
$originalLocation = Get-Location
$temp = Join-Path ([IO.Path]::GetTempPath()) "osmo-release-tests-$([guid]::NewGuid())"
New-Item -ItemType Directory -Path (Join-Path $temp 'dist'), (Join-Path (Join-Path $temp 'target') 'package') -Force | Out-Null
try {
    Set-Location $temp
    $env:CARGO_REGISTRY_TOKEN = ''
    Assert-Fails { & $publish -Version $version } 'Configure the CARGO_REGISTRY_TOKEN*'
    $env:CARGO_REGISTRY_TOKEN = 'synthetic-test-token'
    $state = @{
        Existing = @{}
        Calls = [Collections.Generic.List[string]]::new()
        AlterArchive = $false
    }
    foreach ($name in @('osmo-camera', 'gst-plugin-osmo')) {
        Set-Content (Join-Path 'dist' "$name-$version.crate") "synthetic archive for $name"
    }
    function Invoke-RestMethod {
        param([string] $Uri, [string] $UserAgent)
        $name = ($Uri -split '/')[-2]
        if ($state.Existing.ContainsKey($name)) {
            return @{ version = @{ checksum = $state.Existing[$name] } }
        }
        $response = [Net.Http.HttpResponseMessage]::new([Net.HttpStatusCode]::NotFound)
        throw [Microsoft.PowerShell.Commands.HttpResponseException]::new('Synthetic 404', $response)
    }
    function cargo {
        $name = $args[($args.IndexOf('-p') + 1)]
        if ($args -contains '--dry-run') {
            Copy-Item (Join-Path 'dist' "$name-$version.crate") (Join-Path 'target' (Join-Path 'package' "$name-$version.crate")) -Force
            if ($state.AlterArchive) {
                Add-Content (Join-Path 'target' (Join-Path 'package' "$name-$version.crate")) 'different contents'
            }
        } else {
            $state.Calls.Add($name)
        }
        $global:LASTEXITCODE = 0
    }

    & $publish -Version $version
    if (($state.Calls -join ',') -ne 'osmo-camera,gst-plugin-osmo') {
        throw 'Crates were not published in dependency order.'
    }
    $state.Calls.Clear()
    $state.Existing['gst-plugin-osmo'] = 'different-checksum'
    Assert-Fails { & $publish -Version $version } '*already exists with different contents*'
    if ($state.Calls.Count -ne 0) {
        throw 'A crate was published before preflight rejected an existing version.'
    }
    foreach ($name in @('osmo-camera', 'gst-plugin-osmo')) {
        $state.Existing[$name] = (Get-FileHash (Join-Path 'dist' "$name-$version.crate") -Algorithm SHA256).Hash.ToLowerInvariant()
    }
    & $publish -Version $version
    if ($state.Calls.Count -ne 0) {
        throw 'Matching existing crates should not be republished.'
    }
    $state.Existing.Clear()
    $state.AlterArchive = $true
    Assert-Fails { & $publish -Version $version } '*refusing to publish*'
    if ($state.Calls.Count -ne 0) {
        throw 'A changed source archive was published.'
    }
    Write-Host 'Release tests passed without registry uploads or hardware.'
} finally {
    Set-Location $originalLocation
    $env:CARGO_REGISTRY_TOKEN = $originalToken
    Remove-Item -LiteralPath $temp -Recurse -Force
}
