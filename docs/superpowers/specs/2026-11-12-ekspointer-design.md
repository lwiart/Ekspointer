# Ekspointer — spec de design

Date : 12 novembre 2026
Statut : à valider

## 1. But

Ekspointer est une application Windows qui aide un formateur à guider le regard de ses apprenants, en visio (Google Meet ou autre, écran entier partagé) comme en présentiel. Elle met le pointeur de souris en évidence et permet de tracer des formes par-dessus n'importe quelle application. Les formes s'effacent seules après quelques secondes.

Critères de réussite :

- On dessine une flèche en moins de deux secondes depuis n'importe quelle application, sans quitter ce qu'on montrait.
- La première utilisation ne demande aucune configuration.
- L'application est légère : exécutable de moins de 6 Mo, moins de 50 Mo de mémoire, 0 % de processeur au repos, démarrage en moins de 300 ms.
- L'interface suit le design system Eksplodo 2026 et les formes, en particulier les flèches, sont soignées.

## 2. Périmètre

Dans cette version :

- Six outils de dessin : flèche, rectangle, ellipse, trait libre, surligneur, numéros d'étapes.
- Trois effets de pointeur : halo coloré, onde au clic, traînée laser.
- Six couleurs, trois épaisseurs.
- Effacement automatique en fondu, avec option pour épingler.
- Deux façons de dessiner : mode bascule et touche maintenue.
- Dessin sur tous les écrans branchés.
- Barre d'outils flottante, panneau de réglages, icône de zone de notification.
- Raccourcis clavier personnalisables.
- Programme d'installation et version portable.

Hors périmètre : texte, zoom, tableau blanc, capture ou enregistrement d'écran, signature de code, autres systèmes que Windows, applications en plein écran exclusif (certains jeux).

## 3. Choix techniques

- Langage : Rust, édition 2021, cible `x86_64-pc-windows-msvc`. Windows 10 version 1903 ou plus récent, Windows 11.
- Fenêtres et entrées : API Win32 via la crate `windows`.
- Rendu : Direct3D 11 + DXGI + DirectComposition + Direct2D, texte par DirectWrite. Repli sur le rendu logiciel WARP si aucune carte graphique n'est utilisable.
- Réglages : `serde` et `serde_json`.
- Les deux écoutes vivent sur un fil d'exécution dédié, qui ne fait rien d'autre : Windows attend leur réponse avant de livrer chaque frappe et chaque mouvement, elles ne doivent donc jamais patienter derrière le rendu.
- Aucune autre dépendance à l'exécution. Pas de WebView, pas de bibliothèque d'interface.
- Polices embarquées dans l'exécutable (licence OFL), en sous-ensembles latins : Instrument Sans (libellés) et Noto Sans Mono (étiquettes, raccourcis). Chargement en mémoire par DirectWrite. Repli sur Segoe UI et Consolas si le chargement échoue.
- Icônes de l'interface : tracés vectoriels définis dans le code, aucune image.
- Prérequis de compilation : `rustup` avec la chaîne stable MSVC et les Build Tools Visual Studio (éditeur de liens C++), Inno Setup 6 pour l'installateur. Aucun n'est installé sur la machine à ce jour.

## 4. Interface

### 4.1 Barre d'outils

Pastille flottante, hauteur 56 px logiques, rayon complet, toujours au premier plan, déplaçable sur n'importe quel écran. Sa position est mémorisée. Si l'écran mémorisé n'existe plus, elle revient en haut au centre de l'écran principal.

Contenu, de gauche à droite, séparé par des filets verticaux 1 px :

1. **Logo** : poignée de déplacement. Double-clic : replie la barre en un disque de 56 px qui ne montre que le logo ; double-clic à nouveau : la déplie.
2. **Outils** : Curseur (mode normal), Flèche, Rectangle, Ellipse, Trait libre, Surligneur, Numéros.
3. **Style** : pastille de la couleur courante (ouvre un volet avec les 6 couleurs), épaisseur (ouvre un volet avec 3 tailles).
4. **Pointeur** : Halo, Onde au clic, Traînée laser. Chaque bouton s'active et se désactive indépendamment.
5. **Actions** : Épingler, Tout effacer, Réglages.

