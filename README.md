# Vigilant

Application de prise de notes **temporaires et chiffrées**, distribuée **uniquement en Flatpak**.

## Principe de sécurité

- **Aucune écriture disque** : les notes vivent exclusivement en mémoire (RAM).
- **Chiffrement ChaCha20-Poly1305** : chaque note est scellée sous une clé de session aléatoire, non persistée.
- **Session unique** : à la fermeture de l'application, tout est détruit — clés et textes en clair sont *zéroïsés*.
- **Core dumps désactivés** (`RLIMIT_CORE=0`) : aucun fichier core ne peut être écrit en cas de crash.
- **Sandbox Flatpak sans accès disque persistant** : aucun système de fichiers hors `/app` et XDG config.
- **UI GTK4 / libadwaita**, native GNOME, adaptée à Flatpak.

### Auto-wipe (optionnel, désactivé par défaut)

L'application reste ouverte toute la journée sans rien effacer. Pour activer
l'effacement automatique après N minutes d'existence des notes :

```bash
flatpak override --user --env=VIGILANT_AUTO_WIPE_MINUTES=10 io.github.nariod.Vigilant
```

`0` (valeur par défaut) = jamais d'effacement automatique.

### Blocage des captures d'écran

Sur **Wayland**, le compositeur contrôle les captures : GTK4 permet de
marquer la surface comme *non-enregistreable* (`xdg_session_lock` /
`constraint`), mais le comportement des outils de screenshot (noir vs
capture ignorée) dépend du compositeur (GNOME, KDE, wlroots). Sur Wayland
pur, la fuite de contenu par capture d'écran système est de toute façon
limitée à l'utilisateur lui-même. Sur X11, cette protection n'existe pas.

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
