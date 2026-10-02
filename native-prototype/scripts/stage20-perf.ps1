<#
.SYNOPSIS
  Stage 20 Windows runtime measurements (Break Room, Wabi Rest, album, games). Same methodology as
  stage19-perf.ps1: release exe, a throw-away data directory per scenario, synthetic fixtures only
  (scripts/visual-parity/gen-break-fixture.mjs) - never a real profile. State is set with the app's
  diagnostic environment hooks (STUDY_NATIVE_VIEW=break, _REST_ROOM, _OPEN_GAME, _INPUT); minimize
  and hide-to-tray use real Win32 messages.

  Per scenario: process count, threads, Private WS / Private Bytes (MB), CPU % of one core over the
  window, frames rendered inside the window (STUDY_NATIVE_FRAME_STATS lines whose 10 s interval
  lies wholly inside it), Sakura ticks, whether store.json was written during the window, and the
  Break Room counters (writes/pushes/evaluations) from the last STATS line.

  stage20-perf.ps1 -Fixtures <dir with full.json/unlocked.json> [-Only P2] [-Seconds 20]
                   [-Now 2026-09-30T12:00:00-07:00]
#>
param(
    [Parameter(Mandatory)][string] $Fixtures,
    [string] $Exe = (Join-Path $PSScriptRoot '..\target\release\study-tracker-native-prototype.exe'),
    [int] $Seconds = 20,
    [string] $Only = '',
    [string] $Now = '2026-09-30T12:00:00-07:00',
    [string] $Out = (Join-Path $env:TEMP 'stage20_perf.json')
)
. (Join-Path $PSScriptRoot 'win-metrics.ps1')
Add-Type -TypeDefinition @'
using System; using System.Runtime.InteropServices;
public static class W20 {
    [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int cmd);
    [DllImport("user32.dll")] public static extern bool IsIconic(IntPtr h);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
    [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h, uint m, IntPtr w, IntPtr l);
}
'@
$Exe = (Resolve-Path $Exe).Path
$Fixtures = (Resolve-Path $Fixtures).Path
$results = New-Object System.Collections.Generic.List[object]

function Run-Scenario([string]$Name, [hashtable]$Env, [string]$Action = 'none', [int]$Window = $Seconds, [int]$Settle = 5) {
    if ($Only -and $Name -notlike "*$Only*") { return }
    $data = Join-Path $env:TEMP ("st20-" + [Guid]::NewGuid().ToString('N'))
    New-Item -ItemType Directory $data | Out-Null
    $all = @{ STUDY_NATIVE_DATA_DIR = $data; STUDY_NATIVE_FRAME_STATS = '1'; STUDY_NATIVE_NOW = $Now; STUDY_NATIVE_BREAK_PICK = '0.25'
              STUDY_NATIVE_IMPORT_BACKUP = "$Fixtures\full.json" }
    foreach ($k in $Env.Keys) { $all[$k] = $Env[$k] }
    $stdout = Join-Path $data 'stdout.txt'
    $saved = @{}
    foreach ($k in $all.Keys) { $saved[$k] = [Environment]::GetEnvironmentVariable($k); [Environment]::SetEnvironmentVariable($k, [string]$all[$k]) }
    $clock = [Diagnostics.Stopwatch]::StartNew()
    $p = Start-Process -FilePath $Exe -PassThru -RedirectStandardOutput $stdout
    foreach ($k in $all.Keys) { [Environment]::SetEnvironmentVariable($k, $saved[$k]) }
    try {
        Start-Sleep -Seconds $Settle
        $p.Refresh(); $h = $p.MainWindowHandle
        switch ($Action) {
            'minimize' { [void][W20]::ShowWindow($h, 6); Start-Sleep -Seconds 1 }
            'tray'     { [void][W20]::PostMessage($h, 0x0010, [IntPtr]::Zero, [IntPtr]::Zero); Start-Sleep -Seconds 1 }
        }
        $store = Join-Path $data 'store.json'
        $storeBefore = if (Test-Path $store) { (Get-Item $store).LastWriteTimeUtc.Ticks } else { 0 }
        $startSec = $clock.Elapsed.TotalSeconds
        $m = Measure-Tree $p.Id $Window $Name
        $endSec = $clock.Elapsed.TotalSeconds
        Start-Sleep -Milliseconds 1500
        $frames = 0; $sakura = 0; $intervals = 0; $last = ''
        $lines = @(Get-Content $stdout -ErrorAction SilentlyContinue)
        foreach ($l in $lines) {
            if ($l -match '^STATS (\d+) frames=(\d+) ticks=(\d+) sakura_ticks=(\d+)') {
                $t = [int]$Matches[1]
                if ($t - 10 -ge $startSec - 0.5 -and $t -le $endSec + 0.5) { $frames += [int]$Matches[2]; $sakura += [int]$Matches[4]; $intervals++ }
                $last = $l
            }
        }
        $storeAfter = if (Test-Path $store) { (Get-Item $store).LastWriteTimeUtc.Ticks } else { 0 }
        $break = if ($last -match '(break_writes=\S+ break_pushes=\S+ break_evaluations=\S+)') { $Matches[1] } else { '' }
        $live = if ($last -match 'live_petals=(\d+)') { [int]$Matches[1] } else { -1 }
        $secs = [math]::Max(1, $intervals * 10)
        $results.Add([pscustomobject]@{
            Scenario = $Name; Procs = $m.Procs; Threads = $m.Threads; PrivWS_MB = $m.PrivateWSMB; PrivBytes_MB = $m.PrivateBytesMB
            Cpu_pct_1core = $m.CpuPctOfOneCore; Iconic = [W20]::IsIconic($h); Visible = [W20]::IsWindowVisible($h)
            Stats_s = $intervals * 10; Frames = $frames; Fps = [math]::Round($frames / $secs, 2); SakuraTicks = $sakura; LivePetals = $live
            StoreWritten = ($storeAfter -ne $storeBefore); Break = $break
            Notes = ($lines | Where-Object { $_ -like '*done*' }) -join '; '
        })
        Write-Host ("{0}: {1:N2}% CPU, {2}/{3} MB, frames {4} in {5}s, sakura {6}, store written {7}, {8}" -f $Name, $m.CpuPctOfOneCore, $m.PrivateWSMB, $m.PrivateBytesMB, $frames, ($intervals * 10), $sakura, ($storeAfter -ne $storeBefore), $break)
    } finally {
        if (-not $p.HasExited) { $p.Kill() }
        Start-Sleep -Milliseconds 600
        Remove-Item -Recurse -Force $data -ErrorAction SilentlyContinue
    }
}
function With([hashtable]$base, [hashtable]$extra) { $h = @{}; foreach ($k in $base.Keys) { $h[$k] = $base[$k] }; foreach ($k in $extra.Keys) { $h[$k] = $extra[$k] }; $h }

