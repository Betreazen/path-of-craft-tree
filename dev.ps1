# Fast development loop for the Path-of-craft-tree mod.
#   .\dev.ps1 test     rules tests only (seconds, no game build)
#   .\dev.ps1 tree     regenerate data\tree.json from examples\gen_tree.rs
#   .\dev.ps1 data     copy data\tree.json to the dev server: it reloads without any build
#   .\dev.ps1 server   rebuild only the server half
#   .\dev.ps1 client   rebuild only the client half
#   .\dev.ps1 both     rebuild both halves
# Add -BuildProfile performance or release to leave the default dev profile, -Perf for timers.
param(
    [ValidateSet('test', 'tree', 'data', 'server', 'client', 'both')]
    [string]$Part = 'test',
    [ValidateSet('dev', 'performance', 'release')]
    [string]$BuildProfile = 'dev',
    # Compiles in the hot-path timers of perf.rs; they log twice a minute (docs/perf-spec.md).
    [switch]$Perf
)
$ErrorActionPreference = 'Stop'

$HorizonVersion = 'nightly-2026-11-08-fdc87ae'
$Instance = 'path-of-craft-tree-dev'
$Launcher = "$env:LOCALAPPDATA\HorizonLauncher\horizon-launcher.exe"
$ServerTool = "$env:LOCALAPPDATA\Horizon\current\horizon-server.exe"
$DevRoot = "$env:LOCALAPPDATA\Horizon\dev"
$DevServer = "$DevRoot\server"
$Engines = "$DevRoot\engines"
# Optional lock file that keeps heavy jobs on one PC from running at once: PCT_HEAVY_LOCK, or the
# author's shared lock when that folder exists. Without either, builds start straight away.
$Lock = if ($env:PCT_HEAVY_LOCK) { $env:PCT_HEAVY_LOCK }
    elseif (Test-Path 'C:\ai-bridge') { 'C:\ai-bridge\heavy.lock' }
    else { $null }
$Bundle = Join-Path $PSScriptRoot 'mixins\path_of_craft_tree'
$EnginesKept = 3
$CoreShare = 0.9

function Invoke-Tests {
    Push-Location $PSScriptRoot
    try {
        cargo test --quiet
        if ($LASTEXITCODE -ne 0) { throw 'tests failed' }
    } finally {
        Pop-Location
    }
}

function Invoke-TreeGenerator {
    Push-Location $PSScriptRoot
    try {
        cargo run --quiet --example gen_tree
        if ($LASTEXITCODE -ne 0) { throw 'tree generator failed' }
    } finally {
        Pop-Location
    }
}

function Enter-HeavyLock {
    while ($true) {
        try {
            $stream = [IO.File]::Open($Lock, 'CreateNew', 'Write')
            $stamp = (Get-Date).ToUniversalTime().ToString('s')
            $text = [Text.Encoding]::UTF8.GetBytes("path_of_craft_tree dev.ps1 $Part, pid $PID, ${stamp}Z`n")
            $stream.Write($text, 0, $text.Length)
            $stream.Close()
            return
        } catch [IO.IOException] {
            Write-Host "Waiting for $Lock"
            Start-Sleep -Seconds 5
        }
    }
}

function Invoke-Build([string[]]$Arguments, [string]$Name) {
    # The launcher downloads faster without the editor's proxy variables.
    $env:HTTP_PROXY = $null
    $env:HTTPS_PROXY = $null
    # Read at compile time: only dev builds take the tree from the config file.
    $env:PCT_EXTERNAL_TREE = if ($BuildProfile -eq 'dev') { '1' } else { $null }
    $env:PCT_PERF = if ($Perf) { '1' } else { $null }
    $timer = [Diagnostics.Stopwatch]::StartNew()
    $process = Start-Process $Launcher -ArgumentList $Arguments -PassThru -NoNewWindow
    $null = $process.Handle
    $threads = [int][Math]::Floor([Environment]::ProcessorCount * $CoreShare)
    $process.ProcessorAffinity = [IntPtr]((1L -shl $threads) - 1)
    $process.WaitForExit()
    if ($process.ExitCode -ne 0) { throw "$Name build failed with exit code $($process.ExitCode)" }
    Write-Host "$Name built in $([int]$timer.Elapsed.TotalSeconds) s"
}

function Build-Server {
    $output = Join-Path $Engines (Get-Date -Format 'yyyyMMdd-HHmmss')
    New-Item -ItemType Directory -Force $Engines | Out-Null
    Invoke-Build @('native-build', "`"$Bundle`"", '--server-output', $output, '--version', $HorizonVersion,
        '--profile', $BuildProfile, '--trust-native-code') 'Server'
    & $ServerTool --dir $DevServer config set "engine.path=$output\horizon-engine.exe"
    # A config file from an older build would otherwise win over the tree just compiled in.
    Copy-Item (Join-Path $Bundle 'data\tree.json') "$DevServer\config\path_of_craft_tree.json" -Force
    Get-ChildItem $Engines -Directory | Sort-Object Name -Descending | Select-Object -Skip $EnginesKept |
        ForEach-Object {
            try { Remove-Item -LiteralPath $_.FullName -Recurse -Force -ErrorAction Stop } catch { Write-Host "Kept $($_.Name): in use" }
        }
}

function Build-Client {
    Invoke-Build @('native-build', "`"$Bundle`"", '--instance', $Instance, '--version', $HorizonVersion,
        '--profile', $BuildProfile, '--trust-native-code') 'Client'
}

if ($Part -eq 'tree') {
    Invoke-TreeGenerator
    Invoke-Tests
    return
}
Invoke-Tests
if ($Part -eq 'test') { return }
if ($Part -eq 'data') {
    Copy-Item (Join-Path $Bundle 'data\tree.json') "$DevServer\config\path_of_craft_tree.json" -Force
    Write-Host 'Tree copied; the running server picks it up within a few seconds.'
    return
}
if ($Lock) { Enter-HeavyLock }
try {
    if ($Part -in 'server', 'both') { Build-Server }
    if ($Part -in 'client', 'both') { Build-Client }
} finally {
    if ($Lock) { [IO.File]::Delete($Lock) }
}
