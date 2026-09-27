# Dot-source: . .\win-input.ps1
# Small Win32 helpers for driving the native prototype with real (synthetic) input and grabbing screenshots.
# The PowerShell host is made DPI-aware so all coordinates below are PHYSICAL screen pixels, matching the app.
Add-Type -AssemblyName System.Drawing
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
using System.Text;
public static class W32 {
    [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
    [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
    [DllImport("user32.dll")] public static extern void mouse_event(uint f, int dx, int dy, int data, UIntPtr extra);
    [DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint flags, UIntPtr extra);
    [DllImport("user32.dll")] public static extern uint MapVirtualKey(uint code, uint mapType);
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
    [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr h, out RECT r);
    [DllImport("user32.dll")] public static extern bool ClientToScreen(IntPtr h, ref POINT p);
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
    [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int cmd);
    [DllImport("user32.dll")] public static extern uint GetDpiForWindow(IntPtr h);
    [DllImport("user32.dll")] public static extern bool MoveWindow(IntPtr h, int x, int y, int w, int hgt, bool repaint);
    [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h, uint msg, IntPtr w, IntPtr l);
    [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc cb, IntPtr l);
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
    [DllImport("user32.dll")] public static extern bool IsIconic(IntPtr h);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetWindowText(IntPtr h, StringBuilder s, int n);
    public delegate bool EnumProc(IntPtr h, IntPtr l);
    [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L, T, R, B; }
    [StructLayout(LayoutKind.Sequential)] public struct POINT { public int X, Y; }
    public static IntPtr FindTopWindow(uint pid) {
        IntPtr found = IntPtr.Zero;
        EnumWindows((h, l) => { uint p; GetWindowThreadProcessId(h, out p);
            if (p == pid && IsWindowVisible(h)) { var sb = new StringBuilder(256); GetWindowText(h, sb, 256); if (sb.Length > 0) { found = h; return false; } }
            return true; }, IntPtr.Zero);
        return found;
    }
}
'@
[void][W32]::SetProcessDPIAware()

function Get-AppWindow([int] $ProcId, [int] $TimeoutMs = 10000) {
    $sw = [Diagnostics.Stopwatch]::StartNew()
    while ($sw.ElapsedMilliseconds -lt $TimeoutMs) {
        $h = [W32]::FindTopWindow([uint32]$ProcId); if ($h -ne [IntPtr]::Zero) { return $h }
        Start-Sleep -Milliseconds 50
    }
    throw "no visible window for pid $ProcId"
}
# Client area origin + size in physical pixels.
function Get-ClientBox([IntPtr] $h) {
    $r = New-Object W32+RECT; [void][W32]::GetClientRect($h, [ref]$r)
    $p = New-Object W32+POINT; [void][W32]::ClientToScreen($h, [ref]$p)
    [pscustomobject]@{ X = $p.X; Y = $p.Y; W = $r.R; H = $r.B; Dpi = [W32]::GetDpiForWindow($h) }
}
function Move-Mouse([int] $x, [int] $y) { [void][W32]::SetCursorPos($x, $y) }
function Click-At([int] $x, [int] $y) {
    Move-Mouse $x $y; Start-Sleep -Milliseconds 40
    [W32]::mouse_event(0x2, 0, 0, 0, [UIntPtr]::Zero); Start-Sleep -Milliseconds 40   # left down
    [W32]::mouse_event(0x4, 0, 0, 0, [UIntPtr]::Zero); Start-Sleep -Milliseconds 60   # left up
}
function Drag-Mouse([int] $x0, [int] $y0, [int] $x1, [int] $y1, [int] $Steps = 40, [int] $StepMs = 8) {
    Move-Mouse $x0 $y0; Start-Sleep -Milliseconds 40
    [W32]::mouse_event(0x2, 0, 0, 0, [UIntPtr]::Zero)
    for ($i = 1; $i -le $Steps; $i++) { Move-Mouse ([int]($x0 + ($x1 - $x0) * $i / $Steps)) ([int]($y0 + ($y1 - $y0) * $i / $Steps)); Start-Sleep -Milliseconds $StepMs }
    [W32]::mouse_event(0x4, 0, 0, 0, [UIntPtr]::Zero)
}
# Wheel: positive = away from the user (zoom in on the map). One classic notch = 120.
function Wheel-At([int] $x, [int] $y, [int] $Notches, [int] $Delta = 120, [int] $StepMs = 30) {
    Move-Mouse $x $y; Start-Sleep -Milliseconds 30
    for ($i = 0; $i -lt [math]::Abs($Notches); $i++) { [W32]::mouse_event(0x800, 0, 0, [int]([math]::Sign($Notches) * $Delta), [UIntPtr]::Zero); Start-Sleep -Milliseconds $StepMs }
}
function Hover-Sweep([int] $x0, [int] $y0, [int] $x1, [int] $y1, [int] $Steps = 200, [int] $StepMs = 10) {
    for ($i = 0; $i -le $Steps; $i++) { Move-Mouse ([int]($x0 + ($x1 - $x0) * $i / $Steps)) ([int]($y0 + ($y1 - $y0) * (0.5 + 0.5 * [math]::Sin($i / 8.0)) )); Start-Sleep -Milliseconds $StepMs }
}
# Sends one key transition with a proper scan code; navigation keys (PageUp..Down, arrows, Insert, Delete) carry the
# extended-key flag. Without both, arrows are seen as numpad keys and Shift+Arrow does not extend a selection.
function Send-Vk([byte] $vk, [bool] $up) {
    $scan = [byte][W32]::MapVirtualKey($vk, 0)
    $ext = (($vk -ge 0x21 -and $vk -le 0x28) -or $vk -eq 0x2D -or $vk -eq 0x2E)
    $flags = [uint32]0; if ($ext) { $flags = $flags -bor 1 }; if ($up) { $flags = $flags -bor 2 }
    [W32]::keybd_event($vk, $scan, $flags, [UIntPtr]::Zero)
}
function Press-Key([byte] $vk, [switch] $Ctrl, [switch] $Shift, [switch] $Alt) {
    if ($Ctrl) { Send-Vk 0xA2 $false }
    if ($Shift) { Send-Vk 0xA0 $false }
    if ($Alt) { Send-Vk 0xA4 $false }
    Send-Vk $vk $false; Start-Sleep -Milliseconds 30
    Send-Vk $vk $true
    if ($Alt) { Send-Vk 0xA4 $true }
    if ($Shift) { Send-Vk 0xA0 $true }
    if ($Ctrl) { Send-Vk 0xA2 $true }
    Start-Sleep -Milliseconds 60
}
function Focus-Window([IntPtr] $h) { [void][W32]::ShowWindow($h, 9); [void][W32]::SetForegroundWindow($h); Start-Sleep -Milliseconds 200 }
function Save-Screenshot([int] $x, [int] $y, [int] $w, [int] $h, [string] $Path) {
    $bmp = New-Object Drawing.Bitmap $w, $h
    $g = [Drawing.Graphics]::FromImage($bmp); $g.CopyFromScreen($x, $y, 0, 0, $bmp.Size); $g.Dispose()
    $bmp.Save($Path, [Drawing.Imaging.ImageFormat]::Png); $bmp.Dispose()
}
function Save-ClientShot([IntPtr] $win, [string] $Path) { $b = Get-ClientBox $win; Save-Screenshot $b.X $b.Y $b.W $b.H $Path }

# Types Unicode text as KEYEVENTF_UNICODE packets (layout independent: '[' works on a Swiss-German keyboard too).
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class W32Text {
    [StructLayout(LayoutKind.Explicit, Size = 40)]
    public struct INPUT {
        [FieldOffset(0)] public uint type;
        [FieldOffset(8)] public ushort wVk;
        [FieldOffset(10)] public ushort wScan;
        [FieldOffset(12)] public uint dwFlags;
        [FieldOffset(16)] public uint time;
        [FieldOffset(24)] public IntPtr extra;
    }
    [DllImport("user32.dll", SetLastError = true)] public static extern uint SendInput(uint n, INPUT[] inputs, int size);
    public static void Char(char c) {
        var a = new INPUT[2];
        a[0].type = 1; a[0].wScan = c; a[0].dwFlags = 4;        // KEYEVENTF_UNICODE
        a[1].type = 1; a[1].wScan = c; a[1].dwFlags = 4 | 2;    // | KEYEVENTF_KEYUP
        SendInput(2, a, Marshal.SizeOf(typeof(INPUT)));
    }
}
'@
function Send-Text([string] $Text) { foreach ($c in $Text.ToCharArray()) { [W32Text]::Char($c); Start-Sleep -Milliseconds 30 } }
