<#
.SYNOPSIS
  Map stress matrix for Windows (Stage 12). For each level and transform mode it launches the release exe with the
  built-in self-terminating bench driver, reads Slint's own fps counter (SLINT_DEBUG_PERFORMANCE) from stderr and
  samples CPU/memory of the running process over the middle of the run.
  Levels: 0 World, 1 Light, 2 Dense x3, 3 Dense x10, 4 Dense x50, 5 Cells, 6 Giant.
  Note: fps is capped by the panel refresh rate (164 Hz on the Stage 12 machine); it is NOT a target.
#>
param(
    [int[]] $Levels = @(0, 3, 4, 5, 6),
    [string[]] $Modes = @('pan', 'zoom'),
    [int] $Seconds = 12,
    [int] $Notches = 8,
    [string] $Exe = (Join-Path $PSScriptRoot '..\target\release\study-tracker-native-prototype.exe'),
    [string] $OutDir = $env:TEMP
)
. (Join-Path $PSScriptRoot 'win-metrics.ps1')
$names = 'World', 'Light', 'Dense x3', 'Dense x10', 'Dense x50', 'Cells', 'Giant'
$rows = foreach ($lv in $Levels) {
    foreach ($xform in $Modes) {
        $err = Join-Path $OutDir "stress_${lv}_$xform.err"
        $env:STUDY_NATIVE_VIEW = 'map'; $env:STUDY_NATIVE_MAP_LEVEL = "$lv"
        $env:STUDY_NATIVE_MAP_BENCH = "${xform}:${Seconds}:${Notches}"; $env:SLINT_DEBUG_PERFORMANCE = 'refresh_full_speed,console'
        $p = Start-Process $Exe -PassThru -RedirectStandardError $err -RedirectStandardOutput "$err.out"
        Remove-Item Env:\STUDY_NATIVE_VIEW, Env:\STUDY_NATIVE_MAP_LEVEL, Env:\STUDY_NATIVE_MAP_BENCH, Env:\SLINT_DEBUG_PERFORMANCE
        Start-Sleep -Seconds 3
        $m = Measure-Tree $p.Id ([Math]::Max(3, $Seconds - 5)) 'x'
        [void]$p.WaitForExit(($Seconds + 30) * 1000)
        $fps = @(Get-Content $err | Where-Object { $_ -match 'average frames per second: (\d+)' } | ForEach-Object { [int]$Matches[1] })
        if ($fps.Count -gt 1) { $fps = $fps[1..($fps.Count - 1)] }   # drop the first (warm-up) sample
        $work = (Get-Content $err | Where-Object { $_ -match 'work avg ([\d.]+) ms' } | ForEach-Object { $Matches[1] })
        [pscustomobject]@{
            Level = $names[$lv]; Mode = $xform
            FpsMean = if ($fps) { [math]::Round(($fps | Measure-Object -Average).Average, 1) } else { $null }
            FpsMin = if ($fps) { ($fps | Measure-Object -Minimum).Minimum } else { $null }
            FrameMs = if ($fps) { [math]::Round(1000 / ($fps | Measure-Object -Average).Average, 1) } else { $null }
            CpuOneCore = $m.CpuPctOfOneCore; PrivateWSMB = $m.PrivateWSMB; PrivateBytesMB = $m.PrivateBytesMB; WorkingSetMB = $m.WorkingSetMB
            RustWorkMsPerTick = $work
        }
    }
}
$rows | Format-Table -AutoSize | Out-String -Width 220 | Write-Host
$rows
