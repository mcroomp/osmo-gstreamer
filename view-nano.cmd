@echo off
setlocal EnableExtensions DisableDelayedExpansion

set "REPO_ROOT=%~dp0"
set "CAMERA_PROFILE=camera"
set "CHECK_ONLY="
if /I "%~1"=="--check" (
    set "CHECK_ONLY=1"
    shift
)
if not "%~1"=="" (
    set "CAMERA_PROFILE=%~1"
    shift
)
if not "%~1"=="" goto :Usage
if not "%CAMERA_PROFILE:~64,1%"=="" goto :InvalidProfile
for /f "delims=ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789_-" %%I in ("%CAMERA_PROFILE%") do goto :InvalidProfile

pushd "%REPO_ROOT%"
if errorlevel 1 exit /b 1

if not defined GSTREAMER_ROOT_X86_64 set "GSTREAMER_ROOT_X86_64=%LOCALAPPDATA%\Programs\gstreamer\1.0\msvc_x86_64"
set "PATH=%GSTREAMER_ROOT_X86_64%\bin;%PATH%"
set "GST_PLUGIN_PATH=%REPO_ROOT%target\debug;%GST_PLUGIN_PATH%"

if not exist "%GSTREAMER_ROOT_X86_64%\bin\gst-launch-1.0.exe" (
    echo ERROR: GStreamer was not found. Run setup.cmd or set GSTREAMER_ROOT_X86_64.
    goto :Failed
)
if not exist "%GSTREAMER_ROOT_X86_64%\bin\gst-inspect-1.0.exe" (
    echo ERROR: gst-inspect-1.0.exe is missing from the GStreamer installation.
    goto :Failed
)
if not exist "target\debug\gstosmo.dll" (
    echo ERROR: Build the plugin first: cargo build --locked -p gst-plugin-unofficial-osmo
    goto :Failed
)
if not exist ".credentials\%CAMERA_PROFILE%.toml" (
    echo ERROR: The saved camera profile "%CAMERA_PROFILE%" was not found.
    echo Configure it first with osmo-pair --camera %CAMERA_PROFILE%.
    goto :Failed
)

for %%E in (osmosrc h264parse avdec_h264 videoconvert queue autovideosink) do (
    "%GSTREAMER_ROOT_X86_64%\bin\gst-inspect-1.0.exe" %%E >nul
    if errorlevel 1 (
        echo ERROR: Required GStreamer element %%E is unavailable.
        goto :Failed
    )
)
if defined CHECK_ONLY (
    echo Viewer prerequisites are ready. No camera connection was made.
    popd
    exit /b 0
)

echo Opening the live viewer for profile "%CAMERA_PROFILE%".
echo Preview startup writes are required. If LAN pairing is unreachable,
echo startup may use BLE and reapply the saved camera Wi-Fi settings.
choice /C YN /N /M "Authorize camera preview startup? [Y/N] "
if not "%ERRORLEVEL%"=="1" (
    echo Preview cancelled.
    popd
    exit /b 1
)
echo Stop with Ctrl+C to close the camera session.
"%GSTREAMER_ROOT_X86_64%\bin\gst-launch-1.0.exe" osmosrc camera-profile="%CAMERA_PROFILE%" allow-writes=true ! h264parse ! avdec_h264 ! videoconvert ! queue ! autovideosink sync=true
set "VIEW_EXIT_CODE=%ERRORLEVEL%"
if not "%VIEW_EXIT_CODE%"=="0" echo ERROR: The viewer exited with code %VIEW_EXIT_CODE%.
popd
exit /b %VIEW_EXIT_CODE%

:Failed
popd
exit /b 1

:InvalidProfile
echo ERROR: Profile names must contain 1-64 letters, digits, hyphens, or underscores.
exit /b 1

:Usage
echo Usage: view-nano.cmd [--check] [PROFILE]
echo The default profile is camera. --check validates prerequisites without connecting.
exit /b 1
