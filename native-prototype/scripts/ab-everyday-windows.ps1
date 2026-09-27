<#
.SYNOPSIS
  Same-machine A/B (Stage 12): identical phases and measurement windows for the native prototype and the production
  Tauri/WebView2 app. Whole-application process-tree metrics (strict ancestry from the fresh root PID), interval CPU per PID.
  Production runs with an ISOLATED WebView2 profile (WEBVIEW2_USER_DATA_FOLDER) so real user data is never touched.
  Needs an interactive desktop; do not touch mouse/keyboard while it runs. Never clicks anything update-related.
  Phases: P0 (launch+20s) P1 (60s idle) P2 (navigation) P3/T0 (back on Timer, idle) T1 (running, Timer visible)
          T2 (running, other view visible) T3 (running, minimized 15s settle) T5a (restored, running) T4 (paused) T5b (resumed)
#>
param(
    [Parameter(Mandatory)] [ValidateSet('native', 'production')] [string] $Target,
    [string] $Exe = '',
    [string] $ProfileDir = '',
    [string] $OutPrefix = (Join-Path $env:TEMP 'ab'),
    [int] $CpuSeconds = 30
)
. (Join-Path $PSScriptRoot 'win-input.ps1')
. (Join-Path $PSScriptRoot 'win-tree.ps1')
$ErrorActionPreference = 'Continue'
if (-not $Exe) { $Exe = if ($Target -eq 'native') { Join-Path $PSScriptRoot '..\target\release\study-tracker-native-prototype.exe' } else { Join-Path $PSScriptRoot '..\..\desktop\src-tauri\target\release\app.exe' } }
if ($Target -eq 'production' -and -not $ProfileDir) { $ProfileDir = Join-Path $env:TEMP ("ab_prof_" + [guid]::NewGuid().ToString('N').Substring(0, 8)) }

# UI coordinates in client physical pixels at the default window size / 125 % scaling
$ui = if ($Target -eq 'native') {
    @{ Timer = @(394, 164); Other1 = @(697, 164); Other2 = @(536, 164); Exam = @(788, 370); StartPause = @(625, 870); Dismiss = $null; Nav = @('Other1', 'Other2'); OtherVisible = 'Other1' }
} else {
    # Production: Start enters an immersive full-window timer (Pause at 830,1032; "Minimize" at 1803,58 leaves it); in the regular
    # Timer tab the layout has an extra "Exam minutes" row, so Start is at y=1111 and Pause at (564,1111).
    @{ Timer = @(490, 218); Other1 = @(347, 218); Other2 = @(620, 218); Dash = @(200, 218); Exam = @(548, 448); StartPause = @(600, 1111); PauseBtn = @(564, 1111); ExitImmersive = @(1803, 58); Dismiss = @(1818, 147); Nav = @('Other1', 'Other2'); OtherVisible = 'Other1'; Immersive = $true }
}
if ($Target -eq 'native') { $ui.PauseBtn = $ui.StartPause; $ui.Immersive = $false; $ui.Dash = $ui.Other1 }
if ($Target -eq 'production') { New-Item -ItemType Directory -Force $ProfileDir | Out-Null; $env:WEBVIEW2_USER_DATA_FOLDER = $ProfileDir }
if ($Target -eq 'native') { $env:STUDY_NATIVE_VIEW = 'timer' }
$p = Start-Process $Exe -PassThru
$env:WEBVIEW2_USER_DATA_FOLDER = $null; $env:STUDY_NATIVE_VIEW = $null
$h = Get-RootMainWindow $p.Id; Focus-Window $h; $b = Get-ClientBox $h
function Click-Ui($name) { Click-At ($b.X + $ui[$name][0]) ($b.Y + $ui[$name][1]) }
function Shot($tag) { try { Save-ClientShot $h "$OutPrefix`_$tag.png" } catch { } }
$rows = [System.Collections.Generic.List[object]]::new(); $details = [System.Collections.Generic.List[object]]::new()
function Step($tag, $sec) { $m = Measure-TreeDetail $p.Id $sec $tag; $rows.Add(($m | Select-Object * -ExcludeProperty Detail)); foreach ($d in $m.Detail) { $details.Add([pscustomobject]@{ Step = $tag; Pid = $d.Pid; Ppid = $d.Ppid; Name = $d.Name; Role = $d.Role; WorkingSetMB = [math]::Round($d.WorkingSetMB, 1); PrivateWSMB = [math]::Round($d.PrivateWSMB, 1); PrivateBytesMB = [math]::Round($d.PrivateBytesMB, 1); Threads = $d.Threads }) }
    "{0,-34} procs {1,2} thr {2,3} WS {3,6} PrivWS {4,6} PrivB {5,7} cpu {6,6}% gpu {9,5}% | root PrivWS {7,5} cpu {8,5}%" -f $tag, $m.Procs, $m.Threads, $m.WorkingSetMB, $m.PrivateWSMB, $m.PrivateBytesMB, $m.CpuPctOneCore, $m.RootPrivateWSMB, $m.RootCpuPct, $m.GpuPctSum | Write-Host }

