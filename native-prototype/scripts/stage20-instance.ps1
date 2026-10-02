<#
Stage 20 single-instance / tray regression on Windows (stage18-lifecycle.ps1 with the Break Room
open). Throw-away profile, synthetic fixture, release exe; every launch carries the same import
and Break Room environment, so a secondary that wrongly became a writer would show up as a store
change. Never sends global keyboard input: the tray menu is driven by posting keys to the popup
menu window itself.

  stage20-instance.ps1 -Fixture <full.json> [-Race 8] [-Second 20]
Reports: race survivors, second-launch activation (hidden -> visible), store.json hash before and
after the second launches, Break Room/Timer state, tray icon count, tray Quit, icon cleanup.
#>
param([Parameter(Mandatory)][string]$Fixture, [int]$Race = 8, [int]$Second = 20,
      [string]$Exe = (Join-Path $PSScriptRoot '..\target\release\study-tracker-native-prototype.exe'))
$ErrorActionPreference = 'Stop'
Add-Type -TypeDefinition @'
using System; using System.Runtime.InteropServices; using System.Text;
public static class L20i {
    public delegate bool EnumProc(IntPtr h, IntPtr l);
    [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc p, IntPtr l);
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetWindowText(IntPtr h, StringBuilder s, int n);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetClassName(IntPtr h, StringBuilder s, int n);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern IntPtr FindWindowEx(IntPtr p, IntPtr a, string c, string t);
    [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h, uint m, IntPtr w, IntPtr l);
    public static IntPtr MainWindow(uint pid) {
        IntPtr found = IntPtr.Zero;
        EnumWindows((h, l) => { uint p; GetWindowThreadProcessId(h, out p); if (p != pid) return true;
            var tt = new StringBuilder(256); GetWindowText(h, tt, 256);
            if (tt.ToString().StartsWith("Study Tracker")) { found = h; return false; } return true; }, IntPtr.Zero);
        return found;
    }
    // the visible popup menu (#32768) belonging to process `pid`
    public static IntPtr PopupMenu(uint pid) {
        IntPtr found = IntPtr.Zero;
        EnumWindows((h, l) => { uint p; GetWindowThreadProcessId(h, out p); if (p != pid || !IsWindowVisible(h)) return true;
            var c = new StringBuilder(64); GetClassName(h, c, 64);
            if (c.ToString() == "#32768") { found = h; return false; } return true; }, IntPtr.Zero);
        return found;
    }
}
'@
function Fnv([string]$s) { [uint64]$h = 14695981039346656037; foreach ($b in [Text.Encoding]::UTF8.GetBytes($s)) { $h = $h -bxor [uint64]$b; $h = [uint64](([System.Numerics.BigInteger]$h * 1099511628211) % [System.Numerics.BigInteger]::Pow(2, 64)) }; $h }
function Wait-Until([scriptblock]$c, [int]$ms = 8000) { $sw = [Diagnostics.Stopwatch]::StartNew(); while ($sw.ElapsedMilliseconds -lt $ms) { if (& $c) { return $true }; Start-Sleep -Milliseconds 25 }; $false }

