<#
.SYNOPSIS
  Stage 14 bounded memory-stability run: one Focus session left running for the requested
  duration, sampled periodically. No synthetic mouse/keyboard input at all - the timer is
  started via STUDY_NATIVE_TIMER_AUTOSTART, so this is safe to run unattended.
#>
param(
    [int] $Minutes = 30,
    [int] $SampleEverySeconds = 120,
    [string] $Exe = (Join-Path $PSScriptRoot '..\target\release\study-tracker-native-prototype.exe'),
    [string] $OutCsv = (Join-Path $env:TEMP 'stage14_memory_stability.csv')
)
. (Join-Path $PSScriptRoot 'win-metrics.ps1')
$env:STUDY_NATIVE_VIEW = 'timer'
$env:STUDY_NATIVE_TIMER_MODE = '2'          # Exam (120 min) - long enough to never complete during this run
$env:STUDY_NATIVE_TIMER_AUTOSTART = '1'
$p = Start-Prototype $Exe @{}
Remove-Item Env:\STUDY_NATIVE_VIEW, Env:\STUDY_NATIVE_TIMER_MODE, Env:\STUDY_NATIVE_TIMER_AUTOSTART
$rows = [System.Collections.Generic.List[object]]::new()
$end = (Get-Date).AddMinutes($Minutes)
while ((Get-Date) -lt $end) {
    $m = Measure-Tree $p.Process.Id $SampleEverySeconds 'running'
    $rows.Add([pscustomobject]@{ ElapsedMin = [math]::Round(($Minutes - (($end - (Get-Date)).TotalMinutes)), 2); WorkingSetMB = $m.WorkingSetMB; PrivateWSMB = $m.PrivateWSMB; PrivateBytesMB = $m.PrivateBytesMB; CpuPctOneCore = $m.CpuPctOfOneCore; Threads = $m.Threads })
    $rows | Export-Csv $OutCsv -NoTypeInformation
    Write-Host ("{0,6:N1} min  PrivWS {1,6} MB  PrivBytes {2,6} MB  cpu {3,5}%  threads {4}" -f $rows[-1].ElapsedMin, $m.PrivateWSMB, $m.PrivateBytesMB, $m.CpuPctOfOneCore, $m.Threads)
}
Stop-Process -Id $p.Process.Id -Force -ErrorAction SilentlyContinue
Write-Host "done: $OutCsv"
