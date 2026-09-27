<#
.SYNOPSIS
  Long-running everyday-efficiency test (Stage 12). ONE app session, sampled as a time series.
  Phases (minutes are parameters):  idle (Timer ready) -> timer running, Timer visible -> running, Dashboard visible
  -> running, Map visible -> running, MINIMIZED -> restored (Timer visible) -> paused.
  Uses the 120-minute Exam preset so the countdown cannot complete mid-run.
  Every -SampleSeconds it appends: phase, elapsed, WorkingSet, PrivateWS, PrivateBytes, threads, processes and
  interval CPU (% of one core). Stdout of the app (STUDY_NATIVE_FRAME_STATS) goes to <OutPrefix>.stats.txt:
  frames rendered and Rust timer ticks per 10 s. Screenshots at phase boundaries: <OutPrefix>_<phase>.png.
  The display is kept awake (SetThreadExecutionState) so display sleep does not confound results.
  Do not touch the mouse/keyboard during phase transitions; do not run builds/other heavy work concurrently.
#>
param(
    [double] $IdleMin = 3, [double] $TimerMin = 12, [double] $DashMin = 8, [double] $MapMin = 8,
    [double] $MinimizedMin = 15, [double] $RestoredMin = 4, [double] $PausedMin = 6,
    [int] $SampleSeconds = 30,
    [string] $OutPrefix = (Join-Path $env:TEMP 'longrun'),
    [string] $Exe = (Join-Path $PSScriptRoot '..\target\release\study-tracker-native-prototype.exe')
)
. (Join-Path $PSScriptRoot 'win-input.ps1')
. (Join-Path $PSScriptRoot 'win-metrics.ps1')
Add-Type -MemberDefinition '[DllImport("kernel32.dll")] public static extern uint SetThreadExecutionState(uint f);' -Name Pwr -Namespace W32x
[void][W32x.Pwr]::SetThreadExecutionState([uint32]2147483651)  # CONTINUOUS | DISPLAY | SYSTEM

$ErrorActionPreference = 'Continue'
$env:STUDY_NATIVE_VIEW = 'timer'; $env:STUDY_NATIVE_FRAME_STATS = '1'
$p = Start-Process $Exe -PassThru -RedirectStandardOutput "$OutPrefix.stats.txt" -RedirectStandardError "$OutPrefix.stderr.txt"
Remove-Item Env:\STUDY_NATIVE_VIEW, Env:\STUDY_NATIVE_FRAME_STATS
$h = Get-AppWindow $p.Id; Focus-Window $h; $b = Get-ClientBox $h
$t0 = Get-Date; $rows = [System.Collections.Generic.List[object]]::new(); $phaseLog = [System.Collections.Generic.List[string]]::new()
$prev = Get-TreeSnapshot $p.Id

# Screenshot only when the window is not minimized (a minimized client area is 0x0).
function Snap([string] $tag) { try { if (-not [W32]::IsIconic($h)) { Save-ClientShot $h ("{0}_{1}.png" -f $OutPrefix, $tag) } } catch { Write-Host "screenshot skipped: $_" } }
function Run-Phase([string] $name, [double] $minutes) {
    $phaseLog.Add(("{0}  start  {1:HH:mm:ss}  (+{2:N1} min)" -f $name, (Get-Date), ((Get-Date) - $t0).TotalMinutes)) ; Write-Host $phaseLog[-1]
    $end = (Get-Date).AddMinutes($minutes)
    while ((Get-Date) -lt $end) {
        Start-Sleep -Seconds $SampleSeconds
        if ($p.HasExited) { throw 'app exited during the run' }
        $s = Get-TreeSnapshot $p.Id
        $wall = ($s.At - $prev.At) / [Diagnostics.Stopwatch]::Frequency
        $cpu = ($s.Cpu - $prev.Cpu).TotalSeconds / $wall * 100
        $rows.Add([pscustomobject]@{
            Phase = $name; ElapsedMin = [math]::Round(((Get-Date) - $t0).TotalMinutes, 2)
            WorkingSetMB = [math]::Round($s.WorkingSet, 1); PrivateWSMB = [math]::Round($s.PrivateWS, 1); PrivateBytesMB = [math]::Round($s.PrivateBytes, 1)
            Threads = $s.Threads; Procs = $s.Procs; CpuPctOneCore = [math]::Round($cpu, 2)
        })
        $prev = $s
        $rows | Export-Csv "$OutPrefix.csv" -NoTypeInformation
    }
    Snap ($name -replace '[^A-Za-z0-9]', '')
}
function Tab([int] $x) { Click-At ($b.X + $x) ($b.Y + 164); Start-Sleep -Milliseconds 800 }

Run-Phase 'idle-timer-ready' $IdleMin
Click-At ($b.X + 788) ($b.Y + 370); Start-Sleep -Milliseconds 500      # Exam (120 min)
Click-At ($b.X + 625) ($b.Y + 870); Start-Sleep -Milliseconds 500      # Start
$startedAt = Get-Date; $phaseLog.Add("TIMER STARTED {0:HH:mm:ss.fff} (Exam 120:00)" -f $startedAt)
Run-Phase 'running-timer-visible' $TimerMin
Tab 697; Run-Phase 'running-dashboard-visible' $DashMin
Tab 845; Run-Phase 'running-map-visible' $MapMin
Tab 394
$beforeMin = Get-Date
[void][W32]::ShowWindow($h, 6)                                          # SW_MINIMIZE
$phaseLog.Add("MINIMIZED {0:HH:mm:ss.fff}" -f (Get-Date))
Run-Phase 'running-minimized' $MinimizedMin
[void][W32]::ShowWindow($h, 9); Start-Sleep -Milliseconds 1200          # SW_RESTORE
$expected = [TimeSpan]::FromMinutes(120) - ((Get-Date) - $startedAt); $phaseLog.Add(("RESTORED {0:HH:mm:ss.fff}  expected remaining ~ {1}" -f (Get-Date), $expected.ToString("hh\:mm\:ss")))
Snap 'restored_immediately'
Run-Phase 'running-restored-timer-visible' $RestoredMin
Click-At ($b.X + 625) ($b.Y + 870); Start-Sleep -Milliseconds 800       # Pause
$phaseLog.Add("PAUSED {0:HH:mm:ss.fff}" -f (Get-Date))
Run-Phase 'paused' $PausedMin
$phaseLog.Add("END {0:HH:mm:ss}" -f (Get-Date))
$phaseLog | Set-Content "$OutPrefix.phases.txt"
[void]$p.CloseMainWindow(); Start-Sleep 1; if (-not $p.HasExited) { $p.Kill() }
$rows | Export-Csv "$OutPrefix.csv" -NoTypeInformation
Write-Host "done: $OutPrefix.csv"
