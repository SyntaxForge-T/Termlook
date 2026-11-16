# Changelog

All notable changes to TermLook are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project uses
[Semantic Versioning](https://semver.org/). Earlier 0.x releases are listed on the
[releases page](https://github.com/SyntaxForge-T/Termlook/releases).

## [1.0.0] - 2026-11-16

The first stable release. Existing settings and layouts keep working unchanged.

### Added

- Find in scrollback with Ctrl+Shift+F: Enter and Shift+Enter move between matches.
- Ctrl+click opens web links shown in a terminal.
- Text size shortcuts: Ctrl+=, Ctrl+- and Ctrl+0, the number pad, and Ctrl+scroll.
- Minimize and maximize buttons next to the window close button.
- Settings: a font picker, color pickers, a dropdown for the cursor shape, 16 palette
  swatches instead of a JSON text box, and shortcut fields that record the keys you
  press.
- The right-click menu offers Select all; Copy is disabled without a selection.
- The pane header shows the pane number and its working directory.
- A friendly empty state in the Projects window with a button to create the first
  project.
- Rust 1.88 or newer is now the documented minimum for building from source.

### Changed

- A shell that exits normally closes its pane. After an error the output stays on
  screen. Closing or quitting no longer asks for confirmation when nothing is running.
- The close button tooltip says whether it quits, hides to the tray or minimizes.
- Messages in the status line fade out after ten seconds.
- The tab bar scrolls to the active tab, accepts the mouse wheel and no longer draws a
  scrollbar over the tabs.

### Fixed

- Settings, Help, Projects and the confirmation dialogs were hard to read on a light
  system theme: inactive Settings tabs, Profiles labels and the Help title were dark on
  dark. TermLook now asks for the dark theme and styles its own dialogs explicitly.
- The saved-projects list in Settings showed nothing when empty.
- The build broke when `gio` was updated without the rest of the gtk-rs crates; they now
  stay on one release line.
