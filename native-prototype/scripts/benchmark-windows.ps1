<#
.SYNOPSIS
  Windows benchmark helper for the native prototype (Stage 12). Simple and inspectable on purpose.

.DESCRIPTION
  Modes
    sample   One measurement line for a running process tree (memory + interval CPU).
               -ProcessId <pid> [-Seconds 10] [-Label text] [-Csv file]
    startup  Launches the release exe N times and reports process-spawn -> first rendered frame.
               [-Runs 10] [-Exe path] [-View timer|text|dashboard|map]
    launch   Starts the exe (optionally with a view / map level / bench env), waits for the window, prints PID.

  Metrics (all in MB = 1048576 bytes, per process, summed over the process tree unless noted)
    WorkingSet     Physical RAM currently mapped into the process, including shared pages (DLLs, driver mappings).
    PrivateWS      Working-set pages that are NOT shareable with other processes ("Working Set - Private").
                   Best Windows analogue of "what this process alone costs in RAM". NOT the same as Linux PSS.
    PrivateBytes   Committed private virtual memory (Task Manager "Commit size"). Includes paged-out and never-touched
                   committed pages, so it is >= PrivateWS in normal cases.
    CPU%           (delta of user+kernel processor time) / (wall interval). 100% = one logical core fully busy.
                   Also printed as % of the whole machine (divide by logical processor count).
#>
param(
    [ValidateSet('sample', 'startup', 'launch')] [string] $Mode = 'sample',
    [int] $ProcessId = 0,
    [double] $Seconds = 10,
    [string] $Label = 'sample',
    [string] $Csv = '',
    [int] $Runs = 10,
    [string] $Exe = (Join-Path $PSScriptRoot '..\target\release\study-tracker-native-prototype.exe'),
    [string] $View = '',
    [string] $MapLevel = '',
    [string] $MapBench = ''
)

. (Join-Path $PSScriptRoot 'win-metrics.ps1')
$ErrorActionPreference = 'Stop'

function New-EnvFromParams {
    $e = @{}
    if ($View) { $e['STUDY_NATIVE_VIEW'] = $View }
    if ($MapLevel) { $e['STUDY_NATIVE_MAP_LEVEL'] = $MapLevel }
    if ($MapBench) { $e['STUDY_NATIVE_MAP_BENCH'] = $MapBench }
    $e
}

if ($MyInvocation.InvocationName -eq '.') { return }   # dot-sourced: expose the functions only

switch ($Mode) {
    'sample' {
        if ($ProcessId -le 0) { throw '-ProcessId is required' }
        $r = Measure-Tree $ProcessId $Seconds $Label
        $r | Format-List | Out-String | Write-Host
        if ($Csv) { $r | Export-Csv -Append -NoTypeInformation -Path $Csv }
        $r
    }
    'launch' {
        $s = Start-Prototype $Exe (New-EnvFromParams)
        Start-Sleep -Milliseconds 1500
        Write-Host "PID $($s.Process.Id)"
        $s.Process.Id
    }
    'startup' {
        # "Startup" = Process.Start() returned -> first frame rendered (AfterRendering notifier printed FIRST_FRAME).
        # Includes process creation, DLL/driver load, GL context creation, Slint layout and the first draw.
        $e = New-EnvFromParams; $e['STUDY_NATIVE_STARTUP_REPORT'] = '1'
        $rows = @()
        for ($i = 1; $i -le $Runs; $i++) {
            $s = Start-Prototype $Exe $e -CaptureStdout
            $line = $null
            while ($null -eq $line -or $line -notlike 'FIRST_FRAME*') {
                $line = $s.Process.StandardOutput.ReadLine()
                if ($null -eq $line) { break }
            }
            $total = $s.Clock.Elapsed.TotalMilliseconds
            $inApp = if ($line) { [double]($line -split ' ')[1] } else { [double]::NaN }
            $pws = (Get-TreeSnapshot $s.Process.Id).PrivateWS
            $s.Process.Kill(); $s.Process.WaitForExit()
            $rows += [pscustomobject]@{ Run = $i; SpawnToFirstFrameMs = [math]::Round($total, 1); MainToFirstFrameMs = [math]::Round($inApp, 1); PrivateWSMB = [math]::Round($pws, 1) }
            Start-Sleep -Milliseconds 500
        }
        $rows | Format-Table | Out-String | Write-Host
        $v = $rows.SpawnToFirstFrameMs | Sort-Object
        $med = if ($v.Count % 2) { $v[($v.Count - 1) / 2] } else { ($v[$v.Count / 2 - 1] + $v[$v.Count / 2]) / 2 }
        "spawn->first frame ms: min {0}  median {1}  mean {2:N1}  max {3}" -f $v[0], $med, ($v | Measure-Object -Average).Average, $v[-1] | Write-Host
    }
}
