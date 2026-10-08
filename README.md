# Vigilant

**Ephemeral, encrypted** note-taking application, distributed **Flatpak only**.

## Security model

What Vigilant actually guarantees:

- **No disk writes by the app itself**: notes live only in process memory,
  sealed with **ChaCha20-Poly1305** under a random session key that is never
  persisted.
- **Zeroized cryptography**: the cipher key material is zeroized on drop
  (`chacha20poly1305` built with the `zeroize` feature) and plaintext buffers
  handled in Rust are zeroized after use.
- **Explicit shutdown wipe**: on window close the store, the search field, the
  input field and the session clipboards are cleared, instead of relying on
  process teardown.
- **Non-dumpable process** (`prctl(PR_SET_DUMPABLE, 0)` plus `RLIMIT_CORE=0`):
  other processes of the same user cannot read this process's memory through
  `/proc/<pid>/mem`, `ptrace` or core dumps.
- **Notes are masked by default** and only decrypted into a label when you
  explicitly reveal them, one note at a time.
- **Private input**: input fields set `GTK_INPUT_HINT_PRIVATE` /
  `NO_SPELLCHECK`, and undo history is disabled on the note input.
- **GTK4 / libadwaita UI**, native GNOME, Flatpak-only distribution.

What Vigilant **cannot** guarantee (documented residual risks):

- **GTK makes its own plaintext copies.** A revealed note is copied by GTK and
  Pango (label text, Pango layout, accessibility tree) and freed without
  zeroization. Reveal notes sparingly.
- **Accessibility bus**: revealed text is exposed to AT-SPI clients of the
  session, like in any GTK application.
- **Clipboard managers**: copying a revealed note hands it to the session's
  clipboard history (GNOME extensions, CopyQ, Klipper), which may persist it on
  disk. Vigilant clears its clipboards on close but cannot recall what a
  clipboard manager already stored.
- **Swap / hibernation**: `mlockall` is attempted but usually fails under
  Flatpak because of the memlock limit; a banner is shown when it fails. If
  your machine hibernates, the RAM image — including plaintext — hits the
  disk. Disable hibernation or use encrypted swap.
- **Screenshots, screen sharing, shoulder-surfing**: notes are masked by
  default, but a revealed note is on screen.
- **Flatpak always provides a writable `~/.var/app/<id>/`**: the app never
  writes there, but the sandbox itself is not disk-less.
- For kernel-level hardening, consider `init_on_free=1` and encrypted swap.

### Automatic wiping (optional, off by default)

The in-app ⚙ menu provides:

- an "Automatic note wiping" toggle (off by default; the window can stay
  open all day without wiping anything);
- an adjustable **idle** delay from 1 to 480 minutes (10 by default):
  notes, the search field and the input field are erased after that much
  inactivity;
- an "Erase all notes now" button.

Note deletion always asks for confirmation and cannot be triggered by a
single accidental click.

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

Then log back in. Without this, Vigilant shows a warning banner in the
window (and on stderr when run from a terminal) and in-memory encryption
remains the only protection against swap forensics.

### Swap

If `mlockall` fails, plaintext pages can be swapped out. Notes are stored
encrypted, but anything currently revealed in the UI exists in plaintext in
process memory. Use encrypted swap and disable hibernation for a hardened
setup.

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
