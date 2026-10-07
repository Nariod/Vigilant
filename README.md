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

```bash
cargo test -p vigilant-core
cargo run -p vigilant   # requires gtk4 and libadwaita
```

## Flatpak build

```bash
# 1. Vendor the Rust dependencies (offline build inside the Flatpak sandbox)
cargo vendor vendor/
# (a .cargo/config.toml pointing to vendor/ is generated)

# 2. Build
flatpak-builder --force-clean build-dir io.github.nariod.Vigilant.yml

# 3. Local install
flatpak-builder --user --install --force-clean build-dir io.github.nariod.Vigilant.yml
```

The required SDK is `org.gnome.Sdk` (runtime `org.gnome.Platform`).

## Distribution

The application is not intended to be distributed outside Flatpak. To publish
on Flathub, the `io.github.nariod.Vigilant.yml` manifest follows the
flatpak-builder format and can be adapted to point at a git tag instead of
`type: dir`.
