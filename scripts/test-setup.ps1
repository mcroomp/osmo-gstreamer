$ErrorActionPreference = 'Stop'
$setup = Join-Path (Split-Path $PSScriptRoot -Parent) 'setup.cmd'
if ([IO.File]::ReadAllText($setup) -match "(?<!`r)`n") {
    throw 'setup.cmd must use CRLF line endings for reliable CMD subroutine calls.'
}
$testRoot = Join-Path ([IO.Path]::GetTempPath()) ("osmo setup & tests ({0})" -f [Guid]::NewGuid())
$environmentNames = @(
    'PATH', 'USERPROFILE', 'LOCALAPPDATA', 'ProgramFiles(x86)', 'VSCMD_ARG_TGT_ARCH',
    'GSTREAMER_ROOT_X86_64', 'PKG_CONFIG_PATH', 'PKG_CONFIG', 'SETUP_TEST_ROOT',
    'SETUP_TEST_PACKAGE', 'SETUP_TEST_EXIT'
)
$originalEnvironment = @{}
$originalLocation = Get-Location
foreach ($name in $environmentNames) {
    $originalEnvironment[$name] = [Environment]::GetEnvironmentVariable($name, 'Process')
}

try {
    $bin = Join-Path $testRoot '.cargo\bin'
    $installedBin = Join-Path $testRoot 'installed tools'
    $vsTools = Join-Path $testRoot 'vs\Common7\Tools'
    $vsInstaller = Join-Path $testRoot 'Microsoft Visual Studio\Installer'
    New-Item -ItemType Directory -Path $bin, $installedBin, $vsTools, $vsInstaller | Out-Null
    Copy-Item $setup (Join-Path $testRoot 'setup.cmd')
    Set-Content (Join-Path $vsTools 'VsDevCmd.bat') '@exit /b 0' -Encoding Ascii

    $source = @'
using System;
using System.IO;

public static class SetupTestTool {
    public static int Main(string[] args) {
        string root = Environment.GetEnvironmentVariable("SETUP_TEST_ROOT");
        string tool = Path.GetFileNameWithoutExtension(Environment.GetCommandLineArgs()[0]);
        string package = Environment.GetEnvironmentVariable("SETUP_TEST_PACKAGE");
        string marker = Path.Combine(root, "installed");
        File.AppendAllText(Path.Combine(root, "trace"), tool + " " + String.Join(" ", args) + "\n");
        if (tool == "powershell") {
            if (Array.IndexOf(args, "-Command") >= 0 && File.Exists(marker))
                Console.WriteLine(Path.Combine(root, "installed tools"));
            return 0;
        }
        if (tool == "where") {
            string requested = args[0];
            if (requested == "pkg-config") return 1;
            bool missing = (requested == "pkgconf" && package == "pkgconf.pkgconf")
                || (requested == "rustup" && package == "Rustlang.Rustup")
                || (requested == "uv" && package == "astral-sh.uv");
            if (missing && !File.Exists(marker)) return 1;
            if (requested == "pkgconf" && missing) {
                string path = Environment.GetEnvironmentVariable("PATH");
                if (Array.IndexOf(path.Split(';'), Path.Combine(root, "installed tools")) < 0)
                    return 1;
            }
            return 0;
        }
        if (tool == "winget") {
            if (package == "pkgconf.pkgconf" && Array.IndexOf(args, "--scope") >= 0)
                return unchecked((int)0x8A150010);
            int result = Int32.Parse(Environment.GetEnvironmentVariable("SETUP_TEST_EXIT"));
            if (result == 0) File.WriteAllText(marker, "");
            return result;
        }
        if (tool == "vswhere") {
            if (package != "Microsoft.VisualStudio.2022.BuildTools" || File.Exists(marker))
                Console.WriteLine(Path.Combine(root, "vs"));
        }
        return 0;
    }
}
'@
    $executable = Join-Path $bin 'where.exe'
    Add-Type -TypeDefinition $source -OutputAssembly $executable -OutputType ConsoleApplication
    foreach ($tool in @('winget', 'rustup', 'uv', 'powershell', 'cl')) {
        Copy-Item $executable (Join-Path $bin "$tool.exe")
    }
    Copy-Item $executable (Join-Path $vsInstaller 'vswhere.exe')

    $env:SETUP_TEST_ROOT = $testRoot
    $env:USERPROFILE = $testRoot
    $env:LOCALAPPDATA = $testRoot
    ${env:ProgramFiles(x86)} = $testRoot
    $env:VSCMD_ARG_TGT_ARCH = ''
    $env:GSTREAMER_ROOT_X86_64 = Join-Path $testRoot 'gstreamer'
    $env:PKG_CONFIG_PATH = Join-Path $testRoot 'existing SDK'
    $testPath = "$bin;$env:SystemRoot\System32"
    Set-Location -LiteralPath $testRoot

    $cases = @(
        @{ Name = 'Existing dependencies'; Package = ''; Code = 0 },
        @{ Name = 'Installing pkgconf and refreshing PATH'; Package = 'pkgconf.pkgconf'; Code = 0 },
        @{ Name = 'Installing Rustup'; Package = 'Rustlang.Rustup'; Code = 0 },
        @{ Name = 'Installing uv'; Package = 'astral-sh.uv'; Code = 0 },
        @{ Name = 'Installing Build Tools'; Package = 'Microsoft.VisualStudio.2022.BuildTools'; Code = 0 },
        @{ Name = 'Positive installer failure'; Package = 'pkgconf.pkgconf'; Code = 1602 },
        @{ Name = 'Negative installer failure'; Package = 'pkgconf.pkgconf'; Code = -1978335216 },
        @{ Name = 'Cancelled installer'; Package = 'pkgconf.pkgconf'; Code = -1978334964 },
        @{ Name = 'Rustup installer failure'; Package = 'Rustlang.Rustup'; Code = -1978334964 },
        @{ Name = 'uv installer failure'; Package = 'astral-sh.uv'; Code = -1978334964 },
        @{ Name = 'Build Tools installer failure'; Package = 'Microsoft.VisualStudio.2022.BuildTools'; Code = -1978334964 }
    )
    foreach ($case in $cases) {
        $env:PATH = $testPath
        $env:SETUP_TEST_PACKAGE = $case.Package
        $env:SETUP_TEST_EXIT = [string]$case.Code
        foreach ($file in @('trace', 'installed')) {
            $path = Join-Path $testRoot $file
            if (Test-Path $path) { Remove-Item $path }
        }
        $output = & $env:ComSpec /d /c 'call setup.cmd 2>&1'
        $result = $LASTEXITCODE
        $trace = Get-Content (Join-Path $testRoot 'trace') -Raw
        $expected = if ($case.Code -eq 0) { 0 } else { 1 }
        if ($result -ne $expected) {
            throw "$($case.Name): expected exit $expected, got $result.`n$(($output | Select-Object -First 180) -join "`n")"
        }
        if ($expected -eq 1 -and $trace -match 'rustup toolchain|uv sync|powershell -NoProfile -ExecutionPolicy') {
            throw "$($case.Name): setup continued after an installer failure.`n$trace"
        }
        if ($expected -eq 0 -and ($trace -notmatch 'uv sync --locked' -or ($output -join "`n") -notmatch 'dependencies are ready')) {
            throw "$($case.Name): setup did not complete.`n$trace"
        }
        if ($expected -eq 0) {
            $output = & $env:ComSpec /d /c '(call setup.cmd && call setup.cmd) 2>&1'
            if ($LASTEXITCODE -ne 0) {
                throw "$($case.Name): rerunning setup failed.`n$($output -join "`n")"
            }
        }
        Write-Host "PASS: $($case.Name)"
    }
}
finally {
    Set-Location -LiteralPath $originalLocation.Path
    foreach ($name in $environmentNames) {
        [Environment]::SetEnvironmentVariable($name, $originalEnvironment[$name], 'Process')
    }
    if (Test-Path $testRoot) { Remove-Item -LiteralPath $testRoot -Recurse -Force }
}
