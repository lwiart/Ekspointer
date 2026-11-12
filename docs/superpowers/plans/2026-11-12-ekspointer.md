# Ekspointer Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Construire Ekspointer, une application Windows en Rust qui met le pointeur en évidence et permet de tracer des formes éphémères par-dessus n'importe quelle application, avec son installateur.

**Architecture:** Un seul exécutable, un seul fil d'exécution avec la boucle de messages Win32. La logique (formes, pointeur, raccourcis, réglages, filtre d'entrées, état de l'application) est pure, sans Windows, et testée par `cargo test`. Une couche Windows fine l'entoure : fenêtres transparentes DirectComposition par écran, rendu Direct2D, écoute bas niveau clavier et souris, barre d'outils dessinée sur mesure.

**Tech Stack:** Rust stable (édition 2021, cible `x86_64-pc-windows-msvc`), crate `windows`, `serde`, `serde_json`, Direct3D 11 / DXGI / DirectComposition / Direct2D / DirectWrite, Inno Setup 6.

**Spec:** `docs/superpowers/specs/2026-11-12-ekspointer-design.md`. À lire avant toute tâche : les valeurs (couleurs, durées, dimensions) y font foi.

## Global Constraints

- Windows 10 version 1903 ou plus récent, Windows 11, x64 uniquement.
- Dépendances à l'exécution limitées à `windows`, `serde`, `serde_json`. Pas de WebView, pas de bibliothèque d'interface.
- Exécutable de moins de 6 Mo, moins de 50 Mo de mémoire, 0 % de processeur au repos, démarrage en moins de 300 ms.
- Les modules `geom`, `shapes`, `pointer`, `shortcuts`, `config`, `input_filter`, `app` ne dépendent pas de la crate `windows` et reçoivent le temps en paramètre (`now: u64`, millisecondes depuis le démarrage).
- `app` est le seul module qui modifie l'état. `render` ne fait que lire.
- Seules les touches Ctrl et Alt de gauche comptent pour les raccourcis. AltGr ne déclenche jamais rien.
- Les fenêtres de dessin ne prennent jamais le focus.
- Toutes les dimensions de la spec sont en pixels logiques, multipliées par l'échelle de l'écran concerné.
- Couleurs de dessin, dans l'ordre 1 à 6 : `#F0631F`, `#0C659D`, `#003C58`, `#FFC400`, `#1FA971`, `#FFFFFF`. Épaisseurs : 3, 5, 8.
- Interface en français. Jetons du design system Eksplodo (spec 4.2) : aucun gris neutre pur, aucun fond sombre, pastille complète pour tout ce qui est interactif.
- Réglages dans `config.json` à côté de l'exécutable, écriture atomique.
- Identifiants de code en anglais, textes d'interface et messages de commit en français.
- Avant d'écrire du code Win32, Direct2D ou DirectComposition, vérifier les signatures dans la documentation de la crate `windows` (outil Context7) : ce plan fixe quelles API utiliser, pas leur syntaxe exacte.
- Un commit par tâche au minimum, terminé par la ligne `Co-Authored-By` de la session.

## Review Focus

Cas que la spec implique sans les nommer, les plus susceptibles de gêner l'utilisateur. Chacun a son test dans la tâche indiquée.

1. **Touche de modification relâchée sans que l'écoute le voie** (fenêtre d'une application élevée au premier plan, verrouillage de session) : le dessin « touche maintenue » ne doit pas rester bloqué actif. Test `resync_clears_stuck_hold`, tâche 8.
2. **`config.json` modifié à la main avec des valeurs hors limites** (durée 0 ou 200, couleur 9, épaisseur 7) : valeurs ramenées dans les bornes, pas d'erreur. Test `sanitize_clamps_out_of_range`, tâche 7.
3. **Raccourcis du fichier en conflit entre eux ou illisibles** : l'action concernée reprend son raccourci par défaut, les autres sont conservés. Test `invalid_or_conflicting_bindings_fall_back`, tâche 7.
4. **Trait libre très long** (plusieurs minutes épinglé, dizaines de milliers de points) : le tracé reste fluide, le nombre de points est plafonné. Test `free_stroke_is_capped`, tâche 5.
5. **Flèche de longueur nulle ou clic sans glisser, avec Maj** : aucune valeur `NaN`, la forme est abandonnée. Tests `zero_length_is_finite` (tâche 3) et `tiny_shapes_are_dropped` (tâche 5).

---

## Structure des fichiers

```
Cargo.toml              paquet ekspointer, profil release, espace de travail
build.rs                intègre l'icône et le manifeste
app.manifest            per-monitor DPI v2, Windows 10/11
src/lib.rs              déclare les modules purs
src/main.rs             point d'entrée, instance unique, boucle de messages
src/geom.rs             Pt, RectF, Rgba
src/shapes/mod.rs       Tool, Shape, Scene, cycle de vie
src/shapes/arrow.rs     géométrie de la flèche
src/shapes/stroke.rs    lissage, contraintes Maj
src/pointer.rs          ondes, traînée
src/shortcuts.rs        Shortcut, Action, Bindings, KeyState
src/config.rs           Config, lecture, écriture
src/input_filter.rs     Filter : décide quoi intercepter, produit les InputEvent
src/app.rs              App : état et transitions
src/win/mod.rs          déclare la couche Windows (compilée par le binaire)
src/win/gfx.rs          périphériques D3D/D2D/DComp, polices
src/win/overlay.rs      fenêtres de dessin, une par écran
src/win/render.rs       dessin des formes et des effets
src/win/input.rs        écoutes bas niveau, alimente Filter
src/win/panel.rs        fenêtre flottante générique (barre, volets, réglages, bulle)
src/win/tray.rs         zone de notification
src/win/autostart.rs    lancement au démarrage
src/win/log.rs          journal ekspointer.log
src/ui/theme.rs         jetons Eksplodo
src/ui/icons.rs         tracés des icônes et du logo
src/ui/widgets.rs       boutons, curseurs, interrupteurs
src/ui/toolbar.rs       disposition et dessin de la barre
src/ui/settings.rs      panneau de réglages
assets/                 logo.svg, ekspointer.ico, polices, images de l'assistant
tools/icon-gen/         outil de génération de l'icône (hors exécutable)
installer/ekspointer.iss
scripts/build-release.ps1
```

`src/ui/theme.rs` et les fonctions de disposition (`layout`) de `toolbar.rs` et `settings.rs` sont pures et déclarées dans `lib.rs`. Le reste de `ui/` et tout `win/` appartiennent au binaire.

---

### Task 1: Environnement et squelette

**Files:**
- Create: `Cargo.toml`, `build.rs`, `app.manifest`, `src/lib.rs`, `src/main.rs`, `src/geom.rs`

**Interfaces:**
- Produces: `geom::Pt { x: f32, y: f32 }` (`Copy`, `PartialEq`, `Debug`, méthodes `new`, `dist(self, o) -> f32`, `lerp(self, o, t) -> Pt`) ; `geom::RectF { x, y, w, h: f32 }` avec `contains(Pt) -> bool` ; `geom::Rgba { r, g, b, a: f32 }` avec `const fn hex(rgb: u32) -> Rgba` et `with_alpha(f32)` ; `geom::PALETTE: [Rgba; 6]` ; `geom::WIDTHS: [f32; 3] = [3.0, 5.0, 8.0]`.

- [ ] **Step 1: Vérifier puis installer les outils**

Vérifier `%USERPROFILE%\.cargo\bin\cargo.exe` et la présence de `link.exe` (Build Tools). S'ils manquent, demander l'accord de l'utilisateur avant d'installer (plusieurs Go, fenêtre de confirmation Windows) :

```powershell
winget install Rustlang.Rustup
winget install Microsoft.VisualStudio.2022.BuildTools --override "--passive --wait --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
rustup default stable-x86_64-pc-windows-msvc
```

Expected: `cargo --version` répond dans un nouveau terminal.

- [ ] **Step 2: Créer le paquet**

`Cargo.toml` : paquet `ekspointer` version `0.1.0`, `[lib]` + `[[bin]]`, dépendances `windows` (dernière version stable, fonctionnalités Win32 ajoutées au fil des tâches), `serde` avec `derive`, `serde_json`. Dépendance de build `embed-resource`. Profil release : `opt-level = "s"`, `lto = true`, `codegen-units = 1`, `panic = "abort"`, `strip = true`. Espace de travail incluant `tools/icon-gen` (créé en tâche 15).

`app.manifest` : `dpiAwareness` = `PerMonitorV2`, `supportedOS` Windows 10/11. `build.rs` l'intègre.

`src/main.rs` : `#![windows_subsystem = "windows"]`, `fn main()` vide pour l'instant.

- [ ] **Step 3: Écrire les tests de `geom`**

```rust
#[test] fn hex_parses_channels() {
    let c = Rgba::hex(0xF0631F);
    assert!((c.r - 240.0/255.0).abs() < 1e-6 && (c.g - 99.0/255.0).abs() < 1e-6 && (c.b - 31.0/255.0).abs() < 1e-6 && c.a == 1.0);
}
#[test] fn palette_order_matches_spec() {
    assert_eq!(PALETTE[0], Rgba::hex(0xF0631F)); assert_eq!(PALETTE[1], Rgba::hex(0x0C659D));
    assert_eq!(PALETTE[2], Rgba::hex(0x003C58)); assert_eq!(PALETTE[3], Rgba::hex(0xFFC400));
    assert_eq!(PALETTE[4], Rgba::hex(0x1FA971)); assert_eq!(PALETTE[5], Rgba::hex(0xFFFFFF));
}
#[test] fn dist_and_contains() {
    assert_eq!(Pt::new(0.,0.).dist(Pt::new(3.,4.)), 5.0);
    assert!(RectF{x:0.,y:0.,w:10.,h:10.}.contains(Pt::new(5.,5.)));
    assert!(!RectF{x:0.,y:0.,w:10.,h:10.}.contains(Pt::new(10.5,5.)));
}
```

- [ ] **Step 4: Lancer, constater l'échec, implémenter `geom.rs`, relancer**

Run: `cargo test geom` — Expected: PASS (3 tests). Run: `cargo build --release` — Expected: `target/release/ekspointer.exe` produit.

- [ ] **Step 5: Commit** — `git commit -m "Squelette du projet et types géométriques"`

---

### Task 2: Jetons du thème

**Files:**
- Create: `src/ui/mod.rs`, `src/ui/theme.rs`

**Interfaces:**
- Consumes: `geom::Rgba`.
- Produces: constantes `theme::BG`, `LINE`, `INK`, `MUTED`, `HOVER`, `BLUE`, `EMBER`, `WHITE` (`Rgba`) ; `theme::BAR_H: f32 = 56.0`, `BTN: f32 = 40.0`, `ICON_STROKE: f32 = 1.75`, `R_PANEL: f32 = 18.0`, `R_POPOVER: f32 = 14.0` ; `theme::ease(t: f32) -> f32` (courbe `cubic-bezier(.16,.84,.3,1)`) ; durées `T_PILL_MS: u64 = 220`, `T_POPOVER_MS: u64 = 180`.

- [ ] **Step 1: Écrire les tests**

```rust
#[test] fn tokens_match_design_system() {
    assert_eq!(BG, Rgba::hex(0xFCFDFE)); assert_eq!(LINE, Rgba::hex(0xCEDEE6));
    assert_eq!(INK, Rgba::hex(0x003C58)); assert_eq!(MUTED, Rgba::hex(0x4E6F7E));
    assert_eq!(HOVER, Rgba::hex(0xE3F1F7)); assert_eq!(BLUE, Rgba::hex(0x0C659D));
    assert_eq!(EMBER, Rgba::hex(0xF0631F));
}
#[test] fn ease_is_monotonic_and_bounded() {
    assert!(ease(0.0).abs() < 1e-3 && (ease(1.0) - 1.0).abs() < 1e-3);
    let mut prev = 0.0; for i in 1..=20 { let v = ease(i as f32 / 20.0); assert!(v >= prev); prev = v; }
    assert!(ease(0.25) > 0.5); // courbe très rapide au départ
}
```

- [ ] **Step 2: Implémenter**, `ease` par résolution numérique de la Bézier cubique (Newton, 6 itérations). Run: `cargo test theme` — Expected: PASS.

- [ ] **Step 3: Commit** — `git commit -m "Jetons du thème Eksplodo"`

---

### Task 3: Géométrie de la flèche

