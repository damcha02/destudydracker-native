# Shared Windows metrics helpers (Stage 12). Dot-source this file: it deliberately has NO param() block, so it cannot
# overwrite the caller's parameters (an earlier version of these scripts dot-sourced benchmark-windows.ps1, whose param()
# defaults silently replaced the caller's $Exe/$Seconds/$Csv). See docs/stage12-windows-platform.md.

Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class ProcMem {
    [StructLayout(LayoutKind.Sequential)]
    public struct PROCESS_MEMORY_COUNTERS_EX2 {
        public uint cb; public uint PageFaultCount; // x64 only: SIZE_T fields declared as ulong
        public ulong PeakWorkingSetSize, WorkingSetSize, QuotaPeakPagedPoolUsage, QuotaPagedPoolUsage,
                       QuotaPeakNonPagedPoolUsage, QuotaNonPagedPoolUsage, PagefileUsage, PeakPagefileUsage,
                       PrivateUsage, PrivateWorkingSetSize;
        public ulong SharedCommitUsage;
    }
    [DllImport("psapi.dll", SetLastError = true)]
    public static extern bool GetProcessMemoryInfo(IntPtr h, ref PROCESS_MEMORY_COUNTERS_EX2 c, uint cb);
}
'@

function Get-ProcessTreeIds([int] $RootId) {
    # Root plus all descendants (helper/child processes), via CIM parent links.
    $all = Get-CimInstance Win32_Process | Select-Object ProcessId, ParentProcessId
    $ids = [System.Collections.Generic.List[int]]::new(); $ids.Add($RootId)
    for ($i = 0; $i -lt $ids.Count; $i++) {
        foreach ($c in ($all | Where-Object { $_.ParentProcessId -eq $ids[$i] })) { $ids.Add([int]$c.ProcessId) }
    }
    $ids
}

function Get-TreeSnapshot([int] $RootId) {
    $mb = 1MB; $ws = 0.0; $pws = 0.0; $priv = 0.0; $threads = 0; $cpu = [TimeSpan]::Zero; $n = 0
    foreach ($id in (Get-ProcessTreeIds $RootId)) {
        $p = Get-Process -Id $id -ErrorAction SilentlyContinue
        if (-not $p -or $p.ProcessName -eq 'conhost') { continue }   # conhost appears only when stdio is redirected; not part of the app
        $c = New-Object ProcMem+PROCESS_MEMORY_COUNTERS_EX2
        $c.cb = [System.Runtime.InteropServices.Marshal]::SizeOf($c)
        if (-not [ProcMem]::GetProcessMemoryInfo($p.Handle, [ref]$c, $c.cb)) { continue }
        $ws += [double]$c.WorkingSetSize / $mb
        $pws += [double]$c.PrivateWorkingSetSize / $mb
        $priv += [double]$c.PrivateUsage / $mb
        $threads += $p.Threads.Count
        $cpu += $p.TotalProcessorTime
        $n++
    }
    [pscustomobject]@{ Procs = $n; WorkingSet = $ws; PrivateWS = $pws; PrivateBytes = $priv; Threads = $threads; Cpu = $cpu; At = [Diagnostics.Stopwatch]::GetTimestamp() }
}

function Measure-Tree([int] $RootId, [double] $Sec, [string] $Tag) {
    $a = Get-TreeSnapshot $RootId
    Start-Sleep -Milliseconds ([int]($Sec * 1000))
    $b = Get-TreeSnapshot $RootId
    $wall = ($b.At - $a.At) / [Diagnostics.Stopwatch]::Frequency
    $cpuPct = ($b.Cpu - $a.Cpu).TotalSeconds / $wall * 100
    [pscustomobject]@{
        Label = $Tag; Pid = $RootId; Procs = $b.Procs; Threads = $b.Threads
        WorkingSetMB = [math]::Round($b.WorkingSet, 1); PrivateWSMB = [math]::Round($b.PrivateWS, 1)
        PrivateBytesMB = [math]::Round($b.PrivateBytes, 1)
        CpuPctOfOneCore = [math]::Round($cpuPct, 2)
        CpuPctOfMachine = [math]::Round($cpuPct / [Environment]::ProcessorCount, 3)
        WindowSec = [math]::Round($wall, 1)
    }
}

function Start-Prototype([string] $ExePath, [hashtable] $EnvVars, [switch] $CaptureStdout) {
    $psi = New-Object Diagnostics.ProcessStartInfo $ExePath
    $psi.UseShellExecute = $false
    $psi.RedirectStandardOutput = [bool]$CaptureStdout
    foreach ($k in $EnvVars.Keys) { $psi.EnvironmentVariables[$k] = [string]$EnvVars[$k] }
    $sw = [Diagnostics.Stopwatch]::StartNew()
    $p = [Diagnostics.Process]::Start($psi)
    [pscustomobject]@{ Process = $p; Clock = $sw }
}
