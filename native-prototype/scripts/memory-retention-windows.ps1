<#
.SYNOPSIS
  Extreme-geometry memory retention experiment (Stage 12). One app session, N cycles of:
  World -> Dense x50 -> interact -> World -> Dashboard -> Timer -> Map(World).
  Levels are switched with the map's ']' / '[' keys (so Light, Dense x3, Dense x10 are visited on the way).
  Requires the release exe and an interactive desktop; coordinates assume the default window (client ~1525x1025 px).
#>
param(
    [int] $Cycles = 3,
    [int] $Level = 4,                     # target level (4 = Dense x50, 6 = Giant)
    [string] $Exe = (Join-Path $PSScriptRoot '..\target\release\study-tracker-native-prototype.exe'),
    [string] $Csv = ''
)
. (Join-Path $PSScriptRoot 'win-input.ps1')
. (Join-Path $PSScriptRoot 'win-metrics.ps1')
$env:STUDY_NATIVE_VIEW = 'map'
$p = Start-Process $Exe -PassThru
Remove-Item Env:\STUDY_NATIVE_VIEW
$h = Get-AppWindow $p.Id; Focus-Window $h; $b = Get-ClientBox $h
$rows = [System.Collections.Generic.List[object]]::new()
function Step([string] $name) { $r = Measure-Tree $p.Id 4 $name; $rows.Add($r); "{0,-34} WS {1,7} PrivWS {2,7} PrivBytes {3,8} cpu {4,6}%" -f $name, $r.WorkingSetMB, $r.PrivateWSMB, $r.PrivateBytesMB, $r.CpuPctOfOneCore | Write-Host }
function Tab([int] $x) { Click-At ($b.X + $x) ($b.Y + 164); Start-Sleep -Milliseconds 800 }
function Focus-Map { Click-At ($b.X + 700) ($b.Y + 600); Start-Sleep -Milliseconds 300 }
function Level-Keys([string] $key, [int] $n) { for ($i = 0; $i -lt $n; $i++) { Send-Text $key; Start-Sleep -Milliseconds 500 } }
function Interact {
    Hover-Sweep ($b.X + 500) ($b.Y + 450) ($b.X + 1200) ($b.Y + 750) 120 12
    Wheel-At ($b.X + 800) ($b.Y + 550) 12
    Drag-Mouse ($b.X + 800) ($b.Y + 550) ($b.X + 500) ($b.Y + 450) 40 10
    Wheel-At ($b.X + 800) ($b.Y + 550) -12
}
Start-Sleep 3
Step 'C0 World at open'
for ($c = 1; $c -le $Cycles; $c++) {
    Focus-Map; Level-Keys ']' $Level; Start-Sleep -Seconds 2; Step "C$c a Level $Level loaded"
    Interact; Step "C$c b Level $Level after interaction"
    Focus-Map; Level-Keys '[' $Level; Start-Sleep -Seconds 2; Step "C$c c back to World"
    Tab 697; Step "C$c d Dashboard"
    Tab 394; Step "C$c e Timer"
    Tab 845; Step "C$c f Map (World)"
}
Save-ClientShot $h (Join-Path $env:TEMP 'retention_end.png')
[void]$p.CloseMainWindow(); Start-Sleep 1; if (-not $p.HasExited) { $p.Kill() }
if ($Csv) { $rows | Export-Csv -NoTypeInformation -Path $Csv }
$rows