$Exe = (Resolve-Path $Exe).Path
$name = [IO.Path]::GetFileNameWithoutExtension($Exe)
function Procs { @(Get-Process -Name $name -ErrorAction SilentlyContinue) }
if ((Procs).Count) { throw "another $name is running; refusing to start" }
$data = Join-Path $env:TEMP ("st20-inst-" + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory $data | Out-Null
$class = "com.damcha.studytracker.native-shell.{0:x16}.platform-window" -f (Fnv $data.ToLowerInvariant())
function Msg { [L20i]::FindWindowEx([IntPtr](-3), [IntPtr]::Zero, $class, [NullString]::Value) }
function Tray { & (Join-Path $PSScriptRoot 'win-tray-probe.ps1') -DataDir $data }
function StoreHash { (Get-FileHash (Join-Path $data 'store.json') -Algorithm SHA256).Hash.Substring(0, 16) }
$envs = @{ STUDY_NATIVE_DATA_DIR = $data; STUDY_NATIVE_IMPORT_BACKUP = (Resolve-Path $Fixture).Path; STUDY_NATIVE_NOW = '2026-09-30T12:00:00-07:00'
           STUDY_NATIVE_VIEW = 'break'; STUDY_NATIVE_OPEN_GAME = '1'; STUDY_NATIVE_TIMER_AUTOSTART = '1'; STUDY_NATIVE_INSTANCE_REPORT = '1' }
foreach ($k in $envs.Keys) { [Environment]::SetEnvironmentVariable($k, $envs[$k]) }
try {
    # ---- near-simultaneous launches against the empty profile
    1..$Race | ForEach-Object { Start-Process -FilePath $Exe -WindowStyle Hidden | Out-Null }
    Start-Sleep -Seconds 6
    "RACE launched=$Race survivors=$((Procs).Count)"
    $primary = (Procs)[0]
    $main = [L20i]::MainWindow([uint32]$primary.Id)
    "PRIMARY main=$($main -ne [IntPtr]::Zero) tray: $(Tray)"
    $before = StoreHash
    $plays0 = ((Get-Content (Join-Path $data 'store.json') -Raw | ConvertFrom-Json).break_room.playedBreaks | Measure-Object).Count
    # ---- second launches: primary hidden in the tray each time, must come back
    $fail = 0; $times = @()
    for ($i = 0; $i -lt $Second; $i++) {
        [void][L20i]::PostMessage($main, 0x0010, [IntPtr]::Zero, [IntPtr]::Zero)
        if (-not (Wait-Until { -not [L20i]::IsWindowVisible($main) } 3000)) { $fail++; continue }
        $sw = [Diagnostics.Stopwatch]::StartNew()
        Start-Process -FilePath $Exe -Wait -WindowStyle Hidden | Out-Null
        $times += $sw.ElapsedMilliseconds
        if (-not (Wait-Until { [L20i]::IsWindowVisible($main) } 3000) -or (Procs).Count -ne 1) { $fail++ }
    }
    $after = StoreHash
    $plays1 = ((Get-Content (Join-Path $data 'store.json') -Raw | ConvertFrom-Json).break_room.playedBreaks | Measure-Object).Count
    $sorted = $times | Sort-Object
    "SECOND n=$Second failures=$fail procs=$((Procs).Count) median_ms=$($sorted[[int]($sorted.Count / 2)]) max_ms=$($sorted[-1])"
    "STORE before=$before after=$after unchanged=$($before -eq $after) playedBreaks $plays0 -> $plays1"
    "TRAY after second launches: $(Tray)"
    # ---- tray Quit through the real popup menu, keys posted to the menu window only
    [void][L20i]::PostMessage((Msg), 0x8002, [IntPtr]1, [IntPtr]0x0205)
    $menuUp = Wait-Until { [L20i]::PopupMenu([uint32]$primary.Id) -ne [IntPtr]::Zero } 3000
    $menu = [L20i]::PopupMenu([uint32]$primary.Id)
    "MENU shown=$menuUp"
    if ($menuUp) {
        foreach ($vk in 0x28, 0x28, 0x0D) { [void][L20i]::PostMessage($menu, 0x0100, [IntPtr]$vk, [IntPtr]0); Start-Sleep -Milliseconds 150 }
    }
    $gone = Wait-Until { (Procs).Count -eq 0 } 8000
    $code = try { (Get-Process -Id $primary.Id -ErrorAction Stop) | Out-Null; 'running' } catch { 'exited' }
    "QUIT process_exited=$gone msg_window_gone=$((Msg) -eq [IntPtr]::Zero) tray: $(Tray)"
    $log = Get-Content (Join-Path $data 'logs\study-tracker.log') -Tail 3
    "LOG tail: $($log[-1])"
    "PANICS in log: $(@(Select-String -Path (Join-Path $data 'logs\study-tracker.log') -Pattern 'panic').Count)"
} finally {
    foreach ($k in $envs.Keys) { [Environment]::SetEnvironmentVariable($k, $null) }
    Procs | Stop-Process -Force
    Start-Sleep -Milliseconds 500
    Remove-Item -Recurse -Force $data -ErrorAction SilentlyContinue
}
