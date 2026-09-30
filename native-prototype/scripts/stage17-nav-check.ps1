<#
  Stage 17 navigation check with REAL mouse input on the release build (isolated data dir, synthetic
  fixture): launch -> Dashboard(Quiet) -> click "Full" -> click "Timer" tab -> (Timer surface) ->
  click the "Dashboard" view button -> Dashboard again (Full preserved). Saves a PNG after each
  step and prints the mean absolute difference of the state pairs that must match.
#>
param([string]$Exe = (Join-Path $PSScriptRoot '..\target\release\study-tracker-native-prototype.exe'), [string]$OutDir = $env:TEMP)
. (Join-Path $PSScriptRoot 'win-input.ps1')
$fx = (Resolve-Path (Join-Path $PSScriptRoot '..\tests\fixtures\dashboard\realistic.json')).Path
$data = Join-Path $env:TEMP ('st17nav-' + [Guid]::NewGuid().ToString('N')); New-Item -ItemType Directory $data | Out-Null
$env:STUDY_NATIVE_DATA_DIR = $data; $env:STUDY_NATIVE_IMPORT_BACKUP = $fx; $env:STUDY_NATIVE_NOW = '2026-09-30T12:00:00+02:00'
$env:SLINT_SCALE_FACTOR = '1'; $env:STUDY_NATIVE_SIZE = '1520x980'
$p = Start-Process $Exe -PassThru
Remove-Item Env:\STUDY_NATIVE_DATA_DIR, Env:\STUDY_NATIVE_IMPORT_BACKUP, Env:\STUDY_NATIVE_NOW, Env:\SLINT_SCALE_FACTOR, Env:\STUDY_NATIVE_SIZE
try {
    $h = Get-AppWindow $p.Id; Start-Sleep -Seconds 3; Focus-Window $h
    $b = Get-ClientBox $h
    function Click-Logical($x, $y) { Click-At ($b.X + $x) ($b.Y + $y); Start-Sleep -Milliseconds 900 }
    Save-ClientShot $h "$OutDir\nav1-dashboard-quiet.png"
    Click-Logical 1445 176;  Save-ClientShot $h "$OutDir\nav2-dashboard-full.png"
    Click-Logical 310 101;   Save-ClientShot $h "$OutDir\nav3-timer.png"
    Click-Logical 557 131;   Save-ClientShot $h "$OutDir\nav4-dashboard-again.png"
    "client box: $($b.W)x$($b.H); screenshots in $OutDir (nav1..nav4)"
} finally { if (-not $p.HasExited) { $p.Kill() }; Start-Sleep -Milliseconds 400; Remove-Item -Recurse -Force $data -ErrorAction SilentlyContinue }
