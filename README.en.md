<h1 align="center">IBRÆ</h1>
<p align="center"><em>A different layer of your desktop.</em><br><sub>DESIGNED BY IBRA · EST. 2026</sub></p>

<p align="center"><a href="README.md">Español</a> · <b>English</b></p>

<p align="center">
  <a href="https://github.com/ibrarmz/ibrae/releases/latest"><img alt="Download" src="https://img.shields.io/github/v/release/ibrarmz/ibrae?style=for-the-badge&label=Download&color=e07a5f"></a>
  <a href="https://github.com/sponsors/ibrarmz"><img alt="Sponsor on GitHub Sponsors" src="https://img.shields.io/badge/Sponsor-GitHub%20Sponsors-ea4aaa?style=for-the-badge&logo=githubsponsors&logoColor=white"></a>
  <a href="https://github.com/ibrarmz/ibrae/stargazers"><img alt="Stars" src="https://img.shields.io/github/stars/ibrarmz/ibrae?style=for-the-badge&color=f5c84b"></a>
  <a href="LICENSE"><img alt="MIT license" src="https://img.shields.io/badge/license-MIT-8fd6a0?style=for-the-badge"></a>
</p>

https://github.com/user-attachments/assets/9225234e-9b3e-475a-bcdc-e772a08a3817

A dynamic island for **Windows and Linux** that lives at the top of your screen: what's playing, what Claude Code is doing, your next meeting and your everyday tools, at a glance. Free and open source.

