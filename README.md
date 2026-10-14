<p align="center"><img src="assets/io.termlook.TermLook.svg" width="88" alt="TermLook icon"></p>
<h1 align="center">TermLook</h1>
<p align="center"><strong>A little room for every project.</strong><br>Native Linux terminal with workspaces, split panes, and your own shell.</p>

![TermLook workspace interface](docs/images/termlook.png)

TermLook **0.4.2** is written in Rust, using GTK 3 for the desktop interface and
VTE for terminal emulation. Python is no longer required. The application runs
shells directly on the host and keeps opened sessions alive across workspace and
tab switches. The screenshot above shows the earlier Python interface; the Rust
version retains its workspace/sidebar layout.

## Build and run (Ubuntu 24.04)

Install Rust **1.85 or newer**, then the native development libraries:

```sh
sudo apt update
sudo apt install build-essential pkg-config libgtk-3-dev libvte-2.91-dev librsvg2-common
cargo run --release --locked
```

After building, launch `./target/release/termlook` directly. Cargo is only needed
to build; the installed application uses the system GTK/VTE libraries.

```sh
./scripts/build_deb.sh
sudo apt install ./dist/termlook_0.4.2_amd64.deb
termlook
```

The package architecture follows the build host. For a user-local installation
instead, run `./scripts/install_desktop.sh`; it installs the executable in
`~/.local/bin` and adds an application-menu entry. Quit an older running TermLook
before launching the new version: both share the same application ID.

## Workspaces, tabs and panes

- Create a workspace with the `+` beside **spaces**. Create a tab with the top-bar `+`.
- Double-click a workspace or tab name to rename it. F2 renames the active tab.
- Split left/right or top/bottom, nesting up to **four panes** per tab.
- Drag dividers to resize. Drag a pane header onto another pane to swap positions.
- Use **Four panes** for a 2×2 layout, or **Focus pane** to temporarily hide others.
- Closing a pane, tab or workspace asks for confirmation before ending sessions. Cancel is the default; quitting the application also asks.
- Closing the final tab creates a new `main` tab; deleting the final workspace creates `work`.
- Tab close buttons are visible by default; accidental clicks open a confirmation dialog.

Restored tabs allocate no terminal widgets or shells until selected. Opened tabs
keep their PTYs, output, commands and shell variables. Restarting restores names,
directories and split geometry, but starts fresh shells; it does not restore
processes or scrollback. The shell can report working-directory changes using
OSC 7; otherwise TermLook retains the initial directory.

## Settings and customization

Open **⚙ Settings** or press **Ctrl+,**. **Apply** validates and saves changes.
Appearance changes affect existing terminals without restarting their shells.
Save and close or the window close button saves before closing. Validation or disk errors keep the editor open. Discard changes explicitly discards edits made since the last save.

- **Appearance:** font, foreground/background colors, 16-color ANSI palette,
  cursor shape/blinking, terminal padding and sidebar width. Choose a background photo with cover/contain/stretch fitting and 0–100% dimming. Charcoal, Midnight
  and Light buttons provide starting colors.
- **Terminal:** shell executable, scrollback, bell and automatic scrolling.
- **Behavior:** close-to-tray/minimize and tab close buttons.
- **Shortcuts:** edit every application action; duplicate or invalid shortcuts
  are rejected. Empty shortcuts disable an action.
- **Profiles:** named shell/directory configurations and a default profile.
  Profiles have a form for their name, folder and shell. Choose a folder and press Save project, Apply, or Save and close; no separate add step is needed. An empty name defaults to the folder name. Saved profiles appear in a visible list above the form; select a row to edit it. New project starts another profile. The ANSI palette remains editable as JSON.

Preferences are stored in `~/.config/termlook/settings.json`, respecting
`XDG_CONFIG_HOME`. Existing 0.1.x settings and `layout.json` remain readable;
new fields receive defaults. Back up these two files before switching between
versions if you want to preserve a particular layout.

A partial settings file is sufficient. For example:

```json
{
  "font": "Monospace 12",
  "background": "#11111b",
  "foreground": "#cdd6f4",
  "show_tab_close_buttons": true,
  "keybindings": { "new-tab": "<Primary><Shift>t", "grid": "<Primary><Shift>g" },
  "profiles": { "project": { "shell": "/bin/zsh", "cwd": "/home/you/project" } },
  "default_profile": "project"
}
```

Use **Projects…** in the sidebar or **Ctrl+Shift+P** for quick opening. Each saved project has **Редагувати** (edit) and **Видалити** (delete) buttons. Confirmed deletion is saved immediately and removes only the profile, preserving files and running terminals. A profile
opens its own workspace with the configured folder and shell. Opening it again
selects the existing workspace. Its profile association survives restarts; new
panes use that project's shell, while existing processes keep running. Removing
a profile does not delete a workspace or its files; it falls back to global shell
settings for future sessions.

The **F1 → Проєкти** help page includes a Ukrainian guide to creation, saving, startup tasks, logs and troubleshooting.

**Project task templates:** in Settings → Profiles, enter one task per line:

```text
Server | npm run dev
Tests | npm test
Logs | tail -f app.log
```

Use commands appropriate to your project. Each task runs concurrently in its own
terminal tab, in the profile folder, through the profile shell with `-c`. Both
stdout and stderr stream directly to the terminal. Keep servers in the foreground
(no trailing `&`) so completion status follows the actual process. Shell startup
files for interactive sessions may not load; configure needed environment variables
in the command or invoke a project script.

Enable **Run tasks automatically when I open this project**, then press Save project or Apply. **Projects → Open project** runs its template once per workspace in the
current application session. Opening it again selects it without rerunning commands.
**Open + run tasks** explicitly reruns a finished template in fresh tabs, preserving
old logs; it refuses while any template task is still running. Stop tasks with
Ctrl+C or close their tabs with confirmation. Each template supports up to 12 tasks.

