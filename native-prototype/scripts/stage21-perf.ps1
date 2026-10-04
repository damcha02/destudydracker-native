<#
.SYNOPSIS
  Stage 21 Windows runtime measurements (Travle). Same methodology as stage20-perf.ps1: release
  exe, a throw-away data directory per scenario, synthetic fixtures only
  (scripts/visual-parity/gen-break-fixture.mjs --travle fresh|mid|won|lost) - never a real profile.
  Travle is opened with STUDY_NATIVE_OPEN_GAME=2 (the card's Play path); minimize and hide-to-tray
  use real Win32 messages; stress runs use STUDY_NATIVE_TRAVLE_STRESS.

  Per scenario: process count, threads, Private WS / Private Bytes (MB), CPU % of one core over the
  window, frames rendered inside the window (STUDY_NATIVE_FRAME_STATS lines whose 10 s interval
  lies wholly inside it), Sakura ticks, whether store.json was written during the window, and the
  Break Room counters from the last STATS line. Stress scenarios also print memory every 5 s while
  the stress runs ("MEM" lines) and the write counter before/after.

  stage21-perf.ps1 -Fixtures <dir with fixture-{fresh,mid,won,lost}.json> [-Only P21W-S1] [-Seconds 20]
                   [-Now 2026-09-30T12:00:00-07:00]
#>
param(
    [Parameter(Mandatory)][string] $Fixtures,
    [string] $Exe = (Join-Path $PSScriptRoot '..\target\release\study-tracker-native-prototype.exe'),
    [int] $Seconds = 20,
    [string] $Only = '',
    [string] $Now = '2026-09-30T12:00:00-07:00',
    [string] $Out = (Join-Path $env:TEMP 'stage21_perf.json')
)
. (Join-Path $PSScriptRoot 'win-metrics.ps1')
Add-Type -TypeDefinition @'
using System; using System.Runtime.InteropServices;
public static class W21 {
    [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int cmd);
    [DllImport("user32.dll")] public static extern bool IsIconic(IntPtr h);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
    [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h, uint m, IntPtr w, IntPtr l);
}
'@
$Exe = (Resolve-Path $Exe).Path
$Fixtures = (Resolve-Path $Fixtures).Path
$results = New-Object System.Collections.Generic.List[object]

function Stats-Report([string[]]$lines) {
    $last = ($lines | Where-Object { $_ -like 'STATS*' } | Select-Object -Last 1)
    if ($last -match '(break_writes=\S+ break_pushes=\S+)') { $Matches[1] } else { '' }
}

# Action: none | minimize | tray | stress (wait for "TRAVLE_STRESS done", sampling memory)
function Run-Scenario([string]$Name, [string]$State, [hashtable]$Env, [string]$Action = 'none', [int]$Window = $Seconds, [int]$Settle = 6) {
    if ($Only -and $Name -notlike "*$Only*") { return }
    $data = Join-Path $env:TEMP ("st21-" + [Guid]::NewGuid().ToString('N'))
    New-Item -ItemType Directory $data | Out-Null
    $all = @{ STUDY_NATIVE_DATA_DIR = $data; STUDY_NATIVE_FRAME_STATS = '1'; STUDY_NATIVE_NOW = $Now; STUDY_NATIVE_BREAK_PICK = '0.25'
              STUDY_NATIVE_IMPORT_BACKUP = "$Fixtures\fixture-$State.json"; STUDY_NATIVE_VIEW = 'break'; STUDY_NATIVE_OPEN_GAME = '2'
              GALLIUM_DRIVER = 'llvmpipe' }
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
        $store = Join-Path $data 'store.json'
        $notes = ''
        switch ($Action) {
            'minimize' { [void][W21]::ShowWindow($h, 6); Start-Sleep -Seconds 1 }
            'tray'     { [void][W21]::PostMessage($h, 0x0010, [IntPtr]::Zero, [IntPtr]::Zero); Start-Sleep -Seconds 1 }
            'stress' {
                $first = Get-TreeSnapshot $p.Id
                $w0 = Stats-Report @(Get-Content $stdout -ErrorAction SilentlyContinue)
                $marks = @(); $sw = [Diagnostics.Stopwatch]::StartNew()
                while ($sw.Elapsed.TotalMinutes -lt 20 -and -not $p.HasExited) {
                    Start-Sleep -Seconds 5
                    $s = Get-TreeSnapshot $p.Id
                    $marks += ("{0:N0}s {1:N1}/{2:N1} thr={3}" -f $sw.Elapsed.TotalSeconds, $s.PrivateWS, $s.PrivateBytes, $s.Threads)
                    if ((Get-Content $stdout -ErrorAction SilentlyContinue) -match 'TRAVLE_STRESS done') { break }
                }
                Start-Sleep -Seconds 11   # one more STATS line after the stress ended
                $w1 = Stats-Report @(Get-Content $stdout -ErrorAction SilentlyContinue)
                $notes = "first={0:N1}/{1:N1} thr={2}; before[{3}] after[{4}]; MEM {5}" -f $first.PrivateWS, $first.PrivateBytes, $first.Threads, $w0, $w1, ($marks -join ' | ')
                Write-Host "  $notes"
            }
        }
        $storeBefore = if (Test-Path $store) { (Get-Item $store).LastWriteTimeUtc.Ticks } else { 0 }
        $startSec = $clock.Elapsed.TotalSeconds
        $m = Measure-Tree $p.Id $Window $Name
        $endSec = $clock.Elapsed.TotalSeconds
        Start-Sleep -Milliseconds 1500
        $frames = 0; $sakura = 0; $intervals = 0
        $lines = @(Get-Content $stdout -ErrorAction SilentlyContinue)
        foreach ($l in $lines) {
            if ($l -match '^STATS (\d+) frames=(\d+) ticks=(\d+) sakura_ticks=(\d+)') {
                $t = [int]$Matches[1]
                if ($t - 10 -ge $startSec - 0.5 -and $t -le $endSec + 0.5) { $frames += [int]$Matches[2]; $sakura += [int]$Matches[4]; $intervals++ }
            }
        }
        $last = ($lines | Where-Object { $_ -like 'STATS*' } | Select-Object -Last 1)
        $storeAfter = if (Test-Path $store) { (Get-Item $store).LastWriteTimeUtc.Ticks } else { 0 }
        $live = if ($last -match 'live_petals=(\d+)') { [int]$Matches[1] } else { -1 }
        $secs = [math]::Max(1, $intervals * 10)
        $results.Add([pscustomobject]@{
            Scenario = $Name; Procs = $m.Procs; Threads = $m.Threads; PrivWS_MB = $m.PrivateWSMB; PrivBytes_MB = $m.PrivateBytesMB
            Cpu_pct_1core = $m.CpuPctOfOneCore; Iconic = [W21]::IsIconic($h); Visible = [W21]::IsWindowVisible($h)
            Stats_s = $intervals * 10; Frames = $frames; Fps = [math]::Round($frames / $secs, 2); SakuraTicks = $sakura; LivePetals = $live
            StoreWritten = ($storeAfter -ne $storeBefore); Break = (Stats-Report $lines); Notes = $notes
        })
        Write-Host ("{0}: {1:N2}% CPU, {2}/{3} MB, thr {4}, frames {5} in {6}s, sakura {7}, store written {8}, {9}" -f $Name, $m.CpuPctOfOneCore, $m.PrivateWSMB, $m.PrivateBytesMB, $m.Threads, $frames, ($intervals * 10), $sakura, ($storeAfter -ne $storeBefore), (Stats-Report $lines))
    } finally {
        if (-not $p.HasExited) { $p.Kill() }
        Start-Sleep -Milliseconds 600
        Remove-Item -Recurse -Force $data -ErrorAction SilentlyContinue
    }
}

