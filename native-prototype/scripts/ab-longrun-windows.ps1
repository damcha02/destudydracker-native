<#
.SYNOPSIS
  Long-running timer time series for the production Tauri/WebView2 app (or the native app), Stage 12 A/B.
  Same phase design as long-run-windows.ps1 so the series line up:
    idle (Timer tab, ready) -> timer running, Timer view visible (production: immersive) -> running, other view A
    -> running, other view B -> running, MINIMIZED (from the regular Timer tab) -> restored -> paused.
  Every -SampleSeconds it records whole-process-tree Working Set / Private Working Set / Private Bytes / threads / process count,
  interval CPU (per-PID CPU-time deltas) and GPU engine utilisation. Per-process detail is saved at every sample.
  Production runs with an ISOLATED WebView2 profile; the real user data is never touched. Uses the 120-min Exam preset.
#>
param(
    [Parameter(Mandatory)] [ValidateSet('native', 'production')] [string] $Target,
    [double] $IdleMin = 3, [double] $TimerMin = 12, [double] $OtherAMin = 8, [double] $OtherBMin = 8,
    [double] $MinimizedMin = 15, [double] $RestoredMin = 4, [double] $PausedMin = 6,
    [int] $SampleSeconds = 30,
    [string] $Exe = '',
    [string] $ProfileDir = '',
    [string] $OutPrefix = (Join-Path $env:TEMP 'ablong')
)
. (Join-Path $PSScriptRoot 'win-input.ps1')
. (Join-Path $PSScriptRoot 'win-tree.ps1')
$ErrorActionPreference = 'Continue'
Add-Type -MemberDefinition '[DllImport("kernel32.dll")] public static extern uint SetThreadExecutionState(uint f);' -Name Pwr -Namespace W32x
[void][W32x.Pwr]::SetThreadExecutionState([uint32]2147483651)   # keep display + system awake for the whole run
if (-not $Exe) { $Exe = if ($Target -eq 'native') { Join-Path $PSScriptRoot '..\target\release\study-tracker-native-prototype.exe' } else { Join-Path $PSScriptRoot '..\..\desktop\src-tauri\target\release\app.exe' } }
if ($Target -eq 'production' -and -not $ProfileDir) { $ProfileDir = Join-Path $env:TEMP ("ablong_prof_" + [guid]::NewGuid().ToString('N').Substring(0, 8)) }

# Coordinates in client physical pixels (default window, 125 % scaling). See ab-everyday-windows.ps1 for how they were determined.
$ui = if ($Target -eq 'native') {
    @{ Timer = @(394, 164); OtherA = @(697, 164); OtherB = @(845, 164); Exam = @(788, 370); StartBtn = @(625, 870); PauseBtn = @(625, 870); ExitImmersive = $null; Dismiss = $null }
} else {
    @{ Timer = @(490, 218); OtherA = @(347, 218); OtherB = @(620, 218); Exam = @(548, 448); StartBtn = @(600, 1111); PauseBtn = @(564, 1111); ExitImmersive = @(1803, 58); Dismiss = @(1818, 147) }
}
if ($Target -eq 'production') { New-Item -ItemType Directory -Force $ProfileDir | Out-Null; $env:WEBVIEW2_USER_DATA_FOLDER = $ProfileDir } else { $env:STUDY_NATIVE_VIEW = 'timer' }
$p = Start-Process $Exe -PassThru
$env:WEBVIEW2_USER_DATA_FOLDER = $null; $env:STUDY_NATIVE_VIEW = $null
$h = Get-RootMainWindow $p.Id; Focus-Window $h; $b = Get-ClientBox $h
function Click-Ui($name) { Click-At ($b.X + $ui[$name][0]) ($b.Y + $ui[$name][1]) }
function Snap([string] $tag) { try { if (-not [W32]::IsIconic($h)) { Save-ClientShot $h ("{0}_{1}.png" -f $OutPrefix, $tag) } } catch { } }
$t0 = Get-Date; $rows = [System.Collections.Generic.List[object]]::new(); $detail = [System.Collections.Generic.List[object]]::new(); $phaseLog = [System.Collections.Generic.List[string]]::new()
Start-Sleep -Seconds 12
if ($ui.Dismiss) { Click-Ui 'Dismiss'; Start-Sleep -Milliseconds 800 }
if ($Target -eq 'production') { Click-Ui 'Timer'; Start-Sleep -Seconds 3 }     # production lands on the Dashboard; the idle phase is the Timer tab

