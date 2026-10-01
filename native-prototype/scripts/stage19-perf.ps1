<#
.SYNOPSIS
  Stage 19 runtime measurements (Wabi-Sabi + Sakura). Release exe, a throw-away data directory per
  scenario, synthetic fixtures only - never a real profile. State is set with the app's diagnostic
  environment hooks; minimize and hide-to-tray use real Win32 messages (ShowWindow SW_MINIMIZE,
  WM_CLOSE while a session runs = Stage 18's hide-to-tray).

  Per scenario: process count, threads, Private WS / Private Bytes (MB), CPU % of one core over the
  window, frames rendered and Sakura clock ticks inside the window (parsed from the
  STUDY_NATIVE_FRAME_STATS lines whose 10 s interval lies wholly inside the window), whether
  store.json was written during the window, the live petal count, and an optional memory series.

  stage19-perf.ps1 [-Only P3] [-Seconds 20]
#>
param(
    [string] $Exe = (Join-Path $PSScriptRoot '..\target\release\study-tracker-native-prototype.exe'),
    [int] $Seconds = 20,
    [string] $Only = '',
    [string] $Out = (Join-Path $env:TEMP 'stage19_perf.json')
)
. (Join-Path $PSScriptRoot 'win-metrics.ps1')
Add-Type -TypeDefinition @'
using System; using System.Runtime.InteropServices;
public static class W19 {
    [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int cmd);
    [DllImport("user32.dll")] public static extern bool IsIconic(IntPtr h);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
    [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h, uint m, IntPtr w, IntPtr l);
}
'@
$Exe = (Resolve-Path $Exe).Path
$fx = (Resolve-Path (Join-Path $PSScriptRoot '..\tests\fixtures\dashboard')).Path
$results = New-Object System.Collections.Generic.List[object]
$now = '2026-09-30T12:00:00+02:00'

function Run-Scenario([string]$Name, [hashtable]$Env, [string]$Action = 'none', [int]$Window = $Seconds, [int]$Samples = 0, [int]$SampleSec = 15, [int]$Settle = 4) {
    if ($Only -and $Name -notlike "*$Only*") { return }
    $data = Join-Path $env:TEMP ("st19-" + [Guid]::NewGuid().ToString('N'))
    New-Item -ItemType Directory $data | Out-Null
    $all = @{ STUDY_NATIVE_DATA_DIR = $data; STUDY_NATIVE_FRAME_STATS = '1'; STUDY_NATIVE_SIZE = '1520x980' }
    foreach ($k in $Env.Keys) { $all[$k] = $Env[$k] }
    $stdout = Join-Path $data 'stdout.txt'
    $saved = @{}
    foreach ($k in $all.Keys) { $saved[$k] = [Environment]::GetEnvironmentVariable($k); [Environment]::SetEnvironmentVariable($k, [string]$all[$k]) }
    $clock = [Diagnostics.Stopwatch]::StartNew()
    $p = Start-Process -FilePath $Exe -PassThru -RedirectStandardOutput $stdout
    foreach ($k in $all.Keys) { [Environment]::SetEnvironmentVariable($k, $saved[$k]) }
    $proc = [pscustomobject]@{ Clock = $clock }
    try {
        Start-Sleep -Seconds $Settle
        $p.Refresh(); $h = $p.MainWindowHandle
        switch ($Action) {
            'minimize' { [void][W19]::ShowWindow($h, 6); Start-Sleep -Seconds 1 }
            'tray'     { [void][W19]::PostMessage($h, 0x0010, [IntPtr]::Zero, [IntPtr]::Zero); Start-Sleep -Seconds 1 }
        }
        $store = Join-Path $data 'store.json'
        $storeBefore = if (Test-Path $store) { (Get-Item $store).LastWriteTimeUtc.Ticks } else { 0 }
        $startSec = $proc.Clock.Elapsed.TotalSeconds
        $m = Measure-Tree $p.Id $Window $Name
        $endSec = $proc.Clock.Elapsed.TotalSeconds
        $series = @()
        if ($Samples -gt 0) { $series = @(for ($k = 0; $k -lt $Samples; $k++) { $s = Measure-Tree $p.Id $SampleSec "$Name#$k"; "{0}/{1:N2}%" -f $s.PrivateWSMB, $s.CpuPctOfOneCore }) }
        Start-Sleep -Milliseconds 1500
        $frames = 0; $ticks = 0; $sakura = 0; $intervals = 0; $last = ''
        $lines = @(Get-Content $stdout -ErrorAction SilentlyContinue)
        foreach ($l in $lines) {
            if ($l -match '^STATS (\d+) frames=(\d+) ticks=(\d+) sakura_ticks=(\d+)') {
                $t = [int]$Matches[1]
                if ($t - 10 -ge $startSec - 0.5 -and $t -le $endSec + 0.5) { $frames += [int]$Matches[2]; $ticks += [int]$Matches[3]; $sakura += [int]$Matches[4]; $intervals++ }
                $last = $l
            }
        }
        $storeAfter = if (Test-Path $store) { (Get-Item $store).LastWriteTimeUtc.Ticks } else { 0 }
        $live = if ($last -match 'live_petals=(\d+)') { [int]$Matches[1] } else { -1 }
        $running = if ($last -match 'sakura_running=(\w+)') { $Matches[1] } else { '?' }
        $secs = [math]::Max(1, $intervals * 10)
        $results.Add([pscustomobject]@{
            Scenario = $Name; Procs = $m.Procs; Threads = $m.Threads; PrivWS_MB = $m.PrivateWSMB; PrivBytes_MB = $m.PrivateBytesMB
            Cpu_pct_1core = $m.CpuPctOfOneCore; Iconic = [W19]::IsIconic($h); Visible = [W19]::IsWindowVisible($h)
            Stats_s = $intervals * 10; Frames = $frames; Fps = [math]::Round($frames / $secs, 1); SakuraTicks = $sakura; TimerTicks = $ticks
            SakuraRunning = $running; LivePetals = $live; StoreWritten = ($storeAfter -ne $storeBefore); Series = ($series -join ' ')
            Notes = ($lines | Where-Object { $_ -like 'THEME_STRESS*' -or $_ -like 'NAV_STRESS*' }) -join '; '
        })
        Write-Host ("{0}: {1:N2}% CPU, {2} MB, frames {3} ({4}/s), sakura ticks {5}, store written {6}" -f $Name, $m.CpuPctOfOneCore, $m.PrivateWSMB, $frames, [math]::Round($frames / $secs, 1), $sakura, ($storeAfter -ne $storeBefore))
    } finally {
        if (-not $p.HasExited) { $p.Kill() }
        Start-Sleep -Milliseconds 600
        Remove-Item -Recurse -Force $data -ErrorAction SilentlyContinue
    }
}

$wabi = @{ STUDY_NATIVE_IMPORT_BACKUP = "$fx\wabi.json"; STUDY_NATIVE_NOW = $now }
function With([hashtable]$base, [hashtable]$extra) { $h = @{}; foreach ($k in $base.Keys) { $h[$k] = $base[$k] }; foreach ($k in $extra.Keys) { $h[$k] = $extra[$k] }; $h }
$timer = @{ STUDY_NATIVE_TIMER_MODE = '2'; STUDY_NATIVE_TIMER_AUTOSTART = '1' }   # 120 min exam: runs through the window

Run-Scenario 'G19-P0 fresh default style, idle'          @{ STUDY_NATIVE_VIEW = 'dashboard' }
Run-Scenario 'G19-P1 Field Notebook, realistic, idle'    @{ STUDY_NATIVE_IMPORT_BACKUP = "$fx\realistic.json"; STUDY_NATIVE_NOW = $now; STUDY_NATIVE_VIEW = 'dashboard' }
Run-Scenario 'G19-P2 Wabi-Sabi static (default palette)' (With $wabi @{ STUDY_NATIVE_STYLE = 'wabi-sabi'; STUDY_NATIVE_THEME = 'light' })
Run-Scenario 'G19-P3 Wabi-Sabi + Sakura visible'         (With $wabi @{ STUDY_NATIVE_STYLE = 'wabi-sabi'; STUDY_NATIVE_PALETTE = 'sakura' })
Run-Scenario 'G19-P3b Field Notebook + Sakura visible'   (With $wabi @{ STUDY_NATIVE_PALETTE = 'sakura' })
foreach ($fps in 165, 60, 30, 24, 20, 15) {
    Run-Scenario "G19-P3-fps$fps Wabi-Sabi + Sakura at $fps Hz" (With $wabi @{ STUDY_NATIVE_STYLE = 'wabi-sabi'; STUDY_NATIVE_PALETTE = 'sakura'; STUDY_NATIVE_SAKURA_FPS = "$fps" })
}
Run-Scenario 'G19-P3r Wabi-Sabi + Sakura, reduced motion' (With $wabi @{ STUDY_NATIVE_STYLE = 'wabi-sabi'; STUDY_NATIVE_PALETTE = 'sakura'; STUDY_NATIVE_REDUCED_MOTION = '1' })
Run-Scenario 'G19-P4 Sakura + Timer running (visible)'    (With (With $wabi $timer) @{ STUDY_NATIVE_STYLE = 'wabi-sabi'; STUDY_NATIVE_PALETTE = 'sakura'; STUDY_NATIVE_VIEW = 'timer' })
Run-Scenario 'G19-P5 Sakura + Timer, minimized'           (With (With $wabi $timer) @{ STUDY_NATIVE_STYLE = 'wabi-sabi'; STUDY_NATIVE_PALETTE = 'sakura'; STUDY_NATIVE_VIEW = 'timer' }) 'minimize'
Run-Scenario 'G19-P6 Sakura + Timer, hidden to tray'      (With (With $wabi $timer) @{ STUDY_NATIVE_STYLE = 'wabi-sabi'; STUDY_NATIVE_PALETTE = 'sakura'; STUDY_NATIVE_VIEW = 'timer' }) 'tray'
Run-Scenario 'G19-P7 Sakura palette, debug lab on screen' (With $wabi @{ STUDY_NATIVE_STYLE = 'wabi-sabi'; STUDY_NATIVE_PALETTE = 'sakura'; STUDY_NATIVE_VIEW = 'text' })
Run-Scenario 'G19-P8 500 style switches (Sakura on)'      (With $wabi @{ STUDY_NATIVE_PALETTE = 'sakura'; STUDY_NATIVE_THEME_STRESS = '250' }) 'none' 10 5
# P10: ~30 minutes of visible Sakura (Wabi-Sabi Timer running), memory/CPU sampled every 5 minutes.
Run-Scenario 'G19-P10 30-minute Sakura soak'              (With (With $wabi $timer) @{ STUDY_NATIVE_STYLE = 'wabi-sabi'; STUDY_NATIVE_PALETTE = 'sakura'; STUDY_NATIVE_VIEW = 'timer' }) 'none' 20 6 300
Run-Scenario 'G19-P11 Wabi-Sabi, stress dataset'          @{ STUDY_NATIVE_GENERATE_SYNTHETIC_ACADEMIC_DATA = '1'; STUDY_NATIVE_STYLE = 'wabi-sabi' }

$results | Format-List | Out-String -Width 300
$results | ConvertTo-Json | Set-Content $Out
