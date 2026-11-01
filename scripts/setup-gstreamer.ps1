& {
    $ErrorActionPreference = 'Stop'

    if (-not [Environment]::Is64BitOperatingSystem) {
        throw 'The GStreamer setup requires 64-bit Windows.'
    }

    $pkgConfigCommand = Get-Command pkg-config -ErrorAction SilentlyContinue
    if (-not $pkgConfigCommand) {
        $pkgConfigCommand = Get-Command pkgconf -ErrorAction SilentlyContinue
    }
    if (-not $pkgConfigCommand) {
        throw 'pkg-config or pkgconf is required. Install one and make it available on PATH before running this setup.'
    }

    $installRoot = $env:GSTREAMER_ROOT_X86_64
    if ([string]::IsNullOrWhiteSpace($installRoot)) {
        $installRoot = Join-Path $env:LOCALAPPDATA 'Programs\gstreamer\1.0\msvc_x86_64'
    }

    $version = '1.28.8'
    $pkgConfigPath = Join-Path $installRoot 'lib\pkgconfig'

    $missingPackageFiles = @('gstreamer-1.0.pc', 'gstreamer-base-1.0.pc') |
        Where-Object { -not (Test-Path (Join-Path $pkgConfigPath $_)) }
    if ($missingPackageFiles.Count -gt 0 -or -not (Test-Path (Join-Path $installRoot 'bin\gst-inspect-1.0.exe'))) {
        if (-not (Get-Command winget -ErrorAction SilentlyContinue)) {
            throw 'WinGet is required to install GStreamer. Install or update App Installer, then rerun this script.'
        }

        Write-Host "Installing GStreamer $version with WinGet for the current user, including development files..."
        $installerArguments = "/CURRENTUSER /TYPE=devel /VERYSILENT /NORESTART /DIR=`"$installRoot`""
        & winget install --exact --id gstreamerproject.gstreamer --version $version --scope user --force --accept-package-agreements --accept-source-agreements --override $installerArguments
        if ($LASTEXITCODE -ne 0) {
            throw "WinGet failed to install GStreamer $version (exit code $LASTEXITCODE)."
        }
    }

    foreach ($package in @('gstreamer-1.0', 'gstreamer-base-1.0')) {
        if (-not (Test-Path (Join-Path $pkgConfigPath "$package.pc"))) {
            throw "GStreamer development files were not found under '$pkgConfigPath'. Set GSTREAMER_ROOT_X86_64 to the SDK installation path and rerun."
        }
    }

    $binPath = Join-Path $installRoot 'bin'
    $env:GSTREAMER_ROOT_X86_64 = $installRoot
    $env:PKG_CONFIG = $pkgConfigCommand.Name
    $pkgConfigEntries = @($env:PKG_CONFIG_PATH -split ';' | Where-Object { $_ })
    if ($pkgConfigEntries -notcontains $pkgConfigPath) {
        $env:PKG_CONFIG_PATH = (@($pkgConfigPath) + $pkgConfigEntries) -join ';'
    }

    $pathEntries = @($env:PATH -split ';' | Where-Object { $_ })
    if ($pathEntries -notcontains $binPath) {
        $env:PATH = (@($binPath) + $pathEntries) -join ';'
    }

    foreach ($package in @('gstreamer-1.0', 'gstreamer-base-1.0')) {
        & $pkgConfigCommand.Name --modversion $package
        if ($LASTEXITCODE -ne 0) {
            throw "pkg-config could not resolve $package. Check that pkg-config is installed and GSTREAMER_ROOT_X86_64 points to the SDK."
        }
    }

    & gst-inspect-1.0 --version
    if ($LASTEXITCODE -ne 0) {
        throw 'GStreamer was installed, but gst-inspect-1.0 could not run.'
    }

    Write-Host 'GStreamer is ready in this PowerShell session.'
}
