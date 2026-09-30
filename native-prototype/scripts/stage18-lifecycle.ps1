<#
Stage 18 real-Windows lifecycle verification (release exe, throw-away data dir, no user data).

  stage18-lifecycle.ps1 -Exe <path> -DataDir <empty temp dir> [-Cycles 200] [-Second 50]

Drives the real window/tray with real Win32 messages (WM_CLOSE to the main window, the tray icon's
own callback message for left click, real second process launches) and reports:
  - hide/show cycles: window visibility after each step, process count, private bytes before/after
  - second launches: each must exit quickly, leaving exactly one process
  - concurrent start: N near-simultaneous launches against an empty profile -> exactly one survives
  - tray Quit via the real popup menu (keyboard), process gone + icon gone
#>
param([Parameter(Mandatory)][string]$Exe, [Parameter(Mandatory)][string]$DataDir, [int]$Cycles = 200, [int]$Second = 50, [int]$Race = 8)
$ErrorActionPreference = 'Stop'
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
using System.Text;
public static class L18 {
    public delegate bool EnumProc(IntPtr h, IntPtr l);
    [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc p, IntPtr l);
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetWindowText(IntPtr h, StringBuilder s, int n);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
    [DllImport("user32.dll")] public static extern bool IsIconic(IntPtr h);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetClassName(IntPtr h, StringBuilder s, int n);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern IntPtr FindWindowEx(IntPtr p, IntPtr a, string c, string t);
    [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h, uint m, IntPtr w, IntPtr l);
    [DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint flags, UIntPtr extra);
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
    public static IntPtr MainWindow(uint pid) {
        IntPtr found = IntPtr.Zero;
        EnumWindows((h, l) => {
            uint p; GetWindowThreadProcessId(h, out p);
            if (p != pid) return true;
            var sb = new StringBuilder(256); GetClassName(h, sb, 256);
            var tt = new StringBuilder(256); GetWindowText(h, tt, 256);
            if (tt.ToString().StartsWith("Study Tracker")) { found = h; return false; }
            return true;
        }, IntPtr.Zero);
        return found;
    }
    public static void Key(byte vk) { keybd_event(vk, 0, 0, UIntPtr.Zero); keybd_event(vk, 0, 2, UIntPtr.Zero); }
}
'@
function Fnv([string]$s) {
    [uint64]$h = 14695981039346656037
    foreach ($b in [Text.Encoding]::UTF8.GetBytes($s)) { $h = $h -bxor [uint64]$b; $h = [uint64](([System.Numerics.BigInteger]$h * 1099511628211) % [System.Numerics.BigInteger]::Pow(2, 64)) }
    $h
}
$canon = $DataDir.Replace('/', '\').TrimEnd('\').ToLowerInvariant()
$class = "com.damcha.studytracker.native-shell.{0:x16}.platform-window" -f (Fnv $canon)
function Msg-Window { [L18]::FindWindowEx([IntPtr](-3), [IntPtr]::Zero, $class, [NullString]::Value) }
function Procs { @(Get-Process -Name ([IO.Path]::GetFileNameWithoutExtension($Exe)) -ErrorAction SilentlyContinue) }
function Wait-Until([scriptblock]$c, [int]$ms = 8000) { $sw = [Diagnostics.Stopwatch]::StartNew(); while ($sw.ElapsedMilliseconds -lt $ms) { if (& $c) { return $true }; Start-Sleep -Milliseconds 25 }; $false }

New-Item -ItemType Directory -Force $DataDir | Out-Null
$env:STUDY_NATIVE_DATA_DIR = $DataDir
$env:STUDY_NATIVE_TIMER_AUTOSTART = '1'     # a running focus session => closing hides to tray
Get-Process -Name ([IO.Path]::GetFileNameWithoutExtension($Exe)) -ErrorAction SilentlyContinue | Stop-Process -Force
Start-Sleep -Milliseconds 300

# ---- concurrent start race --------------------------------------------------------------------
$launched = 1..$Race | ForEach-Object { Start-Process -FilePath $Exe -PassThru }
Start-Sleep -Seconds 4
"RACE launched=$Race survivors=$((Procs).Count)"
$primary = (Procs)[0]
$hwnd = Msg-Window
"RACE tray_window=$($hwnd -ne [IntPtr]::Zero)"

# ---- hide / show cycles ----------------------------------------------------------------------
$main = [L18]::MainWindow([uint32]$primary.Id)
"MAIN window_found=$($main -ne [IntPtr]::Zero) visible=$([L18]::IsWindowVisible($main))"
function Sample { $p = Get-Process -Id $primary.Id; [pscustomobject]@{ PrivateMB = [math]::Round($p.PrivateMemorySize64 / 1MB, 1); WorkingMB = [math]::Round($p.WorkingSet64 / 1MB, 1); Threads = $p.Threads.Count; Handles = $p.HandleCount } }
$before = Sample
$bad = 0
for ($i = 0; $i -lt $Cycles; $i++) {
    [void][L18]::PostMessage($main, 0x0010, [IntPtr]::Zero, [IntPtr]::Zero)          # WM_CLOSE -> hide to tray
    if (-not (Wait-Until { -not [L18]::IsWindowVisible($main) } 3000)) { $bad++; "CYCLE $i did not hide"; continue }
    [void][L18]::PostMessage((Msg-Window), 0x8002, [IntPtr]1, [IntPtr]0x0202)       # tray callback: WM_LBUTTONUP
    if (-not (Wait-Until { [L18]::IsWindowVisible($main) } 3000)) { $bad++; "CYCLE $i did not show" }
}
$after = Sample
"CYCLES n=$Cycles failures=$bad procs=$((Procs).Count)"
"CYCLES before=$($before | ConvertTo-Json -Compress)"
"CYCLES after=$($after | ConvertTo-Json -Compress)"

# ---- second launches ---------------------------------------------------------------------------
$env:STUDY_NATIVE_INSTANCE_REPORT = '1'
[void][L18]::PostMessage($main, 0x0010, [IntPtr]::Zero, [IntPtr]::Zero)
[void](Wait-Until { -not [L18]::IsWindowVisible($main) } 3000)                       # start hidden
$times = @(); $fail = 0
for ($i = 0; $i -lt $Second; $i++) {
    $sw = [Diagnostics.Stopwatch]::StartNew()
    $p = Start-Process -FilePath $Exe -PassThru -Wait -RedirectStandardOutput "$env:TEMP\st18-second.txt" -WindowStyle Hidden
    $sw.Stop(); $times += $sw.ElapsedMilliseconds
    if ((Procs).Count -ne 1 -or -not [L18]::IsWindowVisible($main)) { $fail++ }
    [void][L18]::PostMessage($main, 0x0010, [IntPtr]::Zero, [IntPtr]::Zero)
    [void](Wait-Until { -not [L18]::IsWindowVisible($main) } 3000)
}
$sorted = $times | Sort-Object
"SECOND n=$Second failures=$fail median_ms=$($sorted[[int]($Second / 2)]) max_ms=$($sorted[-1]) procs=$((Procs).Count)"
$later = Sample
"SECOND after=$($later | ConvertTo-Json -Compress)"

# ---- tray Quit through the real popup menu -----------------------------------------------------
[void][L18]::PostMessage((Msg-Window), 0x8002, [IntPtr]1, [IntPtr]0x0205)            # WM_RBUTTONUP -> popup menu
Start-Sleep -Milliseconds 600
[L18]::Key(0x28); Start-Sleep -Milliseconds 100; [L18]::Key(0x28); Start-Sleep -Milliseconds 100; [L18]::Key(0x0D)   # Down, Down, Enter (Show, Quit)
$gone = Wait-Until { (Procs).Count -eq 0 } 6000
"QUIT process_exited=$gone tray_window_gone=$((Msg-Window) -eq [IntPtr]::Zero)"
Get-Process -Name ([IO.Path]::GetFileNameWithoutExtension($Exe)) -ErrorAction SilentlyContinue | Stop-Process -Force
