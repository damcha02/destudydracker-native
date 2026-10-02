<#
Stage 20 lifecycle stress (Windows). stage19-lifecycle.ps1 with the Stage 20 surfaces visited first:
the app starts in Wabi-Sabi + Sakura with a running Timer and the Break Room stress hook
(STUDY_NATIVE_BREAK_STRESS) walks every Break Room surface - FN page, all six game screens, Wabi
Games/Meditation/Achievements, album open/close - before the window lifecycle starts. Then, with
real Win32 messages only:
  - N x (WM_CLOSE -> hidden to tray -> tray icon left click -> shown)
  - N x (ShowWindow SW_MINIMIZE -> SW_RESTORE)
  - 20 s holds visible / hidden in the tray / minimized, measuring frames, Sakura ticks and CPU
Reports memory per 50 cycles, Sakura clock starts/stops (one clock: starts - stops <= 1), live
petals, Break Room writes/pushes, tray icon presence and the process count.

  stage20-lifecycle.ps1 -Fixture <full.json> [-Cycles 200] [-StressCycles 20] [-View break|album]
#>
param(
    [Parameter(Mandatory)][string] $Fixture,
    [string] $Exe = (Join-Path $PSScriptRoot '..\target\release\study-tracker-native-prototype.exe'),
    [int] $Cycles = 200,
    [int] $StressCycles = 20,
    [string] $Room = 'album-open'
)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'win-metrics.ps1')
Add-Type -TypeDefinition @'
using System; using System.Runtime.InteropServices; using System.Text;
public static class L20 {
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
$fx = (Resolve-Path $Fixture).Path
$data = Join-Path $env:TEMP ("st20-life-" + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory $data | Out-Null
$class = "com.damcha.studytracker.native-shell.{0:x16}.platform-window" -f (Fnv $data.Replace('/', '\').TrimEnd('\').ToLowerInvariant())
$stdout = Join-Path $data 'stdout.txt'
$envs = @{ STUDY_NATIVE_DATA_DIR = $data; STUDY_NATIVE_IMPORT_BACKUP = $fx; STUDY_NATIVE_STYLE = 'wabi-sabi'; STUDY_NATIVE_PALETTE = 'sakura'
           STUDY_NATIVE_VIEW = 'break'; STUDY_NATIVE_REST_ROOM = $Room; STUDY_NATIVE_TIMER_MODE = '2'; STUDY_NATIVE_TIMER_AUTOSTART = '1'
           STUDY_NATIVE_FRAME_STATS = '1'; STUDY_NATIVE_BREAK_STRESS = "$StressCycles" }
foreach ($k in $envs.Keys) { [Environment]::SetEnvironmentVariable($k, $envs[$k]) }
$p = Start-Process -FilePath $Exe -PassThru -RedirectStandardOutput $stdout
foreach ($k in $envs.Keys) { [Environment]::SetEnvironmentVariable($k, $null) }
try {
    $visited = Wait-Until { (Get-Content $stdout -ErrorAction SilentlyContinue) -match 'BREAK_STRESS done' } (60000 + $StressCycles * 1000)
    Start-Sleep -Seconds 2
    $main = [L20]::MainWindow([uint32]$p.Id)
    $msg = [L20]::FindWindowEx([IntPtr](-3), [IntPtr]::Zero, $class, [NullString]::Value)
    function Mem { $s = Get-TreeSnapshot $p.Id; "{0:N1}/{1:N1}MB thr={2}" -f $s.PrivateWS, $s.PrivateBytes, $s.Threads }
    "START surfaces_visited=$visited main=$($main -ne [IntPtr]::Zero) tray=$($msg -ne [IntPtr]::Zero) mem=$(Mem)"
    $tray = & (Join-Path $PSScriptRoot 'win-tray-probe.ps1') -DataDir $data
    "TRAYPROBE $tray"
    $bad = 0; $marks = @()
    for ($i = 0; $i -lt $Cycles; $i++) {
        [void][L20]::PostMessage($main, 0x0010, [IntPtr]::Zero, [IntPtr]::Zero)
        if (-not (Wait-Until { -not [L20]::IsWindowVisible($main) })) { $bad++ }
        [void][L20]::PostMessage($msg, 0x8002, [IntPtr]1, [IntPtr]0x0202)
        if (-not (Wait-Until { [L20]::IsWindowVisible($main) })) { $bad++ }
        if ($i % 50 -eq 49) { $marks += "tray#$($i + 1)=$(Mem)" }
    }
    "TRAY cycles=$Cycles failures=$bad procs=$(@(Get-Process -Name ([IO.Path]::GetFileNameWithoutExtension($Exe)) -ErrorAction SilentlyContinue).Count)"
    $bad = 0
    for ($i = 0; $i -lt $Cycles; $i++) {
        [void][L20]::ShowWindow($main, 6)
        if (-not (Wait-Until { [L20]::IsIconic($main) })) { $bad++ }
        [void][L20]::ShowWindow($main, 9)
        if (-not (Wait-Until { -not [L20]::IsIconic($main) })) { $bad++ }
        if ($i % 50 -eq 49) { $marks += "min#$($i + 1)=$(Mem)" }
    }
    "MINIMIZE cycles=$Cycles failures=$bad"
    $marks | ForEach-Object { "MEM $_" }
    "TRAYPROBE-after $(& (Join-Path $PSScriptRoot 'win-tray-probe.ps1') -DataDir $data)"
    Start-Sleep -Seconds 2
    function Hold([string]$tag) {
        $n0 = @(Get-Content $stdout | Where-Object { $_ -like 'STATS*' }).Count
        $m = Measure-Tree $p.Id 21 $tag
        $new = @(Get-Content $stdout | Where-Object { $_ -like 'STATS*' }) | Select-Object -Skip $n0
        $f = 0; $s = 0; foreach ($l in $new) { if ($l -match 'frames=(\d+) ticks=\d+ sakura_ticks=(\d+)') { $f += [int]$Matches[1]; $s += [int]$Matches[2] } }
        "HOLD $tag cpu=$($m.CpuPctOfOneCore)% frames=$f sakura_ticks=$s over $($new.Count) STATS lines"
    }
    Hold 'visible'
    [void][L20]::PostMessage($main, 0x0010, [IntPtr]::Zero, [IntPtr]::Zero); Start-Sleep -Seconds 1
    Hold 'tray'
    [void][L20]::PostMessage($msg, 0x8002, [IntPtr]1, [IntPtr]0x0202); Start-Sleep -Seconds 1
    [void][L20]::ShowWindow($main, 6); Start-Sleep -Seconds 1
    Hold 'minimized'
    [void][L20]::ShowWindow($main, 9); Start-Sleep -Seconds 1
    Hold 'restored'
    "END mem=$(Mem) procs=$(@(Get-Process -Name ([IO.Path]::GetFileNameWithoutExtension($Exe)) -ErrorAction SilentlyContinue).Count)"
    @(Get-Content $stdout | Where-Object { $_ -like 'STATS*' }) | Select-Object -Last 3 | ForEach-Object { "  $_" }
} finally {
    if (-not $p.HasExited) { $p.Kill() }
    Start-Sleep -Milliseconds 500
    Remove-Item -Recurse -Force $data -ErrorAction SilentlyContinue
}