Boutons de 40 px, icônes au trait de 1,75 px. Au survol, une bulle affiche le nom de l'outil et son raccourci en Noto Sans Mono.

### 4.2 Style

Jetons du design system Eksplodo 2026 :

| Rôle | Valeur |
|---|---|
| Fond de la barre et des volets | `#FCFDFE` |
| Filets, bord de la barre | `#CEDEE6` |
| Icônes, texte | `#003C58` |
| Texte secondaire | `#4E6F7E` |
| Survol d'un bouton | fond `#E3F1F7` |
| Outil actif | pastille pleine `#003C58`, icône `#FFFFFF` |
| Effet de pointeur activé | icône `#0C659D`, fond `#E3F1F7` |
| Dessin actif | anneau `#F0631F` autour du logo de la barre |
| Ombre de la barre | `0 20px 44px -20px rgba(0,60,88,.45)` |

Rayons : pastille complète pour tout ce qui est interactif, 18 px pour le panneau de réglages, 14 px pour les volets. Aucun gris neutre pur, aucun fond sombre.

### 4.3 Mouvement

- La pastille de l'outil actif glisse d'un bouton à l'autre en 220 ms, courbe `cubic-bezier(.16,.84,.3,1)`.
- Un halo radial bleu clair suit le curseur au survol de la barre.
- Les volets et le panneau apparaissent par fondu et montée de 8 px en 180 ms.
- Seules la position et l'opacité sont animées.
- Si Windows a désactivé les animations (réglage « Afficher les animations »), tous les états sont appliqués sans transition, y compris pour les formes.

### 4.4 Panneau de réglages

Fenêtre de 420 × 560 px logiques, rayon 18 px, ouverte depuis la barre ou la zone de notification. Sections :

- **Dessin** : durée avant effacement (2 à 15 s, par pas de 1 s), épaisseur par défaut.
- **Pointeur** : taille du halo (40 à 120 px), couleur du halo liée ou non à la couleur de dessin.
- **Raccourcis** : une ligne par action. On clique sur la ligne, on tape la combinaison, elle est enregistrée. Échap annule la saisie. Pour les raccourcis globaux (bascule du dessin, tout effacer, halo), un sélecteur à deux positions choisit le type : « Combinaison » ou « Double appui » ; en double appui, on tape une seule touche et la ligne affiche `2×Touche`. Une combinaison déjà utilisée par une autre action est refusée, avec un message qui nomme l'action en conflit. Un bouton rétablit les raccourcis par défaut.
- **Général** : lancer au démarrage de Windows, masquer la barre d'outils du partage d'écran.

Chaque changement est appliqué et enregistré immédiatement, sans bouton de validation.

### 4.5 Zone de notification

Icône Ekspointer avec un menu : Afficher ou masquer la barre, Réglages, Quitter. Un clic gauche sur l'icône affiche la barre.

### 4.6 Logo

Dérivé du logo Eksplodo : l'écran et les nuages de décollage sont conservés, la fusée est remplacée par une flèche-pointeur qui décolle, avec un point ambre `#F0631F`. Encre `#003C58`.

- Source : `assets/logo.svg`.
- Icône d'application `assets/ekspointer.ico` (16, 24, 32, 48, 64, 256 px), générée depuis le SVG par un script et versionnée. En 16 et 24 px, version simplifiée : la flèche-pointeur et le point ambre seuls.
- La même forme est dessinée en vectoriel dans la barre d'outils.

## 5. Comportement

### 5.1 Modes

- **Mode normal** : les fenêtres de dessin laissent passer la souris et le clavier. L'utilisateur travaille normalement. Les effets de pointeur activés restent visibles.
- **Mode dessin (bascule)** : activé par un clic sur un outil de la barre ou par le raccourci de bascule. Les fenêtres de dessin captent la souris, le curseur devient une croix. Échap, le raccourci de bascule ou l'outil Curseur reviennent au mode normal. Les fenêtres de dessin ne prennent jamais le focus : l'application en cours reste active.
- **Touche maintenue** : tant que la combinaison de maintien est tenue, un appui du bouton gauche dessine avec le dernier outil utilisé. Les déplacements de souris sans bouton enfoncé continuent de passer aux applications. Au relâchement de la combinaison, le tracé en cours est terminé et tout revient au mode normal.