Start-Sleep -Seconds 10                                       # settle after first paint (P0 starts ~20 s after launch)
if ($ui.Dismiss) { Click-Ui 'Dismiss'; Start-Sleep -Milliseconds 800 }
Start-Sleep -Seconds 10
Step 'P0 launch+20s' 10
Step 'P1 idle 60s' 60
foreach ($n in $ui.Nav) { Click-Ui $n; Start-Sleep -Milliseconds 2500 }
Click-Ui 'Timer'; Start-Sleep -Seconds 3; Shot 'timer_idle'
Step 'P2 after navigation' 10
Step 'P3/T0 Timer idle' 20
Click-Ui 'Exam'; Start-Sleep -Milliseconds 600
Click-Ui 'StartPause'; $startedAt = Get-Date; Start-Sleep -Seconds 10; Shot 'timer_running'
if ($ui.Immersive) { Step 'T1a running, immersive timer' $CpuSeconds; Click-Ui 'ExitImmersive'; Start-Sleep -Seconds 5; Shot 'timer_regular_running' }
Step 'T1 running, Timer visible' $CpuSeconds
Click-Ui $ui.OtherVisible; Start-Sleep -Seconds 10
Step 'T2 running, other view visible' ($CpuSeconds - 10)
Click-Ui 'Timer'; Start-Sleep -Seconds 2
[void][W32]::ShowWindow($h, 6); $minAt = Get-Date; Start-Sleep -Seconds 15
Step 'T3 running, minimized' $CpuSeconds
[void][W32]::ShowWindow($h, 9); Start-Sleep -Milliseconds 2500; $restoredAt = Get-Date; Shot 'restored'
$expected = [TimeSpan]::FromMinutes(120) - ($restoredAt - $startedAt)
"RESTORED at {0:HH:mm:ss}; started {1:HH:mm:ss}; minimized {2:HH:mm:ss}; expected remaining ~ {3}" -f $restoredAt, $startedAt, $minAt, $expected.ToString('hh\:mm\:ss') | Write-Host
Step 'T5a restored, running' ($CpuSeconds - 10)
Click-Ui 'PauseBtn'; Start-Sleep -Seconds 3; Shot 'paused'
Step 'T4 paused' ($CpuSeconds - 10)
Click-Ui 'PauseBtn'; Start-Sleep -Seconds 3; Shot 'resumed'
Step 'T5b resumed, running' ($CpuSeconds - 10)
Click-Ui 'Other2'; Start-Sleep -Seconds 3                                  # T3b: minimized while a different (static) view is showing
[void][W32]::ShowWindow($h, 6); Start-Sleep -Seconds 15
Step 'T3b running, minimized from other view' $CpuSeconds
[void][W32]::ShowWindow($h, 9); Start-Sleep -Seconds 3
Click-Ui 'Dash'; Start-Sleep -Seconds 10                                   # Dashboard reported separately (production: animated garden)
Step 'D1 running, Dashboard visible' ($CpuSeconds - 10)
$treeAtEnd = @(Get-TreeDetail $p.Id).Count
Stop-Process -Id $p.Id -Force; Start-Sleep -Seconds 2
$rows | Export-Csv "$OutPrefix.steps.csv" -NoTypeInformation -Delimiter ';'; $details | Export-Csv "$OutPrefix.detail.csv" -NoTypeInformation -Delimiter ';'
"restored-check expected remaining: {0}; profile: {1}" -f $expected.ToString('hh\:mm\:ss'), $ProfileDir | Set-Content "$OutPrefix.notes.txt"
Write-Host "done: $OutPrefix.steps.csv"
