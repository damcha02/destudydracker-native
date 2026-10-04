<#
Stage 21 (Windows): one deterministic Travle screenshot pair (production | native) and its mean abs
difference - the Windows twin of travle-pair-linux.sh. Both apps import the same synthetic backup
(gen-break-fixture.mjs full --travle <state> --tz -07:00), frozen at 2026-09-30 12:00 Pacific,
DPR/scale 1 by default. Production renders in headless Edge (throw-away profile); native renders
in-process via STUDY_NATIVE_SNAPSHOT in a throw-away STUDY_NATIVE_DATA_DIR.

  travle-pair.ps1 -Dist <prod-dist> -Fixtures <dir> -OutDir <dir> -State fresh|mid|won|lost
                  -Style field-notebook|wabi-sabi -Theme dark|light [-Palette default|sakura]
                  [-Width 1520 -Height 980] [-Scale 1]
#>
param(
    [Parameter(Mandatory)][string]$Dist,
    [Parameter(Mandatory)][string]$Fixtures,
    [Parameter(Mandatory)][string]$OutDir,
    [Parameter(Mandatory)][string]$State,
    [string]$Style = 'field-notebook',
    [string]$Theme = 'dark',
    [string]$Palette = 'default',
    [int]$Width = 1520,
    [int]$Height = 980,
    [double]$Scale = 1.0,
    [string]$Exe = (Join-Path $PSScriptRoot '..\..\target\release\study-tracker-native-prototype.exe')
)
$ErrorActionPreference = 'Stop'
New-Item -ItemType Directory -Force $OutDir | Out-Null
$tag = "travle-$State-$Style-$Theme"
if ($Palette -ne 'default') { $tag += "-$Palette" }
if ($Width -ne 1520 -or $Height -ne 980) { $tag += "-${Width}x$Height" }
if ($Scale -ne 1.0) { $tag += "-s$Scale" }
$fx = (Resolve-Path (Join-Path $Fixtures "fixture-$State.json")).Path
$open = (Get-Content (Join-Path $PSScriptRoot 'open-travle.js') | Where-Object { $_ -notmatch '^//' }) -join "`n"
# Windows PowerShell 5.1 drops embedded double quotes when passing arguments to native programs.
$open = $open.Replace('"', '\"')
$prod = Join-Path $OutDir "prod-$tag.png"; $nat = Join-Path $OutDir "nat-$tag.png"
# Production viewport is the logical size; DPR = Scale.
$pw = [int][math]::Round($Width / $Scale); $ph = [int][math]::Round($Height / $Scale)
$sak = @(); if ($Palette -eq 'sakura') { $sak = @('--palette', 'sakura', '--anim-time', '4000') }
& node (Join-Path $PSScriptRoot 'capture-prod.mjs') --dist $Dist --fixture $fx --out $prod --w $pw --h $ph --dpr "$Scale" `
    --tab break --style $Style --theme $Theme @sak --now 2026-09-30T12:00:00-07:00 --tz America/Los_Angeles `
    --math-random 0.25 --js $open | Out-Null

$data = Join-Path $env:TEMP ("st21-pair-" + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory $data | Out-Null
$envs = @{ STUDY_NATIVE_DATA_DIR = $data; STUDY_NATIVE_IMPORT_BACKUP = $fx; STUDY_NATIVE_NOW = '2026-09-30T12:00:00-07:00'
           STUDY_NATIVE_SIZE = "${pw}x$ph"; SLINT_SCALE_FACTOR = "$Scale"; STUDY_NATIVE_VIEW = 'break'; STUDY_NATIVE_OPEN_GAME = '2'
           STUDY_NATIVE_STYLE = $Style; STUDY_NATIVE_THEME = $Theme; STUDY_NATIVE_PALETTE = $Palette; STUDY_NATIVE_BREAK_PICK = '0.25'
           STUDY_NATIVE_SNAPSHOT = $nat; GALLIUM_DRIVER = 'llvmpipe' }
if ($Palette -eq 'sakura') { $envs['STUDY_NATIVE_SAKURA_TIME'] = '4000' }
if ($Style -eq 'wabi-sabi') { $envs['STUDY_NATIVE_REST_ROOM'] = 'games' }
foreach ($k in $envs.Keys) { [Environment]::SetEnvironmentVariable($k, [string]$envs[$k]) }
try {
    $p = Start-Process -FilePath (Resolve-Path $Exe).Path -PassThru
    if (-not $p.WaitForExit(60000)) { $p.Kill(); throw "native snapshot timed out" }
} finally {
    foreach ($k in $envs.Keys) { [Environment]::SetEnvironmentVariable($k, $null) }
    Remove-Item -Recurse -Force $data -ErrorAction SilentlyContinue
}
$d = & node (Join-Path $PSScriptRoot 'imgtool.mjs') diff $prod $nat
"$tag`t$d"