**[Download the latest version](https://github.com/ibrarmz/ibrae/releases/latest)** · Windows 10/11, Debian/Ubuntu, Fedora · [♥ Sponsor](https://github.com/sponsors/ibrarmz)

> The app's interface is in Spanish for now. This page names each button as it appears in the app, with its English meaning in parentheses.

## What it does

- **Music** from any player (Spotify, YouTube in the browser, Apple Music, VLC…): cover art, controls and a progress bar. Optional HD covers.
- **Claude Code**, live: which phase it's in (reading, editing, testing), the diff it's writing, its subagents, and it peeks out when it's done. When it **asks for permission**, you answer from the island: **Permitir** (allow), **Siempre** (always) or **Denegar** (deny), without going back to the terminal.
- **Volume and brightness**, Mac style: when you change the volume, mute, change the brightness or plug in headphones, the pill shows it. Under Sistema → *Sonido y pantalla* (System → Sound & display) there are volume and brightness sliders, and you switch the audio output with one click. On Linux it works with PipeWire or PulseAudio (`pactl`), and brightness only with a laptop screen.
- **Calendar** from Google, Outlook or iCloud (iCal link, no sign-in): today's agenda, "Daily in 12 min" in the pill, a heads-up one minute before and a **Unirse** (join) button for Meet, Zoom or Teams.
- **Focus** (pomodoro), **quick notes** and **system** (CPU, memory, disk, battery).
- **Multiple monitors:** on the screen you choose, or following the cursor from one monitor to another.
- **Stays out of the way:** with nothing going on it's a 64 px line; if the cursor just passes over, it turns transparent and clicks go through; it hides behind maximized windows.
- **Light and private:** ~150 MB of RAM, near 0 % CPU at idle, no accounts and no servers of its own ([what connects to the internet](#privacy-what-connects-to-the-internet)).
- **Updates itself**, with a verified signature.

<p align="center"><img src="docs/demo.gif" alt="The island: music in the pill, Claude Code working and asking for permission, a meeting heads-up, and the music, Claude and calendar panels" width="760"></p>

Built with **Tauri 2 + Rust + Svelte 5**. The name is written IBRÆ and searched as *ibrae*.

## Install

Download the installer from the [latest release](https://github.com/ibrarmz/ibrae/releases/latest).

| System | File | Notes |
| --- | --- | --- |
| Windows 10/11 | `IBRAE_<version>_x64-setup.exe` | Recommended. Per-user install, no admin rights. |
| Windows 10/11 | `IBRAE_<version>_x64-portable.zip` | If your network blocks `.exe`/`.msi` downloads. Unzip and open `ibrae.exe`; nothing is installed and no admin is needed. Instructions in `LEEME.txt`. |
| Windows 10/11 | `IBRAE_<version>_x64.msi` | Machine-wide, in Program Files. Needs admin; handy for Intune / GPO deployment (`msiexec /i … /qn`). |
| Debian / Ubuntu | `ibrae_<version>_amd64.deb` | `sudo apt install ./ibrae_<version>_amd64.deb` |
| Fedora | `ibrae-<version>-1.x86_64.rpm` | `sudo dnf install ./ibrae-<version>-1.x86_64.rpm` |

**Updates:** the island checks GitHub at startup and every 12 hours. With the `.exe` or `.msi` installer it updates itself from **Ajustes → Actualizaciones** (Settings → Updates), checking the download's signature before installing; with the portable `.zip` and on Linux it tells you and opens the download page. You can turn it off in Settings.

Use only one of the Windows options. Both remove the Claude hooks and autostart when uninstalled (`ibrae uninstall-cleanup`). They aren't code-signed: if SmartScreen warns you, click "More info" → "Run anyway".

On Linux, before uninstalling, run `ibrae hooks uninstall` if you connected Claude Code, and turn off "Iniciar con el sistema" (start with the system) in Settings: the package is removed as root and can't touch your user's configuration.

## Build

Requirements: Node 20+ and Rust (MSVC toolchain on Windows).

```sh
npm install
npm run tauri dev                    # development with hot reload
npx tauri build                      # installer: src-tauri/target/release/bundle/nsis/*.exe (Windows)
npm test                             # UI tests (Vitest)
cd src-tauri && cargo test --workspace   # Rust tests
```

`island-hook` is built first (`npm run build:hook`; `tauri build` does it for you) and the installer ships it next to the app.

**Linux** needs the WebKitGTK libraries:

```sh
# Debian/Ubuntu
sudo apt install libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev libxdo-dev libssl-dev pkg-config build-essential
# Fedora
sudo dnf install webkit2gtk4.1-devel gtk3-devel libappindicator-gtk3-devel librsvg2-devel libxdo-devel openssl-devel pkgconf-pkg-config
```

Or, without installing anything, build the `.deb` and `.rpm` in Docker (from Windows too): see `ci/linux-bundle.Dockerfile`.

The island lives at the top of the main screen; the tray icon has **Configuración** (settings), **Mostrar / ocultar** (show / hide) and **Salir** (quit). Only one instance runs: a second one closes itself.

## Release a version

```sh
npm run release -- 0.1.2
```

Sets the version in `package.json`, `tauri.conf.json` and `Cargo.toml`, commits, creates the `v0.1.2` tag and pushes it. GitHub Actions (`.github/workflows/release.yml`) builds the Windows installers and the `.deb`/`.rpm` packages and publishes them in the release, in about 15–20 minutes. Every push and pull request also runs the tests (`.github/workflows/test.yml`).

To re-record the GIF above with sample data (none of your music, calendar or sessions): `scripts/record-demo.mjs`; instructions are at the top of the file.

## Settings

Gear icon in the dock, or tray → **Configuración**. Saved in `settings.json` inside the app's config folder.

- **Start with the system** (Windows registry / `~/.config/autostart` on Linux)
- **Position:** left, center or right
- **Screen** (with more than one monitor): the main one, a fixed one (remembered by name even if you replug cables; if it's unplugged, the island waits on the main screen) or **wherever the cursor is**: it moves to the monitor where you rest the cursor for a second. On Linux with Wayland, "wherever the cursor is" only sees the cursor over X11 apps.
- **Delay before opening** on hover, idle and busy (in 100 ms steps)
- **Ghost mode** and **hide behind maximized windows**
- **Focus:** focus and break minutes (the Focus panel's slider saves them too)
- Which **dock sections** are shown
- **Volume and brightness:** which notices show in the pill (volume, brightness, audio output) and for how long
- **Claude Code:** connect / disconnect, whether the island opens when it asks for permission, and whether you can answer permissions from the island
- **Music:** HD covers from the iTunes catalog (off by default)
- **Calendar:** connect calendars by iCal link and when to be notified
- **Updates:** version, check and install, and automatic checks

### Connect a calendar

**Calendario** section (or Settings → Calendario) → **Google Calendar** or **Outlook** opens the page with the iCal link. Copy it, go back to the island and **Conectar** (connect) appears; for iCloud or others, use **Pegar link** (paste link). It's read-only, with no sign-in: the link is stored only on your PC and downloaded every 5 minutes.

- **Google:** "Secret address in iCal format", not the public one (that only works if the calendar is public). On work accounts the admin can hide the secret address; if you can only publish free/busy, the island shows events as "Busy".
- **Outlook:** "Publish a calendar" → ICS link. Work or school accounts: [outlook.office.com](https://outlook.office.com/calendar/options/calendar/SharedCalendars); personal accounts: [outlook.live.com](https://outlook.live.com/calendar/0/options/calendar/SharedCalendars).

### Connect Claude Code

In the island, **Claude** section → **Conectar** (connect). That adds hooks to `~/.claude/settings.json` (after saving a `settings.json.bak-dynamic-island` backup) and applies to new Claude Code sessions. From a terminal: `ibrae hooks install` / `hooks uninstall`.

**Permissions from the island.** When Claude asks for permission (a command, editing a file, opening a web page…), the island shows what it wants to do and three buttons:
- **Permitir** (allow): just this once.
- **Siempre** (always): adds the same rule as the terminal's "don't ask again" (e.g. `Bash(npm test:*)`). Only shown when Claude Code offers one.
- **Denegar** (deny): Claude doesn't do it and carries on without it.

The terminal asks at the same time; the first answer wins. If you answer there, the card goes away by itself. Multiple-choice questions and plan review are only answered in the terminal. If a background subagent asks, the island waits 30 s and then lets the terminal ask. Turn it off in Settings → "Responder permisos desde la isla" (answer permissions from the island).

The hooks call `island-hook`, a 200 KB executable that only forwards the event over a local channel (a named pipe on Windows, a Unix socket on Linux; no network ports). They run with `"async": true`, so Claude never waits for the island; the permission hook is the one exception, since it waits for your answer.

### How it gets out of the way

- **Idle**, it's just a 64×5 px line. With music, focus or Claude working, it shows the pill.
- **Ghost mode** (Windows): if the cursor just passes over, the island fades to 15 % and clicks go to whatever is underneath. It opens if you rest the cursor 0.4 s (0.6 s when something is active) or hold `Ctrl`. On Linux it opens the same way, but the island itself doesn't let clicks through.
- **Maximized or fullscreen window** on that monitor: the island slides up leaving 2 px visible. It peeks out by itself when a focus session ends or Claude asks for permission.

## Privacy: what connects to the internet

Nothing leaves your PC for a project server: there is no server. The only connections are:

| What | Where | When |
| --- | --- | --- |
| Update check | `github.com` (the latest release's `latest.json`) | At startup and every 12 h; can be turned off in Settings |
| Calendars | The iCal link you added | Every 5 min, only if you added one |
| HD covers | `itunes.apple.com` (artist and song title) | Only if you turn it on in Settings |

Claude Code hooks use a local channel (named pipe / Unix socket), never the network.

## Status

| Piece | Windows | Linux |
| --- | --- | --- |
| Transparent, always-on-top, click-through window | Tested | Builds; window created in place (Xvfb) |
| Pill, expansion, ghost mode, hides behind maximized windows | Tested | Builds; not tested on a real desktop |
| Home, Focus | Tested | Builds |
| Music | SMTC, tested | MPRIS, builds; not tested |
| Claude: phases, diff, permissions, done | Tested with simulated events | Channel and `island-hook` tested in a container |
| Notes: quick note with autosave, dock **+** button | Tested (render) | Builds |
| System: CPU with history, memory, disk, battery, uptime, island RAM | Tested | Builds |
| Volume, mute, audio output and brightness (notices and controls) | Tested | Volume, mute and output tested in a container (PulseAudio); brightness untested |
| Settings, position, autostart | Tested in the installed app | Builds |
| iCal calendar: agenda, notices, join, connect by copying the link | Tested with Google (real calendar) | Builds |
| Auto-update | Tested from 0.1.13 to 0.1.14 with the `.exe` installer | Notifies and opens the download |
| Installer / uninstaller | Per-user NSIS, tested install → use → uninstall | `.deb` / `.rpm` built in Docker, not installed on a real desktop |

Notes are saved in `notes.md` inside the app's data folder (`%APPDATA%\dev.dynamicisland.island` on Windows, `~/.local/share/dev.dynamicisland.island` on Linux). While you type, the island doesn't close when the cursor leaves; it closes when you click another app, and on Windows it gives the keyboard back to the app you were in.

On Linux the island runs through X11/XWayland on every desktop: Wayland doesn't let a regular app position itself or stay on top, and GNOME has no layer-shell. On KDE Plasma (Wayland), a small KWin script loaded over D-Bus pins the island to the top edge above the panel and reports maximized windows. On other Wayland sessions, "maximized window" only detects X11 apps.

## Measured performance (Windows 11, release build)

| Metric | Value |
| --- | --- |
| Binaries | 3.9 MB + 0.2 MB (`island-hook`) |
| Private RAM, whole tree (app + WebView2) | ~140–170 MB (with every panel opened; the renderer grows with use and shrinks on close) |
| — of which the Rust core | ~10 MB |
| CPU at idle | 0.3–1.3 % of one core (the Rust core, ~0.05 %) |

The original < 60 MB goal can't be met with WebView2: its floor is ~100 MB spread over 6 processes. What brought it down from 165 MB:

- No GPU process (`--disable-gpu`): −55 MB. The island is small; software rendering is enough.
- A single renderer, no extensions, no crashpad, V8 in *lite mode*.
- The cursor is read with `GetCursorPos` directly, and the clock only ticks seconds when something shows them.
- Music only notifies the UI when something actually changes.

To get under 60 MB, plan B still stands: draw the compact pill natively and create the WebView only on expand.

## Layout

```
src/                       UI (Svelte 5)
  App.svelte               island, expansion, dock, glass tint, hover on Linux
  lib/store.svelte.js      shared state; Rust pushes events
  lib/pill.js              what the pill shows and how wide it is (tested)
  lib/Pill|Home|Claude|Music|Focus|Notes|System|Settings.svelte
src-tauri/src/
  main.rs                  startup, tray, CLI (hook / hooks install)
  hit.rs                   position, click-through, ghost mode, maximized windows
  focus.rs                 timer
  claude/                  Claude Code sessions, local channel, hook installer, permissions
  media/                   music: windows.rs (SMTC), linux.rs (MPRIS)
  audio/                   volume, audio output and brightness: win.rs, linux.rs (pactl, backlight)
  calendar.rs              iCal feeds and recurring events
  notes.rs                 save / load the note
  settings.rs              settings, autostart
  sys.rs                   system stats, island RAM
src-tauri/hook/            island-hook: the tiny client Claude Code calls
src-tauri/windows/hooks.nsh  cleanup on uninstall (Windows)
ci/                        Linux build and smoke test in Docker
```

`ISLAND_PEEK=home,claude,music,focus,notes,sys` opens each panel at startup (handy for screenshots).

## Support

IBRÆ is free, with no ads and no accounts. If it's useful to you and you'd like it to keep growing, you can support it on [GitHub Sponsors](https://github.com/sponsors/ibrarmz) (one-time or monthly). A ⭐ on the repo and telling someone about it help a lot too. In the island it's under Ajustes → **♥ Apoyar**.

## License

[MIT](LICENSE): use it, copy it, change it and share it freely, in commercial projects too, keeping the copyright notice.
