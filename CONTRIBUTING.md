# Contributing to TermLook

Install Rust 1.85+ and Ubuntu's `build-essential`, `pkg-config`, `libgtk-3-dev`
and `libvte-2.91-dev`. Run `cargo run --release --locked` from the checkout.

Keep changes focused. The model, settings and storage modules contain no GTK
objects. UI widgets and shell processes belong to their respective adapters.
Keep FFI in `terminal.rs` / `tray.rs`, document pointer ownership, and test spawn
cancellation whenever terminal lifetime changes. UI event handlers defer model
mutations to the GTK main loop; callbacks hold weak references to application state.

Before submitting:

```sh
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
GDK_BACKEND=x11 G_DEBUG=fatal-criticals GSETTINGS_BACKEND=memory GIO_USE_VFS=local NO_AT_BRIDGE=1 \
  timeout 90s dbus-run-session -- xvfb-run -a cargo test --locked --bin termlook gui_lifecycle -- --ignored --test-threads=1
./scripts/build_deb.sh
```

The GUI regression requires `xvfb`, `xauth` and `dbus-x11`. It checks lazy
restoration, persistent shell variables, splits, zoom, swaps, live preferences,
OSC 7 directories, PTY cleanup and cancellation during rapid close operations.
Also test affected interactions on a real desktop, including X11/Wayland and
tray behavior where relevant. Commit `Cargo.lock`; do not commit `target/` or
built packages. Keep `VERSION` and the package version in `Cargo.toml` in sync.

Target `main` in pull requests. Explain the resulting behavior, relevant tests,
and limitations. Report your distribution, desktop, display server, version and
shell for desktop issues. Remove private terminal output and credentials from logs.

Contributions are made under the [MIT license](LICENSE).
