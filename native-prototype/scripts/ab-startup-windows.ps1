<#
.SYNOPSIS
  Startup A/B (Stage 12): process spawn -> first painted, usable window, measured the same way for native and production.
  "Painted" = the window's client area is mostly dark (app background) AND contains bright pixels (text); sampled from the screen
  (renderer-neutral). Also reports spawn -> first visible top-level window and the process count 3 s after first paint.
  Production runs with an isolated WebView2 profile: -Fresh N launches use a NEW empty profile each time (cold profile),
  -Warm N launches reuse one profile that has been used before.
#>
param(
    [Parameter(Mandatory)] [ValidateSet('native', 'production')] [string] $Target,
    [string] $Exe = '',
    [int] $Warm = 10,
    [int] $Fresh = 3,
    [string] $ProfileRoot = $env:TEMP,
    [string] $OutCsv = ''
)
. (Join-Path $PSScriptRoot 'win-input.ps1')
. (Join-Path $PSScriptRoot 'win-tree.ps1')
$ErrorActionPreference = 'Continue'
if (-not $Exe) { $Exe = if ($Target -eq 'native') { Join-Path $PSScriptRoot '..\target\release\study-tracker-native-prototype.exe' } else { Join-Path $PSScriptRoot '..\..\desktop\src-tauri\target\release\app.exe' } }

# The window's OWN content is captured with PrintWindow(PW_RENDERFULLCONTENT), so other windows (or a transparent, not yet painted
# WebView2 host showing whatever is behind it) can never produce a false "painted". Verified: production is fully dark with 0 bright
# samples at +0.3 s and has content (>=4 bright samples) by +1.5 s; the native window has content at +0.3 s.
Add-Type -MemberDefinition '[DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h, IntPtr hdc, uint flags);' -Name Pw -Namespace W32p
function Test-Painted([IntPtr] $win) {
    $r = New-Object W32+RECT; [void][W32]::GetWindowRect($win, [ref]$r); $w = $r.R - $r.L; $hh = $r.B - $r.T
    if ($w -lt 300 -or $hh -lt 300) { return $false }
    $bmp = New-Object Drawing.Bitmap $w, $hh
    $g = [Drawing.Graphics]::FromImage($bmp); $hdc = $g.GetHdc(); [void][W32p.Pw]::PrintWindow($win, $hdc, 2); $g.ReleaseHdc($hdc); $g.Dispose()
    $dark = 0; $bright = 0; $n = 0
    for ($x = 20; $x -lt $w - 20; $x += [int]($w / 32)) { for ($y = 60; $y -lt $hh - 40; $y += [int]($hh / 20)) {
        $px = $bmp.GetPixel($x, $y); $l = 0.299 * $px.R + 0.587 * $px.G + 0.114 * $px.B; $n++
        if ($l -lt 80) { $dark++ } elseif ($l -gt 170) { $bright++ } } }
    $bmp.Dispose()
    return (($dark / $n) -ge 0.80 -and $bright -ge 4)
}

function Launch-Once([string] $profile) {
    if ($Target -eq 'production') { New-Item -ItemType Directory -Force $profile | Out-Null; $env:WEBVIEW2_USER_DATA_FOLDER = $profile } else { $env:STUDY_NATIVE_VIEW = 'timer' }
    $sw = [Diagnostics.Stopwatch]::StartNew(); $p = [Diagnostics.Process]::Start((New-Object Diagnostics.ProcessStartInfo $Exe -Property @{ UseShellExecute = $false }))
    $env:WEBVIEW2_USER_DATA_FOLDER = $null; $env:STUDY_NATIVE_VIEW = $null
    $win = [IntPtr]::Zero; $visibleMs = $null; $paintMs = $null
    while ($sw.ElapsedMilliseconds -lt 15000 -and $null -eq $paintMs) {
        if ($win -eq [IntPtr]::Zero) { $win = [W32]::FindTopWindow([uint32]$p.Id); if ($win -ne [IntPtr]::Zero -and $null -eq $visibleMs) { $visibleMs = $sw.Elapsed.TotalMilliseconds } }
        $big = $null; try { $big = Get-RootMainWindow $p.Id 5 } catch { }
        if ($big -and $big -ne [IntPtr]::Zero) { if (Test-Painted $big) { $paintMs = $sw.Elapsed.TotalMilliseconds } }
    }
    Start-Sleep -Seconds 3
    $tree = Get-TreeDetail $p.Id; $procs = $tree.Count   # (not @(Get-TreeDetail ...): that wraps the returned array and counts 1)
    Stop-Process -Id $p.Id -Force -ErrorAction SilentlyContinue; Start-Sleep -Milliseconds 1500
    [pscustomobject]@{ PaintedMs = $paintMs; FirstWindowMs = $visibleMs; ProcsAfter3s = $procs }
}

$rows = @()
if ($Target -eq 'production') { for ($i = 1; $i -le $Fresh; $i++) { $r = Launch-Once (Join-Path $ProfileRoot ("startup_fresh_" + [guid]::NewGuid().ToString('N').Substring(0, 8))); $rows += [pscustomobject]@{ Kind = 'fresh profile'; Run = $i; PaintedMs = [math]::Round($r.PaintedMs, 0); FirstWindowMs = [math]::Round($r.FirstWindowMs, 0); Procs = $r.ProcsAfter3s } } }
$warmProfile = Join-Path $ProfileRoot 'startup_warm_profile'
if ($Target -eq 'production') { [void](Launch-Once $warmProfile) }   # prime the profile (not counted)
for ($i = 1; $i -le $Warm; $i++) { $r = Launch-Once $warmProfile; $rows += [pscustomobject]@{ Kind = 'warm'; Run = $i; PaintedMs = [math]::Round($r.PaintedMs, 0); FirstWindowMs = [math]::Round($r.FirstWindowMs, 0); Procs = $r.ProcsAfter3s } }
$rows | Format-Table -AutoSize | Out-String | Write-Host
foreach ($k in 'fresh profile', 'warm') { $v = @($rows | Where-Object { $_.Kind -eq $k -and $_.PaintedMs } | ForEach-Object PaintedMs | Sort-Object); if ($v.Count) { $med = if ($v.Count % 2) { $v[($v.Count - 1) / 2] } else { ($v[$v.Count / 2 - 1] + $v[$v.Count / 2]) / 2 }; "{0,-14} n={1}  min {2}  median {3}  mean {4:N0}  max {5}" -f $k, $v.Count, $v[0], $med, ($v | Measure-Object -Average).Average, $v[-1] | Write-Host } }
if ($OutCsv) { $rows | Export-Csv $OutCsv -NoTypeInformation -Delimiter ';' }
