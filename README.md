# Ekspointer

Ekspointer met le pointeur de souris en évidence et permet de tracer des formes par-dessus n'importe quelle application Windows. Les formes s'effacent seules après quelques secondes. Il est pensé pour les formations en visio (Google Meet ou autre, en partage d'écran entier) et en présentiel.

![Logo Ekspointer](assets/logo-preview.png)

- Six outils : flèche, rectangle, ellipse, trait libre, surligneur, numéros d'étapes.
- Trois effets de pointeur : halo coloré, onde au clic, traînée laser.
- Six couleurs, trois épaisseurs, effacement automatique réglable de 2 à 15 secondes, épingle pour garder les formes.
- Dessin sur tous les écrans branchés.
- Un seul exécutable de moins de 1 Mo, écrit en Rust, sans dépendance à installer.

## Installation

Lancez `Ekspointer-Setup-x.y.z.exe`. L'installation se fait pour votre compte, sans droits administrateur, dans `%LOCALAPPDATA%\Programs\Ekspointer`. Les réglages sont enregistrés dans le même dossier (`config.json`).

L'installateur et l'exécutable ne sont pas signés : au premier lancement, Windows SmartScreen affiche un avertissement. Cliquez sur « Informations complémentaires » puis « Exécuter quand même ».

La version portable (`Ekspointer-x.y.z-portable.exe`) fonctionne sans installation : elle enregistre ses réglages à côté d'elle.

## Utilisation

Une barre d'outils flottante apparaît en haut de l'écran. Glissez-la par son logo, double-cliquez sur le logo pour la replier.

Il y a deux façons de dessiner :

- **Mode dessin** : cliquez sur un outil, ou tapez `Ctrl+Alt+D`. La souris trace. `Échap` rend la main aux applications.
- **Touche maintenue** : tenez `Ctrl+Alt` (touches de gauche) et glissez la souris. Tout redevient normal au relâchement.

| Action | Raccourci par défaut |
|---|---|
| Activer ou quitter le dessin | `Ctrl+Alt+D` |
| Dessiner en maintenant | `Ctrl+Alt` |
| Tout effacer | `Ctrl+Alt+E` |
| Afficher le halo | `Ctrl+Alt+H` |
| Flèche, Rectangle, Ellipse, Trait libre, Surligneur, Numéros | `F`, `R`, `C`, `D`, `S`, `N` (en mode dessin) |
| Couleurs 1 à 6 | `1` à `6` (en mode dessin) |
| Annuler la dernière forme | `Ctrl+Z` (en mode dessin) |
| Contraindre (angle de 15°, carré, cercle, ligne droite) | `Maj` pendant le tracé |

Tous les raccourcis se changent dans les réglages. Les raccourcis globaux peuvent être une combinaison de touches ou un **double appui** rapide sur une même touche, par exemple `2×Ctrl` ou `2×Tab`. Avec une touche ordinaire comme Tab, le premier appui atteint l'application en cours ; avec Ctrl ou Maj, le double appui est sans effet dans l'application. Un raccourci global doit comporter Ctrl, Alt ou Win (ou être une touche de fonction), pour ne jamais gêner la frappe.

Sur un clavier français, AltGr n'est jamais pris pour `Ctrl+Alt` : seules les touches Ctrl et Alt de gauche comptent.

## Partage d'écran

Les formes et les effets sont visibles par vos interlocuteurs quand vous partagez **l'écran entier**. Si vous ne partagez qu'une fenêtre ou un onglet, l'outil de visio ne les transmet pas.

Par défaut, la barre d'outils est masquée du partage d'écran : seuls les tracés sont transmis. Cela se change dans les réglages.

## Compilation

Prérequis : [Rust](https://rustup.rs/), les Build Tools Visual Studio avec les outils C++, et [Inno Setup 6](https://jrsoftware.org/isinfo.php) pour l'installateur.

```powershell
scripts\cargo.cmd test                                            # tests unitaires et d'intégration
scripts\cargo.cmd test --test process -- --ignored --test-threads=1   # tests du vrai exécutable
powershell -ExecutionPolicy Bypass -File scripts\build-release.ps1    # tout, puis dist\
```

`scripts\cargo.cmd` lance `cargo` dans l'environnement de compilation de Visual Studio.

L'icône et les images de l'installateur sont générées depuis `assets/logo.svg` par `scripts\cargo.cmd run -p icon-gen`.

## Organisation du code

- `src/` (bibliothèque) : la logique, sans dépendance à Windows et couverte par les tests. Formes et leur cycle de vie (`shapes`), effets de pointeur (`pointer`), raccourcis (`shortcuts`), réglages (`config`), filtre d'entrées (`input_filter`), état de l'application (`app`), dispositions de l'interface (`ui`).
- `src/win/` (exécutable) : la couche Windows. Fenêtres transparentes par écran (`overlay`), rendu Direct2D (`gfx`, `render`), écoute clavier et souris (`input`), barre d'outils et réglages (`panel`, `ui`, `settings_view`, `icons`), zone de notification (`tray`).
- `tests/` : tests d'intégration.
- `docs/superpowers/` : spec de design et plan d'implémentation.

Les polices Instrument Sans et Noto Sans Mono sont embarquées sous licence OFL (`assets/fonts/`).
