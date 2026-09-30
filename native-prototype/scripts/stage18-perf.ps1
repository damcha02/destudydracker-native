<#
Stage 18 idle/CPU/frame measurements (release exe, throw-away profile).

  stage18-perf.ps1 -Exe <path> -DataDir <temp dir> [-Seconds 40]

Scenarios (each a fresh process, a running study timer so the tray/timer paths are live):
  visible     window shown, tray present
  minimized   window minimized
  hidden      window hidden to the tray (WM_CLOSE while a session runs)
Prints CPU% (process CPU time / wall), private MB, threads, and the STUDY_NATIVE_FRAME_STATS
intervals (frames rendered, ticks) after a 20 s settle.
#>
param([Parameter(Mandatory)][string]$Exe, [Parameter(Mandatory)][string]$DataDir, [int]$Seconds = 40)
Add-Type -TypeDefinition @'
using System; using System.Text; using System.Runtime.InteropServices;
public static class P18 {
    public delegate bool EP(IntPtr h, IntPtr l);
    [DllImport("user32.dll")] static extern bool EnumWindows(EP p, IntPtr l);
    [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] static extern int GetWindowText(IntPtr h, StringBuilder s, int n);
    [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int cmd);
    [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h, uint m, IntPtr w, IntPtr l);
    public static IntPtr FindMain(uint pid) { IntPtr f = IntPtr.Zero; EnumWindows((h, l) => { uint p; GetWindowThreadProcessId(h, out p); if (p != pid) return true; var t = new StringBuilder(256); GetWindowText(h, t, 256); if (t.ToString().StartsWith("Study Tracker")) { f = h; return false; } return true; }, IntPtr.Zero); return f; }
}
'@
$name = [IO.Path]::GetFileNameWithoutExtension($Exe)
foreach ($scenario in 'visible', 'minimized', 'hidden') {
    Get-Process -Name $name -ErrorAction SilentlyContinue | Stop-Process -Force
    Remove-Item -Recurse -Force $DataDir -ErrorAction SilentlyContinue
    New-Item -ItemType Directory -Force $DataDir | Out-Null
    $env:STUDY_NATIVE_DATA_DIR = $DataDir; $env:STUDY_NATIVE_TIMER_AUTOSTART = '1'; $env:STUDY_NATIVE_FRAME_STATS = '1'
    $out = Join-Path $DataDir 'stdout.txt'
    $p = Start-Process -FilePath $Exe -PassThru -RedirectStandardOutput $out
    Start-Sleep -Seconds 3
    $hwnd = [P18]::FindMain([uint32]$p.Id)
    if ($scenario -eq 'minimized') { [void][P18]::ShowWindow($hwnd, 6) }
    if ($scenario -eq 'hidden') { [void][P18]::PostMessage($hwnd, 0x0010, [IntPtr]::Zero, [IntPtr]::Zero) }
    Start-Sleep -Seconds 20
    $p.Refresh(); $cpu0 = $p.TotalProcessorTime.TotalSeconds; $t0 = Get-Date
    Start-Sleep -Seconds $Seconds
    $p.Refresh(); $cpu1 = $p.TotalProcessorTime.TotalSeconds; $wall = ((Get-Date) - $t0).TotalSeconds
    $cpuPct = [math]::Round(100 * ($cpu1 - $cpu0) / $wall, 2)
    "PERF $scenario cpu_pct=$cpuPct private_mb=$([math]::Round($p.PrivateMemorySize64 / 1MB, 1)) threads=$($p.Threads.Count)"
    Get-Content $out | Where-Object { $_ -match '^STATS' } | ForEach-Object { "  $_" }
    $p.Kill()
}