La barre d'outils reste cliquable dans tous les modes.

### 5.2 Raccourcis par défaut

Raccourcis globaux, actifs dans tous les modes :

| Action | Raccourci |
|---|---|
| Basculer le mode dessin | `Ctrl+Alt+D` |
| Dessiner tant que tenu | `Ctrl+Alt` |
| Tout effacer | `Ctrl+Alt+E` |
| Activer ou couper le halo | `Ctrl+Alt+H` |

Raccourcis actifs seulement en mode dessin (ils sont interceptés et n'atteignent pas l'application en cours) :

| Action | Raccourci |
|---|---|
| Flèche, Rectangle, Cercle, Trait libre, Surligneur, Numéros | `F`, `R`, `C`, `D`, `S`, `N` |
| Couleurs 1 à 6 | `1` à `6` (rangée du haut et pavé numérique) |
| Annuler la dernière forme | `Ctrl+Z` |
| Quitter le mode dessin | `Échap` |

Tous sont modifiables dans les réglages.

**Double appui** (ajout demandé par l'utilisateur le 13/11/2026). Un raccourci global peut être, au choix, une combinaison de touches ou un double appui rapide sur une même touche, par exemple `2×Tab` ou `2×Ctrl`. Les deux appuis doivent se suivre en moins de 400 ms, sans autre touche entre eux.

- Touche ordinaire (`2×Tab`) : le premier appui atteint l'application en cours, car rien ne permet encore de savoir qu'un second suivra. Le second est intercepté et déclenche l'action.
- Touche de modification (`2×Ctrl` ou `2×Maj`) : l'action se déclenche au second relâchement, sans effet dans l'application en cours. C'est le choix le plus discret. Alt et Win ne sont pas proposés : tapés seuls, ils ouvrent un menu.
- Une touche de modification utilisée dans un raccourci (`Ctrl+C` deux fois), suivie d'un clic (`Ctrl+clic` deux fois) ou AltGr ne comptent pas comme un double appui.

**Raccourcis refusés.** Pour ne jamais gêner la frappe, un raccourci global (bascule du dessin, tout effacer, halo) doit comporter Ctrl, Alt ou Win, ou être une touche de fonction : une lettre seule serait interceptée dans toutes les applications. Le raccourci de maintien ne comporte que des touches de modification. Un raccourci refusé affiche sa raison sous la ligne, comme un conflit. Cliquer hors du panneau abandonne une saisie en cours.

Sur un clavier français, la touche AltGr est vue par Windows comme `Ctrl+Alt`. Ekspointer ne compte donc que les touches Ctrl et Alt **de gauche** : AltGr ne déclenche jamais un raccourci, et la saisie de `@`, `#` ou `{` n'est pas perturbée.

### 5.3 Outils

Toutes les dimensions sont en pixels logiques, mis à l'échelle selon l'écran.

- **Flèche** : glisser de l'origine vers la cible. Maj contraint l'angle par pas de 15°. Géométrie en 5.4.
- **Rectangle** : glisser d'un coin à l'autre, contour seul, coins arrondis de 6 px. Maj : carré.
- **Ellipse** : glisser le rectangle englobant, contour seul. Maj : cercle.
- **Trait libre** : tracé à main levée, lissé (courbes de Catmull-Rom sur des points espacés d'au moins 3 px), extrémités arrondies.
- **Surligneur** : comme le trait libre, largeur 22 px, opacité 35 %, extrémités carrées. Maj : ligne horizontale ou verticale.
- **Numéros** : chaque clic pose une pastille de 32 px de la couleur courante, chiffre en Noto Sans Mono 600, blanc (encre sur la pastille jaune ou blanche). Le compteur repart à 1 quand il n'y a plus aucune pastille à l'écran ou après « Tout effacer ».

Une forme de moins de 12 px (flèche, rectangle, ellipse) est abandonnée au relâchement.

Épaisseurs : 3, 5 et 8 px. Elles s'appliquent à la flèche, au rectangle, à l'ellipse et au trait libre.

Couleurs, dans l'ordre des touches 1 à 6 : ambre `#F0631F` (par défaut), bleu `#0C659D`, encre `#003C58`, jaune `#FFC400`, vert `#1FA971`, blanc `#FFFFFF`.

Lisibilité sur tout fond : chaque forme, sauf le surligneur, porte un liseré blanc de 1,5 px à 85 % d'opacité et une ombre douce teintée encre. Les formes blanches portent un liseré encre.

### 5.4 Géométrie de la flèche

Style demandé par l'utilisateur le 13/11/2026, d'après une image de référence : un corps qui part en pointe fine de l'origine et s'élargit nettement, une tête large aux ailes tirées vers l'arrière.

Pour une flèche de longueur `L` et d'épaisseur `w` :

- **Tête** : longueur `h = min(5,2 × w ; 0,45 × L)`, envergure `5,5 × w`. Les deux ailes sont tirées vers l'arrière : le creux à la base de la tête est avancé de `0,22 × h` vers la pointe.
- **Corps** : en pointe à l'origine (largeur nulle), large de `2,2 × w` à la jonction avec la tête.
- **Flèche courte** : quand `h` est plafonnée, l'envergure et la largeur du corps sont réduites dans la même proportion.
- **Angles** : très légèrement adoucis, rayon `0,15 × w`.
- Le tout forme un seul contour fermé, rempli de la couleur courante.

### 5.5 Cycle de vie d'une forme

1. **Tracé** : la forme suit la souris.
2. **Apparition** : au relâchement, finition en 150 ms (la tête de flèche s'ouvre, la pastille de numéro grossit de 80 % à 100 %).
3. **Maintien** : pendant la durée réglée (5 s par défaut), comptée depuis le relâchement.
4. **Fondu** : opacité de 100 % à 0 % en 600 ms, puis la forme est supprimée.

Épingler suspend le passage au fondu pour toutes les formes, présentes et à venir. Désépingler relance la durée de maintien de chaque forme à partir de cet instant. « Tout effacer » fait disparaître toutes les formes en 200 ms. `Ctrl+Z` supprime la forme la plus récente encore visible.

### 5.6 Effets de pointeur

- **Halo** : disque de 64 px par défaut, couleur courante à 35 % d'opacité, bord adouci, centré sur le curseur. Il suit le curseur sur tous les écrans.
- **Onde au clic** : anneau qui s'élargit de 8 à 48 px de rayon et s'efface en 450 ms. Clic gauche : un anneau de la couleur courante. Clic droit : deux anneaux concentriques bleus `#0C659D`.
- **Traînée laser** : le curseur laisse une traînée ambre couvrant ses 700 dernières millisecondes, qui s'amincit et s'efface vers l'arrière, avec une lueur.

Les trois sont indépendants et fonctionnent dans tous les modes. Leur état est mémorisé.

### 5.7 Partage d'écran

Les formes et les effets de pointeur sont visibles par les apprenants quand l'écran entier est partagé. Ils ne le sont pas si seule une fenêtre ou un onglet est partagé : c'est une limite du partage de fenêtre, pas d'Ekspointer.

Option « Masquer la barre d'outils du partage d'écran », activée par défaut : la barre, les volets et le panneau de réglages sont exclus de la capture (`SetWindowDisplayAffinity`), les apprenants ne voient que les annotations.

## 6. Architecture

Un seul exécutable, un seul processus, un fil d'exécution principal avec la boucle de messages Windows.

```
src/
  main.rs        point d'entrée, instance unique, boucle de messages
  app.rs         état de l'application, aiguillage des événements
  config.rs      réglages : modèle, lecture, écriture, valeurs par défaut
  shortcuts.rs   modèle d'un raccourci, analyse, affichage, détection de conflits
  shapes/
    mod.rs       modèle des formes et de leur cycle de vie
    arrow.rs     géométrie de la flèche
    stroke.rs    lissage du trait libre
  pointer.rs     état du halo, des ondes et de la traînée
  input.rs       écoute bas niveau du clavier et de la souris
  overlay.rs     fenêtres de dessin, une par écran
  render.rs      rendu Direct2D des formes et des effets
  ui/
    toolbar.rs   barre d'outils
    settings.rs  panneau de réglages
    widgets.rs   boutons, volets, curseurs, bulles
    icons.rs     tracés des icônes et du logo
    theme.rs     jetons Eksplodo
  tray.rs        zone de notification
  autostart.rs   lancement au démarrage
```

Règles de dépendance :

- `shapes`, `pointer`, `shortcuts` et `config` sont de la logique pure. Ils ne dépendent pas de Windows et reçoivent le temps en paramètre, ce qui les rend testables.
- `render` lit l'état de `shapes` et `pointer` et le dessine. Il ne le modifie pas.
- `input` traduit les événements bruts en événements de l'application (`BasculerDessin`, `DebutTrace`, `Clic`, etc.). Il ne connaît ni les formes ni les fenêtres.
- `app` est le seul module qui modifie l'état.

### 6.1 Fenêtres de dessin

Une fenêtre par écran, couvrant tout l'écran, sans bordure, toujours au premier plan, absente de la barre des tâches et d'Alt+Tab, qui ne prend jamais le focus. Elle est composée par DirectComposition avec transparence par pixel (`WS_EX_NOREDIRECTIONBITMAP`).

- Mode normal : `WS_EX_TRANSPARENT`, la souris traverse.
- Mode bascule : `WS_EX_TRANSPARENT` est retiré, la fenêtre reçoit la souris sur toute sa surface.
- Touche maintenue : la fenêtre reste traversante, l'appui et le relâchement du bouton gauche sont interceptés par l'écoute bas niveau.

Chaque fenêtre gère sa propre mise à l'échelle (per-monitor DPI v2). Les fenêtres sont recréées quand un écran est branché, débranché ou change de résolution. Les formes en cours sont alors effacées.

La barre d'outils est une fenêtre distincte, placée au-dessus des fenêtres de dessin.

### 6.2 Boucle de rendu

Le rendu est à la demande. Une image n'est produite que si quelque chose change : tracé en cours, animation, déplacement du curseur avec halo ou traînée actifs. Tant qu'une animation est en cours, le rendu suit le rafraîchissement de l'écran. Sinon, aucune image n'est produite et le processus dort.

Seules les fenêtres concernées sont redessinées.

### 6.3 Entrées

Une écoute bas niveau du clavier (`WH_KEYBOARD_LL`) et une de la souris (`WH_MOUSE_LL`) :

- Clavier : raccourcis globaux, combinaison de maintien, raccourcis du mode dessin. Seules les touches consommées par Ekspointer sont interceptées.
- Souris : position pour le halo et la traînée, clics pour les ondes, tracé en touche maintenue.

Les fonctions d'écoute ne font que poster un événement à la boucle principale, pour ne jamais ralentir la souris ou le clavier du système.

### 6.4 Réglages

Fichier `config.json` placé **à côté de l'exécutable**, soit `%LOCALAPPDATA%\Programs\Ekspointer\config.json` après installation. La version portable lit et écrit de la même façon, dans son propre dossier.

Contenu : version du format, durée d'effacement, épaisseur, couleur courante, dernier outil, état et taille du halo, état de l'onde et de la traînée, état de l'épingle, raccourcis, position et état replié de la barre, masquage au partage. Le lancement au démarrage n'y figure pas : sa source de vérité est le registre.

Écriture atomique : fichier temporaire puis renommage.

### 6.5 Divers

- **Instance unique** : un mutex nommé. Un second lancement affiche la barre de l'instance en cours puis se termine.
- **Lancement au démarrage** : valeur `Ekspointer` dans `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`.

## 7. Gestion des erreurs

- **`config.json` absent** : valeurs par défaut, le fichier est créé au premier changement.
- **`config.json` illisible ou invalide** : il est renommé en `config.invalide.json`, les valeurs par défaut sont utilisées, une notification le signale.
- **Champ inconnu ou manquant** : ignoré ou remplacé par sa valeur par défaut, sans erreur.
- **Dossier non inscriptible** (version portable sur support protégé) : les réglages restent en mémoire pour la session, une notification le signale une fois.
- **Périphérique graphique perdu** (mise en veille, changement de pilote) : toutes les ressources de rendu sont recréées, celles des fenêtres de dessin comme celles de la barre et des réglages. Les formes en cours, la position de la barre et l'état des réglages sont conservés.
- **Relâchement de touche non vu** (écran de verrouillage, fenêtre d'une application élevée) : avant chaque frappe, l'état mémorisé est recalé sur l'état réel du clavier, pour qu'une touche crue enfoncée ne bloque ni le maintien ni le double appui.
- **Aucune accélération graphique** : repli sur WARP.
- **Échec de l'écoute clavier ou souris au démarrage** : message d'erreur explicite, puis arrêt. L'application est inutilisable sans elle.
- **Erreur inattendue** : écrite dans `ekspointer.log` à côté de l'exécutable (limité à 1 Mo). Les fenêtres de dessin repassent en mode traversant avant tout arrêt, pour ne jamais laisser l'écran bloqué.

## 8. Programme d'installation

Construit avec Inno Setup 6, script `installer/ekspointer.iss`. Produit `Ekspointer-Setup-x.y.z.exe`.

- Installation par utilisateur, sans droits administrateur (`PrivilegesRequired=lowest`), dans `%LOCALAPPDATA%\Programs\Ekspointer`.
- Assistant en français, icône Ekspointer, visuels aux couleurs Eksplodo.
- Raccourci dans le menu Démarrer, toujours. Cases à cocher : raccourci sur le Bureau, lancer au démarrage de Windows, lancer Ekspointer à la fin.
- Mise à jour : relancer un installateur plus récent ferme l'application si elle tourne, remplace les fichiers et conserve `config.json`.
- Désinstallation depuis « Applications installées » : ferme l'application, retire le programme, les raccourcis et la valeur de lancement au démarrage, puis demande s'il faut supprimer ou garder les réglages.
- L'installateur et l'exécutable ne sont pas signés : SmartScreen affiche un avertissement au premier lancement.

Version portable : l'exécutable seul, utilisable sans installation.

Script `scripts/build-release.ps1` : compile en mode release, lance les tests, construit l'installateur, dépose l'installateur et l'exécutable portable dans `dist/`.

## 9. Tests

Tests automatiques (`cargo test`), sur la logique pure :

- **Flèche** : proportions de la tête selon `L` et `w`, plafonnement à `0,4 × L`, contrainte d'angle à 15°, abandon sous 12 px.
- **Formes** : carré et cercle avec Maj, lissage du trait libre, ligne droite du surligneur avec Maj.
- **Cycle de vie** : passage tracé, apparition, maintien, fondu, suppression aux bons instants ; épingler et désépingler ; tout effacer ; annulation ; remise à 1 du compteur de numéros.
- **Pointeur** : durée de vie des ondes, élagage de la traînée à 700 ms.
- **Raccourcis** : analyse et affichage d'une combinaison, détection des conflits, exclusion d'AltGr.
- **Réglages** : aller-retour lecture et écriture, fichier invalide, champs manquants ou inconnus.

Vérification manuelle, application lancée, avec captures d'écran :

- Chaque outil, chaque couleur, chaque épaisseur, sur fond clair et sur fond sombre.
- Les trois effets de pointeur.
- Mode bascule et touche maintenue, saisie avec AltGr non perturbée.
- Deux écrans avec des mises à l'échelle différentes.
- Modification d'un raccourci, refus d'un conflit.
- Visibilité des annotations dans un partage d'écran entier Google Meet, barre masquée.
- Installation, mise à jour, désinstallation.
- Mesures : taille de l'exécutable, mémoire, processeur au repos.
