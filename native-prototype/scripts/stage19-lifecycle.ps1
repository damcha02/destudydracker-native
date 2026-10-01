<#
Stage 19 lifecycle stress with Wabi-Sabi + Sakura and a running Timer (release exe, throw-away data
directory, synthetic fixture). Real Win32 messages only:
  - N x (WM_CLOSE -> hidden to tray -> tray icon left click -> shown)
  - N x (ShowWindow SW_MINIMIZE -> SW_RESTORE)
  - a 20 s hold hidden in the tray and a 20 s hold minimized, measuring frames/Sakura ticks/CPU
Reports memory before/after/plateau, Sakura clock starts/stops (one clock: starts - stops <= 1),
live petal count, and that the process stayed single.

  stage19-lifecycle.ps1 [-Cycles 200]
#>
param(
    [string] $Exe = (Join-Path $PSScriptRoot '..\target\release\study-tracker-native-prototype.exe'),
    [int] $Cycles = 200
)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'win-metrics.ps1')
Add-Type -TypeDefinition @'
using System; using System.Runtime.InteropServices; using System.Text;
public static class L19 {
    public delegate bool EnumProc(IntPtr h, IntPtr l);
    [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc p, IntPtr l);
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetWindowText(IntPtr h, StringBuilder s, int n);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
    [DllImport("user32.dll")] public static extern bool IsIconic(IntPtr h);
    [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int cmd);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern IntPtr FindWindowEx(IntPtr p, IntPtr a, string c, string t);
    [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h, uint m, IntPtr w, IntPtr l);
    public static IntPtr MainWindow(uint pid) {
        IntPtr found = IntPtr.Zero;
        EnumWindows((h, l) => { uint p; GetWindowThreadProcessId(h, out p); if (p != pid) return true;
            var tt = new StringBuilder(256); GetWindowText(h, tt, 256);
            if (tt.ToString().StartsWith("Study Tracker")) { found = h; return false; } return true; }, IntPtr.Zero);
        return found;
    }
}
'@
function Fnv([string]$s) {
    [uint64]$h = 14695981039346656037
    foreach ($b in [Text.Encoding]::UTF8.GetBytes($s)) { $h = $h -bxor [uint64]$b; $h = [uint64](([System.Numerics.BigInteger]$h * 1099511628211) % [System.Numerics.BigInteger]::Pow(2, 64)) }
    $h
}
function Wait-Until([scriptblock]$c, [int]$ms = 5000) { $sw = [Diagnostics.Stopwatch]::StartNew(); while ($sw.ElapsedMilliseconds -lt $ms) { if (& $c) { return $true }; Start-Sleep -Milliseconds 20 }; $false }

$Exe = (Resolve-Path $Exe).Path
$fx = (Resolve-Path (Join-Path $PSScriptRoot '..\tests\fixtures\dashboard\wabi.json')).Path
$data = Join-Path $env:TEMP ("st19-life-" + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory $data | Out-Null
$class = "com.damcha.studytracker.native-shell.{0:x16}.platform-window" -f (Fnv $data.Replace('/', '\').TrimEnd('\').ToLowerInvariant())
$stdout = Join-Path $data 'stdout.txt'
$envs = @{ STUDY_NATIVE_DATA_DIR = $data; STUDY_NATIVE_IMPORT_BACKUP = $fx; STUDY_NATIVE_STYLE = 'wabi-sabi'; STUDY_NATIVE_PALETTE = 'sakura';
           STUDY_NATIVE_VIEW = 'timer'; STUDY_NATIVE_TIMER_MODE = '2'; STUDY_NATIVE_TIMER_AUTOSTART = '1'; STUDY_NATIVE_FRAME_STATS = '1' }
foreach ($k in $envs.Keys) { [Environment]::SetEnvironmentVariable($k, $envs[$k]) }
$p = Start-Process -FilePath $Exe -PassThru -RedirectStandardOutput $stdout
foreach ($k in $envs.Keys) { [Environment]::SetEnvironmentVariable($k, $null) }
try {
    Start-Sleep -Seconds 4
    $main = [L19]::MainWindow([uint32]$p.Id)
    $msg = [L19]::FindWindowEx([IntPtr](-3), [IntPtr]::Zero, $class, [NullString]::Value)
    function Mem { $s = Get-TreeSnapshot $p.Id; "{0:N1}/{1:N1}MB thr={2}" -f $s.PrivateWS, $s.PrivateBytes, $s.Threads }
    "START main=$($main -ne [IntPtr]::Zero) tray=$($msg -ne [IntPtr]::Zero) mem=$(Mem)"
    $bad = 0; $marks = @()
    for ($i = 0; $i -lt $Cycles; $i++) {
        [void][L19]::PostMessage($main, 0x0010, [IntPtr]::Zero, [IntPtr]::Zero)
        if (-not (Wait-Until { -not [L19]::IsWindowVisible($main) })) { $bad++ }
        [void][L19]::PostMessage($msg, 0x8002, [IntPtr]1, [IntPtr]0x0202)
        if (-not (Wait-Until { [L19]::IsWindowVisible($main) })) { $bad++ }
        if ($i % 50 -eq 49) { $marks += "tray#$($i + 1)=$(Mem)" }
    }
    "TRAY cycles=$Cycles failures=$bad procs=$(@(Get-Process -Id $p.Id -ErrorAction SilentlyContinue).Count)"
    $bad = 0
    for ($i = 0; $i -lt $Cycles; $i++) {
        [void][L19]::ShowWindow($main, 6)
        if (-not (Wait-Until { [L19]::IsIconic($main) })) { $bad++ }
        [void][L19]::ShowWindow($main, 9)
        if (-not (Wait-Until { -not [L19]::IsIconic($main) })) { $bad++ }
        if ($i % 50 -eq 49) { $marks += "min#$($i + 1)=$(Mem)" }
    }
    "MINIMIZE cycles=$Cycles failures=$bad"
    $marks | ForEach-Object { "MEM $_" }
    Start-Sleep -Seconds 2
    $visible = Measure-Tree $p.Id 20 'visible'
    [void][L19]::PostMessage($main, 0x0010, [IntPtr]::Zero, [IntPtr]::Zero); Start-Sleep -Seconds 1
    $hidden = Measure-Tree $p.Id 20 'tray'
    [void][L19]::PostMessage($msg, 0x8002, [IntPtr]1, [IntPtr]0x0202); Start-Sleep -Seconds 1
    [void][L19]::ShowWindow($main, 6); Start-Sleep -Seconds 1
    $minimized = Measure-Tree $p.Id 20 'minimized'
    [void][L19]::ShowWindow($main, 9); Start-Sleep -Seconds 3
    "HOLD visible cpu=$($visible.CpuPctOfOneCore)% tray cpu=$($hidden.CpuPctOfOneCore)% minimized cpu=$($minimized.CpuPctOfOneCore)% end mem=$(Mem)"
    $stats = @(Get-Content $stdout | Where-Object { $_ -like 'STATS*' })
    $stats | Select-Object -Last 9 | ForEach-Object { "  $_" }
} finally {
    if (-not $p.HasExited) { $p.Kill() }
    Start-Sleep -Milliseconds 500
    Remove-Item -Recurse -Force $data -ErrorAction SilentlyContinue
}
