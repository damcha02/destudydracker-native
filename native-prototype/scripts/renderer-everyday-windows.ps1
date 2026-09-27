<#
.SYNOPSIS
  Everyday-workload comparison for one renderer build (Stage 12): startup, memory per view, timer-running CPU
  (visible / dashboard / minimized). Run once per executable; set STUDY_NATIVE_SKIA_API in the environment first
  for the scratch Skia build (d3d | opengl | default=software).
  Needs an interactive desktop and the default window geometry; do not touch mouse/keyboard while it runs.
#>
param(
    [Parameter(Mandatory)] [string] $Exe,
    [Parameter(Mandatory)] [string] $Label,
    [int] $StartupRuns = 8,
    [int] $CpuSeconds = 30
)
. (Join-Path $PSScriptRoot 'win-input.ps1')
. (Join-Path $PSScriptRoot 'win-metrics.ps1')
$ErrorActionPreference = 'Continue'
$out = [System.Collections.Generic.List[object]]::new()

# 1) startup: spawn -> first rendered frame (Timer view)
# Renderer-neutral "first paint": Slint's rendering notifier is not available on every Skia surface, so poll the screen
# instead: the window exists AND light (text) pixels appear in the sidebar label area of its client rectangle.
function Test-Painted([IntPtr] $win) {
    $c = Get-ClientBox $win
    if ($c.W -lt 300) { return $false }
    $bmp = New-Object Drawing.Bitmap 90, 24
    $g = [Drawing.Graphics]::FromImage($bmp); $g.CopyFromScreen($c.X + 100, $c.Y + 176, 0, 0, $bmp.Size); $g.Dispose()
    $hit = $false
    for ($x = 0; $x -lt 90 -and -not $hit; $x += 3) { for ($y = 0; $y -lt 24; $y += 3) { if ($bmp.GetPixel($x, $y).R -gt 120) { $hit = $true; break } } }
    $bmp.Dispose(); $hit
}
$ms = @()
for ($i = 0; $i -lt $StartupRuns; $i++) {
    $s = Start-Prototype $Exe @{ STUDY_NATIVE_VIEW = 'timer' }
    $win = [IntPtr]::Zero; $painted = $false
    while ($s.Clock.ElapsedMilliseconds -lt 8000 -and -not $painted) {
        if ($win -eq [IntPtr]::Zero) { $win = [W32]::FindTopWindow([uint32]$s.Process.Id) }
        if ($win -ne [IntPtr]::Zero) { $painted = Test-Painted $win }
    }
    $ms += $s.Clock.Elapsed.TotalMilliseconds
    $s.Process.Kill(); $s.Process.WaitForExit(); Start-Sleep -Milliseconds 500
}
$sorted = $ms | Sort-Object
$startup = "min {0:N0} / median {1:N0} / max {2:N0} ms" -f $sorted[0], $sorted[[int](($sorted.Count - 1) / 2)], $sorted[-1]

# 2) one session
$statsFile = Join-Path $env:TEMP "everyday_$Label.stats.txt"
$env:STUDY_NATIVE_VIEW = 'timer'; $env:STUDY_NATIVE_FRAME_STATS = '1'
$p = Start-Process $Exe -PassThru -RedirectStandardOutput $statsFile
Remove-Item Env:\STUDY_NATIVE_VIEW, Env:\STUDY_NATIVE_FRAME_STATS
$h = Get-AppWindow $p.Id; Focus-Window $h; $b = Get-ClientBox $h; Start-Sleep 4
function Add-Row($tag, $m) { $out.Add([pscustomobject]@{ Renderer = $Label; State = $tag; WorkingSetMB = $m.WorkingSetMB; PrivateWSMB = $m.PrivateWSMB; PrivateBytesMB = $m.PrivateBytesMB; Threads = $m.Threads; CpuPctOneCore = $m.CpuPctOfOneCore }) }
Add-Row 'idle Timer (ready)' (Measure-Tree $p.Id 8 'x')
Click-At ($b.X + 788) ($b.Y + 370); Start-Sleep -Milliseconds 400
Click-At ($b.X + 625) ($b.Y + 870); Start-Sleep -Milliseconds 800
$t0 = Get-Date
Add-Row 'timer running, Timer visible' (Measure-Tree $p.Id $CpuSeconds 'x')
Click-At ($b.X + 697) ($b.Y + 164); Start-Sleep -Milliseconds 800
Add-Row 'timer running, Dashboard visible' (Measure-Tree $p.Id ($CpuSeconds / 2) 'x')
Click-At ($b.X + 845) ($b.Y + 164); Start-Sleep -Milliseconds 800
Add-Row 'timer running, Map visible' (Measure-Tree $p.Id ($CpuSeconds / 2) 'x')
Click-At ($b.X + 394) ($b.Y + 164); Start-Sleep -Milliseconds 800
[void][W32]::ShowWindow($h, 6); Start-Sleep -Milliseconds 800
Add-Row 'timer running, minimized' (Measure-Tree $p.Id $CpuSeconds 'x')
[void][W32]::ShowWindow($h, 9); Start-Sleep -Milliseconds 1200
Click-At ($b.X + 625) ($b.Y + 870); Start-Sleep -Milliseconds 800   # pause
Add-Row 'timer paused, Timer visible' (Measure-Tree $p.Id ($CpuSeconds / 2) 'x')
$p.Kill()
$frames = @(Get-Content $statsFile | ForEach-Object { if ($_ -match 'frames=(\d+) ticks=(\d+)') { [pscustomobject]@{ f = [int]$Matches[1]; k = [int]$Matches[2] } } } | Where-Object { $_.k -gt 90 })
$visible = @($frames | Select-Object -First 3 | ForEach-Object f)   # first intervals with the timer ticking = Timer visible
Write-Host ("== {0}: startup {1}; frames per 10 s while Timer visible+running: {2}" -f $Label, $startup, ($visible -join ','))
$out | Format-Table -AutoSize | Out-String -Width 200 | Write-Host
[pscustomobject]@{ Renderer = $Label; Startup = $startup; FramesPer10sTimerVisible = ($visible -join ','); Rows = $out }
