# Packaging and Windows support

## Native Debian package (Ubuntu 24.04)

Install Rust 1.85+, `build-essential`, `pkg-config`, `libgtk-3-dev` and
`libvte-2.91-dev`, then build without root:

```sh
./scripts/build_deb.sh
sudo apt install ./dist/termlook_0.4.2_amd64.deb
```

The script verifies that `VERSION` matches `Cargo.toml`, builds the locked release
profile and packages the native executable, desktop entry, icon and license.
Packages use the build host's architecture; they are no longer `Architecture: all`.
Cross-compilation is not handled by this script. GTK/VTE are runtime dependencies;
Python and Rust are not required on the destination machine.

Run `./scripts/install_desktop.sh` for a user-local binary and desktop entry
instead. If switching from a previous checkout installation to a system `.deb`,
remove its `~/.local/share/applications/io.termlook.TermLook.desktop` launcher
and any `~/.local/bin/termlook` that shadows `/usr/bin/termlook`.
Quit existing TermLook instances before testing a different build.

To test an installed package without a real desktop:

```sh
sudo apt install xvfb xauth xdotool dbus-x11 desktop-file-utils
desktop-file-validate /usr/share/applications/io.termlook.TermLook.desktop
GDK_BACKEND=x11 G_DEBUG=fatal-criticals GSETTINGS_BACKEND=memory GIO_USE_VFS=local NO_AT_BRIDGE=1 \
  timeout 90s dbus-run-session -- xvfb-run -a ./tests/smoke_package.sh
```

The smoke test uses a temporary configuration, runs a command through the host
shell, checks custom shortcuts and quits through a confirmation dialog and checks saved layout. CI also runs the Rust
lifecycle regression before package installation. Release publication runs only
on successful main-branch CI; the script refuses to replace a published release
or a tag pointing at a different commit.

APT upgrades retain `~/.config/termlook`; removal also leaves user configuration
in place. Packages do not configure an automatic-update repository.

## Windows

The Rust application still uses GTK 3, Unix PTYs and VTE. It does not provide a
native Windows executable. Build and run inside Ubuntu under WSLg using the Linux
instructions. Disable close-to-tray if your WSLg environment has no suitable tray.

A native Windows port requires a ConPTY backend, compatible terminal rendering,
Windows shell startup/cleanup, desktop integration and Windows build/installer
tests. The model and settings code can be reused; switching to Rust alone does
not make the current GTK/VTE frontend a native Windows terminal.
