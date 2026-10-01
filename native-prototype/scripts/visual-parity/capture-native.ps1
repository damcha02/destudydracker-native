<#
Stage 17: renders the NATIVE Dashboard in an isolated profile against a synthetic backup fixture
and saves a PNG of the window's client area.

  capture-native.ps1 -Exe <study-tracker-native-prototype.exe> -Fixture <backup.json> -Out <shot.png>
                     [-Layout quiet|full] [-Range week|7|14|30|60|365] [-Width 1520 -Height 980]
                     [-Scale 1.0] [-Now 2026-09-30T12:00:00+02:00] [-Dark 1] [-Tab dashboard|timer]

Isolation: STUDY_NATIVE_DATA_DIR points at a throw-away temp directory (never the real
%LOCALAPPDATA% profile); the fixture is imported with the app's own STUDY_NATIVE_IMPORT_BACKUP
path into that directory. SLINT_SCALE_FACTOR pins the device scale (1.0 = one logical px per
physical px, matching the headless-Chrome DPR-1 production renders; the real machine runs at 125%,
use -Scale 1.25 for that pairing).
#>
param(
    [Parameter(Mandatory)][string]$Exe,
    [Parameter(Mandatory)][string]$Fixture,
    [Parameter(Mandatory)][string]$Out,
    [string]$Layout = 'quiet',
    [string]$Range = 'week',
    [int]$Width = 1520,
    [int]$Height = 980,
    [double]$Scale = 1.0,
    [string]$Now = '2026-09-30T12:00:00+02:00',
    [string]$Tab = 'dashboard',
    [string]$Theme = 'dark',
    [string]$Style = 'field-notebook',
    [string]$Palette = 'default',
    [int]$Quiet = 0,
    [string]$SakuraTime = '',
    [string]$Extra = '',
    [int]$WaitMs = 2500
)

Add-Type -AssemblyName System.Drawing
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class Win {
    [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
    [StructLayout(LayoutKind.Sequential)] public struct POINT { public int X, Y; }
    [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr h, out RECT r);
    [DllImport("user32.dll")] public static extern bool ClientToScreen(IntPtr h, ref POINT p);
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
    [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
    [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr h, IntPtr after, int x, int y, int cx, int cy, uint flags);
    [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h, IntPtr hdc, uint flags);
}
'@
[void][Win]::SetProcessDPIAware()

$data = Join-Path ([IO.Path]::GetTempPath()) ("st-native-parity-" + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $data | Out-Null
$psi = New-Object Diagnostics.ProcessStartInfo $Exe
$psi.UseShellExecute = $false
$env = @{
    STUDY_NATIVE_DATA_DIR = $data
    STUDY_NATIVE_IMPORT_BACKUP = (Resolve-Path $Fixture).Path
    STUDY_NATIVE_NOW = $Now
    STUDY_NATIVE_SIZE = "${Width}x${Height}"
    STUDY_NATIVE_DASHBOARD_LAYOUT = $Layout
    STUDY_NATIVE_DASHBOARD_RANGE = $Range
    STUDY_NATIVE_VIEW = $Tab
    STUDY_NATIVE_DASHBOARD_DARK = $(if ($Theme -eq 'light') { '0' } else { '1' })
    SLINT_SCALE_FACTOR = "$Scale"
    STUDY_NATIVE_STYLE = $Style
    STUDY_NATIVE_PALETTE = $Palette
    STUDY_NATIVE_THEME = $Theme
}
if ($Quiet -eq 1) { $env['STUDY_NATIVE_WABI_QUIET'] = '1' }
if ($SakuraTime -ne '') { $env['STUDY_NATIVE_SAKURA_TIME'] = $SakuraTime }
# Extra: "NAME=value;NAME2=value2" (Stage 19 diagnostics)
foreach ($pair in ($Extra -split ';' | Where-Object { $_ })) { $kv = $pair -split '=', 2; $env[$kv[0]] = $kv[1] }
foreach ($k in $env.Keys) { $psi.EnvironmentVariables[$k] = [string]$env[$k] }
$p = [Diagnostics.Process]::Start($psi)
try {
    for ($i = 0; $i -lt 40 -and $p.MainWindowHandle -eq 0; $i++) { Start-Sleep -Milliseconds 250; $p.Refresh() }
    Start-Sleep -Milliseconds $WaitMs
    $p.Refresh()
    $h = $p.MainWindowHandle
    [void][Win]::SetForegroundWindow($h)
    [void][Win]::SetWindowPos($h, [IntPtr]::Zero, 0, 0, 0, 0, 0x0001 -bor 0x0004)   # SWP_NOSIZE | SWP_NOZORDER: move to the origin, keep the size
    Start-Sleep -Milliseconds 800
    $r = New-Object Win+RECT; [void][Win]::GetClientRect($h, [ref]$r)
    $pt = New-Object Win+POINT; [void][Win]::ClientToScreen($h, [ref]$pt)
    $w = $r.Right - $r.Left; $hh = $r.Bottom - $r.Top
    $bmp = New-Object Drawing.Bitmap $w, $hh
    $g = [Drawing.Graphics]::FromImage($bmp)
    $g.CopyFromScreen($pt.X, $pt.Y, 0, 0, (New-Object Drawing.Size $w, $hh))
    $g.Dispose()
    $bmp.Save($Out, [Drawing.Imaging.ImageFormat]::Png)
    $bmp.Dispose()
    "captured $Out ${w}x${hh} (client) layout=$Layout scale=$Scale"
} finally {
    if (-not $p.HasExited) { $p.Kill() }
    Start-Sleep -Milliseconds 300
    Remove-Item -Recurse -Force $data -ErrorAction SilentlyContinue
}
