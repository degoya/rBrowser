<#
.SYNOPSIS
    Builds rBrowser in WSL, copies the app folder to %LOCALAPPDATA%\rbrowser-dev and starts it -
    run on the Windows host.

.DESCRIPTION
    The Windows-side twin of scripts/run-windows.sh, for machines whose C: is mounted read-only in
    WSL (then the bash script cannot copy). It is started from the repository on the WSL share:

        powershell -ExecutionPolicy Bypass -File \\wsl.localhost\<distro>\<repo>\scripts\run-windows.ps1

    1. Builds in WSL: `wsl.exe -d <distro> --cd <repo> -- bash -lc 'scripts/run-windows.sh
       --build-only'` (cargo-xwin, the build lock and the job limits of the bash script). It
       assembles the app folder target\x86_64-pc-windows-msvc\<profile>\app in the sandboxed
       layout: CEF's bootstrap.exe as rbrowser.exe, rbrowser.dll, the CEF runtime, locales\ and
       the built shell as ui\ (scripts/lib/cef.sh).
    2. Copies that folder to %LOCALAPPDATA%\rbrowser-dev. Windows never runs rBrowser from
       \\wsl.localhost: the CEF sandbox refuses it (ADR 0001).
    3. Starts it there, detached, with -Arguments.

    With -Package it builds nothing: it unpacks the package of scripts/package-windows.sh (the
    version's artifacts\windows\rbrowser-<version>-windows-x64.zip, or -Zip) into
    %LOCALAPPDATA%\rbrowser-package - that package's folder emptied first - and starts the
    rbrowser.exe in it, as a tester would (RB-010-15). The twin of `run-windows.sh --package`.

.PARAMETER Release
    Release build instead of the dev build.

.PARAMETER NoBuild
    Skip step 1 and take what the last build left.

.PARAMETER Package
    Start the unpacked package instead of a build.

.PARAMETER Zip
    With -Package: the ZIP to unpack. Default: the workspace version's package.

.PARAMETER Distro
    The WSL distribution. Default: read from the script's own \\wsl.localhost\<distro>\ path.

.PARAMETER Arguments
    Passed to rbrowser.exe, e.g. -Arguments '--version'.

.EXAMPLE
    .\run-windows.ps1
.EXAMPLE
    .\run-windows.ps1 -Release -Arguments '--version'
.EXAMPLE
    .\run-windows.ps1 -Package
#>
[CmdletBinding()]
param(
    [switch]$Release,
    [switch]$NoBuild,
    [switch]$Package,
    [string]$Zip,
    [string]$Distro,
    [string[]]$Arguments = @()
)
$ErrorActionPreference = 'Stop'

# The repository on the WSL share and its Linux path, from where this script lies.
$repo = Split-Path -Parent $PSScriptRoot
if ($repo -notmatch '^\\\\(wsl\.localhost|wsl\$)\\([^\\]+)(\\.*)$') {
    throw "run this script from the WSL share (\\wsl.localhost\<distro>\...), not from $repo"
}
if (-not $Distro) { $Distro = $Matches[2] }
$linuxRepo = $Matches[3] -replace '\\', '/'
$directory = if ($Release) { 'release' } else { 'debug' }

# Starts rbrowser.exe in folder $folder, detached, with -Arguments.
function Start-RBrowser([string]$folder) {
    $target = Join-Path $folder 'rbrowser.exe'
    if (-not [System.IO.File]::Exists($target)) {
        throw "$target is missing (removed by an antivirus? see Windows Security > Protection history)"
    }
    Write-Host "==> starting $target $($Arguments -join ' ')"
    if ($Arguments.Count -gt 0) {
        Start-Process -FilePath $target -WorkingDirectory $folder -ArgumentList $Arguments
    } else {
        Start-Process -FilePath $target -WorkingDirectory $folder
    }
    Write-Host '==> started'
}

# Throws when an rbrowser.exe from under $folder runs: its files are locked.
function Assert-NotRunning([string]$folder) {
    $running = Get-Process -Name rbrowser -ErrorAction SilentlyContinue |
        Where-Object { $_.Path -and $_.Path.StartsWith($folder, 'OrdinalIgnoreCase') }
    if ($running) { throw "rbrowser.exe from $folder is running (pid $($running.Id -join ', ')): close it first" }
}

if ($Package) {
    if (-not $Zip) {
        $cargo = [System.IO.File]::ReadAllText((Join-Path $repo 'Cargo.toml'))
        if ($cargo -notmatch '(?s)\[workspace\.package\][^\[]*?\nversion = "([^"]+)"') {
            throw 'Cargo.toml has no [workspace.package] version'
        }
        $Zip = Join-Path $repo "artifacts\windows\rbrowser-$($Matches[1])-windows-x64.zip"
    }
    if (-not (Test-Path -LiteralPath $Zip)) { throw "no package $Zip (scripts/package-windows.sh makes it)" }
    $destination = Join-Path $env:LOCALAPPDATA 'rbrowser-package'
    # The package's own top folder, read from the archive: a ZIP renamed for testing (say
    # rbrowser-0.0.0-<commit>-test-windows-x64.zip) still unpacks to rbrowser-<version>-windows-x64.
    Add-Type -AssemblyName System.IO.Compression.FileSystem
    $archive = [System.IO.Compression.ZipFile]::OpenRead($Zip)
    try { $root = ($archive.Entries[0].FullName -split '[\\/]')[0] } finally { $archive.Dispose() }
    if (-not $root) { throw "$Zip has no top folder" }
    $folder = Join-Path $destination $root
    Assert-NotRunning $destination
    if ([System.IO.Directory]::Exists($folder)) { Remove-Item -Recurse -Force -LiteralPath $folder }
    New-Item -ItemType Directory -Force -Path $destination | Out-Null
    Write-Host "==> unpacking $Zip into $destination"
    Expand-Archive -LiteralPath $Zip -DestinationPath $destination
    Start-RBrowser $folder
    return
}

if (-not $NoBuild) {
    $mode = if ($Release) { '--release ' } else { '' }
    Write-Host "==> building in WSL ($Distro, $linuxRepo)"
    & wsl.exe -d $Distro --cd $linuxRepo -- bash -lc "scripts/run-windows.sh $mode--build-only"
    if ($LASTEXITCODE -ne 0) { throw "the build in WSL failed (exit $LASTEXITCODE)" }
}

$source = Join-Path $repo "target\x86_64-pc-windows-msvc\$directory\app"
$exe = Join-Path $source 'rbrowser.exe'
if (-not (Test-Path $exe)) { throw "no app folder found: $exe (run without -NoBuild)" }

$destination = Join-Path $env:LOCALAPPDATA 'rbrowser-dev'
New-Item -ItemType Directory -Force -Path $destination | Out-Null
Assert-NotRunning $destination

Write-Host "==> copying to $destination"
# .NET instead of Get-ChildItem -Include: the provider's wildcard handling on the \\wsl.localhost
# share is unreliable, System.IO is not.
$copied = 0
foreach ($file in [System.IO.Directory]::GetFiles($source)) {
    $to = Join-Path $destination ([System.IO.Path]::GetFileName($file))
    [System.IO.File]::Copy($file, $to, $true)
    $copied++
}
foreach ($folder in 'locales', 'ui') {
    $from = Join-Path $source $folder
    $to = Join-Path $destination $folder
    if ([System.IO.Directory]::Exists($to)) { Remove-Item -Recurse -Force -LiteralPath $to }
    if ([System.IO.Directory]::Exists($from)) {
        Copy-Item -Recurse -Force -LiteralPath $from -Destination $destination
    }
}
Write-Host "==> copied $copied file(s)"

Start-RBrowser $destination
