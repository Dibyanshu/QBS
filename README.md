# Quick Browser Switcher (QBS)

Quick Browser Switcher is a lightweight native Windows 11 widget for viewing
and switching the default web browser between Google Chrome and Microsoft Edge.
It runs as a floating card and notification-area application, uses CPU-rendered
graphics, and requires no administrator privileges.

QBS is inspired by the small native architecture and interaction model of
[AI Limits](https://github.com/napxlexn/ailimits), while using original QBS
branding, UI, and source code.

## Highlights

- Shows whether Chrome, Edge, another browser, or a mixed configuration is the
  current default.
- Checks `HTTP`, `HTTPS`, `.htm`, and `.html` independently.
- Switches from the floating card, tray menu, or command line.
- Uses Windows' supported Default Apps screen by default.
- Offers an explicitly acknowledged experimental silent mode with post-change
  verification and automatic recovery through Windows Settings.
- Discovers browsers from Windows Registered Applications and Capabilities;
  installation paths and ProgIDs are not hard-coded.
- Uses a compact 169×120 card and supports dragging, edge snapping, position locking, remembered placement,
  light/dark/system themes, and 60%, 75%, 90%, or 100% opacity.
- Can be kept above other windows or used as a normal non-topmost window.
- Stays out of the taskbar while remaining available from the tray.
- Uses an event-driven loop with no continuous high-frequency polling.
- Includes single-instance protection, optional per-user autostart, portable
  distribution, and a per-user installer.
- Contains no Electron, WebView, browser framework, background service,
  telemetry, API key, or automatic updater.

## Requirements and scope

- Windows 11 x64
- Google Chrome and/or Microsoft Edge
- A standard user account is sufficient

Version 1 manages only these web associations:

| Association | Purpose |
|---|---|
| `HTTP` | Unencrypted web links |
| `HTTPS` | Secure web links |
| `.htm` | HTML documents |
| `.html` | HTML documents |

QBS never changes PDF, SVG, FTP, `microsoft-edge:`, or unrelated file and URI
associations.

## Installation

### Per-user installer

Run `QBS-Setup-0.1.0-x64.exe`. It installs under the current user's local
application directory without elevation. Optional installer tasks create a
desktop shortcut and start QBS with Windows.

### Portable version

Extract `QBS-0.1.0-Windows-x64.zip` and run `qbs.exe`. Keep `qbs-cli.exe` beside
it for command-line status and switching.

Portable and installed copies share the current user's configuration. A named
Windows mutex ensures that only one widget instance runs at a time.

## Using the widget

The card displays the detected current browser and a primary button targeting
the other installed browser. A target is disabled when its Registered
Application entry or required capabilities are unavailable.

- Click the main button to run the configured switching flow.
- Drag outside the button to reposition the card.
- Release near a monitor work-area edge to snap the card to that edge.
- Right-click the card to open the complete context menu.
- Left-click the tray icon to show or hide the card.
- Closing the card hides it; select **Quit** to exit QBS.

Status refreshes after a switch attempt, when the card regains focus, and when
**Refresh status** is selected. QBS does not poll continuously.

## Context menu

| Command | Behavior |
|---|---|
| Switch to Google Chrome | Runs the configured Chrome flow |
| Switch to Microsoft Edge | Runs the configured Edge flow |
| Supported confirmation | Selects the safe Windows Settings flow |
| Experimental silent | Selects the unsupported UserChoice backend |
| I understand the experimental risk | Records the required acknowledgement |
| Open Windows Default Apps | Opens the general Default Apps page |
| Refresh status | Re-reads registrations and all four associations |
| Lock position | Prevents accidental dragging |
| Always on top | Toggles whether the card stays above normal windows |
| Start with Windows | Adds or removes the current-user startup entry |
| Theme | Selects System, Light, or Dark |
| Opacity | Selects 60%, 75%, 90%, or 100% |
| Show / Hide widget | Toggles card visibility |
| About QBS | Displays version and project information |
| Quit | Exits the application |

## Switching modes

### Supported confirmation — default

QBS opens the selected browser's registered Windows Default Apps page:

```text
ms-settings:defaultapps?registeredAppMachine=<registered-application-name>
```

The user then clicks **Set default**. This follows the supported Windows model:
applications register capabilities, while the user chooses defaults through
Windows UI. See Microsoft's
[Default Apps documentation](https://learn.microsoft.com/en-us/windows/apps/develop/windows-integration/default-apps-platform).

### Experimental silent — explicit opt-in

This unsupported mode uses a versioned implementation of the legacy protected
`UserChoice` hash and attempts to update only the four QBS web associations
under the current user's profile.

Before it can run, select both **Experimental silent** and **I understand the
experimental risk**. After an attempt, QBS reads all four associations again.
It reports success only if every association resolves to the selected browser.
A partial result, rejected write, unsupported mechanism, or detected
`UserChoiceLatest` configuration produces an explicit failure and opens the
supported Default Apps page.

Windows can change or block this mechanism at any time. Test experimental mode
in a disposable Windows 11 VM. Registry writes alone are never treated as proof
of success.

## Command line

`qbs.exe` is the GUI application. `qbs-cli.exe` is its console companion.

```powershell
# Launch the floating widget
.\qbs.exe

# Run the saved switching mode and show the widget
.\qbs.exe --browser chrome
.\qbs.exe --browser edge

# Force the supported Windows Settings flow
.\qbs.exe --supported --browser chrome
.\qbs.exe --supported --browser edge

# Print machine-readable discovery and status JSON
.\qbs-cli.exe --status

# Switch from a console
.\qbs-cli.exe --browser chrome
.\qbs-cli.exe --browser edge
.\qbs-cli.exe --supported --browser edge
```

Status JSON includes discovered browsers, registration names, executable and
icon sources, resolved ProgIDs, installation state, the four current
associations, and the classified current state.

## Configuration and logging

Preferences are stored in:

```text
%LOCALAPPDATA%\QuickBrowserSwitcher\config.toml
```

The versioned schema stores switching mode, warning acknowledgement, window
position, lock state, always-on-top preference, theme, opacity, autostart preference, and last selected
browser. Defaults are supported mode, no acknowledgement, unlocked position,
always on top, system theme, 90% opacity, and autostart disabled. Missing fields in older or
partial files receive safe defaults.

Diagnostics are disabled by default. To enable local logging:

```powershell
$env:QBS_LOG = '1'
$env:RUST_LOG = 'qbs=debug'
.\qbs.exe
```

QBS does not transmit usage data or diagnostics.

## Architecture

QBS uses Rust 2021 with a minimum supported Rust version of 1.86. One `tao`
event loop owns the window, renderer, configuration, browser snapshot, native
menu, and tray icon.

```text
User input / activation / tray event
                |
                v
          src/main.rs
       tao event-loop owner
        /      |       \
       v       v        v
   ui/*    browser.rs   switching/*
 renderer   discovery    supported / experimental
 menu          |                |
               v                v
       associations.rs <--- verification
               |
               v
        Windows registry

 config.rs <---- main ----> platform.rs
 TOML state                Win32 integration
```

### Source layout

| Path | Responsibility |
|---|---|
| `src/main.rs` | GUI entry point, mutex, event loop, input, refresh lifecycle, tray |
| `src/bin/qbs-cli.rs` | Console status and switching interface |
| `src/browser.rs` | Browser model, registration discovery, current-state classification |
| `src/associations.rs` | Managed associations, UserChoice inspection, ProgID and path parsing |
| `src/switching/mod.rs` | Result model, supported flow, backend selection |
| `src/switching/experimental.rs` | Isolated legacy hash backend, constrained writes, verification |
| `src/config.rs` | Versioned TOML schema, defaults, loading, persistence |
| `src/platform.rs` | Shell, DWM, opacity, monitor, SID, startup, and notification helpers |
| `src/ui/renderer.rs` | CPU-rendered card and tray graphics |
| `src/ui/menu.rs` | Native context-menu construction and action mapping |
| `installer/qbs.iss` | Per-user Inno Setup definition |
| `scripts/package.ps1` | Release, portable ZIP, and installer packaging |

### Runtime stack

- `tao`: event loop and floating window
- `softbuffer`: CPU pixel-buffer presentation
- `tiny-skia`: card and icon drawing
- `fontdue`: text rasterization
- `muda`: native context menus
- `tray-icon`: notification-area integration
- `windows` and `winreg`: Win32 and registry integration
- `serde` and `toml`: configuration
- `anyhow`: contextual error handling
- `tracing`: opt-in diagnostics

There is no GPU renderer or embedded browser. `ControlFlow::Wait` lets the
application sleep while idle and wake for OS or user events.

## Discovery and current-state classification

QBS reads Registered Applications from current-user and machine registry views,
follows each registration's Capabilities record, and resolves declared URL and
file associations. This supports per-user and machine-wide installations
without assuming a fixed path or ProgID.

- **Chrome:** all four associations resolve to Chrome's declared ProgIDs.
- **Edge:** all four resolve to Edge's declared ProgIDs.
- **Mixed:** the values do not agree on one browser.
- **Other:** they consistently resolve to another registered choice.
- **Unknown:** one or more values cannot be read or classified.

Switch attempts return `Changed`, `ConfirmationRequired`,
`UnsupportedWindows`, `VerificationFailed`, `PartialResult`, or
`BrowserUnavailable`, together with a user-facing explanation. This prevents
the UI and CLI from inferring success from registry writes.

## Building from source

Install Rust 1.86+ with `x86_64-pc-windows-msvc`, Visual Studio 2022 Build
Tools with MSVC and a Windows SDK, and optionally Inno Setup 6.

```powershell
Set-Location 'D:\Own\QuickBrowserSwitcher-QBS\QBS-App'

cargo fmt --check
cargo test --all-targets --locked
cargo clippy --all-targets --locked -- -D warnings
cargo build --release --locked
```

Release binaries are written to `target\release\qbs.exe` and
`target\release\qbs-cli.exe`.

## Packaging

From a shell with MSVC tools available:

```powershell
.\scripts\package.ps1
```

This creates:

```text
dist\QBS-0.1.0-Windows-x64.zip
dist\QBS-Setup-0.1.0-x64.exe
```

Packages include the MIT license, third-party notices, and generated dependency
license inventory.

## Testing

Unit tests cover browser-state classification, command/icon parsing,
configuration defaults and migration, Settings URI construction, and
experimental hash primitives.

Destructive association tests belong in disposable Windows 11 x64 VMs. Test
Chrome to Edge and Edge to Chrome, standard users, restart persistence,
partial-write and unsupported-hash behavior, each of the four associations,
multi-monitor and DPI placement, Explorer restart, startup entries, and
installed/portable coexistence.

## Security and privacy

- Normal operation requires no elevation.
- Preferences contain no secrets.
- Logging is opt-in and local.
- QBS has no telemetry or network service.
- Supported mode delegates the final decision to Windows.
- Experimental writes are limited to the four QBS associations and verified.

## Licensing and attribution

QBS-owned source is licensed under the [MIT License](LICENSE). Dependency
attribution is in [THIRD_PARTY_LICENSES.md](THIRD_PARTY_LICENSES.md). The
experimental legacy UserChoice backend is an independent Rust port based on the
MIT-licensed [PS-SFTA](https://github.com/DanysysTeam/PS-SFTA) algorithm; see
[THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).
