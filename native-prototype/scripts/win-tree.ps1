# Whole-application process-tree measurement (Stage 12 production comparison). Dot-source; deliberately NO param() block.
# Works for a single-process native app and for Tauri/WebView2 (app + msedgewebview2 browser/renderer/GPU/utility children).
# A process belongs to the tree only if it is a DESCENDANT (by parent-PID ancestry) of the fresh root PID.
# Unrelated msedgewebview2/Edge/Chrome processes on the machine are never included.
. (Join-Path $PSScriptRoot 'win-metrics.ps1')   # ProcMem type (GetProcessMemoryInfo with PrivateWorkingSetSize / PrivateUsage)

function Get-ProcRole([string] $name, [string] $cmd) {
    if ($name -ne 'msedgewebview2') { return 'app root' }
    if ($cmd -match '--type=renderer') { return 'webview2 renderer' }
    if ($cmd -match '--type=gpu-process') { return 'webview2 gpu' }
    if ($cmd -match '--type=crashpad-handler') { return 'webview2 crashpad' }
    if ($cmd -match '--type=utility') { if ($cmd -match '--utility-sub-type=([\w\.]+)') { return "webview2 utility ($($Matches[1] -replace 'network\.mojom\.','' -replace 'mojom\.',''))" } else { return 'webview2 utility' } }
    if ($cmd -match '--type=') { return 'webview2 other' }
    return 'webview2 browser'
}

# Returns one row per process in the tree: Pid, Ppid, Name, Role, WorkingSet/PrivateWS/PrivateBytes (MB), Threads, CpuSec, Readable
function Get-TreeDetail([int] $RootId) {
    $all = @(Get-CimInstance Win32_Process | Select-Object ProcessId, ParentProcessId, Name, CommandLine)
    $ids = [System.Collections.Generic.List[int]]::new(); $ids.Add($RootId)
    for ($i = 0; $i -lt $ids.Count; $i++) { foreach ($c in ($all | Where-Object { $_.ParentProcessId -eq $ids[$i] })) { if (-not $ids.Contains([int]$c.ProcessId)) { $ids.Add([int]$c.ProcessId) } } }
    $rows = foreach ($id in $ids) {
        $meta = $all | Where-Object { $_.ProcessId -eq $id } | Select-Object -First 1
        if (-not $meta -or $meta.Name -match '^conhost') { continue }
        $p = Get-Process -Id $id -ErrorAction SilentlyContinue
        if (-not $p) { continue }
        $c = New-Object ProcMem+PROCESS_MEMORY_COUNTERS_EX2
        $c.cb = [System.Runtime.InteropServices.Marshal]::SizeOf($c)
        $ok = $false; try { $ok = [ProcMem]::GetProcessMemoryInfo($p.Handle, [ref]$c, $c.cb) } catch {}
        $cpu = 0.0; try { $cpu = $p.TotalProcessorTime.TotalSeconds } catch {}
        [pscustomobject]@{
            Pid = $id; Ppid = [int]$meta.ParentProcessId; Name = ($meta.Name -replace '\.exe$', ''); Role = (Get-ProcRole ($meta.Name -replace '\.exe$', '') $meta.CommandLine)
            WorkingSetMB = if ($ok) { [double]$c.WorkingSetSize / 1MB } else { 0 }
            PrivateWSMB = if ($ok) { [double]$c.PrivateWorkingSetSize / 1MB } else { 0 }
            PrivateBytesMB = if ($ok) { [double]$c.PrivateUsage / 1MB } else { 0 }
            Threads = $p.Threads.Count; CpuSec = $cpu; Readable = $ok
        }
    }
    ,@($rows)
}

