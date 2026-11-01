@echo off

set "REPO_ROOT=%~dp0"
call :AddPath "%USERPROFILE%\.cargo\bin"
call :AddPath "%LOCALAPPDATA%\Microsoft\WinGet\Links"

where winget >nul 2>nul
if errorlevel 1 (
    echo ERROR: WinGet is required. Install or update App Installer, then rerun setup.cmd.
    exit /b 1
)

where rustup >nul 2>nul
if errorlevel 1 (
    echo Installing Rustup...
    winget install --exact --id Rustlang.Rustup --scope user --accept-package-agreements --accept-source-agreements
    if errorlevel 1 exit /b 1
    call :AddPath "%USERPROFILE%\.cargo\bin"
    call :AddPath "%LOCALAPPDATA%\Microsoft\WinGet\Links"
)

where uv >nul 2>nul
if errorlevel 1 (
    echo Installing uv...
    winget install --exact --id astral-sh.uv --scope user --accept-package-agreements --accept-source-agreements
    if errorlevel 1 exit /b 1
    call :AddPath "%LOCALAPPDATA%\Microsoft\WinGet\Links"
)

where pkg-config >nul 2>nul
if not errorlevel 1 set "PKG_CONFIG=pkg-config"
if errorlevel 1 (
    where pkgconf >nul 2>nul
    if errorlevel 1 (
        echo Installing pkgconf...
        winget install --exact --id pkgconf.pkgconf --scope user --accept-package-agreements --accept-source-agreements
        if errorlevel 1 exit /b 1
        call :AddPath "%LOCALAPPDATA%\Microsoft\WinGet\Links"
        where pkgconf >nul 2>nul
        if errorlevel 1 (
            echo ERROR: pkgconf installed, but neither pkg-config nor pkgconf is available on PATH.
            exit /b 1
        )
    )
    set "PKG_CONFIG=pkgconf"
)

set "VSWHERE=%ProgramFiles(x86)%\Microsoft Visual Studio\Installer\vswhere.exe"
set "VS_ROOT="
if exist "%VSWHERE%" (
    for /f "usebackq delims=" %%I in (`"%VSWHERE%" -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath`) do set "VS_ROOT=%%I"
)

if not defined VS_ROOT (
    echo Installing Visual Studio 2022 C++ Build Tools...
    winget install --exact --id Microsoft.VisualStudio.2022.BuildTools --accept-package-agreements --accept-source-agreements --override "--wait --quiet --norestart --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
    if errorlevel 1 exit /b 1
    if exist "%VSWHERE%" (
        for /f "usebackq delims=" %%I in (`"%VSWHERE%" -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath`) do set "VS_ROOT=%%I"
    )
)

if not defined VS_ROOT (
    echo ERROR: Visual Studio C++ Build Tools were not found after setup.
    exit /b 1
)

if not exist "%VS_ROOT%\Common7\Tools\VsDevCmd.bat" (
    echo ERROR: VsDevCmd.bat was not found under "%VS_ROOT%".
    exit /b 1
)

if /I not "%VSCMD_ARG_TGT_ARCH%"=="x64" (
    call "%VS_ROOT%\Common7\Tools\VsDevCmd.bat" -no_logo -arch=amd64 -host_arch=amd64
    if errorlevel 1 (
        echo ERROR: Could not initialize the MSVC build environment.
        exit /b 1
    )
)

where cl >nul 2>nul
if errorlevel 1 (
    echo ERROR: MSVC compiler cl.exe is unavailable after initializing Visual Studio.
    exit /b 1
)

echo Installing the pinned Rust toolchain and components...
rustup toolchain install 1.99.1 --profile minimal --component rustfmt --component clippy
if errorlevel 1 exit /b 1

if not defined GSTREAMER_ROOT_X86_64 set "GSTREAMER_ROOT_X86_64=%LOCALAPPDATA%\Programs\gstreamer\1.0\msvc_x86_64"
echo Installing and configuring GStreamer...
powershell -NoProfile -ExecutionPolicy Bypass -File "%REPO_ROOT%scripts\setup-gstreamer.ps1"
if errorlevel 1 (
    echo ERROR: GStreamer setup failed.
    exit /b 1
)

call :AddPkgConfigPath "%GSTREAMER_ROOT_X86_64%\lib\pkgconfig"
call :AddPath "%GSTREAMER_ROOT_X86_64%\bin"

pushd "%REPO_ROOT%"
if errorlevel 1 exit /b 1

echo Syncing the locked Python development environment...
uv sync --locked --project python\unofficial-osmo-camera
if errorlevel 1 (
    popd
    exit /b 1
)

echo.
echo Windows development dependencies are ready.
echo This shell now has MSVC, Rust, Python, pkg-config, and GStreamer configured.
echo Run the build and test commands from README.md as needed.
popd
exit /b 0

:AddPath
set "PATH_ENTRY=%~1"
if not defined PATH_ENTRY exit /b 0
echo ;%PATH%;| findstr.exe /I /L /C:";%PATH_ENTRY%;" >nul
if errorlevel 1 set "PATH=%PATH_ENTRY%;%PATH%"
exit /b 0

:AddPkgConfigPath
set "PKG_CONFIG_ENTRY=%~1"
if not defined PKG_CONFIG_ENTRY exit /b 0
if not defined PKG_CONFIG_PATH (
    set "PKG_CONFIG_PATH=%PKG_CONFIG_ENTRY%"
    exit /b 0
)
echo ;%PKG_CONFIG_PATH%;| findstr.exe /I /L /C:";%PKG_CONFIG_ENTRY%;" >nul
if errorlevel 1 set "PKG_CONFIG_PATH=%PKG_CONFIG_ENTRY%;%PKG_CONFIG_PATH%"
exit /b 0