**Files:**
- Create: `src/shapes/mod.rs` (déclaration des sous-modules seulement), `src/shapes/arrow.rs`

**Interfaces:**
- Consumes: `geom::Pt`.
- Produces:
  - `pub struct ArrowGeom { pub head_len: f32, pub head_width: f32, pub tail_width: f32, pub neck_width: f32, pub notch: f32, pub corner_radius: f32 }`
  - `pub fn arrow_geom(len: f32, w: f32) -> ArrowGeom`
  - `pub fn arrow_outline(from: Pt, to: Pt, w: f32) -> [Pt; 7]` — ordre : queue gauche, col gauche, aile gauche, pointe, aile droite, col droit, queue droite. Le col est à `head_len - notch` de la pointe, les ailes à `head_len`.
  - `pub fn snap_angle(from: Pt, to: Pt, step_deg: f32) -> Pt` — conserve la longueur.

- [ ] **Step 1: Écrire les tests**

```rust
fn close(a: f32, b: f32) -> bool { (a - b).abs() < 1e-3 }
#[test] fn proportions_nominal() {           // L=200, w=5
    let g = arrow_geom(200.0, 5.0);
    assert!(close(g.head_len, 22.5) && close(g.head_width, 18.0) && close(g.tail_width, 1.75)
        && close(g.neck_width, 5.0) && close(g.notch, 5.625) && close(g.corner_radius, 2.5));
}
#[test] fn head_has_minimum_16() {           // w=3 : 4,5×w = 13,5 < 16
    let g = arrow_geom(200.0, 3.0); assert!(close(g.head_len, 16.0) && close(g.head_width, 12.8));
}
#[test] fn head_capped_at_40_percent() {     // L=40, w=8 : 36 plafonné à 16
    let g = arrow_geom(40.0, 8.0); assert!(close(g.head_len, 16.0) && close(g.head_width, 12.8));
}
#[test] fn outline_tip_and_symmetry() {
    let o = arrow_outline(Pt::new(0.,0.), Pt::new(200.,0.), 5.0);
    assert!(close(o[3].x, 200.0) && close(o[3].y, 0.0));
    assert!(close(o[2].x, 177.5) && close(o[2].y, -9.0) && close(o[4].y, 9.0));
    assert!(close(o[1].x, 183.125) && close(o[1].y, -2.5));
    assert!(close(o[0].x, 0.0) && close(o[0].y, -0.875) && close(o[6].y, 0.875));
}
#[test] fn snap_to_15_degrees() {
    let p = snap_angle(Pt::new(0.,0.), Pt::new(100.,10.), 15.0);   // 5,7° -> 0°
    assert!(close(p.y, 0.0) && close(p.x, (100.0f32*100.0+100.0).sqrt()));
    let q = snap_angle(Pt::new(0.,0.), Pt::new(100.,20.), 15.0);   // 11,3° -> 15°
    assert!(close(q.y / q.x, (15.0f32).to_radians().tan()));
}
#[test] fn zero_length_is_finite() {
    let p = snap_angle(Pt::new(5.,5.), Pt::new(5.,5.), 15.0);
    assert!(p.x.is_finite() && p.y.is_finite());
    for pt in arrow_outline(Pt::new(5.,5.), Pt::new(5.,5.), 5.0) { assert!(pt.x.is_finite() && pt.y.is_finite()); }
}
```

- [ ] **Step 2: Lancer** `cargo test arrow` — Expected: FAIL (fonctions absentes).

- [ ] **Step 3: Implémenter** selon la spec 5.4 : `head_len = min(max(4.5*w, 16), 0.4*len)`, `head_width = 0.8*head_len`, `tail_width = 0.35*w`, `neck_width = w`, `notch = 0.25*head_len`, `corner_radius = 0.5*w`. Longueur nulle : direction par défaut `(1, 0)`.

- [ ] **Step 4: Relancer** `cargo test arrow` — Expected: PASS (6 tests).

- [ ] **Step 5: Commit** — `git commit -m "Géométrie de la flèche"`

---

### Task 4: Trait libre et contraintes

**Files:**
- Create: `src/shapes/stroke.rs`

**Interfaces:**
- Produces:
  - `pub const MIN_POINT_DIST: f32 = 3.0;` `pub const MAX_POINTS: usize = 5000;`
  - `pub fn push_point(pts: &mut Vec<Pt>, p: Pt, min_dist: f32) -> bool` — ajoute si la distance au dernier point atteint `min_dist` et si `MAX_POINTS` n'est pas atteint.
  - `pub fn catmull_rom_beziers(pts: &[Pt]) -> Vec<[Pt; 4]>` — un segment de Bézier cubique par paire de points consécutifs (Catmull-Rom uniforme, extrémités dupliquées).
  - `pub fn constrain_square(a: Pt, b: Pt) -> Pt` — côté = plus grande des deux dimensions, signes conservés.
  - `pub fn constrain_axis(a: Pt, b: Pt) -> Pt` — horizontale ou verticale, selon le plus grand écart.

- [ ] **Step 1: Écrire les tests**

```rust
#[test] fn push_respects_min_distance() {
    let mut v = vec![Pt::new(0.,0.)];
    assert!(!push_point(&mut v, Pt::new(2.,0.), 3.0)); assert!(push_point(&mut v, Pt::new(3.,0.), 3.0)); assert_eq!(v.len(), 2);
}
#[test] fn beziers_pass_through_points() {
    let pts = [Pt::new(0.,0.), Pt::new(10.,0.), Pt::new(20.,10.)];
    let b = catmull_rom_beziers(&pts);
    assert_eq!(b.len(), 2); assert_eq!(b[0][0], pts[0]); assert_eq!(b[0][3], pts[1]); assert_eq!(b[1][3], pts[2]);
}
#[test] fn beziers_degenerate_inputs() { assert!(catmull_rom_beziers(&[]).is_empty()); assert!(catmull_rom_beziers(&[Pt::new(1.,1.)]).is_empty()); }
#[test] fn square_keeps_signs() {
    assert_eq!(constrain_square(Pt::new(0.,0.), Pt::new(30.,-10.)), Pt::new(30.,-30.));
    assert_eq!(constrain_square(Pt::new(0.,0.), Pt::new(-5.,20.)), Pt::new(-20.,20.));
}
#[test] fn axis_picks_dominant() {
    assert_eq!(constrain_axis(Pt::new(0.,0.), Pt::new(50.,8.)), Pt::new(50.,0.));
    assert_eq!(constrain_axis(Pt::new(0.,0.), Pt::new(4.,-30.)), Pt::new(0.,-30.));
}
```

- [ ] **Step 2: Lancer, constater l'échec, implémenter, relancer** — Run: `cargo test stroke` — Expected: PASS (5 tests).

- [ ] **Step 3: Commit** — `git commit -m "Lissage du trait libre et contraintes Maj"`

---

### Task 5: Scène et cycle de vie des formes

**Files:**
- Modify: `src/shapes/mod.rs`

**Interfaces:**
- Consumes: `arrow::snap_angle`, `stroke::{push_point, constrain_square, constrain_axis, MIN_POINT_DIST}`.
- Produces:
  - `#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)] pub enum Tool { Arrow, Rect, Ellipse, Free, Highlighter, Number }`
  - `pub enum ShapeKind { Arrow { from: Pt, to: Pt }, Rect { a: Pt, b: Pt }, Ellipse { a: Pt, b: Pt }, Free { pts: Vec<Pt> }, Highlighter { pts: Vec<Pt> }, Number { at: Pt, n: u32 } }`
  - `pub struct Shape { pub id: u64, pub kind: ShapeKind, pub color: u8, pub width: f32, pub scale: f32, .. }` — `color` est un indice dans `PALETTE`, `width` en pixels logiques, `scale` l'échelle de l'écran où la forme a commencé. Les points sont en pixels physiques du bureau virtuel.
  - `pub enum Phase { Drawing, Appearing(f32), Holding, Fading(f32), Gone }` — le `f32` est la progression de 0 à 1.
  - Constantes `APPEAR_MS: u64 = 150`, `FADE_MS: u64 = 600`, `CLEAR_MS: u64 = 200`, `MIN_SIZE: f32 = 12.0`.
  - `pub struct Scene` avec :
    - `new(hold_ms: u64) -> Scene`, `set_hold_ms(&mut self, ms: u64)`
    - `begin(&mut self, tool: Tool, at: Pt, color: u8, width: f32, scale: f32, now: u64)`
    - `drag(&mut self, to: Pt, shift: bool)`
    - `release(&mut self, now: u64)`
    - `cancel(&mut self)` — abandonne la forme en cours
    - `set_pinned(&mut self, pinned: bool, now: u64)`, `pinned(&self) -> bool`
    - `clear_all(&mut self, now: u64)`, `undo(&mut self)`
    - `tick(&mut self, now: u64) -> bool` — supprime les formes `Gone`, renvoie vrai tant qu'une forme est en `Drawing`, `Appearing` ou `Fading`
    - `next_deadline(&self, now: u64) -> Option<u64>` — instant où la prochaine forme en `Holding` passera en `Fading`
    - `shapes(&self) -> &[Shape]`, `phase(&self, s: &Shape, now: u64) -> Phase`, `opacity(&self, s: &Shape, now: u64) -> f32`

- [ ] **Step 1: Écrire les tests**

```rust
fn scene() -> Scene { Scene::new(5000) }
fn draw(s: &mut Scene, tool: Tool, a: Pt, b: Pt, t: u64) { s.begin(tool, a, 0, 5.0, 1.0, t); s.drag(b, false); s.release(t); }

#[test] fn lifecycle_timings() {
    let mut s = scene(); draw(&mut s, Tool::Arrow, Pt::new(0.,0.), Pt::new(100.,0.), 1000);
    let sh = s.shapes()[0].clone();
    assert!(matches!(s.phase(&sh, 1075), Phase::Appearing(p) if (p - 0.5).abs() < 0.01));
    assert!(matches!(s.phase(&sh, 1150), Phase::Holding));
    assert!(matches!(s.phase(&sh, 5999), Phase::Holding));
    assert!(matches!(s.phase(&sh, 6300), Phase::Fading(p) if (p - 0.5).abs() < 0.01));
    assert!((s.opacity(&sh, 6300) - 0.5).abs() < 0.01);
    assert!(matches!(s.phase(&sh, 6600), Phase::Gone));
    assert!(!s.tick(6600)); assert!(s.shapes().is_empty());
}
#[test] fn next_deadline_is_fade_start() {
    let mut s = scene(); draw(&mut s, Tool::Rect, Pt::new(0.,0.), Pt::new(50.,50.), 1000);
    assert_eq!(s.next_deadline(2000), Some(6000));
}
#[test] fn tiny_shapes_are_dropped() {
    let mut s = scene();
    draw(&mut s, Tool::Arrow, Pt::new(0.,0.), Pt::new(11.,0.), 0);
    draw(&mut s, Tool::Rect, Pt::new(0.,0.), Pt::new(5.,5.), 0);
    s.begin(Tool::Ellipse, Pt::new(9.,9.), 0, 5.0, 1.0, 0); s.drag(Pt::new(9.,9.), true); s.release(0);
    assert!(s.shapes().is_empty());
}
#[test] fn min_size_scales_with_dpi() {     // 12 px logiques = 24 px physiques à 200 %
    let mut s = scene(); s.begin(Tool::Arrow, Pt::new(0.,0.), 0, 5.0, 2.0, 0); s.drag(Pt::new(20.,0.), false); s.release(0);
    assert!(s.shapes().is_empty());
}
#[test] fn shift_constraints() {
    let mut s = scene();
    s.begin(Tool::Rect, Pt::new(0.,0.), 0, 5.0, 1.0, 0); s.drag(Pt::new(30.,-10.), true); s.release(0);
    assert!(matches!(s.shapes()[0].kind, ShapeKind::Rect { b, .. } if b == Pt::new(30.,-30.)));
    s.begin(Tool::Highlighter, Pt::new(0.,0.), 0, 5.0, 1.0, 0); s.drag(Pt::new(20.,3.), true); s.drag(Pt::new(80.,9.), true); s.release(0);
    assert!(matches!(&s.shapes()[1].kind, ShapeKind::Highlighter { pts } if pts.len() == 2 && pts[1] == Pt::new(80.,0.)));
}
#[test] fn pin_suspends_and_unpin_restarts() {
    let mut s = scene(); draw(&mut s, Tool::Arrow, Pt::new(0.,0.), Pt::new(100.,0.), 0);
    s.set_pinned(true, 100);
    let sh = s.shapes()[0].clone(); assert!(matches!(s.phase(&sh, 60_000), Phase::Holding)); assert_eq!(s.next_deadline(60_000), None);
    s.set_pinned(false, 60_000);
    let sh = s.shapes()[0].clone();
    assert!(matches!(s.phase(&sh, 64_999), Phase::Holding)); assert!(matches!(s.phase(&sh, 65_300), Phase::Fading(_)));
}
#[test] fn clear_all_fades_in_200ms() {
    let mut s = scene(); draw(&mut s, Tool::Arrow, Pt::new(0.,0.), Pt::new(100.,0.), 0); s.set_pinned(true, 0);
    s.clear_all(1000); let sh = s.shapes()[0].clone();
    assert!((s.opacity(&sh, 1100) - 0.5).abs() < 0.01); s.tick(1200); assert!(s.shapes().is_empty());
}
#[test] fn undo_removes_latest_visible() {
    let mut s = scene();
    draw(&mut s, Tool::Arrow, Pt::new(0.,0.), Pt::new(100.,0.), 0); draw(&mut s, Tool::Rect, Pt::new(0.,0.), Pt::new(50.,50.), 10);
    s.undo(); assert_eq!(s.shapes().len(), 1); assert!(matches!(s.shapes()[0].kind, ShapeKind::Arrow { .. }));
    s.undo(); s.undo(); assert!(s.shapes().is_empty());
}
#[test] fn numbers_count_and_reset() {
    let mut s = scene(); let n = |s: &Scene, i: usize| match s.shapes()[i].kind { ShapeKind::Number { n, .. } => n, _ => 0 };
    for i in 0..3 { s.begin(Tool::Number, Pt::new(i as f32 * 50., 0.), 0, 5.0, 1.0, 0); s.release(0); }
    assert_eq!((n(&s,0), n(&s,1), n(&s,2)), (1, 2, 3));
    s.tick(10_000); assert!(s.shapes().is_empty());          // toutes effacées : compteur remis à 1
    s.begin(Tool::Number, Pt::new(0.,0.), 0, 5.0, 1.0, 10_000); s.release(10_000); assert_eq!(n(&s,0), 1);
    s.clear_all(10_001);
    s.begin(Tool::Number, Pt::new(0.,0.), 0, 5.0, 1.0, 10_002); s.release(10_002);
    assert_eq!(n(&s, s.shapes().len() - 1), 1);                  // « Tout effacer » remet aussi à 1
}
#[test] fn free_stroke_is_capped() {
    let mut s = scene(); s.begin(Tool::Free, Pt::new(0.,0.), 0, 5.0, 1.0, 0);
    for i in 1..20_000 { s.drag(Pt::new(i as f32 * 4.0, (i % 7) as f32 * 4.0), false); }
    assert!(matches!(&s.shapes()[0].kind, ShapeKind::Free { pts } if pts.len() == stroke::MAX_POINTS));
}
```

