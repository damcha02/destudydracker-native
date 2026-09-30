<#
.SYNOPSIS
  Stage 17 runtime checks D17-P0..P6 (Dashboard). Release exe, isolated throw-away data directories,
  synthetic fixtures only - never a real profile. No synthetic mouse/keyboard input: layout, timer
  and navigation state are driven by the app's own diagnostic environment hooks.
  Each scenario reports: process count, Private WS / Private Bytes, CPU % of one core over the
  measured window, frames actually rendered in that window (STUDY_NATIVE_FRAME_STATS), whether
  store.json / the log file grew during the window (viewing must not write), and the dashboard
  recompute lines in the log.
#>
param(
    [string] $Exe = (Join-Path $PSScriptRoot '..\target\release\study-tracker-native-prototype.exe'),
    [int] $Seconds = 20,
    [string] $Only = ''
)
. (Join-Path $PSScriptRoot 'win-metrics.ps1')
Add-Type -TypeDefinition @'
using System; using System.Runtime.InteropServices;
public static class W17 {
    [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int cmd);
    [DllImport("user32.dll")] public static extern bool IsIconic(IntPtr h);
}
'@
$fx = Join-Path $PSScriptRoot '..\tests\fixtures\dashboard'
$results = New-Object System.Collections.Generic.List[object]

function Run-Scenario([string]$Name, [hashtable]$Env, [bool]$Minimize = $false, [int]$Window = $Seconds, [int]$Samples = 0) {
    if ($Only -and $Name -notlike "*$Only*") { return }
    $data = Join-Path $env:TEMP ("st17-" + [Guid]::NewGuid().ToString('N'))
    New-Item -ItemType Directory $data | Out-Null
    $stdout = Join-Path $data 'stdout.txt'
    $saved = @{}
    $all = @{ STUDY_NATIVE_DATA_DIR = $data; STUDY_NATIVE_FRAME_STATS = '1' }
    foreach ($k in $Env.Keys) { $all[$k] = $Env[$k] }
    foreach ($k in $all.Keys) { $saved[$k] = [Environment]::GetEnvironmentVariable($k); [Environment]::SetEnvironmentVariable($k, [string]$all[$k]) }
    $p = Start-Process -FilePath $Exe -PassThru -RedirectStandardOutput $stdout
    foreach ($k in $all.Keys) { [Environment]::SetEnvironmentVariable($k, $saved[$k]) }
    try {
        Start-Sleep -Seconds 4
        $p.Refresh()
        if ($Minimize) { [void][W17]::ShowWindow($p.MainWindowHandle, 6); Start-Sleep -Seconds 1 }
        $store = Join-Path $data 'store.json'
        $storeBefore = if (Test-Path $store) { (Get-Item $store).LastWriteTimeUtc.Ticks } else { 0 }
        $logFile = Get-ChildItem -Path $data -Recurse -Filter 'study-tracker.log' | Select-Object -First 1
        $logBefore = if ($logFile) { $logFile.Length } else { 0 }
        $statsBefore = @(Get-Content $stdout -ErrorAction SilentlyContinue | Where-Object { $_ -like 'STATS*' }).Count
        $m = Measure-Tree $p.Id $Window $Name
        $series = @()
        if ($Samples -gt 1) { $series = @(for ($k = 0; $k -lt $Samples; $k++) { (Measure-Tree $p.Id 15 "$Name#$k").PrivateWSMB }) }
        Start-Sleep -Seconds 1
        $lines = @(Get-Content $stdout -ErrorAction SilentlyContinue | Where-Object { $_ -like 'STATS*' })
        $framesInWindow = 0; $ticksInWindow = 0
        # STATS lines are 10 s intervals; the first (t=10) also covers startup/minimize, so only intervals wholly inside the measured window (t >= 20) are counted.
        foreach ($l in $lines) { if ($l -match 'STATS (d+) frames=(d+) ticks=(d+)' -and [int]$Matches[1] -ge 20) { $framesInWindow += [int]$Matches[2]; $ticksInWindow += [int]$Matches[3] } }
        $storeAfter = if (Test-Path $store) { (Get-Item $store).LastWriteTimeUtc.Ticks } else { 0 }
        $logAfter = if ($logFile) { (Get-Item $logFile.FullName).Length } else { 0 }
        $recomputes = @(if ($logFile) { Select-String -Path $logFile.FullName -Pattern 'dashboard: recomputed' } else { @() }).Count
        $iconic = [W17]::IsIconic($p.MainWindowHandle)
        $results.Add([pscustomobject]@{
            Scenario = $Name; Procs = $m.Procs; PrivWS_MB = $m.PrivateWSMB; PrivBytes_MB = $m.PrivateBytesMB; Cpu_pct_1core = $m.CpuPctOfOneCore
            Window_s = $m.WindowSec; Minimized = $iconic; Frames_in_stats = $framesInWindow; Timer_ticks = $ticksInWindow
            StoreWrittenInWindow = ($storeAfter -ne $storeBefore); LogGrewBytes = ($logAfter - $logBefore); DashboardRecomputes = $recomputes; PrivWS_series = ($series -join ' ')
        })
        if (Test-Path $stdout) { Get-Content $stdout | Where-Object { $_ -like 'DASHBOARD*' -or $_ -like 'NAV*' } | ForEach-Object { Write-Host "  [$Name] $_" } }
        if ($logFile) { Select-String -Path $logFile.FullName -Pattern 'dashboard: recomputed' | Select-Object -Last 3 | ForEach-Object { Write-Host "  [$Name] $($_.Line.Substring($_.Line.IndexOf('dashboard:')))" } }
    } finally {
        if (-not $p.HasExited) { $p.Kill() }
        Start-Sleep -Milliseconds 500
        Remove-Item -Recurse -Force $data -ErrorAction SilentlyContinue
    }
}

