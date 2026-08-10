# Study Tracker

Study Tracker is a local-first desktop app for planning your semester, tracking focus sessions, monitoring course progress, and exporting study notes to an Obsidian-style vault.

[Landing page](https://damcha02.github.io/destudydracker/) · [Download latest release](https://github.com/damcha02/destudydracker/releases/latest) · [GitHub repository](https://github.com/damcha02/destudydracker)

## Overview

Study Tracker brings courses, exams, tasks, calendars, timers, and progress metrics into one calm desktop workspace. It is designed for students who want more structure than a to-do list, without depending on cloud accounts or scattered productivity tools.

The app runs locally on your machine, stores your study data offline, and can export daily notes and session logs into plain files for workflows like Obsidian.

## Features

- Semester and course organization with color-coded subjects.
- Task and exam planning with due dates, priorities, and workload tracking.
- Focus timer for Pomodoro, deep work, exam prep, sprints, and custom sessions.
- Study session logging with goals, reflections, and linked courses or tasks.
- Dashboard metrics for focused time, streaks, upcoming deadlines, and course health.
- Calendar and timeline views for tasks, exams, and planned study units.
- Obsidian-style vault export for daily notes and study logs.
- Local-first storage with backup/export support.

## Download

Most users do not need to build the app themselves. Download the latest installer from the GitHub releases page:

[Download Study Tracker](https://github.com/damcha02/destudydracker/releases/latest)

Choose the file that matches your operating system:

- Windows: use the `.exe` installer for normal installs. Use `.msi` mainly for managed or admin deployments.
- macOS: use the `.dmg` disk image. Choose `aarch64` for Apple Silicon Macs.
- Linux:
  - Ubuntu, Debian, Linux Mint, Pop!_OS, Zorin: `.deb`.
  - Fedora, RHEL, openSUSE: `.rpm`.
  - Arch, Manjaro, EndeavourOS, NixOS, Void, Gentoo, or unknown Linux distributions: build from source for now. The AppImage is available, but may not work reliably on some Wayland/Hyprland setups.

For normal Intel/AMD computers, choose files named `amd64`, `x86_64`, or `x64`. Avoid `aarch64` or `arm64` unless you are using an ARM device.

Because Study Tracker is a new unsigned app, Windows or macOS may show a one-time security warning. The landing page includes platform-specific install notes.

## Build From Source

Prerequisites:

- Node.js LTS.
- Rust stable.
- Tauri system dependencies for your operating system.

Clone the repository and run the desktop app in development mode:

```bash
git clone https://github.com/damcha02/destudydracker.git
cd destudydracker/desktop
npm install
npm run tauri:dev
```

Create a production desktop build:

```bash
npm run tauri:build
```

Run the production binary:

```bash
./src-tauri/target/release/app
```

Update a source-built install:

```bash
git pull
npm install
npm run tauri:build
./src-tauri/target/release/app
```

On Linux, Tauri requires WebKitGTK and related packages. For Ubuntu/Debian-based systems, the release workflow installs:

```bash
sudo apt-get install -y libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf
```

## Tech Stack

- React
- TypeScript
- Vite
- Tauri
- Rust

## Releases

Tagged versions are built with GitHub Actions for Windows, macOS, and Linux. Release builds are created from `.github/workflows/release.yml` and published as draft GitHub releases.

## Project Status

Study Tracker is an active personal project focused on a polished, offline-first desktop workflow for students.

## License

Study Tracker is released under the [MIT License](LICENSE).