Task tabs show running, success, exit code, or termination signal. Desktop notifications
can report all completions, errors only (default), or be disabled. Notifications depend
on your desktop notification service. Errors printed by a still-running program remain
in its log; TermLook determines failure from process exit status, not text matching.
Log scrollback follows the terminal settings and remains until the tab/application
closes; it is not archived to disk. Restoring the application layout does not rerun
commands: open the profile from Projects to launch its template again. Editing a
template applies to the next run and leaves current processes untouched.

The default profile directory applies to new tabs. Restored panes keep their
saved directories; splits inherit the focused pane's directory. Shell settings
apply when a session starts. An unavailable shell falls back to `$SHELL`, the
login shell, `/bin/bash`, then `/bin/sh`. A missing directory falls back to home.

Print all defaults with `termlook --print-default-settings`. After editing the
file externally, press **Ctrl+Shift+R** to validate and reload it. Invalid reloads
leave the current settings active. Startup tolerates invalid fields and uses
defaults for them.

To use a photo, open **Settings → Appearance → Choose image…**, adjust dimming
and fitting, and Apply. PNG, JPEG, WebP and other formats supported by the system
image loader are accepted. The file must remain available at its selected path.
The image is shared behind all panes, decoded at a maximum of 3840×2160; input
files are limited to 64 MiB and 100 megapixels. An unreadable image is rejected on
Apply, and startup falls back to a solid color if the file has disappeared.

For GTK interface styling, create `~/.config/termlook/theme.css` and reload:

```css
#sidebar { background: #11111b; }
.terminal-tab.active { background: #45475a; }
.terminal-pane.focused { border-color: #89b4fa; }
```

Terminal text/background and ANSI colors come from settings. Invalid custom CSS
is reported and the built-in stylesheet remains available.

## Background mode

Closing the window hides it when an AppIndicator host is available, otherwise
it minimizes so running sessions stay accessible. Launching TermLook again
restores its existing window. **Ctrl+Shift+Q** or tray **Quit** ends the application.
Disable background mode in Settings to ask before quitting on window close.

On Ubuntu, optional tray support uses:

```sh
sudo apt install libayatana-appindicator3-1 gnome-shell-extension-appindicator
```

Enable the desktop's AppIndicator extension. The tray menu includes Open,
Settings and Quit. If the tray host disappears while TermLook is hidden, its
window reappears. Desktop-specific tray integration should also be checked on
your actual X11/Wayland session.

## Default shortcuts

| Action | Shortcut |
| --- | --- |
| Open project | Ctrl+Shift+P |
| New tab / workspace | Ctrl+Shift+T / Ctrl+Shift+N |
| Close tab / pane | Ctrl+Shift+W / Ctrl+Shift+X |
| Split left/right / top/bottom | Ctrl+Shift+E / Ctrl+Shift+D |
| Focus pane / show all | Ctrl+Shift+Enter |
| Next / previous tab | Ctrl+Tab / Ctrl+Shift+Tab |
| Copy / paste | Ctrl+Shift+C / Ctrl+Shift+V |
| Rename tab | F2 |
| Help (guide and searchable shortcuts) | F1 |
| Settings / reload settings | Ctrl+, / Ctrl+Shift+R |
| Quit | Ctrl+Shift+Q |

Press **F1** or click **? Help** in the sidebar for the built-in help window. It
has three tabs: **Quick start** (workspaces, tabs, panes and background mode),
**Shortcuts** (searchable, grouped, and always showing your current bindings) and
**Customize** (config file locations, `theme.css` and a button that opens Settings).
Toolbar tooltips also show each action's current shortcut.

Right-click inside a terminal for Copy/Paste. Ctrl+C still interrupts the shell's
current command. The title/tab bar supports desktop window dragging.

## Implementation and verification

```text
src/model.rs        Workspace/tab/pane model and compatible split-tree serialization
src/settings.rs     Validated settings, palettes, profiles and shortcut defaults
src/storage.rs      Size-bounded JSON reads and atomic, private file replacement
src/terminal.rs     GTK 3 VTE adapter, asynchronous spawn and cancellation
src/tray.rs         Optional dynamically loaded AppIndicator integration
src/ui.rs           Window, navigation, pane layout and application actions
src/help.rs         Help window: quick start, searchable shortcuts, customization
src/preferences.rs  Live settings editor
src/background.rs   Shared image background and fitting
src/closing.rs      Confirmation before ending sessions
src/projects.rs     Profile editor and project workspace launcher
src/main.rs         Single-instance application and GTK/VTE lifecycle regression
assets/             Embedded icon and default GTK stylesheet
scripts/            Native Debian build, local install and release scripts
```

Rust's release profile enables thin LTO, one codegen unit and stripped symbols.
Lazy terminals, cached pane/navigation widgets and coalesced layout writes avoid
unnecessary work. This is not a claim of a measured speedup over Python: VTE was
already native in the old implementation.

```sh
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
sudo apt install xvfb xauth dbus-x11
GDK_BACKEND=x11 G_DEBUG=fatal-criticals GSETTINGS_BACKEND=memory GIO_USE_VFS=local NO_AT_BRIDGE=1 \
  dbus-run-session -- xvfb-run -a cargo test --locked --bin termlook gui_lifecycle -- --ignored --test-threads=1
```

See [Contributing](CONTRIBUTING.md) and [Packaging](docs/packaging.md).
Native Windows is not supported; Linux builds can run under WSLg.

Licensed under the [MIT License](LICENSE). Authors: [Contributors](CONTRIBUTORS.md).