- [ ] **Step 2: Lancer** `cargo test shapes::` — Expected: FAIL.

- [ ] **Step 3: Implémenter `Scene`**

Décisions : chaque `Shape` porte `released_at: Option<u64>`, `hold_from: u64` (instant de départ du maintien, réécrit par `set_pinned(false, now)`) et `clearing_from: Option<u64>`. Fin de maintien = `hold_from + hold_ms`, sauf si épinglé. `MIN_SIZE` est comparé à la diagonale de la forme divisée par `scale`. Le compteur de numéros repart à 1 dès qu'aucune forme `Number` non en cours d'effacement n'existe. `undo` ignore les formes en `Fading` et celles en cours d'effacement.

- [ ] **Step 4: Relancer** `cargo test shapes::` — Expected: PASS (tous les tests des tâches 3 à 5).

- [ ] **Step 5: Commit** — `git commit -m "Scène et cycle de vie des formes"`

---

### Task 6: Effets de pointeur

**Files:**
- Create: `src/pointer.rs`

**Interfaces:**
- Produces:
  - `#[derive(Clone, Copy, PartialEq, Debug)] pub enum Button { Left, Right }`
  - `pub struct Ripple { pub at: Pt, pub button: Button, pub start: u64, pub scale: f32 }`
  - Constantes `RIPPLE_MS: u64 = 450`, `RIPPLE_R0: f32 = 8.0`, `RIPPLE_R1: f32 = 48.0`, `TRAIL_MS: u64 = 700`.
  - `pub struct Pointer` : `new()`, `pos(&self) -> Pt`, `on_move(&mut self, p: Pt, now: u64, trail_on: bool)`, `on_click(&mut self, p: Pt, button: Button, scale: f32, now: u64)`, `tick(&mut self, now: u64) -> bool` (vrai tant qu'une onde ou un point de traînée vit), `ripples(&self) -> &[Ripple]`, `ripple_state(r: &Ripple, now: u64) -> (f32, f32)` (rayon logique, opacité), `trail(&self, now: u64) -> Vec<(Pt, f32)>` (point, âge normalisé de 0 à 1).

- [ ] **Step 1: Écrire les tests**

```rust
#[test] fn ripple_grows_and_fades() {
    let r = Ripple { at: Pt::new(0.,0.), button: Button::Left, start: 1000, scale: 1.0 };
    let (r0, o0) = Pointer::ripple_state(&r, 1000); assert!((r0 - 8.0).abs() < 0.01 && (o0 - 1.0).abs() < 0.01);
    let (r1, o1) = Pointer::ripple_state(&r, 1450); assert!((r1 - 48.0).abs() < 0.01 && o1.abs() < 0.01);
    let (rm, _) = Pointer::ripple_state(&r, 1225); assert!(rm > 28.0);   // ralentit en fin de course
}
#[test] fn ripples_expire() {
    let mut p = Pointer::new(); p.on_click(Pt::new(1.,1.), Button::Right, 1.0, 0);
    assert!(p.tick(449)); assert_eq!(p.ripples().len(), 1); assert!(!p.tick(451)); assert!(p.ripples().is_empty());
}
#[test] fn trail_prunes_after_700ms() {
    let mut p = Pointer::new();
    p.on_move(Pt::new(0.,0.), 0, true); p.on_move(Pt::new(10.,0.), 400, true); p.on_move(Pt::new(20.,0.), 800, true);
    let t = p.trail(800); assert_eq!(t.len(), 2); assert!((t[0].1 - 400.0/700.0).abs() < 0.01 && t[1].1.abs() < 0.01);
    assert!(!p.tick(1501));
}
#[test] fn trail_off_records_nothing() { let mut p = Pointer::new(); p.on_move(Pt::new(5.,5.), 0, false); assert!(p.trail(0).is_empty()); assert_eq!(p.pos(), Pt::new(5.,5.)); }
```

- [ ] **Step 2: Lancer, constater l'échec, implémenter, relancer** — Run: `cargo test pointer` — Expected: PASS (4 tests). Rayon : interpolation avec `1 - (1-t)^3`. Opacité : `1 - t`.

- [ ] **Step 3: Commit** — `git commit -m "Ondes au clic et traînée laser"`

---

### Task 7: Raccourcis et réglages

**Files:**
- Create: `src/shortcuts.rs`, `src/config.rs`

**Interfaces:**
- Produces (`shortcuts`) :
  - `pub struct Mods { pub ctrl: bool, pub alt: bool, pub shift: bool, pub win: bool }`
  - `pub struct Shortcut { pub mods: Mods, pub key: Option<u32> }` — `key` est un code de touche virtuelle Windows ; `None` = combinaison de touches de modification seules. `FromStr` et `Display` : `"Ctrl+Alt+D"`, `"Ctrl+Alt"`, `"F"`, `"Échap"`, `"Ctrl+Z"`, `"F5"`. Sérialisé en chaîne.
  - `pub enum Action { ToggleDraw, HoldDraw, ClearAll, ToggleHalo, ToolArrow, ToolRect, ToolEllipse, ToolFree, ToolHighlighter, ToolNumber, Color(u8), Undo, ExitDraw }` avec `is_global(self) -> bool` (vrai pour les quatre premières) et `label(self) -> &'static str` (libellé français).
  - `pub struct KeyState { pub lctrl, rctrl, lalt, ralt, shift, win: bool }` avec `mods(&self) -> Mods` : `ctrl = lctrl && !ralt`, `alt = lalt`.
  - `pub fn normalize_vk(vk: u32) -> u32` — pavé numérique 0–9 ramené aux chiffres de la rangée du haut.
  - `pub struct Bindings` : `defaults()`, `get(Action) -> Shortcut`, `set(&mut self, a: Action, s: Shortcut) -> Result<(), Action>` (l'erreur nomme l'action en conflit), `action_for(&self, s: Shortcut, draw_mode: bool) -> Option<Action>`, `all(&self) -> Vec<(Action, Shortcut)>`. Sérialisé en table `nom d'action -> chaîne`, désérialisation tolérante.
- Produces (`config`) :
  - `#[serde(default)] pub struct Config { pub version: u32, pub hold_seconds: u8, pub width: u8, pub color: u8, pub last_tool: Tool, pub halo_on: bool, pub halo_size: u32, pub halo_follows_color: bool, pub ripple_on: bool, pub trail_on: bool, pub pinned: bool, pub bindings: Bindings, pub toolbar_pos: Option<(i32, i32)>, pub toolbar_collapsed: bool, pub hide_from_capture: bool }`
  - Valeurs par défaut : `version 1`, `hold_seconds 5`, `width 1`, `color 0`, `last_tool Arrow`, `halo_on false`, `halo_size 64`, `halo_follows_color true`, `ripple_on false`, `trail_on false`, `pinned false`, `hide_from_capture true`.
  - `pub fn sanitize(&mut self)` : `hold_seconds` dans 2..=15, `width` dans 0..=2, `color` dans 0..=5, `halo_size` dans 40..=120.
  - `pub enum LoadOutcome { Loaded, Defaults, Invalid { backup: PathBuf } }`
  - `pub fn load(path: &Path) -> (Config, LoadOutcome)`, `pub fn save(path: &Path, cfg: &Config) -> std::io::Result<()>`, `pub fn default_path() -> PathBuf` (dossier de l'exécutable + `config.json`).

- [ ] **Step 1: Écrire les tests des raccourcis**

```rust
#[test] fn parse_display_roundtrip() {
    for s in ["Ctrl+Alt+D", "Ctrl+Alt", "F", "Échap", "Ctrl+Z", "Ctrl+Maj+F5", "1"] { assert_eq!(s.parse::<Shortcut>().unwrap().to_string(), s); }
    assert!("Ctrl+".parse::<Shortcut>().is_err()); assert!("Bidule".parse::<Shortcut>().is_err());
}
#[test] fn defaults_match_spec() {
    let b = Bindings::defaults(); let g = |a| b.get(a).to_string();
    assert_eq!(g(Action::ToggleDraw), "Ctrl+Alt+D"); assert_eq!(g(Action::HoldDraw), "Ctrl+Alt");
    assert_eq!(g(Action::ClearAll), "Ctrl+Alt+E"); assert_eq!(g(Action::ToggleHalo), "Ctrl+Alt+H");
    assert_eq!(g(Action::ToolArrow), "F"); assert_eq!(g(Action::ToolRect), "R"); assert_eq!(g(Action::ToolEllipse), "C");
    assert_eq!(g(Action::ToolFree), "D"); assert_eq!(g(Action::ToolHighlighter), "S"); assert_eq!(g(Action::ToolNumber), "N");
    assert_eq!(g(Action::Color(0)), "1"); assert_eq!(g(Action::Color(5)), "6");
    assert_eq!(g(Action::Undo), "Ctrl+Z"); assert_eq!(g(Action::ExitDraw), "Échap");
}
#[test] fn conflicts_are_refused_and_named() {
    let mut b = Bindings::defaults();
    assert_eq!(b.set(Action::ToolRect, "F".parse().unwrap()), Err(Action::ToolArrow));
    assert_eq!(b.get(Action::ToolRect).to_string(), "R");
    assert!(b.set(Action::ToolRect, "R".parse().unwrap()).is_ok());        // réaffecter sa propre valeur
    assert!(b.set(Action::ToolRect, "Ctrl+Alt+R".parse().unwrap()).is_ok());
}
#[test] fn draw_only_actions_need_draw_mode() {
    let b = Bindings::defaults(); let f: Shortcut = "F".parse().unwrap();
    assert_eq!(b.action_for(f, false), None); assert_eq!(b.action_for(f, true), Some(Action::ToolArrow));
    assert_eq!(b.action_for("Ctrl+Alt+D".parse().unwrap(), false), Some(Action::ToggleDraw));
}
#[test] fn altgr_never_counts() {
    let altgr = KeyState { lctrl: true, ralt: true, ..Default::default() };       // ce que Windows envoie pour AltGr
    assert_eq!(altgr.mods(), Mods::default());
    let right = KeyState { rctrl: true, lalt: true, ..Default::default() }; assert!(!right.mods().ctrl && right.mods().alt);
    let left = KeyState { lctrl: true, lalt: true, ..Default::default() }; assert!(left.mods().ctrl && left.mods().alt);
}
#[test] fn numpad_digits_normalize() { assert_eq!(normalize_vk(0x61), 0x31); assert_eq!(normalize_vk(0x31), 0x31); assert_eq!(normalize_vk(0x46), 0x46); }
```

- [ ] **Step 2: Écrire les tests des réglages** (dossier temporaire via `std::env::temp_dir()` + nom unique)

```rust
#[test] fn missing_file_gives_defaults() { let (c, o) = load(&tmp("absent.json")); assert!(matches!(o, LoadOutcome::Defaults)); assert_eq!(c.hold_seconds, 5); assert!(c.hide_from_capture); }
#[test] fn roundtrip() {
    let p = tmp("rt.json"); let mut c = Config::default(); c.hold_seconds = 9; c.color = 3; c.toolbar_pos = Some((120, -40));
    c.bindings.set(Action::ToggleDraw, "Ctrl+Alt+X".parse().unwrap()).unwrap();
    save(&p, &c).unwrap(); let (d, o) = load(&p);
    assert!(matches!(o, LoadOutcome::Loaded)); assert_eq!(d.hold_seconds, 9); assert_eq!(d.color, 3);
    assert_eq!(d.toolbar_pos, Some((120, -40))); assert_eq!(d.bindings.get(Action::ToggleDraw).to_string(), "Ctrl+Alt+X");
    assert!(!p.with_extension("json.tmp").exists());
}
#[test] fn invalid_file_is_renamed() {
    let p = tmp("bad.json"); std::fs::write(&p, "{ pas du json").unwrap(); let (c, o) = load(&p);
    assert_eq!(c.hold_seconds, 5);
    match o { LoadOutcome::Invalid { backup } => { assert_eq!(backup.file_name().unwrap(), "config.invalide.json"); assert!(backup.exists()); assert!(!p.exists()); } _ => panic!() }
}
#[test] fn unknown_and_missing_fields_tolerated() {
    let p = tmp("partial.json"); std::fs::write(&p, r#"{"hold_seconds":8,"champ_futur":true}"#).unwrap();
    let (c, o) = load(&p); assert!(matches!(o, LoadOutcome::Loaded)); assert_eq!(c.hold_seconds, 8); assert_eq!(c.halo_size, 64);
}
#[test] fn sanitize_clamps_out_of_range() {
    let p = tmp("range.json"); std::fs::write(&p, r#"{"hold_seconds":200,"width":7,"color":9,"halo_size":5}"#).unwrap();
    let (c, _) = load(&p); assert_eq!((c.hold_seconds, c.width, c.color, c.halo_size), (15, 2, 5, 40));
    std::fs::write(&p, r#"{"hold_seconds":0}"#).unwrap(); assert_eq!(load(&p).0.hold_seconds, 2);
}
#[test] fn invalid_or_conflicting_bindings_fall_back() {
    let p = tmp("binds.json");
    std::fs::write(&p, r#"{"bindings":{"ToolArrow":"Bidule","ToolRect":"C","ToggleDraw":"Ctrl+Alt+X"}}"#).unwrap();
    let (c, o) = load(&p); assert!(matches!(o, LoadOutcome::Loaded));
    assert_eq!(c.bindings.get(Action::ToolArrow).to_string(), "F");        // illisible -> défaut
    assert_eq!(c.bindings.get(Action::ToolRect).to_string(), "R");         // conflit avec Cercle -> défaut
    assert_eq!(c.bindings.get(Action::ToggleDraw).to_string(), "Ctrl+Alt+X");
}
```

Le fichier de sauvegarde `config.invalide.json` est créé dans le même dossier que le fichier lu ; dans ce test, utiliser un sous-dossier temporaire dédié.

- [ ] **Step 3: Lancer** `cargo test shortcuts config` — Expected: FAIL.

- [ ] **Step 4: Implémenter**. Noms de touches acceptés : lettres, chiffres, `F1`–`F12`, `Échap`, `Espace`, `Entrée`, `Tab`, `Suppr`, flèches (`Haut`, `Bas`, `Gauche`, `Droite`). Modificateurs : `Ctrl`, `Alt`, `Maj`, `Win`, toujours affichés dans cet ordre. `save` écrit dans `config.json.tmp` puis renomme. `load` appelle `sanitize`.

- [ ] **Step 5: Relancer** — Expected: PASS (12 tests).

- [ ] **Step 6: Commit** — `git commit -m "Raccourcis et réglages"`

---

### Task 8: Filtre d'entrées

**Files:**
- Create: `src/input_filter.rs`

**Interfaces:**
- Consumes: `shortcuts::{Bindings, KeyState, Shortcut, Action, normalize_vk}`, `pointer::Button`, `geom::{Pt, RectF}`.
- Produces:
  - `pub enum InputEvent { Action(Action), HoldStart, HoldEnd, Shift(bool), PointerMove(Pt), Click(Pt, Button), StrokeBegin(Pt), StrokeMove(Pt), StrokeEnd, Captured(Shortcut), CaptureCancelled }`
  - `pub enum MouseKind { Move, LeftDown, LeftUp, RightDown, RightUp }`
  - `pub struct Decision { pub swallow: bool, pub events: Vec<InputEvent> }`
  - `pub struct Filter` : `new(bindings: Bindings) -> Filter`, `set_bindings(&mut self, Bindings)`, `set_draw_mode(&mut self, bool)`, `set_exempt_rects(&mut self, Vec<RectF>)` (fenêtres d'Ekspointer, jamais interceptées), `set_capturing(&mut self, bool)` (saisie d'un raccourci dans les réglages), `on_key(&mut self, vk: u32, down: bool) -> Decision`, `on_mouse(&mut self, kind: MouseKind, p: Pt) -> Decision`, `resync(&mut self, actual: KeyState) -> Vec<InputEvent>`.
  - Codes utilisés par les tests : `VK_LCONTROL = 0xA2`, `VK_RCONTROL = 0xA3`, `VK_LMENU = 0xA4`, `VK_RMENU = 0xA5`, `VK_LSHIFT = 0xA0`, `VK_ESCAPE = 0x1B`.

- [ ] **Step 1: Écrire les tests**

```rust
fn f() -> Filter { Filter::new(Bindings::defaults()) }
fn hold(f: &mut Filter) { f.on_key(0xA2, true); f.on_key(0xA4, true); }
fn has(d: &Decision, e: InputEvent) -> bool { d.events.contains(&e) }

#[test] fn hold_starts_and_ends() {
    let mut f = f(); assert!(f.on_key(0xA2, true).events.is_empty());
    let d = f.on_key(0xA4, true); assert!(!d.swallow && has(&d, InputEvent::HoldStart));
    let d = f.on_key(0xA4, false); assert!(has(&d, InputEvent::HoldEnd));
}
#[test] fn altgr_does_not_hold() { let mut f = f(); f.on_key(0xA2, true); assert!(f.on_key(0xA5, true).events.is_empty()); }
#[test] fn global_shortcut_fires_once_and_is_swallowed() {
    let mut f = f(); hold(&mut f);
    let d = f.on_key(0x44, true); assert!(d.swallow && has(&d, InputEvent::Action(Action::ToggleDraw)) && has(&d, InputEvent::HoldEnd));
    let d = f.on_key(0x44, true); assert!(d.swallow && d.events.is_empty());            // répétition clavier
    assert!(f.on_key(0x44, false).swallow);                                              // relâchement avalé aussi
}
#[test] fn letters_only_in_draw_mode() {
    let mut f = f(); let d = f.on_key(0x46, true); assert!(!d.swallow && d.events.is_empty()); f.on_key(0x46, false);
    f.set_draw_mode(true);
    let d = f.on_key(0x46, true); assert!(d.swallow && has(&d, InputEvent::Action(Action::ToolArrow)));
    let d = f.on_key(0x1B, true); assert!(d.swallow && has(&d, InputEvent::Action(Action::ExitDraw)));
    let d = f.on_key(0x61, true); assert!(has(&d, InputEvent::Action(Action::Color(0))));  // pavé numérique 1
    let d = f.on_key(0x51, true); assert!(!d.swallow);                                    // touche sans action
}
#[test] fn shift_is_reported() { let mut f = f(); assert!(has(&f.on_key(0xA0, true), InputEvent::Shift(true))); assert!(has(&f.on_key(0xA0, false), InputEvent::Shift(false))); }
#[test] fn hold_stroke_swallows_left_button_only() {
    let mut f = f(); hold(&mut f);
    let d = f.on_mouse(MouseKind::LeftDown, Pt::new(10.,10.)); assert!(d.swallow && has(&d, InputEvent::StrokeBegin(Pt::new(10.,10.))));
    let d = f.on_mouse(MouseKind::Move, Pt::new(20.,10.)); assert!(!d.swallow && has(&d, InputEvent::StrokeMove(Pt::new(20.,10.))) && has(&d, InputEvent::PointerMove(Pt::new(20.,10.))));
    let d = f.on_mouse(MouseKind::LeftUp, Pt::new(20.,10.)); assert!(d.swallow && has(&d, InputEvent::StrokeEnd));
    let d = f.on_mouse(MouseKind::RightDown, Pt::new(20.,10.)); assert!(!d.swallow && has(&d, InputEvent::Click(Pt::new(20.,10.), Button::Right)));
}
#[test] fn releasing_hold_mid_stroke_ends_it_and_swallows_button_up() {
    let mut f = f(); hold(&mut f); f.on_mouse(MouseKind::LeftDown, Pt::new(0.,0.));
    let d = f.on_key(0xA4, false); assert!(has(&d, InputEvent::StrokeEnd) && has(&d, InputEvent::HoldEnd));
    assert!(f.on_mouse(MouseKind::LeftUp, Pt::new(5.,5.)).swallow);
    assert!(!f.on_mouse(MouseKind::LeftDown, Pt::new(5.,5.)).swallow);
}
#[test] fn clicks_pass_through_without_hold() {
    let mut f = f(); let d = f.on_mouse(MouseKind::LeftDown, Pt::new(1.,1.));
    assert!(!d.swallow && has(&d, InputEvent::Click(Pt::new(1.,1.), Button::Left)));
}
#[test] fn exempt_rects_are_never_swallowed() {
    let mut f = f(); f.set_exempt_rects(vec![RectF { x: 0., y: 0., w: 100., h: 56. }]); hold(&mut f);
    let d = f.on_mouse(MouseKind::LeftDown, Pt::new(50.,20.)); assert!(!d.swallow && !d.events.iter().any(|e| matches!(e, InputEvent::StrokeBegin(_))));
}
#[test] fn capture_mode_records_one_shortcut() {
    let mut f = f(); f.set_capturing(true); f.on_key(0xA2, true); f.on_key(0xA4, true);
    let d = f.on_key(0x58, true); assert!(d.swallow && has(&d, InputEvent::Captured("Ctrl+Alt+X".parse().unwrap())));
    f.set_capturing(true); let d = f.on_key(0x1B, true); assert!(d.swallow && has(&d, InputEvent::CaptureCancelled));
}
#[test] fn capture_mode_accepts_modifier_only_on_release() {
    let mut f = f(); f.set_capturing(true); f.on_key(0xA2, true); f.on_key(0xA0, true);
    let d = f.on_key(0xA0, false); assert!(has(&d, InputEvent::Captured("Ctrl+Maj".parse().unwrap())));
}
#[test] fn resync_clears_stuck_hold() {
    let mut f = f(); hold(&mut f); f.on_mouse(MouseKind::LeftDown, Pt::new(0.,0.));
    let ev = f.resync(KeyState::default());                    // relâchements manqués
    assert!(ev.contains(&InputEvent::StrokeEnd) && ev.contains(&InputEvent::HoldEnd));
    assert!(f.resync(KeyState::default()).is_empty());
    assert!(!f.on_mouse(MouseKind::LeftDown, Pt::new(1.,1.)).swallow);
}
```

- [ ] **Step 2: Lancer** `cargo test input_filter` — Expected: FAIL.

- [ ] **Step 3: Implémenter `Filter`**

Décisions : `HoldStart` est émis quand les touches de modification tenues égalent exactement la combinaison `HoldDraw` et qu'aucune autre touche n'est enfoncée ; tout appui d'une autre touche ou relâchement d'une touche de modification émet `HoldEnd`. Le filtre garde l'ensemble des touches dont l'appui a été avalé pour avaler leur relâchement, et un drapeau équivalent pour le bouton gauche. Après `Captured` ou `CaptureCancelled`, le mode saisie se désactive seul. En mode dessin (bascule), les clics ne sont pas avalés par le filtre : c'est la fenêtre de dessin qui les reçoit (tâche 10).

- [ ] **Step 4: Relancer** — Expected: PASS (12 tests).

- [ ] **Step 5: Commit** — `git commit -m "Filtre d'entrées clavier et souris"`

---

### Task 9: État de l'application

**Files:**
- Create: `src/app.rs`

**Interfaces:**
- Consumes: `Scene`, `Tool`, `Pointer`, `Config`, `InputEvent`, `Action`, `Shortcut`.
- Produces:
  - `pub enum UiCommand { SelectTool(Option<Tool>), SetColor(u8), SetWidth(u8), ToggleHalo, ToggleRipple, ToggleTrail, TogglePin, ClearAll, SetHoldSeconds(u8), SetHaloSize(u32), SetHaloFollowsColor(bool), SetBinding(Action, Shortcut), ResetBindings, SetHideFromCapture(bool), SetToolbarPos(i32, i32), SetToolbarCollapsed(bool) }`
  - `pub enum Effect { OverlayCapture(bool), FilterChanged, ConfigChanged, UiChanged, CaptureAffinityChanged, BindingConflict(Action) }`
  - `pub struct App { pub cfg: Config, pub scene: Scene, pub pointer: Pointer, pub tool: Tool, pub draw_mode: bool, pub hold: bool, pub shift: bool }`
  - `App::new(cfg: Config) -> App`
  - `pub fn input(&mut self, ev: InputEvent, now: u64, scale_at: &dyn Fn(Pt) -> f32) -> Vec<Effect>`
  - `pub fn command(&mut self, cmd: UiCommand, now: u64) -> Vec<Effect>`
  - `pub fn tick(&mut self, now: u64) -> bool` — vrai tant qu'une animation est en cours (`scene.tick || pointer.tick`)
  - `pub fn halo_color(&self) -> Rgba` — couleur courante si `halo_follows_color`, sinon ambre.

- [ ] **Step 1: Écrire les tests**

```rust
fn app() -> App { App::new(Config::default()) }
fn one(_: Pt) -> f32 { 1.0 }
#[test] fn toggle_enters_and_leaves_draw_mode() {
    let mut a = app(); let e = a.input(InputEvent::Action(Action::ToggleDraw), 0, &one);
    assert!(a.draw_mode && e.contains(&Effect::OverlayCapture(true)) && e.contains(&Effect::FilterChanged) && e.contains(&Effect::UiChanged));
    let e = a.input(InputEvent::Action(Action::ExitDraw), 1, &one); assert!(!a.draw_mode && e.contains(&Effect::OverlayCapture(false)));
}
#[test] fn toolbar_tool_enters_draw_mode_and_cursor_leaves() {
    let mut a = app(); a.command(UiCommand::SelectTool(Some(Tool::Rect)), 0);
    assert!(a.draw_mode && a.tool == Tool::Rect && a.cfg.last_tool == Tool::Rect);
    a.command(UiCommand::SelectTool(None), 1); assert!(!a.draw_mode && a.tool == Tool::Rect);
}
#[test] fn tool_and_color_keys() {
    let mut a = app(); a.input(InputEvent::Action(Action::ToggleDraw), 0, &one);
    a.input(InputEvent::Action(Action::ToolHighlighter), 1, &one); a.input(InputEvent::Action(Action::Color(3)), 2, &one);
    assert_eq!(a.tool, Tool::Highlighter); assert_eq!(a.cfg.color, 3);
}
#[test] fn stroke_uses_current_style_and_monitor_scale() {
    let mut a = app(); a.command(UiCommand::SetColor(4), 0); a.command(UiCommand::SetWidth(2), 0);
    a.input(InputEvent::HoldStart, 0, &one); assert!(a.hold && !a.draw_mode);
    a.input(InputEvent::StrokeBegin(Pt::new(0.,0.)), 1, &|_| 1.5); a.input(InputEvent::StrokeMove(Pt::new(200.,0.)), 2, &one); a.input(InputEvent::StrokeEnd, 3, &one);
    let s = &a.scene.shapes()[0]; assert_eq!((s.color, s.width, s.scale), (4, 8.0, 1.5)); assert!(matches!(s.kind, ShapeKind::Arrow { .. }));
}
#[test] fn hold_end_finishes_stroke() {
    let mut a = app(); a.input(InputEvent::HoldStart, 0, &one); a.input(InputEvent::StrokeBegin(Pt::new(0.,0.)), 1, &one);
    a.input(InputEvent::StrokeMove(Pt::new(100.,0.)), 2, &one); a.input(InputEvent::HoldEnd, 3, &one);
    assert!(!a.hold); let s = a.scene.shapes()[0].clone(); assert!(!matches!(a.scene.phase(&s, 3), Phase::Drawing));
}
#[test] fn shift_constrains_current_stroke() {
    let mut a = app(); a.input(InputEvent::Action(Action::ToggleDraw), 0, &one); a.input(InputEvent::Action(Action::ToolRect), 0, &one);
    a.input(InputEvent::Shift(true), 0, &one); a.input(InputEvent::StrokeBegin(Pt::new(0.,0.)), 1, &one);
    a.input(InputEvent::StrokeMove(Pt::new(40.,10.)), 2, &one); a.input(InputEvent::StrokeEnd, 3, &one);
    assert!(matches!(a.scene.shapes()[0].kind, ShapeKind::Rect { b, .. } if b == Pt::new(40.,40.)));
}
#[test] fn leaving_draw_mode_cancels_stroke_in_progress() {
    let mut a = app(); a.input(InputEvent::Action(Action::ToggleDraw), 0, &one); a.input(InputEvent::StrokeBegin(Pt::new(0.,0.)), 1, &one);
    a.input(InputEvent::StrokeMove(Pt::new(100.,0.)), 2, &one); a.input(InputEvent::Action(Action::ExitDraw), 3, &one);
    assert!(a.scene.shapes().is_empty());
}
#[test] fn settings_commands_persist() {
    let mut a = app(); let e = a.command(UiCommand::SetHoldSeconds(9), 0); assert!(e.contains(&Effect::ConfigChanged)); assert_eq!(a.cfg.hold_seconds, 9);
    a.command(UiCommand::SetHoldSeconds(99), 0); assert_eq!(a.cfg.hold_seconds, 15);
    a.command(UiCommand::TogglePin, 0); assert!(a.cfg.pinned && a.scene.pinned());
    let e = a.command(UiCommand::ToggleHalo, 0); assert!(a.cfg.halo_on && e.contains(&Effect::ConfigChanged));
    let e = a.input(InputEvent::Action(Action::ToggleHalo), 0, &one); assert!(!a.cfg.halo_on && e.contains(&Effect::UiChanged));
    let e = a.command(UiCommand::SetHideFromCapture(false), 0); assert!(e.contains(&Effect::CaptureAffinityChanged));
}
#[test] fn binding_conflict_is_reported_not_applied() {
    let mut a = app(); let e = a.command(UiCommand::SetBinding(Action::ToolRect, "F".parse().unwrap()), 0);
    assert!(e.contains(&Effect::BindingConflict(Action::ToolArrow))); assert_eq!(a.cfg.bindings.get(Action::ToolRect).to_string(), "R");
    let e = a.command(UiCommand::SetBinding(Action::ToolRect, "X".parse().unwrap()), 0); assert!(e.contains(&Effect::FilterChanged) && e.contains(&Effect::ConfigChanged));
}
#[test] fn pointer_events_feed_effects_only_when_enabled() {
    let mut a = app(); a.input(InputEvent::Click(Pt::new(1.,1.), Button::Left), 0, &one); assert!(a.pointer.ripples().is_empty());
    a.command(UiCommand::ToggleRipple, 0); a.input(InputEvent::Click(Pt::new(1.,1.), Button::Left), 1, &one); assert_eq!(a.pointer.ripples().len(), 1);
}
```

- [ ] **Step 2: Lancer** `cargo test app` — Expected: FAIL.

- [ ] **Step 3: Implémenter `App`**. `App::new` initialise `Scene` avec `hold_seconds * 1000`, `tool = cfg.last_tool`, `scene.set_pinned(cfg.pinned, 0)`. Toute commande qui change `cfg` émet `ConfigChanged`. L'action `Undo` appelle `scene.undo()`, `ClearAll` appelle `scene.clear_all(now)`.

- [ ] **Step 4: Relancer** `cargo test` — Expected: PASS (toute la suite).

- [ ] **Step 5: Commit** — `git commit -m "État de l'application et transitions de mode"`

---

### Task 10: Fenêtres de dessin et rendu des formes

**Files:**
- Create: `src/win/mod.rs`, `src/win/gfx.rs`, `src/win/overlay.rs`, `src/win/render.rs`, `src/win/log.rs`
- Modify: `src/main.rs`, `Cargo.toml` (fonctionnalités `windows`)

**Interfaces:**
- Consumes: `App`, `Scene`, `Phase`, `ShapeKind`, `arrow_outline`, `arrow_geom`, `catmull_rom_beziers`, `PALETTE`, `theme`.
- Produces:
  - `gfx::Gfx::new() -> windows::core::Result<Gfx>` : périphérique D3D11 (matériel, repli WARP), `ID2D1Device`, `IDWriteFactory`, `IDCompositionDevice`. `Gfx::recreate(&mut self)` après perte du périphérique.
  - `gfx::Surface::new(gfx: &Gfx, hwnd: HWND, w: u32, h: u32) -> Result<Surface>` : chaîne d'échange de composition (alpha prémultiplié) + cible DirectComposition + contexte Direct2D. `Surface::draw(&mut self, f: impl FnOnce(&ID2D1DeviceContext)) -> Result<()>` efface en transparent, appelle `f`, présente. `Surface::resize(w, h)`.
  - `overlay::Overlays::new(gfx: &Gfx) -> Result<Overlays>`, `rebuild(&mut self, gfx: &Gfx)`, `set_capture(&mut self, on: bool)`, `scale_at(&self, p: Pt) -> f32`, `render(&mut self, app: &App, now: u64)`, `rects(&self) -> Vec<RectF>`.
  - `render::draw_scene(ctx: &ID2D1DeviceContext, origin: Pt, app: &App, now: u64)` — `origin` est le coin haut-gauche de l'écran dans le bureau virtuel.
  - `log::error(msg: &str)` — ajoute une ligne datée à `ekspointer.log` à côté de l'exécutable, fichier tronqué au-delà de 1 Mo.
  - Arguments de ligne de commande : `--demo` (scène de démonstration épinglée, une forme de chaque outil dans chaque couleur) et `--capturable` (désactive le masquage au partage pour la session, sans toucher au fichier).

- [ ] **Step 1: Créer `Gfx` et `Surface`**

Fenêtre de dessin : classe `EkspointerOverlay`, styles `WS_POPUP`, étendus `WS_EX_NOREDIRECTIONBITMAP | WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_LAYERED | WS_EX_TRANSPARENT`. Une fenêtre par écran (`EnumDisplayMonitors`), couvrant le rectangle complet de l'écran, échelle lue par `GetDpiForMonitor`.

- [ ] **Step 2: Bascule de capture**

`set_capture(true)` retire `WS_EX_TRANSPARENT`, `set_capture(false)` le remet. En capture, la fenêtre répond à `WM_SETCURSOR` par une croix, à `WM_MOUSEACTIVATE` par `MA_NOACTIVATE`, et traduit `WM_LBUTTONDOWN` / `WM_MOUSEMOVE` (bouton tenu) / `WM_LBUTTONUP` en `InputEvent::StrokeBegin` / `StrokeMove` / `StrokeEnd` avec `SetCapture` pendant le tracé. Les coordonnées sont converties en pixels physiques du bureau virtuel.

- [ ] **Step 3: Rendu des formes** dans `render.rs`

Ordre de dessin de chaque forme, opacité globale = `scene.opacity` : ombre douce encre (`rgba(0,60,88,.35)`, décalage 2, flou 6), liseré (blanc à 85 %, encre pour la couleur blanche) plus large de 1,5 de chaque côté, puis la forme dans sa couleur. Toutes les longueurs sont multipliées par `shape.scale`.

- Flèche : polygone `arrow_outline` rempli, plus un contour à jointures rondes d'épaisseur `corner_radius` de la même couleur pour arrondir les angles. Pendant `Appearing(p)`, la tête passe de 60 % à 100 % de sa taille selon `theme::ease(p)`.
- Rectangle : contour, rayon 6. Ellipse : contour.
- Trait libre : `catmull_rom_beziers`, extrémités et jointures rondes.
- Surligneur : même tracé, largeur 22, opacité 35 %, extrémités carrées, ni ombre ni liseré.
- Numéro : disque de diamètre 32, chiffre Noto Sans Mono graisse 600 centré, blanc, ou encre sur jaune et blanc. Pendant `Appearing(p)`, échelle de 80 % à 100 %.

- [ ] **Step 4: Boucle principale** dans `main.rs`

Mutex nommé `Ekspointer.Instance` (instance unique : si déjà pris, sortir). Fenêtre cachée de classe `EkspointerMain` pour les messages. Boucle : tant que `app.tick(now)` est vrai ou qu'un tracé est en cours, vider les messages par `PeekMessage`, rendre, présenter avec synchronisation verticale ; sinon `GetMessage` bloquant, avec un minuteur armé sur `scene.next_deadline`. `WM_DISPLAYCHANGE` et `WM_DPICHANGED` : `scene.cancel()`, effacer les formes, `Overlays::rebuild`. Erreur `DXGI_ERROR_DEVICE_REMOVED` ou `D2DERR_RECREATE_TARGET` : `Gfx::recreate` puis `rebuild`, formes conservées. Crochet de panique : écrit dans le journal puis termine le processus.

- [ ] **Step 5: Vérifier visuellement**

Run: `cargo run -- --demo --capturable`
Expected: les six outils dans les six couleurs sont visibles sur chaque écran, par-dessus les autres fenêtres, qui restent cliquables. Faire une capture d'écran sur fond clair et une sur fond sombre et contrôler : flèches effilées à ailes arrondies, liseré lisible, pas de crénelage. Vérifier dans le gestionnaire des tâches : 0 % de processeur une fois la scène affichée.

- [ ] **Step 6: Commit** — `git commit -m "Fenêtres de dessin et rendu Direct2D des formes"`

---

### Task 11: Écoute clavier et souris, dessin réel

**Files:**
- Create: `src/win/input.rs`
- Modify: `src/main.rs`

**Interfaces:**
- Consumes: `Filter`, `Decision`, `MouseKind`, `InputEvent`, `App::input`, `Effect`, `Overlays`.
- Produces: `input::install(filter: Filter, notify: HWND) -> Result<Hooks>` (désinstalle à la destruction), `input::with_filter(f: impl FnOnce(&mut Filter))`, `input::drain() -> Vec<InputEvent>`, message `WM_APP_INPUT`.

- [ ] **Step 1: Installer les écoutes**

`WH_KEYBOARD_LL` et `WH_MOUSE_LL` sur le fil principal. Chaque fonction d'écoute : ignore les événements injectés par Ekspointer, appelle `Filter::on_key` ou `on_mouse`, pousse les événements dans une file locale au fil, poste `WM_APP_INPUT` à la fenêtre principale, renvoie 1 si `swallow`, sinon `CallNextHookEx`. Aucun autre travail dans l'écoute. Avant chaque événement clavier, et sur un minuteur de 250 ms tant que le maintien est actif, appeler `Filter::resync` avec l'état lu par `GetAsyncKeyState`.

Échec de `SetWindowsHookExW` : boîte de message « Ekspointer n'a pas pu démarrer l'écoute du clavier et de la souris. » puis sortie avec code 1.

- [ ] **Step 2: Appliquer les effets**

À `WM_APP_INPUT` : pour chaque événement, `app.input(ev, now, &|p| overlays.scale_at(p))`, puis appliquer : `OverlayCapture(on)` → `overlays.set_capture(on)` ; `FilterChanged` → `with_filter` (mode dessin, raccourcis) ; `ConfigChanged` → `config::save` (échec : garder en mémoire, notifier une seule fois, tâche 14) ; les autres effets sont traités à partir de la tâche 12. Demander un rendu après tout événement.

- [ ] **Step 3: Vérifier à la main**

Run: `cargo run -- --capturable`
Expected, à contrôler un par un :
- `Ctrl+Alt+D` : le curseur devient une croix, glisser trace une flèche ambre qui s'efface après 5 s. `Échap` rend la main.
- En mode dessin, `R`, `C`, `D`, `S`, `N` changent d'outil, `1` à `6` de couleur, `Ctrl+Z` annule, Maj contraint.
- `Ctrl+Alt` gauche tenu + glisser trace sans changer de mode ; la fenêtre active garde le focus pendant tout le tracé.
- Dans le Bloc-notes, `AltGr+0` saisit `@` et `AltGr+3` saisit `#` sans rien déclencher.
- `Ctrl+Alt+E` efface tout.
- Sur deux écrans d'échelles différentes : épaisseurs visuellement identiques.

- [ ] **Step 4: Commit** — `git commit -m "Écoute bas niveau et dessin en mode bascule et touche maintenue"`

---

### Task 12: Effets de pointeur à l'écran

**Files:**
- Modify: `src/win/render.rs`, `src/main.rs`

**Interfaces:**
- Consumes: `Pointer::{pos, ripples, ripple_state, trail}`, `App::halo_color`, `cfg.halo_on`, `cfg.halo_size`, `cfg.trail_on`.
- Produces: `render::draw_pointer(ctx: &ID2D1DeviceContext, origin: Pt, scale: f32, app: &App, now: u64)`, appelé après `draw_scene`.

- [ ] **Step 1: Dessiner**

- Halo : disque de diamètre `halo_size`, dégradé radial de `halo_color` à 35 % d'opacité au centre et sur 70 % du rayon, puis vers 0 % au bord.
- Onde : anneau de rayon et d'opacité donnés par `ripple_state`, épaisseur 3. Clic gauche : couleur courante. Clic droit : deux anneaux bleus `#0C659D`, le second au rayon multiplié par 0,6.
- Traînée : segments reliant les points de `trail`, épaisseur de 6 (récent) à 1 (ancien), opacité `1 - âge`, ambre `#F0631F`, doublée d'un trait plus large à 25 % d'opacité pour la lueur.

Seules les fenêtres dont l'écran contient le curseur ou une onde sont redessinées.

- [ ] **Step 2: Vérifier à la main** en activant temporairement les trois effets dans `config.json`

Run: `cargo run -- --capturable`
Expected: le halo suit le curseur sans retard visible et passe d'un écran à l'autre ; chaque clic gauche et droit produit son onde ; la traînée s'éteint en moins d'une seconde ; curseur immobile et effets éteints : 0 % de processeur.

- [ ] **Step 3: Commit** — `git commit -m "Halo, onde au clic et traînée laser"`

---

### Task 13: Barre d'outils

**Files:**
- Create: `src/win/panel.rs`, `src/ui/icons.rs`, `src/ui/widgets.rs`, `src/ui/toolbar.rs`
- Modify: `src/win/gfx.rs` (polices), `src/main.rs`, `src/lib.rs`
- Create: `assets/fonts/InstrumentSans.ttf`, `assets/fonts/NotoSansMono.ttf`, `assets/fonts/OFL.txt`

**Interfaces:**
- Consumes: `theme`, `UiCommand`, `App`, `Surface`, `Tool`.
- Produces:
  - `toolbar::Item` : `Logo`, `Tool(Option<Tool>)`, `Color`, `Width`, `Halo`, `Ripple`, `Trail`, `Pin`, `Clear`, `Settings`, `Separator`.
  - `toolbar::layout(collapsed: bool) -> Layout` avec `Layout { pub w: f32, pub h: f32, pub items: Vec<(Item, RectF)> }` et `Layout::hit(&self, p: Pt) -> Option<Item>` — pur, en pixels logiques.
  - `toolbar::tooltip(item: Item, b: &Bindings) -> Option<(String, Option<String>)>` — nom français et raccourci.
  - `panel::Panel::new(gfx, kind: PanelKind, size_dip: (f32, f32)) -> Result<Panel>` avec `PanelKind { Toolbar, Popover, Tooltip, Settings }` ; méthodes `show_at(x, y)`, `hide()`, `rect() -> RectF`, `set_capture_hidden(bool)`, `draw(...)`. Marge transparente de 48 autour du contenu pour l'ombre.
  - `icons::draw(ctx, icon: Icon, rect: RectF, color: Rgba, stroke: f32)` et `icons::draw_logo(ctx, rect: RectF, ember_ring: bool)`.
  - `gfx::Fonts { body: IDWriteTextFormat.., mono: .. }` chargées depuis les octets embarqués.

- [ ] **Step 1: Écrire les tests de disposition**

```rust
#[test] fn expanded_layout() {
    let l = layout(false); assert_eq!(l.h, 56.0);
    let tools: Vec<_> = l.items.iter().filter_map(|(i, _)| if let Item::Tool(t) = i { Some(*t) } else { None }).collect();
    assert_eq!(tools, vec![None, Some(Tool::Arrow), Some(Tool::Rect), Some(Tool::Ellipse), Some(Tool::Free), Some(Tool::Highlighter), Some(Tool::Number)]);
    let order: Vec<_> = l.items.iter().map(|(i, _)| *i).filter(|i| !matches!(i, Item::Tool(_) | Item::Separator)).collect();
    assert_eq!(order, vec![Item::Logo, Item::Color, Item::Width, Item::Halo, Item::Ripple, Item::Trail, Item::Pin, Item::Clear, Item::Settings]);
    assert_eq!(l.items.iter().filter(|(i, _)| *i == Item::Separator).count(), 4);
    for (i, r) in &l.items { if !matches!(i, Item::Separator | Item::Logo) { assert_eq!((r.w, r.h), (40.0, 40.0)); assert_eq!(r.y, 8.0); } }
    for w in l.items.windows(2) { assert!(w[0].1.x + w[0].1.w <= w[1].1.x); }
    assert!(l.items.last().unwrap().1.x + 40.0 <= l.w);
}
#[test] fn collapsed_is_a_56_disc() { let l = layout(true); assert_eq!((l.w, l.h), (56.0, 56.0)); assert_eq!(l.items.len(), 1); assert_eq!(l.items[0].0, Item::Logo); }
#[test] fn hit_test() { let l = layout(false); let (_, r) = l.items.iter().find(|(i, _)| *i == Item::Clear).unwrap(); assert_eq!(l.hit(Pt::new(r.x + 20.0, r.y + 20.0)), Some(Item::Clear)); assert_eq!(l.hit(Pt::new(-5.0, 10.0)), None); }
#[test] fn tooltips_show_french_name_and_shortcut() {
    let b = Bindings::defaults();
    assert_eq!(tooltip(Item::Tool(Some(Tool::Arrow)), &b), Some(("Flèche".into(), Some("F".into()))));
    assert_eq!(tooltip(Item::Clear, &b), Some(("Tout effacer".into(), Some("Ctrl+Alt+E".into()))));
    assert_eq!(tooltip(Item::Pin, &b), Some(("Épingler".into(), None)));
    assert_eq!(tooltip(Item::Separator, &b), None);
}
```

Libellés des autres éléments : Curseur, Rectangle, Ellipse, Trait libre, Surligneur, Numéros, Couleur, Épaisseur, Halo, Onde au clic, Traînée laser, Réglages.

- [ ] **Step 2: Lancer, constater l'échec, implémenter `layout`, `hit`, `tooltip`, relancer** — Run: `cargo test toolbar` — Expected: PASS (4 tests).

- [ ] **Step 3: Polices**

Télécharger Instrument Sans et Noto Sans Mono depuis le dépôt `google/fonts` (licence OFL), après accord de l'utilisateur pour le téléchargement. Si Python et `fonttools` sont disponibles, réduire au latin (`pyftsubset --unicodes=U+0020-017F,U+2018-2026,U+20AC`) ; sinon embarquer les fichiers entiers et le signaler comme écart à la spec dans le compte rendu. `include_bytes!` puis chargement en mémoire par DirectWrite (`CreateInMemoryFontFileLoader`, `IDWriteFontSetBuilder`). Repli : Segoe UI et Consolas.

- [ ] **Step 4: `Panel`, icônes, dessin de la barre**

`Panel` : fenêtre `WS_POPUP`, `WS_EX_NOREDIRECTIONBITMAP | WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE`, placée au-dessus des fenêtres de dessin. `set_capture_hidden(true)` appelle `SetWindowDisplayAffinity(WDA_EXCLUDEFROMCAPTURE)`, `false` appelle `WDA_NONE`.

Icônes : tracés sur une grille de 24, trait 1,75, extrémités rondes, une par `Item`. Le logo suit `assets/logo.svg` (tâche 15 ; d'ici là, flèche-pointeur provisoire).

Dessin, selon la spec 4.2 et 4.3 : fond `BG`, bord `LINE`, ombre `0 20px 44px -20px rgba(0,60,88,.45)` ; survol : fond `HOVER` ; outil actif : pastille `INK`, icône blanche, qui glisse en `T_PILL_MS` avec `theme::ease` ; effet activé et épingle activée : icône `BLUE` sur fond `HOVER` ; pastille de couleur : disque de la couleur courante cerclé de `LINE` ; mode dessin ou maintien actif : anneau `EMBER` autour du logo ; halo radial `HOVER` suivant le curseur sur la barre. Si `SPI_GETCLIENTAREAANIMATION` est faux, aucune transition (appliquer la même règle aux phases `Appearing` et aux fondus de `render.rs` : états finaux directs).

- [ ] **Step 5: Interactions**

- Clic sur un élément → `UiCommand` correspondant. Logo : glisser déplace la barre (`SetToolbarPos` au relâchement), double-clic replie ou déplie.
- Couleur et Épaisseur ouvrent un volet (`PanelKind::Popover`, rayon 14) sous la barre : six pastilles de couleur, ou trois traits d'épaisseur 3, 5, 8. Un clic choisit et ferme ; un clic ailleurs ferme.
- Survol de 500 ms : bulle (`PanelKind::Tooltip`) avec le nom en Instrument Sans et le raccourci en Noto Sans Mono `MUTED`.
- Position initiale : `cfg.toolbar_pos` si elle tombe dans un écran existant, sinon haut centre de l'écran principal, à 24 du bord.
- Effets : `UiChanged` → redessiner la barre ; `CaptureAffinityChanged` → `set_capture_hidden` sur tous les panneaux ; à chaque déplacement ou ouverture, `Filter::set_exempt_rects` reçoit les rectangles de tous les panneaux visibles.

- [ ] **Step 6: Vérifier visuellement**

Run: `cargo run -- --capturable`
Expected: capture d'écran de la barre sur fond clair et sur fond sombre, conforme à la spec 4.1 et 4.2 ; chaque bouton agit ; la pastille active glisse ; les volets s'ouvrent et se ferment ; la barre reste cliquable en mode dessin et pendant le maintien ; repli et dépli ; position conservée après relance. Relancer sans `--capturable` et vérifier qu'une capture d'écran ne montre plus la barre alors que les formes y figurent.

- [ ] **Step 7: Commit** — `git commit -m "Barre d'outils flottante"`

---

### Task 14: Réglages, zone de notification, lancement au démarrage

**Files:**
- Create: `src/ui/settings.rs`, `src/win/tray.rs`, `src/win/autostart.rs`
- Modify: `src/ui/widgets.rs`, `src/main.rs`

**Interfaces:**
- Consumes: `Panel`, `UiCommand`, `Filter::set_capturing`, `InputEvent::{Captured, CaptureCancelled}`, `Effect::BindingConflict`, `Action::label`.
- Produces:
  - `settings::Row` : `HoldSeconds`, `Width`, `HaloSize`, `HaloFollowsColor`, `Binding(Action)`, `ResetBindings`, `Autostart`, `HideFromCapture`, `Header(&'static str)`.
  - `settings::rows() -> Vec<Row>` et `settings::layout(scroll: f32) -> Vec<(Row, RectF)>` — purs.
  - `settings::conflict_message(wanted: Shortcut, with: Action) -> String`.
  - `autostart::is_enabled() -> bool`, `autostart::set_enabled(on: bool) -> std::io::Result<()>`.
  - `tray::Tray::new(hwnd: HWND) -> Result<Tray>`, `tray::Tray::notify(&self, title: &str, text: &str)`, message `WM_APP_TRAY`.
  - Argument `--quit` : demande à l'instance en cours de se fermer, puis sort.

- [ ] **Step 1: Écrire les tests**

```rust
#[test] fn rows_follow_spec_sections() {
    let r = rows();
    let headers: Vec<_> = r.iter().filter_map(|x| if let Row::Header(h) = x { Some(*h) } else { None }).collect();
    assert_eq!(headers, vec!["Dessin", "Pointeur", "Raccourcis", "Général"]);
    assert_eq!(r.iter().filter(|x| matches!(x, Row::Binding(_))).count(), 18);     // 4 globaux + 6 outils + 6 couleurs + annuler + quitter
    assert!(r.contains(&Row::Autostart) && r.contains(&Row::HideFromCapture) && r.contains(&Row::ResetBindings));
}
#[test] fn conflict_message_names_the_action() {
    assert_eq!(conflict_message("F".parse().unwrap(), Action::ToolArrow), "« F » est déjà utilisé par « Flèche ».");
}
```

- [ ] **Step 2: Lancer, constater l'échec, implémenter les fonctions pures, relancer** — Run: `cargo test settings` — Expected: PASS (2 tests).

- [ ] **Step 3: Panneau**

`PanelKind::Settings`, 420 × 560, rayon 18, titre « Réglages » en Instrument Sans graisse 700, bouton de fermeture, défilement à la molette. Widgets dans `widgets.rs` : curseur (durée 2 à 15 s par pas de 1, taille du halo 40 à 120), choix à trois positions (épaisseur), interrupteur en pastille, ligne de raccourci.

Ligne de raccourci : libellé `Action::label`, valeur en Noto Sans Mono dans une pastille. Clic : la pastille affiche « Tapez le raccourci… », `Filter::set_capturing(true)`. `Captured(s)` → `UiCommand::SetBinding`. `BindingConflict(a)` → `conflict_message` affiché sous la ligne en `MUTED` pendant 4 s, valeur inchangée. `CaptureCancelled` → retour à la valeur. Bouton « Rétablir les raccourcis par défaut » → `ResetBindings`.

Chaque changement est appliqué et enregistré tout de suite. Interrupteur « Lancer au démarrage de Windows » : lit `autostart::is_enabled`, écrit `set_enabled`.

- [ ] **Step 4: Lancement au démarrage**

Valeur `Ekspointer` sous `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`, contenu : chemin de l'exécutable entre guillemets. `set_enabled(false)` supprime la valeur.

- [ ] **Step 5: Zone de notification et messages**

Icône avec infobulle « Ekspointer ». Clic gauche : affiche la barre. Clic droit : menu « Afficher la barre » ou « Masquer la barre », « Réglages », « Quitter ». Recréer l'icône sur le message `TaskbarCreated`.

Notifications (`Tray::notify`) : réglages invalides au démarrage (« Le fichier de réglages était illisible. Il a été renommé en config.invalide.json et les réglages par défaut sont utilisés. ») ; dossier non inscriptible, une seule fois par session (« Les réglages ne peuvent pas être enregistrés dans ce dossier. Ils seront perdus à la fermeture. »).

Second lancement : trouve la fenêtre `EkspointerMain` et lui demande d'afficher la barre. `--quit` lui envoie `WM_CLOSE`. À la fermeture : retirer l'icône, désinstaller les écoutes.

- [ ] **Step 6: Vérifier à la main**

Run: `cargo run -- --capturable`
Expected: le panneau s'ouvre depuis la barre et la zone de notification ; changer la durée à 2 s raccourcit l'effacement immédiatement ; affecter `F` au rectangle affiche « « F » est déjà utilisé par « Flèche ». » ; affecter `Ctrl+Alt+X` à la bascule fonctionne aussitôt et survit à une relance ; l'interrupteur de démarrage crée puis supprime la valeur de registre (`reg query HKCU\Software\Microsoft\Windows\CurrentVersion\Run /v Ekspointer`) ; `ekspointer.exe --quit` ferme l'instance ; écrire `{` dans `config.json` puis relancer produit la notification et le fichier `config.invalide.json`.

- [ ] **Step 7: Commit** — `git commit -m "Panneau de réglages, zone de notification et lancement au démarrage"`

---

### Task 15: Logo et icône

**Files:**
- Create: `assets/logo.svg`, `assets/logo-small.svg`, `assets/ekspointer.ico`, `assets/wizard-small.bmp`, `assets/wizard-large.bmp`, `tools/icon-gen/Cargo.toml`, `tools/icon-gen/src/main.rs`, `assets/ekspointer.rc`
- Modify: `build.rs`, `src/ui/icons.rs`, `src/win/tray.rs`

**Interfaces:**
- Produces: icône de l'exécutable (ressource d'identifiant 1), `icons::draw_logo` définitif.

- [ ] **Step 1: Dessiner `assets/logo.svg`** (zone 256 × 256, encre `#003C58`)

Reprendre du logo Eksplodo (`Code-site-web-eksplodo.fr/assets/Logo Eksplodo Bleu.svg`, lecture seule) l'écran à coins arrondis avec son pied et les nuages de décollage en bas de l'écran. Remplacer la fusée par une flèche-pointeur (forme de curseur de souris) orientée vers le haut à droite, qui sort du haut de l'écran, avec un point ambre `#F0631F` à sa base. `assets/logo-small.svg` : la flèche-pointeur et le point ambre seuls, pour 16 et 24 px.

- [ ] **Step 2: Outil `icon-gen`**

Crate séparée de l'espace de travail, dépendances `resvg` et `ico`. Produit `assets/ekspointer.ico` (16 et 24 depuis `logo-small.svg` ; 32, 48, 64, 256 depuis `logo.svg`), `assets/wizard-small.bmp` (55 × 58, logo sur fond `#FCFDFE`) et `assets/wizard-large.bmp` (164 × 314, fond `#E3F1F7`, logo centré dans le tiers haut).

Run: `cargo run -p icon-gen` — Expected: les trois fichiers existent ; ouvrir le `.ico` et vérifier la lisibilité en 16 px.

- [ ] **Step 3: Intégrer**

`assets/ekspointer.rc` référence l'icône et le manifeste ; `build.rs` compile ce fichier. `icons::draw_logo` reprend les tracés de `logo.svg`. La zone de notification charge l'icône de ressource.

- [ ] **Step 4: Vérifier**

Run: `cargo build --release` puis capture de l'Explorateur sur `target/release` et de la barre des tâches.
Expected: l'icône apparaît sur l'exécutable, dans la zone de notification et dans la barre d'outils.

- [ ] **Step 5: Commit** — `git commit -m "Logo et icône Ekspointer"`

---

### Task 16: Programme d'installation

**Files:**
- Create: `installer/ekspointer.iss`, `scripts/build-release.ps1`, `README.md`

**Interfaces:**
- Consumes: `target/release/ekspointer.exe`, argument `--quit`, mutex `Ekspointer.Instance`, `assets/ekspointer.ico`, images de l'assistant.
- Produces: `dist/Ekspointer-Setup-0.1.0.exe`, `dist/Ekspointer-0.1.0-portable.exe`.

- [ ] **Step 1: Installer Inno Setup** après accord de l'utilisateur

```powershell
winget install JRSoftware.InnoSetup
```

- [ ] **Step 2: Écrire `installer/ekspointer.iss`**

Valeurs imposées :

```ini
[Setup]
AppId={{D98EDD00-716E-497D-A73D-636807B092EF}
AppName=Ekspointer
AppVersion={#AppVersion}
AppPublisher=Eksplodo
AppPublisherURL=https://eksplodo.fr/
DefaultDirName={localappdata}\Programs\Ekspointer
PrivilegesRequired=lowest
DisableProgramGroupPage=yes
DisableDirPage=yes
AppMutex=Ekspointer.Instance
CloseApplications=yes
SetupIconFile=..\assets\ekspointer.ico
WizardSmallImageFile=..\assets\wizard-small.bmp
WizardImageFile=..\assets\wizard-large.bmp
UninstallDisplayIcon={app}\ekspointer.exe
OutputDir=..\dist
OutputBaseFilename=Ekspointer-Setup-{#AppVersion}
Compression=lzma2
SolidCompression=yes
WizardStyle=modern

[Languages]
Name: "french"; MessagesFile: "compiler:Languages\French.isl"

[Tasks]
Name: "desktopicon"; Description: "Créer un raccourci sur le Bureau"; Flags: unchecked
Name: "autostart"; Description: "Lancer Ekspointer au démarrage de Windows"; Flags: unchecked

[Files]
Source: "..\target\release\ekspointer.exe"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{userprograms}\Ekspointer"; Filename: "{app}\ekspointer.exe"
Name: "{userdesktop}\Ekspointer"; Filename: "{app}\ekspointer.exe"; Tasks: desktopicon

[Registry]
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueType: string; ValueName: "Ekspointer"; ValueData: """{app}\ekspointer.exe"""; Tasks: autostart; Flags: uninsdeletevalue

[Run]
Filename: "{app}\ekspointer.exe"; Description: "Lancer Ekspointer"; Flags: nowait postinstall skipifsilent
```

L'`AppId` ne doit plus jamais changer : c'est lui qui permet les mises à jour.

Section `[Code]` : avant l'installation (`PrepareToInstall`) et avant la désinstallation (`InitializeUninstall`), si `{app}\ekspointer.exe` existe, l'exécuter avec `--quit` et attendre jusqu'à 3 s la disparition du mutex. À `CurUninstallStepChanged(usPostUninstall)` : `MsgBox` « Voulez-vous aussi supprimer vos réglages Ekspointer ? » (Oui / Non, Non par défaut) ; sur Oui, supprimer `config.json`, `config.invalide.json`, `ekspointer.log` puis le dossier s'il est vide. La désinstallation supprime aussi la valeur `Run` même si elle a été créée depuis les réglages de l'application.

- [ ] **Step 3: Écrire `scripts/build-release.ps1`**

Étapes, arrêt à la première erreur : lire la version dans `Cargo.toml` ; `cargo test` ; `cargo build --release` ; vérifier que l'exécutable pèse moins de 6 Mo (sinon erreur explicite) ; copier vers `dist/Ekspointer-<version>-portable.exe` ; `ISCC.exe /DAppVersion=<version> installer\ekspointer.iss`.

- [ ] **Step 4: Vérifier**

Run: `powershell -ExecutionPolicy Bypass -File scripts\build-release.ps1`
Expected: les deux fichiers dans `dist/`. Puis, à la main :
- Installer : aucune demande de droits administrateur, assistant en français, fichiers dans `%LOCALAPPDATA%\Programs\Ekspointer`, raccourci du menu Démarrer, application lancée à la fin.
- Changer un réglage, relancer l'installateur avec l'application ouverte : elle se ferme, les fichiers sont remplacés, le réglage est conservé.
- Désinstaller depuis « Applications installées » : répondre Non garde `config.json`, répondre Oui supprime le dossier ; la valeur `Run` a disparu dans les deux cas.

- [ ] **Step 5: Écrire `README.md`** : à quoi sert Ekspointer, installation, raccourcis par défaut, limite du partage de fenêtre, avertissement SmartScreen, commandes de compilation.

- [ ] **Step 6: Commit** — `git commit -m "Programme d'installation et script de publication"`

---

### Task 17: Recette finale

**Files:**
- Modify: selon les défauts trouvés

- [ ] **Step 1: Dérouler la vérification manuelle de la spec, section 9**, avec la version installée, et noter le résultat de chaque ligne : outils, couleurs et épaisseurs sur fond clair et sombre ; trois effets de pointeur ; bascule et touche maintenue ; AltGr ; deux écrans d'échelles différentes ; raccourci modifié et conflit refusé ; installation, mise à jour, désinstallation.

- [ ] **Step 2: Mesurer**

- Taille : `(Get-Item dist\Ekspointer-0.1.0-portable.exe).Length / 1MB` — Expected: moins de 6.
- Mémoire et processeur au repos, application lancée depuis 1 minute sans activité : `Get-Process ekspointer | Select WorkingSet64, CPU` deux fois à 30 s d'écart — Expected: moins de 50 Mo, temps processeur inchangé entre les deux mesures.
- Démarrage : `Measure-Command { Start-Process ekspointer.exe; while (-not (Get-Process ekspointer -ErrorAction SilentlyContinue).MainWindowHandle) {} }` ou, à défaut, chronométrage de l'apparition de la barre — Expected: moins de 300 ms.

- [ ] **Step 3: Partage d'écran** — demander à l'utilisateur de vérifier dans Google Meet, en partage d'écran entier, que les annotations et les effets sont visibles et que la barre ne l'est pas. Cette vérification demande un second participant : elle revient à l'utilisateur.

- [ ] **Step 4: Corriger les écarts**, relancer `cargo test` et `scripts\build-release.ps1`.

- [ ] **Step 5: Commit** — `git commit -m "Recette finale et corrections"`

---

### Task 18: Tests d'intégration de la logique

À exécuter juste après la tâche 9. Les tests unitaires vivent dans chaque module (`#[cfg(test)]`) ; les tests d'intégration vivent dans `tests/` et n'utilisent que l'interface publique de la crate, en enchaînant plusieurs modules comme le fera l'application.

**Files:**
- Create: `tests/common/mod.rs`, `tests/drawing_flow.rs`, `tests/hold_flow.rs`, `tests/settings_flow.rs`, `tests/config_persistence.rs`

**Interfaces:**
- Consumes: `Filter`, `App`, `Effect`, `InputEvent`, `UiCommand`, `config::{load, save}`, `Scene`, `Phase`.
- Produces: `common::Rig { pub filter: Filter, pub app: App, pub now: u64 }` avec `Rig::new()`, `key(&mut self, vk: u32, down: bool) -> bool` (renvoie `swallow`), `mouse(&mut self, kind: MouseKind, x: f32, y: f32) -> bool`, `command(&mut self, cmd: UiCommand)`, `advance(&mut self, ms: u64)` (avance le temps et appelle `app.tick`). `Rig` fait ce que fera `main.rs` : il passe chaque événement du filtre à `App::input` et applique `Effect::FilterChanged` au filtre (`set_draw_mode`, `set_bindings`).

- [ ] **Step 1: Écrire `tests/common/mod.rs` et les tests**

`drawing_flow.rs` :
- `toggle_draw_arrow_then_fade` : `Ctrl+Alt+D` (touches 0xA2, 0xA4, 0x44 puis relâchements) ; `StrokeBegin`/`StrokeMove`/`StrokeEnd` injectés comme le ferait la fenêtre de dessin de (100,100) à (400,100) ; une forme `Arrow` existe ; `advance(5000)` → phase `Fading` ; `advance(700)` → scène vide.
- `keys_change_tool_and_color_only_in_draw_mode` : hors mode dessin, `R` n'est pas avalé et l'outil reste `Arrow` ; en mode dessin, `R` puis `4` sont avalés, l'outil vaut `Rect` et la couleur 3.
- `escape_returns_to_normal_and_letters_pass_again` : après `Échap`, `F` n'est plus avalé.
- `undo_and_clear_all_shortcuts` : trois formes, `Ctrl+Z` en laisse deux, `Ctrl+Alt+E` puis `advance(250)` vide la scène.

`hold_flow.rs` :
- `hold_draws_without_entering_draw_mode` : Ctrl et Alt gauches tenus, `LeftDown` avalé, déplacement non avalé, `LeftUp` avalé, relâchement des touches ; `draw_mode` est resté faux et une forme existe.
- `altgr_typing_never_draws` : 0xA2 puis 0xA5 tenus, `LeftDown` non avalé, scène vide.
- `hold_uses_last_tool_chosen_in_toolbar` : `SelectTool(Some(Ellipse))`, `SelectTool(None)`, puis maintien et glisser → forme `Ellipse`.
- `stuck_modifier_is_recovered` : maintien, `LeftDown`, puis `filter.resync(KeyState::default())` passé à l'application ; la forme est terminée, `hold` est faux, le clic suivant n'est pas avalé.

`settings_flow.rs` :
- `captured_shortcut_becomes_active` : `set_capturing(true)`, saisie de `Ctrl+Alt+X`, l'événement `Captured` est converti en `SetBinding(ToggleDraw, ..)` ; ensuite `Ctrl+Alt+X` bascule le mode dessin et `Ctrl+Alt+D` ne fait plus rien.
- `conflicting_capture_keeps_old_binding` : saisie de `F` pour le rectangle → `BindingConflict(ToolArrow)`, `R` sélectionne toujours le rectangle.
- `hold_duration_change_applies_to_next_shape` : `SetHoldSeconds(2)`, une forme, `advance(2100)` → `Fading`.
- `pin_keeps_shapes_until_unpinned` : `TogglePin`, une forme, `advance(60_000)` → toujours là ; `TogglePin`, `advance(5700)` → scène vide.

`config_persistence.rs` (dossier temporaire dédié par test) :
- `state_survives_restart` : commandes `SetColor(3)`, `SetWidth(2)`, `ToggleHalo`, `SetHaloSize(90)`, `SelectTool(Some(Highlighter))`, `SetBinding(ToggleDraw, Ctrl+Alt+X)`, `SetToolbarPos(300, 40)` ; `save` ; `load` ; `App::new` sur la configuration relue retrouve couleur, épaisseur, halo, taille, dernier outil, raccourci et position.
- `corrupt_file_recovers_to_working_app` : fichier illisible → `LoadOutcome::Invalid`, sauvegarde `config.invalide.json` présente, et l'application construite sur le résultat accepte `Ctrl+Alt+D`.
- `save_is_atomic` : après `save`, aucun fichier `.tmp` ne reste et le fichier relu est du JSON valide.

- [ ] **Step 2: Lancer** `cargo test --tests` — Expected: les tests compilent et passent si les tâches 3 à 9 sont correctes. Un échec ici désigne un défaut d'assemblage entre modules : le corriger dans le module concerné, avec d'abord un test unitaire qui le reproduit.

- [ ] **Step 3: Vérifier que les tests détectent une régression** : inverser temporairement `ctrl = lctrl && !ralt` en `ctrl = lctrl` dans `shortcuts.rs`, constater que `altgr_typing_never_draws` échoue, puis rétablir.

- [ ] **Step 4: Commit** — `git commit -m "Tests d'intégration de la logique"`

---

### Task 19: Tests d'intégration de l'exécutable

À exécuter juste après la tâche 14. Ces tests lancent le vrai `ekspointer.exe` (`env!("CARGO_BIN_EXE_ekspointer")`) copié dans un dossier temporaire, pour ne pas toucher aux réglages du développeur. Ils ouvrent de vraies fenêtres : ils sont marqués `#[ignore]` et lancés explicitement, un à la fois.

**Files:**
- Create: `tests/process.rs`
- Modify: `scripts/build-release.ps1` (tâche 16 : ajouter l'appel ci-dessous après `cargo test`)

**Interfaces:**
- Consumes: arguments `--capturable` et `--quit`, mutex `Ekspointer.Instance`, classe de fenêtre `EkspointerMain`, emplacement de `config.json`.

- [ ] **Step 1: Écrire les tests**

- `starts_and_quits_cleanly` : lance l'exécutable avec `--capturable`, attend jusqu'à 5 s que le processus soit en vie et que `--quit` (second lancement) renvoie le code 0 ; le premier processus se termine en moins de 3 s avec le code 0.
- `second_instance_exits_immediately` : avec une instance en cours, un second lancement sans argument se termine en moins de 2 s avec le code 0, et la première instance est toujours en vie.
- `writes_config_next_to_executable_on_quit` : le dossier temporaire ne contient pas de `config.json` au départ ; après lancement puis `--quit`, soit le fichier n'existe pas (aucun changement), soit il est du JSON valide ; aucun fichier n'a été créé hors de ce dossier (vérifier l'absence de `%APPDATA%\Ekspointer`).
- `invalid_config_is_backed_up_at_startup` : `config.json` contenant `{` est déposé avant le lancement ; après démarrage puis `--quit`, `config.invalide.json` existe.
- `quit_without_running_instance_is_a_noop` : `--quit` seul se termine avec le code 0 en moins de 2 s.

Chaque test tue le processus en fin de test s'il vit encore.

- [ ] **Step 2: Lancer**

Run: `cargo test --test process -- --ignored --test-threads=1`
Expected: PASS (5 tests).

- [ ] **Step 3: Commit** — `git commit -m "Tests d'intégration de l'exécutable"`