$now = '2026-09-30T12:00:00+02:00'
Run-Scenario 'D17-P0 empty, dashboard visible'            @{ STUDY_NATIVE_IMPORT_BACKUP = "$fx\empty.json"; STUDY_NATIVE_NOW = $now; STUDY_NATIVE_DASHBOARD_LAYOUT = 'full' }
Run-Scenario 'D17-P1 realistic, dashboard visible (quiet)' @{ STUDY_NATIVE_IMPORT_BACKUP = "$fx\realistic.json"; STUDY_NATIVE_NOW = $now }
Run-Scenario 'D17-P1b realistic, dashboard visible (full)' @{ STUDY_NATIVE_IMPORT_BACKUP = "$fx\realistic.json"; STUDY_NATIVE_NOW = $now; STUDY_NATIVE_DASHBOARD_LAYOUT = 'full' }
Run-Scenario 'D17-P2 stress dataset, dashboard (full, 1y)' @{ STUDY_NATIVE_GENERATE_SYNTHETIC_ACADEMIC_DATA = '1'; STUDY_NATIVE_DASHBOARD_LAYOUT = 'full'; STUDY_NATIVE_DASHBOARD_RANGE = '365' }
Run-Scenario 'D17-P3 timer running, dashboard visible'     @{ STUDY_NATIVE_IMPORT_BACKUP = "$fx\realistic.json"; STUDY_NATIVE_NOW = $now; STUDY_NATIVE_DASHBOARD_LAYOUT = 'full'; STUDY_NATIVE_TIMER_MODE = '2'; STUDY_NATIVE_TIMER_AUTOSTART = '1' }
Run-Scenario 'D17-P4 timer running, minimized'             @{ STUDY_NATIVE_IMPORT_BACKUP = "$fx\realistic.json"; STUDY_NATIVE_NOW = $now; STUDY_NATIVE_DASHBOARD_LAYOUT = 'full'; STUDY_NATIVE_TIMER_MODE = '2'; STUDY_NATIVE_TIMER_AUTOSTART = '1' } $true
# P5: a 10 s Demo timer completes inside the window; the log must show exactly one extra recompute.
Run-Scenario 'D17-P5 timer completion -> dashboard refresh' @{ STUDY_NATIVE_IMPORT_BACKUP = "$fx\realistic.json"; STUDY_NATIVE_DASHBOARD_LAYOUT = 'full'; STUDY_NATIVE_TIMER_MODE = '3'; STUDY_NATIVE_TIMER_AUTOSTART = '1' } $false 14
# P6: 600 Dashboard<->Timer / Quiet<->Full / range cycles.
Run-Scenario 'D17-P6 navigation stress (600 cycles)'       @{ STUDY_NATIVE_IMPORT_BACKUP = "$fx\realistic.json"; STUDY_NATIVE_NOW = $now; STUDY_NATIVE_NAV_STRESS = '600' } $false 10 6

$results | Select-Object Scenario,Procs,PrivWS_MB,PrivBytes_MB,Cpu_pct_1core,Minimized,Frames_in_stats,Timer_ticks,StoreWrittenInWindow,DashboardRecomputes,PrivWS_series | Format-List | Out-String -Width 300
$results | ConvertTo-Json | Set-Content (Join-Path $env:TEMP 'stage17_perf.json')
