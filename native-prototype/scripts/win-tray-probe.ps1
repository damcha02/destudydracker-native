<#
Stage 18 helper: does the native app's tray icon exist for the profile in -DataDir, and where?

Asks the shell directly (Shell_NotifyIconGetRect) about *our* notification icon, identified by
the hidden message window of that profile (class name derived exactly like
platform::single_instance::names_for) and the fixed icon id 1. Reads nothing else on screen.

  win-tray-probe.ps1 -DataDir <path>        prints: TRAY present=<bool> rect=<l,t,r,b> window=<bool>
#>
param([Parameter(Mandatory)][string]$DataDir)
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class TrayProbe {
    [StructLayout(LayoutKind.Sequential)] public struct NOTIFYICONIDENTIFIER { public uint cbSize; public IntPtr hWnd; public uint uID; public Guid guidItem; }
    [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
    [DllImport("shell32.dll")] public static extern int Shell_NotifyIconGetRect(ref NOTIFYICONIDENTIFIER id, out RECT rect);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern IntPtr FindWindowEx(IntPtr parent, IntPtr after, string cls, string title);
}
'@
function Get-Fnv1a64([string]$s) {
    $bytes = [Text.Encoding]::UTF8.GetBytes($s)
    [uint64]$h = 14695981039346656037
    foreach ($b in $bytes) { $h = $h -bxor [uint64]$b; $h = [uint64](([System.Numerics.BigInteger]$h * 1099511628211) % [System.Numerics.BigInteger]::Pow(2, 64)) }
    $h
}
$canon = $DataDir.Replace('/', '\').TrimEnd('\').ToLowerInvariant()
$hash = '{0:x16}' -f (Get-Fnv1a64 $canon)
$class = "com.damcha.studytracker.native-shell.$hash.platform-window"
$msg = [IntPtr](-3)   # HWND_MESSAGE
$hwnd = [TrayProbe]::FindWindowEx($msg, [IntPtr]::Zero, $class, [NullString]::Value)
if ($hwnd -eq [IntPtr]::Zero) { "TRAY present=False rect=- window=False"; return }
$id = New-Object TrayProbe+NOTIFYICONIDENTIFIER
$id.cbSize = [Runtime.InteropServices.Marshal]::SizeOf($id); $id.hWnd = $hwnd; $id.uID = 1
$r = New-Object TrayProbe+RECT
$hr = [TrayProbe]::Shell_NotifyIconGetRect([ref]$id, [ref]$r)
"TRAY present=$($hr -eq 0) rect=$($r.Left),$($r.Top),$($r.Right),$($r.Bottom) window=True"
