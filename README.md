# Vigilant

Application de prise de notes **temporaires et chiffrées**, distribuée **uniquement en Flatpak**.

## Principe de sécurité

- **Aucune écriture disque** : les notes vivent exclusivement en mémoire (RAM).
- **Chiffrement ChaCha20-Poly1305** : chaque note est scellée sous une clé de session aléatoire, non persistée.
- **Mémoire verrouillée** (`mlockall`) : les pages du processus ne peuvent pas partir en swap, clé de session incluse.
- **Session unique** : à la fermeture de l'application, tout est détruit — clés et textes en clair sont *zéroïsés*.
- **Core dumps désactivés** (`RLIMIT_CORE=0`) : aucun fichier core ne peut être écrit en cas de crash.
- **Sandbox Flatpak sans accès disque persistant** : aucun système de fichiers hors `/app` et XDG config.
- **UI GTK4 / libadwaita**, native GNOME, adaptée à Flatpak.

### Effacement automatique (optionnel, désactivé par défaut)

Le menu ⚙ de l'application propose :

- un interrupteur « Effacement automatique des notes » (désactivé par défaut ;
  la fenêtre peut rester ouverte toute la journée sans rien effacer) ;
- un délai réglable de 1 à 480 minutes (10 par défaut) ;
- un bouton « Effacer toutes les notes maintenant ».

### Swap : les données restent chiffrées

Le contenu des notes est chiffré en mémoire (ChaCha20-Poly1305, clé de
session non persistée). Si le noyau place des pages du processus en swap,
elles ne quittent jamais la RAM : les pages du processus sont verrouillées
par `mlockall`, y compris la clé de session. En cas d'échec du verrouillage
(affiché sur stderr), le chiffrement en mémoire reste la seconde ligne de défense.

### Captures d'écran : limite connue

Vigilant ne peut pas garantir qu'une capture d'écran soit noircie. Sur
Wayland, c'est le compositeur qui décide ; sur X11, n'importe quelle
application peut capturer l'écran. Ne considérez pas les captures comme
impossibles : c'est une limite documentée, volontairement.

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