function Run-Phase([string] $name, [double] $minutes) {
    $phaseLog.Add(("{0}  start  {1:HH:mm:ss}  (+{2:N1} min)" -f $name, (Get-Date), ((Get-Date) - $t0).TotalMinutes)); Write-Host $phaseLog[-1]
    $end = (Get-Date).AddMinutes($minutes)
    while ((Get-Date) -lt $end) {
        $m = Measure-TreeDetail $p.Id $SampleSeconds $name
        if ($p.HasExited) { throw 'app exited during the run' }
        $rows.Add([pscustomobject]@{
            Phase = $name; ElapsedMin = [math]::Round(((Get-Date) - $t0).TotalMinutes, 2)
            WorkingSetMB = $m.WorkingSetMB; PrivateWSMB = $m.PrivateWSMB; PrivateBytesMB = $m.PrivateBytesMB
            Threads = $m.Threads; Procs = $m.Procs; CpuPctOneCore = $m.CpuPctOneCore; GpuPctSum = $m.GpuPctSum; RootPrivateWSMB = $m.RootPrivateWSMB; RootCpuPct = $m.RootCpuPct
        })
        foreach ($d in $m.Detail) { $detail.Add([pscustomobject]@{ ElapsedMin = $rows[-1].ElapsedMin; Phase = $name; Pid = $d.Pid; Role = $d.Role; WorkingSetMB = [math]::Round($d.WorkingSetMB, 1); PrivateWSMB = [math]::Round($d.PrivateWSMB, 1); PrivateBytesMB = [math]::Round($d.PrivateBytesMB, 1); Threads = $d.Threads }) }
        $rows | Export-Csv "$OutPrefix.csv" -NoTypeInformation -Delimiter ';'
    }
    Snap ($name -replace '[^A-Za-z0-9]', '')
}

Run-Phase 'idle-timer-ready' $IdleMin
Click-Ui 'Exam'; Start-Sleep -Milliseconds 600
Click-Ui 'StartBtn'; $startedAt = Get-Date; $phaseLog.Add("TIMER STARTED {0:HH:mm:ss.fff} (Exam 120:00)" -f $startedAt)
Start-Sleep -Seconds 2
Run-Phase 'running-timer-visible' $TimerMin
if ($ui.ExitImmersive) { Click-Ui 'ExitImmersive'; Start-Sleep -Seconds 3 }
Click-Ui 'OtherA'; Start-Sleep -Seconds 2; Run-Phase 'running-other-view-A' $OtherAMin
Click-Ui 'OtherB'; Start-Sleep -Seconds 2; Run-Phase 'running-other-view-B' $OtherBMin
Click-Ui 'Timer'; Start-Sleep -Seconds 3; Snap 'timer_tab_before_minimize'
[void][W32]::ShowWindow($h, 6); $phaseLog.Add("MINIMIZED {0:HH:mm:ss.fff}" -f (Get-Date))
Run-Phase 'running-minimized' $MinimizedMin
[void][W32]::ShowWindow($h, 9); Start-Sleep -Milliseconds 2500
$expected = [TimeSpan]::FromMinutes(120) - ((Get-Date) - $startedAt); $phaseLog.Add(("RESTORED {0:HH:mm:ss.fff}  expected remaining ~ {1}" -f (Get-Date), $expected.ToString("hh\:mm\:ss")))
Snap 'restored_immediately'
Run-Phase 'running-restored-timer-visible' $RestoredMin
Click-Ui 'PauseBtn'; Start-Sleep -Seconds 2; $phaseLog.Add("PAUSED {0:HH:mm:ss.fff}" -f (Get-Date)); Snap 'paused_state'
Run-Phase 'paused' $PausedMin
$phaseLog.Add("END {0:HH:mm:ss}" -f (Get-Date))
$phaseLog | Set-Content "$OutPrefix.phases.txt"
Stop-Process -Id $p.Id -Force -ErrorAction SilentlyContinue
$rows | Export-Csv "$OutPrefix.csv" -NoTypeInformation -Delimiter ';'; $detail | Export-Csv "$OutPrefix.detail.csv" -NoTypeInformation -Delimiter ';'
Write-Host "done: $OutPrefix.csv"
