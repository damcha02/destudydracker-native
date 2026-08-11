# Stage 5 Linux Benchmark

Stage 5 compares the existing production Tauri/WebKit Study Tracker against the Stage 4 Slint native prototype on the same Linux desktop. This is benchmark-only work; no production Study Tracker source was modified and no feature migration or optimization was attempted.

## Environment

- Date: 2026-08-11
- Host: `damcha-laptop`
- OS/kernel: Arch Linux, `Linux 7.0.11-arch1-1 x86_64`
- Session: Hyprland `0.55.3`, Wayland, `DISPLAY=:0`, `WAYLAND_DISPLAY=wayland-1`
- CPU: 13th Gen Intel(R) Core(TM) i7-13700H, 20 logical CPUs
- Memory at run time: 29 GiB total, about 14 GiB available, swap mostly used
- Rust: `rustc 1.96.0`, `cargo 1.96.0`
- Node/npm: Node `v25.9.0`, npm `11.12.1`
- Native prototype commit baseline: Stage 4 complete at `4895191`

## Methodology

- Built the native prototype with `cargo build --release` in `native-prototype/`.
- Built the production app with `npm ci` and `npm run tauri:build` in `desktop/`.
- The production build produced `desktop/src-tauri/target/release/app`; Linux bundle generation produced `.deb` and `.rpm`, then failed at AppImage/linuxdeploy. The release executable was available and used for runtime benchmarking.
- Memory was collected from `/proc/<pid>/smaps_rollup`.
- Tauri/WebKit was measured as a process tree rooted at the Tauri app PID, including its relevant `WebKitNetworkProcess` and `WebKitWebProcess` children. Unrelated browser/WebKit processes were excluded.
- CPU was sampled from `/proc/<pid>/stat` deltas over approximately 10 seconds and reported as percent of one CPU.
- Startup was measured as launch-to-visible-window under Hyprland using `hyprctl clients -j`.
- Native timer start/pause was driven with Hyprland `sendshortcut` input.
- Production timer start/pause required manual clicks because `xdotool`, `ydotool`, and `wtype` were unavailable and Hyprland key dispatch did not reliably activate the React timer button.
- Production localStorage was read to confirm the timer tab/state, but benchmark tooling did not edit production source.

New tooling added under `native-prototype/scripts/`:

- `benchmark-process-tree-linux.sh`: sums RSS/PSS/private/shared memory and sampled CPU for a root process tree.
- `startup-time-hyprland.sh`: measures launch-to-visible-window time on Hyprland.

## Fresh Native Results

Executable: `native-prototype/target/release/study-tracker-native-prototype`

Executable size: 25,473,056 bytes, 25 MiB on disk.

Process architecture: one process, multiple threads, no WebKit/browser helper process.

| Scenario | RSS kB | PSS kB | Private_Clean kB | Private_Dirty kB | Shared_Clean kB | Shared_Dirty kB | Private total kB | CPU sample |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| Shortly after launch | 173,980 | 45,692 | 640 | 21,396 | 151,940 | 4 | 22,036 | 0.00% |
| After about 1 minute idle | 173,980 | 45,692 | 640 | 21,396 | 151,940 | 4 | 22,036 | 0.00% |
| Timer running about 30 sec | 174,060 | 54,243 | 11,776 | 21,476 | 140,804 | 4 | 33,252 | 0.00% |
| Timer running several minutes | 174,060 | 54,238 | 11,768 | 21,476 | 140,812 | 4 | 33,244 | 0.00% |
| Paused plus about 15 sec | 174,060 | 54,238 | 11,768 | 21,476 | 140,812 | 4 | 33,244 | 0.00% |
| Normal interaction plus about 15 sec idle | 174,060 | 54,238 | 11,768 | 21,476 | 140,812 | 4 | 33,244 | 0.00% |

Native process tree:

```text
study-tracker-native-prototype
  threads only
```

## Fresh Production Results

Executable: `desktop/src-tauri/target/release/app`

Executable size: 24,249,744 bytes, 24 MiB on disk.

Built frontend assets: `desktop/dist`, 5.4 MiB on disk.

Generated Linux packages before AppImage failure:

- `.deb`: 9,113,938 bytes, 8.7 MiB on disk.
- `.rpm`: 9,114,408 bytes, 8.7 MiB on disk.

Process architecture: Tauri root process plus WebKit network and web content helper processes. The measured root was from this checkout: `desktop/src-tauri/target/release/app`.

| Scenario | RSS kB | PSS kB | Private_Clean kB | Private_Dirty kB | Shared_Clean kB | Shared_Dirty kB | Private total kB | CPU sample |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| Shortly after launch | 745,572 | 381,971 | 82,988 | 215,908 | 446,660 | 20 | 298,896 | 30.80% |
| After about 1 minute idle | 796,176 | 432,197 | 83,376 | 265,408 | 447,400 | 12 | 348,784 | 23.90% |
| Timer running about 30 sec | 784,432 | 417,079 | 86,592 | 246,316 | 451,504 | 20 | 332,908 | 21.30% |
| Timer running several minutes | 779,388 | 411,843 | 86,400 | 241,080 | 451,888 | 20 | 327,480 | 21.20% |
| Paused plus about 15 sec | 780,040 | 412,495 | 86,396 | 241,736 | 451,888 | 20 | 328,132 | 21.50% |

Representative production process tree:

