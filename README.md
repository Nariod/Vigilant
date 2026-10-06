# Vigilant

Application de prise de notes **temporaires et chiffrées**, distribuée **uniquement en Flatpak**.

## Principe de sécurité

- **Aucune écriture disque** : les notes vivent exclusivement en mémoire (RAM).
- **Chiffrement ChaCha20-Poly1305** : chaque note est scellée sous une clé de session aléatoire, non persistée.
- **Session unique** : à la fermeture de l'application, tout est détruit — clés et textes en clair sont *zéroïsés*.
- **UI GTK4 / libadwaita**, native GNOME, adaptée à Flatpak.

## Structure du projet

```
core/   Bibliothèque métier : NoteStore (chiffrement, mémoire, zeroize) + tests
app/    Binaire GTK4/libadwaita (UI)
data/   desktop file, metainfo AppStream, icône
io.github.nariod.Vigilant.yml   Manifest Flatpak
```

## Développement

```bash
cargo test -p vigilant-core
cargo run -p vigilant   # nécessite gtk4 et libadwaita installés
```

## Build Flatpak

```bash
# 1. Vendoring des dépendances Rust (build offline dans le sandbox Flatpak)
cargo vendor vendor/
# (un .cargo/config.toml pointant vers vendor/ est généré)

# 2. Build
flatpak-builder --force-clean build-dir io.github.nariod.Vigilant.yml

# 3. Installation locale
flatpak-builder --user --install --force-clean build-dir io.github.nariod.Vigilant.yml
```

Le SDK requis est `org.gnome.Sdk` (runtime `org.gnome.Platform`).

## Distribution

L'application n'est pas destinée à être distribuée hors Flatpak. Pour publier
sur Flathub, le manifest `io.github.nariod.Vigilant.yml` suit le
format flatpak-builder et peut être adapté pour pointer vers un tag git plutôt
que `type: dir`.
