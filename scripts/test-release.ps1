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
        PackageCalls = 0
        AlterArchive = $false
        AlterArchiveName = 'osmo-camera'
        OmitArchive = $false
        PackageExitCode = 0
        PublishExitCode = 0
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
        if ($args[0] -eq 'package') {
            if ($args -notcontains '--workspace' -or $args -notcontains '--locked' -or $args -notcontains '--all-features') {
                throw 'Expected workspace packaging to match the audited source artifact.'
            }
            $state.PackageCalls++
            foreach ($name in @('osmo-camera', 'gst-plugin-osmo')) {
                if ($state.OmitArchive -and $name -eq 'osmo-camera') {
                    continue
                }
                $rebuilt = Join-Path 'target' (Join-Path 'package' "$name-$version.crate")
                Copy-Item (Join-Path 'dist' "$name-$version.crate") $rebuilt -Force
                if ($state.AlterArchive -and $name -eq $state.AlterArchiveName) {
                    Add-Content $rebuilt 'different contents'
                }
            }
            $global:LASTEXITCODE = $state.PackageExitCode
        } elseif ($args[0] -eq 'publish' -and -not ($args -contains '--dry-run')) {
            $state.Calls.Add($args[($args.IndexOf('-p') + 1)])
            $global:LASTEXITCODE = $state.PublishExitCode
        } else {
            throw "Unexpected cargo invocation: $args"
        }
    }

    & $publish -Version $version
    if (($state.Calls -join ',') -ne 'osmo-camera,gst-plugin-osmo') {
        throw 'Crates were not published in dependency order.'
    }
    if ($state.PackageCalls -ne 1) {
        throw 'Both crates must be packaged and verified before publishing.'
    }
    $state.Calls.Clear()
    $state.PackageCalls = 0
    $state.Existing['gst-plugin-osmo'] = 'different-checksum'
    Assert-Fails { & $publish -Version $version } '*already exists with different contents*'
    if ($state.Calls.Count -ne 0 -or $state.PackageCalls -ne 0) {
        throw 'A crate was published before preflight rejected an existing version.'
    }
    foreach ($name in @('osmo-camera', 'gst-plugin-osmo')) {
        $state.Existing[$name] = (Get-FileHash (Join-Path 'dist' "$name-$version.crate") -Algorithm SHA256).Hash.ToLowerInvariant()
    }
    & $publish -Version $version
    if ($state.Calls.Count -ne 0 -or $state.PackageCalls -ne 0) {
        throw 'Matching existing crates should not be republished.'
    }
    $state.Existing.Remove('gst-plugin-osmo')
    & $publish -Version $version
    if (($state.Calls -join ',') -ne 'gst-plugin-osmo' -or $state.PackageCalls -ne 1) {
        throw 'A partial release must verify the workspace and publish only the missing crate.'
    }
    $state.Calls.Clear()
    $state.Existing.Clear()
    $state.AlterArchive = $true
    foreach ($name in @('osmo-camera', 'gst-plugin-osmo')) {
        $state.AlterArchiveName = $name
        Assert-Fails { & $publish -Version $version } '*refusing to publish*'
        if ($state.Calls.Count -ne 0) {
            throw 'A crate was published before both archive checksums matched.'
        }
    }
    $state.AlterArchive = $false
    $state.PackageExitCode = 1
    Assert-Fails { & $publish -Version $version } 'Pre-publication packaging and verification failed.'
    if ($state.Calls.Count -ne 0) {
        throw 'A crate was published after packaging failed.'
    }
    $state.PackageExitCode = 0
    $state.OmitArchive = $true
    Remove-Item -LiteralPath (Join-Path 'target' (Join-Path 'package' "osmo-camera-$version.crate"))
    Assert-Fails { & $publish -Version $version } 'Packaging did not produce osmo-camera archive*'
    if ($state.Calls.Count -ne 0) {
        throw 'A crate was published without a verified archive.'
    }
    $state.OmitArchive = $false
    $state.PublishExitCode = 1
    Assert-Fails { & $publish -Version $version } 'Publishing osmo-camera failed.*'
    if (($state.Calls -join ',') -ne 'osmo-camera') {
        throw 'Publication continued after an upload failed.'
    }
    Write-Host 'Release tests passed without registry uploads or hardware.'
} finally {
    Set-Location $originalLocation
    $env:CARGO_REGISTRY_TOKEN = $originalToken
    Remove-Item -LiteralPath $temp -Recurse -Force
}