```text
app
  WebKitNetworkProcess
  WebKitWebProcess
  app threads
  WebKit helper threads
```

## Comparison Table

PSS is the primary comparison metric.

| Scenario | Native PSS kB | Production PSS kB | Native advantage |
|---|---:|---:|---:|
| Shortly after launch | 45,692 | 381,971 | 88.0% lower, 8.4x smaller |
| After about 1 minute idle | 45,692 | 432,197 | 89.4% lower, 9.5x smaller |
| Timer running about 30 sec | 54,243 | 417,079 | 87.0% lower, 7.7x smaller |
| Timer running several minutes | 54,238 | 411,843 | 86.8% lower, 7.6x smaller |
| Paused plus about 15 sec | 54,238 | 412,495 | 86.8% lower, 7.6x smaller |

| Artifact | Native | Production |
|---|---:|---:|
| Main executable | 25,473,056 bytes | 24,249,744 bytes |
| Frontend assets | n/a | 5.4 MiB `desktop/dist` |
| Linux package | not produced | 9.1 MB `.deb` / 9.1 MB `.rpm` |
| Process count | 1 process | 3 relevant resident processes |

## Memory Analysis

The native prototype stays near 45.7 MB PSS immediately after launch and idle, then rises to about 54.2 MB PSS after the timer is started. That increase appears to be one-time renderer/font/GPU cache residency rather than a timer-duration leak, because the several-minute running, paused, and post-interaction measurements are effectively flat.

The production Tauri/WebKit app measured much larger in this run: about 382 MB PSS shortly after launch and about 412-432 MB PSS in steady timer scenarios. Most production PSS comes from the `WebKitWebProcess`, with additional cost in the Tauri/GTK root process and `WebKitNetworkProcess`.

RSS is less useful here because both apps share large graphics, font, toolkit, and system libraries. PSS gives the more meaningful attribution, and by PSS the native prototype is about 7.6x to 9.5x smaller across the measured scenarios.

## CPU Analysis

The native prototype sampled at 0.00% of one CPU in all fresh 10-second samples. The sampling method has coarse jiffy resolution, so this should be interpreted as below the measurable threshold, not mathematically zero.

The production app sampled between 21.2% and 30.8% of one CPU in these fresh measurements, including while paused. That is much higher than expected for a quiescent timer view and may reflect the current production UI state, animation/rendering activity, WebKit behavior on this Hyprland session, or app data/state loaded in this profile. It is still a valid measurement of this run, but it should be rechecked in a cleaner profile before treating the exact CPU percentage as universal.

## Startup Comparison

Startup was measured as launch-to-visible-window with `startup-time-hyprland.sh`.

| Run | Native | Production |
|---|---:|---:|
| 1 | 0.123 s | 0.203 s |
| 2 | 0.133 s | 0.194 s |
| 3 | 0.132 s | 0.216 s |
| Average | 0.129 s | 0.204 s |

The native prototype reached a visible window about 37% faster in this small sample. Both are fast in absolute terms on this machine.

## Process Architecture Comparison

The Slint prototype is a single native process with threads for rendering/event-loop/system work. It does not embed React, JavaScript, a DOM, WebKit, Chromium, or a browser process model.

The production app is a Tauri/GTK/WebKit application. On Linux it uses a root app process plus WebKit helper processes, especially `WebKitNetworkProcess` and `WebKitWebProcess`. For app-level memory accounting, measuring only the root Tauri PID would materially undercount production memory; the process tree is the relevant unit.

## Historical Baselines

These are user-collected historical measurements and are preserved with that attribution.

Existing Study Tracker user-collected baseline:

- Idle PSS: about 69 MB.
- Timer-running PSS: about 83-87 MB.
- RSS: about 243-248 MB.

Native Stage 4 user-collected baseline while running:

- RSS: 173,784 kB.
- PSS: 53,958 kB.
- Private_Clean: 11,480 kB.
- Private_Dirty: 21,504 kB.
- Sampled CPU: 1.67% of one CPU.

Native Stage 4 user-collected baseline paused after 15 sec:

- RSS: 173,828 kB.
- PSS: 54,007 kB.
- Private_Clean: 11,480 kB.
- Private_Dirty: 21,548 kB.
- Sampled CPU: 0.00%.

The fresh native measurements align closely with the user-collected native Stage 4 PSS/RSS results. The fresh production measurements are much higher than the known historical production baseline; likely contributors include measuring the full process tree, current app profile/data, current visible timer state, WebKit cache/rendering behavior, and system/session differences.

## Feature-Scope Caveat

This is not an apples-to-apples feature-complete product comparison. The production app includes persistence, planner/dashboard/social/vault/break-room/game/update/tray/audio/WebKit/React/CSS behavior and real app state. The native Stage 4 prototype implements only one representative timer/focus screen with local sample data and no production backend, persistence, updater, tray, vault, social, notification, or full navigation features.

The benchmark answers whether a Slint native renderer remains feasible from a resource perspective after a representative timer screen, not whether the full product has already been replaced.

## Verdict

STRONGLY PROMISING

The native prototype is dramatically lower in PSS in every measured scenario, remains flat over several minutes of timer activity, has negligible sampled CPU in this run, starts faster, and avoids the multi-process WebKit architecture. The caveat is feature scope: these numbers justify continuing native feasibility work, but they do not prove the complete production app can retain the same footprint after all required features are rebuilt.

Do not proceed to Stage 6 from this document; Stage 5 stops at benchmarking.