# Interval measurement: memory at the END of the window (sum over the tree), CPU = sum of per-PID CPU-time deltas / wall time.
# A PID first seen inside the window contributes all of its CPU time; a PID that exits inside the window is not counted (noted by Exited).
function Measure-TreeDetail([int] $RootId, [double] $Seconds, [string] $Label) {
    $t0 = [Diagnostics.Stopwatch]::GetTimestamp(); $a = Get-TreeDetail $RootId
    # Wait out the window; when GPU counters are available sample them about once per second (Get-Counter itself takes ~1 s).
    $pids = @($a | ForEach-Object Pid); $gpu = @()
    while ((([Diagnostics.Stopwatch]::GetTimestamp() - $t0) / [Diagnostics.Stopwatch]::Frequency) -lt $Seconds - 1.2) {
        $g = Get-GpuTreeSample $pids; if ($null -ne $g) { $gpu += $g } else { Start-Sleep -Seconds 1 }
    }
    $left = $Seconds - (([Diagnostics.Stopwatch]::GetTimestamp() - $t0) / [Diagnostics.Stopwatch]::Frequency)
    if ($left -gt 0) { Start-Sleep -Milliseconds ([int]($left * 1000)) }
    $b = Get-TreeDetail $RootId; $t1 = [Diagnostics.Stopwatch]::GetTimestamp()
    $wall = ($t1 - $t0) / [Diagnostics.Stopwatch]::Frequency
    $before = @{}; foreach ($r in $a) { $before[$r.Pid] = $r.CpuSec }
    $cpu = 0.0; foreach ($r in $b) { $base = 0.0; if ($before.ContainsKey($r.Pid)) { $base = $before[$r.Pid] }; $cpu += [Math]::Max(0.0, $r.CpuSec - $base) }
    $exited = @($a | Where-Object { $pid0 = $_.Pid; -not ($b | Where-Object { $_.Pid -eq $pid0 }) }).Count
    $root = $b | Where-Object { $_.Pid -eq $RootId } | Select-Object -First 1
    $rootCpu = 0.0; if ($root -and $before.ContainsKey($RootId)) { $rootCpu = $root.CpuSec - $before[$RootId] }
    [pscustomobject]@{
        Label = $Label; Procs = $b.Count; Threads = ($b | Measure-Object Threads -Sum).Sum
        WorkingSetMB = [math]::Round(($b | Measure-Object WorkingSetMB -Sum).Sum, 1)
        PrivateWSMB = [math]::Round(($b | Measure-Object PrivateWSMB -Sum).Sum, 1)
        PrivateBytesMB = [math]::Round(($b | Measure-Object PrivateBytesMB -Sum).Sum, 1)
        CpuPctOneCore = [math]::Round($cpu / $wall * 100, 2)
        GpuPctSum = if ($gpu.Count) { [math]::Round(($gpu | Measure-Object -Average).Average, 2) } else { $null }
        RootPrivateWSMB = [math]::Round($root.PrivateWSMB, 1); RootPrivateBytesMB = [math]::Round($root.PrivateBytesMB, 1); RootWorkingSetMB = [math]::Round($root.WorkingSetMB, 1)
        RootCpuPct = [math]::Round($rootCpu / $wall * 100, 2)
        Unreadable = @($b | Where-Object { -not $_.Readable }).Count; Exited = $exited; WindowSec = [math]::Round($wall, 1)
        Detail = $b
    }
}

# Main window of a process tree: the largest visible top-level window (with a non-empty client area) owned by ANY process in the tree.
# (Tauri also creates hidden helper windows such as "<identifier>-siw"; FindTopWindow would return those.)
function Get-TreeMainWindow([int] $RootId, [int] $TimeoutMs = 20000) {
    $sw = [Diagnostics.Stopwatch]::StartNew()
    while ($sw.ElapsedMilliseconds -lt $TimeoutMs) {
        $ids = @($RootId) + @(Get-TreeDetail $RootId | ForEach-Object Pid)
        $script:cand = @()
        $cb = [W32+EnumProc]{ param($h, $l) if ([W32]::IsWindowVisible($h)) { $pp = [uint32]0; [void][W32]::GetWindowThreadProcessId($h, [ref]$pp); if ($ids -contains [int]$pp) { $r = New-Object W32+RECT; [void][W32]::GetClientRect($h, [ref]$r); if ($r.R -gt 300 -and $r.B -gt 300) { $script:cand += [pscustomobject]@{ H = $h; Area = $r.R * $r.B } } } }; $true }
        [void][W32]::EnumWindows($cb, [IntPtr]::Zero)
        if ($script:cand.Count -gt 0) { return ($script:cand | Sort-Object Area -Descending | Select-Object -First 1).H }
        Start-Sleep -Milliseconds 100
    }
    throw "no main window found for tree of pid $RootId"
}

# Fast variant: largest visible top-level window owned by the ROOT process itself (Tauri's main window belongs to app.exe).
function Get-RootMainWindow([int] $RootId, [int] $TimeoutMs = 20000) {
    $sw = [Diagnostics.Stopwatch]::StartNew()
    while ($sw.ElapsedMilliseconds -lt $TimeoutMs) {
        $script:cand = @()
        $cb = [W32+EnumProc]{ param($h, $l) if ([W32]::IsWindowVisible($h)) { $pp = [uint32]0; [void][W32]::GetWindowThreadProcessId($h, [ref]$pp); if ([int]$pp -eq $RootId) { $r = New-Object W32+RECT; [void][W32]::GetClientRect($h, [ref]$r); if ($r.R -gt 300 -and $r.B -gt 300) { $script:cand += [pscustomobject]@{ H = $h; Area = $r.R * $r.B } } } }; $true }
        [void][W32]::EnumWindows($cb, [IntPtr]::Zero)
        if ($script:cand.Count -gt 0) { return ($script:cand | Sort-Object Area -Descending | Select-Object -First 1).H }
        Start-Sleep -Milliseconds 15
    }
    throw "no main window for pid $RootId"
}

# GPU utilisation of a process tree from the Windows "GPU Engine" performance counters (instance names contain pid_<PID>_).
# Returns the mean over the sampling window of the SUM of utilisation over every engine of every process in the tree
# (percent of one GPU engine; can exceed 100 if several engines are busy). Returns $null if the counters are unavailable.
function Get-GpuTreeSample([int[]] $Pids) {
    try {
        $s = (Get-Counter -Counter '\GPU Engine(*)\Utilization Percentage' -ErrorAction Stop).CounterSamples
        $sum = 0.0; foreach ($c in $s) { if ($c.InstanceName -match 'pid_(\d+)_' -and ($Pids -contains [int]$Matches[1])) { $sum += $c.CookedValue } }
        return $sum
    } catch { return $null }
}
