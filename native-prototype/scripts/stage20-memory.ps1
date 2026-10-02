<#
Stage 20 memory stability on Windows (counterpart of stage20-memory-linux.sh): runs the release exe
with a stress hook in a throw-away profile and samples Private WS / Private Bytes / threads every
-Every seconds until the hook prints "done", then -Tail seconds more. Reports the Break Room
counters (writes/pushes/evaluations, art decoded) from the last STATS line.

  stage20-memory.ps1 -Fixture <full.json> -Every 15 -Env @{ STUDY_NATIVE_BREAK_STRESS = '500' }
  stage20-memory.ps1 -Fixture <full.json> -Every 5  -Env @{ STUDY_NATIVE_GAME_RESET_STRESS = '200'; STUDY_NATIVE_VIEW = 'break'; STUDY_NATIVE_OPEN_GAME = '0' }
#>
param(
    [Parameter(Mandatory)][string] $Fixture,
    [hashtable] $Env = @{},
    [int] $Every = 15,
    [int] $Tail = 20,
    [string] $Exe = (Join-Path $PSScriptRoot '..\target\release\study-tracker-native-prototype.exe')
)
. (Join-Path $PSScriptRoot 'win-metrics.ps1')
$Exe = (Resolve-Path $Exe).Path
$data = Join-Path $env:TEMP ("st20-mem-" + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory $data | Out-Null
$stdout = Join-Path $data 'stdout.txt'
$all = @{ STUDY_NATIVE_DATA_DIR = $data; STUDY_NATIVE_IMPORT_BACKUP = (Resolve-Path $Fixture).Path; STUDY_NATIVE_NOW = '2026-09-30T12:00:00-07:00'
          STUDY_NATIVE_SIZE = '1520x980'; STUDY_NATIVE_BREAK_PICK = '0.25'; STUDY_NATIVE_FRAME_STATS = '1' }
foreach ($k in $Env.Keys) { $all[$k] = $Env[$k] }
$saved = @{}
foreach ($k in $all.Keys) { $saved[$k] = [Environment]::GetEnvironmentVariable($k); [Environment]::SetEnvironmentVariable($k, [string]$all[$k]) }
$p = Start-Process -FilePath $Exe -PassThru -RedirectStandardOutput $stdout
foreach ($k in $all.Keys) { [Environment]::SetEnvironmentVariable($k, $saved[$k]) }
try {
    $t = 0; $doneAt = -1; $peakWS = 0.0; $peakPB = 0.0
    while (-not $p.HasExited) {
        Start-Sleep -Seconds $Every; $t += $Every
        $s = Get-TreeSnapshot $p.Id
        $peakWS = [math]::Max($peakWS, $s.PrivateWS); $peakPB = [math]::Max($peakPB, $s.PrivateBytes)
        "t={0}s privWS={1:N1}MB privBytes={2:N1}MB threads={3}" -f $t, $s.PrivateWS, $s.PrivateBytes, $s.Threads
        if ($doneAt -lt 0) { $d = @(Get-Content $stdout -ErrorAction SilentlyContinue | Where-Object { $_ -match 'done' }); if ($d.Count) { $doneAt = $t; "  $($d[0])" } }
        if ($doneAt -ge 0 -and $t -ge $doneAt + $Tail) { break }
    }
    "PEAK privWS={0:N1}MB privBytes={1:N1}MB" -f $peakWS, $peakPB
    $last = @(Get-Content $stdout | Where-Object { $_ -like 'STATS*' }) | Select-Object -Last 1
    if ($last -match '(break_writes=.*)$') { "COUNTERS $($Matches[1])" }
} finally {
    if (-not $p.HasExited) { $p.Kill() }
    Start-Sleep -Milliseconds 500
    Remove-Item -Recurse -Force $data -ErrorAction SilentlyContinue
}