$fn   = @{ STUDY_NATIVE_VIEW = 'break' }
$wabi = @{ STUDY_NATIVE_VIEW = 'break'; STUDY_NATIVE_STYLE = 'wabi-sabi' }
$timer = @{ STUDY_NATIVE_TIMER_MODE = '2'; STUDY_NATIVE_TIMER_AUTOSTART = '1' }   # 120 min exam: runs through the window

# Static surfaces (target: 0 frames after settle).
Run-Scenario 'B20W-S1 FN Break Room (full)'          $fn
Run-Scenario 'B20W-S1b FN Break Room (unlocked)'     (With $fn @{ STUDY_NATIVE_IMPORT_BACKUP = "$Fixtures\unlocked.json" })
Run-Scenario 'B20W-S2 Wabi Games light'              (With $wabi @{ STUDY_NATIVE_THEME = 'light'; STUDY_NATIVE_REST_ROOM = 'games' })
Run-Scenario 'B20W-S2b Wabi Games dark'              (With $wabi @{ STUDY_NATIVE_THEME = 'dark'; STUDY_NATIVE_REST_ROOM = 'games' })
Run-Scenario 'B20W-S3 Wabi Meditation idle'          (With $wabi @{ STUDY_NATIVE_REST_ROOM = 'meditation' })
Run-Scenario 'B20W-S4 Album closed'                  (With $wabi @{ STUDY_NATIVE_REST_ROOM = 'achievements' })
Run-Scenario 'B20W-S4b Album open spread'            (With $wabi @{ STUDY_NATIVE_REST_ROOM = 'album-open' })
$games = @('Durak', 'Wordle', 'Travle', 'Flaggle', 'Skribbl', 'Geodle')
for ($i = 0; $i -lt 6; $i++) { Run-Scenario "B20W-S5.$i FN $($games[$i]) open" (With $fn @{ STUDY_NATIVE_OPEN_GAME = "$i" }) }
for ($i = 0; $i -lt 6; $i++) { Run-Scenario "B20W-S6.$i Wabi $($games[$i]) open" (With $wabi @{ STUDY_NATIVE_REST_ROOM = 'games'; STUDY_NATIVE_OPEN_GAME = "$i" }) }
# Timer running underneath a Break Room surface: Break Room must not repaint for Timer ticks.
Run-Scenario 'B20W-S7 FN Break Room + Timer running' (With (With $fn $timer) @{})
# Sakura with Stage 20 surfaces (Sakura is animated by design: 24 Hz logical clock).
Run-Scenario 'B20W-K1 Wabi Games + Sakura visible'   (With $wabi @{ STUDY_NATIVE_PALETTE = 'sakura'; STUDY_NATIVE_REST_ROOM = 'games' })
Run-Scenario 'B20W-K2 FN Break Room + Sakura'        (With $fn @{ STUDY_NATIVE_PALETTE = 'sakura' })
# Minimized / tray-hidden with an open game or the album, Timer running (tray needs a running session).
Run-Scenario 'B20W-M1 Wabi album open + Sakura + Timer, minimized' (With (With $wabi $timer) @{ STUDY_NATIVE_PALETTE = 'sakura'; STUDY_NATIVE_REST_ROOM = 'album-open' }) 'minimize'
Run-Scenario 'B20W-M2 FN Durak open + Timer, minimized'            (With (With $fn $timer) @{ STUDY_NATIVE_OPEN_GAME = '0' }) 'minimize'
Run-Scenario 'B20W-T1 Wabi album open + Sakura + Timer, tray'      (With (With $wabi $timer) @{ STUDY_NATIVE_PALETTE = 'sakura'; STUDY_NATIVE_REST_ROOM = 'album-open' }) 'tray'
Run-Scenario 'B20W-T2 FN Wordle open + Timer, tray'                (With (With $fn $timer) @{ STUDY_NATIVE_OPEN_GAME = '1' }) 'tray'

$results | Format-Table Scenario, Procs, Threads, PrivWS_MB, PrivBytes_MB, Cpu_pct_1core, Iconic, Visible, Stats_s, Frames, SakuraTicks, LivePetals, StoreWritten -AutoSize | Out-String -Width 300
$results | ConvertTo-Json | Set-Content $Out