$fn    = @{}
$wabiL = @{ STUDY_NATIVE_STYLE = 'wabi-sabi'; STUDY_NATIVE_THEME = 'light'; STUDY_NATIVE_REST_ROOM = 'games' }
$wabiD = @{ STUDY_NATIVE_STYLE = 'wabi-sabi'; STUDY_NATIVE_THEME = 'dark'; STUDY_NATIVE_REST_ROOM = 'games' }
$timer = @{ STUDY_NATIVE_TIMER_MODE = '2'; STUDY_NATIVE_TIMER_AUTOSTART = '1' }   # 120 min exam: runs through the window (tray needs a session)
function With([hashtable]$base, [hashtable]$extra) { $h = @{}; foreach ($k in $base.Keys) { $h[$k] = $base[$k] }; foreach ($k in $extra.Keys) { $h[$k] = $extra[$k] }; $h }

# Static Travle surfaces (target: 0 frames after settle, no store write).
Run-Scenario 'P21W-S0 FN Break Room, Travle closed' 'fresh' @{ STUDY_NATIVE_OPEN_GAME = '' }
Run-Scenario 'P21W-S1 FN Travle fresh'      'fresh' $fn
Run-Scenario 'P21W-S2 FN Travle mid'        'mid'   $fn
Run-Scenario 'P21W-S3 FN Travle won'        'won'   $fn
Run-Scenario 'P21W-S3b FN Travle lost'      'lost'  $fn
Run-Scenario 'P21W-S4 FN light Travle mid'  'mid'   @{ STUDY_NATIVE_THEME = 'light' }
Run-Scenario 'P21W-S5 Wabi light fresh'     'fresh' $wabiL
Run-Scenario 'P21W-S6 Wabi dark mid'        'mid'   $wabiD
Run-Scenario 'P21W-S7 Wabi light won'       'won'   $wabiL
# Sakura (animated by design) with Travle open.
Run-Scenario 'P21W-K1 FN dark + Sakura mid' 'mid'   @{ STUDY_NATIVE_PALETTE = 'sakura' }
Run-Scenario 'P21W-K2 Wabi + Sakura mid'    'mid'   (With $wabiL @{ STUDY_NATIVE_PALETTE = 'sakura' })
# Minimized / tray-hidden with Travle open (Timer running so close goes to the tray).
Run-Scenario 'P21W-M1 FN Travle mid + Timer, minimized'           'mid' (With $fn $timer) 'minimize'
Run-Scenario 'P21W-M2 Wabi Travle mid + Sakura + Timer, minimized' 'mid' (With (With $wabiL $timer) @{ STUDY_NATIVE_PALETTE = 'sakura' }) 'minimize'
Run-Scenario 'P21W-T1 FN Travle mid + Timer, tray'                'mid' (With $fn $timer) 'tray'
Run-Scenario 'P21W-T2 Wabi Travle mid + Sakura + Timer, tray'     'mid' (With (With $wabiL $timer) @{ STUDY_NATIVE_PALETTE = 'sakura' }) 'tray'
# Stress (STUDY_NATIVE_TRAVLE_STRESS), then 20 s idle measured after "done".
Run-Scenario 'P21W-X1 open/close x500'  'mid'   @{ STUDY_NATIVE_TRAVLE_STRESS = 'open:500' } 'stress'
Run-Scenario 'P21W-X2 type x500'        'fresh' @{ STUDY_NATIVE_TRAVLE_STRESS = 'type:500' } 'stress'
Run-Scenario 'P21W-X3 guess x100'       'fresh' @{ STUDY_NATIVE_TRAVLE_STRESS = 'guess:100' } 'stress'
Run-Scenario 'P21W-X4 guess x400'       'fresh' @{ STUDY_NATIVE_TRAVLE_STRESS = 'guess:400' } 'stress'
Run-Scenario 'P21W-X5 resize x300'      'mid'   @{ STUDY_NATIVE_TRAVLE_STRESS = 'resize:300' } 'stress'
Run-Scenario 'P21W-X6 Wabi open/close x500' 'mid' (With $wabiD @{ STUDY_NATIVE_TRAVLE_STRESS = 'open:500' }) 'stress'

$results | Format-Table Scenario, Procs, Threads, PrivWS_MB, PrivBytes_MB, Cpu_pct_1core, Iconic, Visible, Stats_s, Frames, SakuraTicks, LivePetals, StoreWritten, Break -AutoSize | Out-String -Width 320
$results | ConvertTo-Json | Set-Content $Out
