<div align="center">
  <img src="icons/app-icon.png" width="96" alt="Codex Sound Manager icon" />
  <h1>Codex Sound Manager</h1>
  <p>Play a sound you choose whenever a Codex task finishes.</p>
  <p>
    <a href="README.md">简体中文</a> · <strong>English</strong>
  </p>
  <p>
    <img alt="Platform" src="https://img.shields.io/badge/platform-Windows-2563eb" />
    <img alt="Tauri" src="https://img.shields.io/badge/Tauri-2-24c8db" />
    <img alt="License" src="https://img.shields.io/badge/license-MIT-16a34a" />
  </p>
</div>

## Community

Scan the QR code to contact the developer, report issues, or join Codex discussions:

<p align="center">
  <img src="docs/images/community-qr.jpg" width="320" alt="Codex Sound Manager community QR code" />
</p>

![Application UI](docs/images/app-screenshot.png)

## Highlights

| Feature | Description |
|---|---|
| Automatic Codex discovery | Scans `CODEX_HOME` and the current user's `~/.codex` directory |
| Global configuration | One setup covers every Codex conversation and project for the Windows user |
| Custom sounds | WAV, MP3, FLAC, OGG, M4A and AAC files up to 50 MB; the picker opens in the sound library |
| Repeat count | Play the completion sound 1–10 times |
| System tray | Choose whether to quit or keep running in the tray; restore the window and switch states from the tray menu |
| Desktop floating control | Optional spherical tech control that drags smoothly from any point and switches completion sounds on double-click |
| Original default sound | `sounds/default-notification.wav` is safe to redistribute with the project |
| Format-preserving edits | Uses `toml_edit` to preserve comments, ordering and unrelated settings |
| Existing callback support | Preserves existing notifiers and Codex Computer Use `--previous-notify` wrappers |
| Reversible setup | Removes this integration and restores the previous notifier |
| Developer contact | Open the community QR code from the small button in the window footer |
| Quiet runtime | Release and notification modes never open a console window |

## Download and Setup

### Installer

1. Download the latest Windows installer from GitHub Releases.
2. Open **Codex Sound Manager**.
3. Confirm that Codex has been detected.
4. Choose the enabled state, repeat count and sound file, and optionally enable the desktop floating control.
5. Select **Apply to Codex**.
6. Fully quit and reopen Codex.

### Portable executable

Download the portable ZIP from GitHub Releases, extract it completely, then run:

```text
codex-sound-manager.exe
```

You can also double-click `Run-Portable.vbs` in the same directory. In a source checkout, this launcher automatically finds the release executable under `target\release`.

## Tray and Floating Control

- Closing the main window offers **Quit**, **Minimize to tray**, and **Cancel**.
- Left-click the tray icon to restore the main window. Its context menu can switch the sound, show or hide the floating control, or quit the app.
- Enable **Desktop floating control** in the main window, then double-click the orb to switch completion sounds instantly. Hold and drag from any point on the orb to move it, or right-click it to open the main window. A single click does not switch sounds, preventing accidental activation during slight movement.
- The orb classifies a double-click first, then calls Tauri's `startDragging()` for native Windows window movement. Motion never passes through WebView coordinate frames or an IPC queue, so it follows the system cursor directly without toggling sound by mistake.
- Sound switches in the main window, floating control, and tray are saved immediately and take effect on the next completed task; **Apply to Codex** and a Codex restart are not required.
- Repeat-count and sound-file changes still require one click on **Apply to Codex**. If the global callback is already configured, those settings do not require a Codex restart either.

## Run from Source

Double-click:

```text
Start-Dev.cmd
```

Or run:

```powershell
npm install
npm run tauri -- dev
```

Development mode keeps a terminal open for Vite hot reload and Rust build output. The Release executable and Codex notification callback do not show that terminal.

## Build a Release

Double-click:

```text
Build-Release.cmd
```

Or run:

```powershell
.\scripts\Build-Release.ps1
```

The script first verifies that every project version matches, then installs frontend dependencies, generates and synchronizes Tauri icons, runs TypeScript checks, Rust tests and Clippy, and builds the Tauri release.

| Artifact | Path |
|---|---|
| Portable EXE | `target\release\codex-sound-manager.exe` |
| NSIS installer | `target\release\bundle\release\CodexSoundManager_1.3.6_x64-setup.exe` |
| Portable ZIP | `target\release\bundle\portable\CodexSoundManager_1.3.6_x64-portable.zip` |
| SHA-256 checksums | `target\release\bundle\release\SHA256SUMS.txt` |

The build environment requires Node.js, Rust, Microsoft C++ Build Tools and WebView2.

## Sounds

The project default lives at:

```text
sounds\default-notification.wav
```

It is generated by `scripts/generate_default_sound.py`, peaks at approximately `-1.1 dBFS`, and is released under the MIT license.

The file picker opens in the sound library below, and imported custom sounds are copied there:

```text
~\.codex\codex-sound-manager\sounds\
```

Select **Restore default** in the app to return to the bundled sound.

Version `1.1.0` automatically migrates settings and custom audio from the old `%LOCALAPPDATA%\CodexSoundManager` location and Codex package storage so every launch path reads the same settings.

## Configuration and Privacy

- Codex configuration: `%CODEX_HOME%\config.toml`
- First-run backup: `%CODEX_HOME%\config.toml.codex-sound-manager.bak`
- App settings: `~\.codex\codex-sound-manager\settings.json`
- UTF-8 runtime log: `~\.codex\codex-sound-manager\notifier.log`

The application does not upload Codex configuration, custom audio or logs, and it does not read conversation content. Completion payload arguments are used only when forwarding a pre-existing notifier.

## Troubleshooting

### Other conversations do not play a sound

Fully restart Codex after applying the configuration. Existing tasks can keep the old configuration while the desktop process remains open in the background.

### A black terminal appears in development

The development terminal hosts hot reload and compiler output. Release, installed and `--notify` modes use the Windows GUI subsystem and do not show a console.

### No sound is audible

Use **Preview** first. If preview is also silent, check the Windows volume mixer, the default output device and whether the selected file still exists.

### How do I quit after minimizing to the tray?

Right-click the app icon in the system tray and select **Quit**. You can also restore the main window, close it, and choose **Quit** from the prompt.

### Moving the portable app breaks notifications

Codex stores the absolute executable path. Open the app in its new location and select **Apply to Codex** again.

## Stack

- Tauri 2 + Rust
- React 19 + TypeScript
- Tailwind CSS + Shadcn-inspired components
- Radix UI + Lucide icons
- rodio + Symphonia audio decoding
- toml_edit configuration editing

## License

The source code and original default sound are released under the [MIT License](LICENSE). See [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md) for font and dependency licenses.
