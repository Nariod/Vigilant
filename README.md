# Vigilant

**Ephemeral, encrypted** note-taking application, distributed **Flatpak only**.

## Security model

- **No disk writes**: notes live exclusively in memory (RAM).
- **ChaCha20-Poly1305 encryption**: every note is sealed under a random session key that is never persisted.
- **Locked memory** (`mlockall`): process pages cannot be swapped out, including the session key.
- **Single session**: when the application closes, everything is destroyed — keys and plaintext are *zeroized*.
- **Core dumps disabled** (`RLIMIT_CORE=0`): no core file can be written on a crash.
- **Flatpak sandbox without persistent disk access**: no filesystem access outside `/app` and XDG config.
- **GTK4 / libadwaita UI**, native GNOME, Flatpak-ready.

### Automatic wiping (optional, off by default)

The in-app ⚙ menu provides:

- an "Automatic note wiping" toggle (off by default; the window can stay
  open all day without wiping anything);
- an adjustable delay from 1 to 480 minutes (10 by default);
- an "Erase all notes now" button.

### Memory locking: system prerequisites

`mlockall` can only succeed if the `RLIMIT_MEMLOCK` limit allows it. An
unprivileged process can raise its soft limit up to its hard limit, but no
further. On Fedora the default is 8192 KiB — not enough for a GTK4 process.
Check your configuration:

```bash
ulimit -l
```

To raise the limit at the session level (systemd), create a drop-in:

```bash
sudo systemctl edit --user   # or edit /etc/systemd/system.conf.d/
```

```ini
[Manager]
DefaultLimitMEMLOCK=infinity
```

Then log back in. Without this, Vigilant prints a warning at startup and
in-memory encryption remains the only protection against swap forensics.

### Swap: data stays encrypted

Note content is encrypted in memory (ChaCha20-Poly1305, session key never
persisted). If the kernel swaps process pages out, they never leave RAM in
plaintext: process pages are locked by `mlockall`, including the session key.
If locking fails (reported on stderr), in-memory encryption remains the
second line of defense.

### Screenshots: known limitation

Vigilant cannot guarantee that a screenshot will be blacked out. On Wayland,
the compositor decides; on X11, any application can capture the screen. Do
not consider screenshots impossible: this is a documented, deliberate
limitation.

## Project structure

```
core/   Core library: NoteStore (encryption, memory, zeroize) + tests
app/    GTK4/libadwaita binary (UI)
data/   desktop file, AppStream metainfo, icon
io.github.nariod.Vigilant.yml   Flatpak manifest
```

## Development

### Prerequisites

- Rust (stable) via [rustup](https://rustup.rs/)
- GTK4 and libadwaita development packages:

```bash
# Fedora
sudo dnf install gtk4-devel libadwaita-devel
# Debian/Ubuntu
sudo apt install libgtk-4-dev libadwaita-1-dev
```

### Build and test

The workspace uses vendored dependencies (`.cargo/config.toml` redirects
`crates.io` to `vendor/`), so most commands work fully offline:

```bash
cargo test -p vigilant-core      # core library tests (8 tests)
cargo check --workspace          # type-check everything
cargo build                      # debug build
cargo run -p vigilant            # run the app (requires gtk4 + libadwaita)
cargo build --release            # optimized binary in target/release/vigilant
```

If you change any dependency in `Cargo.toml`, you **must re-vendor**:

```bash
rm -rf vendor
cargo vendor vendor
```

The `.cargo/config.toml` is already in the repository; `cargo vendor`
prints it again but the existing file can be kept as-is.

### Debugging tips

- **Run with logs on stderr**: Vigilant prints `mlockall` failures and other
  warnings on stderr. Run from a terminal (`cargo run -p vigilant`) to see
  them.
- **GTK logging**: increase verbosity with
  `G_MESSAGES_DEBUG=all cargo run -p vigilant`.
- **Rust backtraces**: `RUST_BACKTRACE=1` (or `full`) before `cargo run`.
- **Check memory-lock status**: `ulimit -l` — see the *Memory locking*
  section above if `mlockall` warns at startup.
- **Run a single test**: `cargo test -p vigilant-core put_get_roundtrip`.
- **Clippy / formatting**:

```bash
cargo clippy --workspace
cargo fmt --check
```

### Flatpak runtime debugging

Once the Flatpak is installed:

```bash
# Shell inside the sandbox
flatpak run --command=sh io.github.nariod.Vigilant

# Run the binary with debug output
flatpak run --env=G_MESSAGES_DEBUG=all io.github.nariod.Vigilant

# See warnings (e.g. mlockall) directly
cargo run -p vigilant 2>&1 | grep -i warn
```

## Flatpak build

### One-time setup

```bash
# flatpak-builder (Fedora)
sudo dnf install flatpak-builder

# Add Flathub remote (--user or --system)
flatpak remote-add --if-not-exists --user flathub \
  https://dl.flathub.org/repo/flathub.flatpakrepo

# Install the GNOME 49 SDK, platform and Rust extension
flatpak install --user flathub \
  org.gnome.Sdk//49 \
  org.gnome.Platform//49 \
  org.freedesktop.Sdk.Extension.rust-stable//25.08
```
> The Rust extension branch is `25.08` (the Freedesktop SDK version GNOME 49
> is based on), even though the GNOME runtime branch is `49`. The manifest
> declares the extension without a branch suffix; flatpak-builder resolves
> the correct version from the SDK metadata.

### Build

```bash
# Dependencies are already vendored in vendor/ for the offline sandbox build.
# Re-vendor only if Cargo.toml changed (see Development section).

# Build
flatpak-builder --force-clean build-dir io.github.nariod.Vigilant.yml

# Build and install locally
flatpak-builder --force-clean --user --install build-dir io.github.nariod.Vigilant.yml
```

If you hit `Requested extension org.freedesktop.Sdk.Extension.rust-stable ... not
installed`, install the `//25.08` branch (see above) — flatpak-builder looks it
up by the version declared in the GNOME 49 SDK metadata.

To uninstall the local build:

```bash
flatpak uninstall --user io.github.nariod.Vigilant
```

The required SDK is `org.gnome.Sdk` (runtime `org.gnome.Platform`).

## Distribution

The application is not intended to be distributed outside Flatpak. To publish
on Flathub, the `io.github.nariod.Vigilant.yml` manifest follows the
flatpak-builder format and can be adapted to point at a git tag instead of
`type: dir`.
