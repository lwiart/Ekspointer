//! Formes de dessin : géométrie et cycle de vie.
pub mod arrow;
pub mod stroke;

use crate::geom::Pt;

use serde::{Deserialize, Serialize};

/// Durée de la finition d'une forme après le relâchement.
pub const APPEAR_MS: u64 = 150;
/// Durée du fondu de disparition.
pub const FADE_MS: u64 = 600;
/// Durée du fondu de « Tout effacer ».
pub const CLEAR_MS: u64 = 200;
/// Taille minimale d'une flèche, d'un rectangle ou d'une ellipse, en pixels logiques.
pub const MIN_SIZE: f32 = 12.0;

/// Outil de dessin.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Tool {
    Arrow,
    Rect,
    Ellipse,
    Free,
    Highlighter,
    Number,
}

/// Géométrie d'une forme, en pixels physiques du bureau virtuel.
#[derive(Clone, PartialEq, Debug)]
pub enum ShapeKind {
    Arrow { from: Pt, to: Pt },
    Rect { a: Pt, b: Pt },
    Ellipse { a: Pt, b: Pt },
    Free { pts: Vec<Pt> },
    Highlighter { pts: Vec<Pt> },
    Number { at: Pt, n: u32 },
}

/// Une forme tracée.
#[derive(Clone, PartialEq, Debug)]
pub struct Shape {
    pub id: u64,
    pub kind: ShapeKind,
    /// Indice dans `geom::PALETTE`.
    pub color: u8,
    /// Épaisseur en pixels logiques.
    pub width: f32,
    /// Échelle de l'écran où la forme a commencé.
    pub scale: f32,
    /// Instant du relâchement ; `None` tant que la forme est en cours de tracé.
    pub released_at: Option<u64>,
    /// Instant de départ de la durée de maintien.
    hold_from: u64,
    /// Instant de départ d'un effacement demandé par « Tout effacer ».
    clearing_from: Option<u64>,
    /// Point de départ du tracé, pour les contraintes de la touche Maj.
    origin: Pt,
}

/// Étape du cycle de vie d'une forme ; le nombre est la progression de 0 à 1.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Phase {
    Drawing,
    Appearing(f32),
    Holding,
    Fading(f32),
    Gone,
}

/// Ensemble des formes à l'écran et règles de leur cycle de vie.
#[derive(Debug)]
pub struct Scene {
    shapes: Vec<Shape>,
    pinned: bool,
    hold_ms: u64,
    next_id: u64,
}

impl Scene {
    pub fn new(hold_ms: u64) -> Scene {
        Scene { shapes: Vec::new(), pinned: false, hold_ms, next_id: 1 }
    }

    pub fn set_hold_ms(&mut self, ms: u64) {
        self.hold_ms = ms;
    }

    pub fn shapes(&self) -> &[Shape] {
        &self.shapes
    }

    pub fn pinned(&self) -> bool {
        self.pinned
    }

    /// Vrai si une forme est en cours de tracé.
    pub fn is_drawing(&self) -> bool {
        self.shapes.last().is_some_and(|s| s.released_at.is_none())
    }

    /// Commence une forme. Une forme encore en cours est d'abord terminée.
    pub fn begin(&mut self, tool: Tool, at: Pt, color: u8, width: f32, scale: f32, now: u64) {
        if self.is_drawing() {
            self.release(now);
        }
        let kind = match tool {
            Tool::Arrow => ShapeKind::Arrow { from: at, to: at },
            Tool::Rect => ShapeKind::Rect { a: at, b: at },
            Tool::Ellipse => ShapeKind::Ellipse { a: at, b: at },
            Tool::Free => ShapeKind::Free { pts: vec![at] },
            Tool::Highlighter => ShapeKind::Highlighter { pts: vec![at] },
            Tool::Number => ShapeKind::Number { at, n: self.next_number() },
        };
        let id = self.next_id;
        self.next_id += 1;
        self.shapes.push(Shape {
            id,
            kind,
            color,
            width,
            scale: if scale > 0.0 { scale } else { 1.0 },
            released_at: None,
            hold_from: now,
            clearing_from: None,
            origin: at,
        });
    }

    /// Numéro de la prochaine pastille : un de plus que la plus grande encore affichée.
    fn next_number(&self) -> u32 {
        self.shapes
            .iter()
            .filter(|s| s.clearing_from.is_none())
            .filter_map(|s| match s.kind {
                ShapeKind::Number { n, .. } => Some(n),
                _ => None,
            })
            .max()
            .unwrap_or(0)
            + 1
    }

    /// Prolonge la forme en cours jusqu'à `to`. `shift` applique la contrainte de l'outil.
    pub fn drag(&mut self, to: Pt, shift: bool) {
        let Some(s) = self.shapes.last_mut().filter(|s| s.released_at.is_none()) else {
            return;
        };
        let origin = s.origin;
        let min_dist = stroke::MIN_POINT_DIST * s.scale;
        match &mut s.kind {
            ShapeKind::Arrow { to: end, .. } => {
                *end = if shift { arrow::snap_angle(origin, to, 15.0) } else { to };
            }
            ShapeKind::Rect { b, .. } | ShapeKind::Ellipse { b, .. } => {
                *b = if shift { stroke::constrain_square(origin, to) } else { to };
            }
            ShapeKind::Free { pts } => {
                stroke::push_point(pts, to, min_dist);
            }
            ShapeKind::Highlighter { pts } => {
                if shift {
                    pts.clear();
                    pts.push(origin);
                    pts.push(stroke::constrain_axis(origin, to));
                } else {
                    stroke::push_point(pts, to, min_dist);
                }
            }
            ShapeKind::Number { at, .. } => *at = to,
        }
    }

    /// Termine la forme en cours. Une forme trop petite est abandonnée.
    pub fn release(&mut self, now: u64) {
        let Some(s) = self.shapes.last_mut().filter(|s| s.released_at.is_none()) else {
            return;
        };
        let too_small = match &s.kind {
            ShapeKind::Arrow { from: a, to: b } | ShapeKind::Rect { a, b } | ShapeKind::Ellipse { a, b } => {
                a.dist(*b) / s.scale < MIN_SIZE
            }
            // Un surligneur d'un seul point ne se dessine pas.
            ShapeKind::Highlighter { pts } => pts.len() < 2,
            _ => false,
        };
        if too_small {
            self.shapes.pop();
            return;
        }
        s.released_at = Some(now);
        s.hold_from = now;
    }

    /// Abandonne la forme en cours de tracé, s'il y en a une.
    pub fn cancel(&mut self) {
        if self.is_drawing() {
            self.shapes.pop();
        }
    }

    /// Épingler suspend l'effacement ; désépingler relance la durée de maintien.
    pub fn set_pinned(&mut self, pinned: bool, now: u64) {
        if self.pinned && !pinned {
            for s in &mut self.shapes {
                s.hold_from = now;
            }
        }
        self.pinned = pinned;
    }

    /// Fait disparaître toutes les formes en `CLEAR_MS`.
    pub fn clear_all(&mut self, now: u64) {
        self.cancel();
        for s in &mut self.shapes {
            if s.clearing_from.is_none() {
                s.clearing_from = Some(now);
            }
        }
    }

    /// Supprime la forme la plus récente encore visible.
    pub fn undo(&mut self) {
        // L'instant n'importe pas ici : seules les formes en cours d'effacement sont ignorées.
        if let Some(i) = self.shapes.iter().rposition(|s| s.clearing_from.is_none()) {
            self.shapes.remove(i);
        }
    }

    pub fn phase(&self, s: &Shape, now: u64) -> Phase {
        if let Some(c) = s.clearing_from {
            let t = now.saturating_sub(c);
            return if t >= CLEAR_MS { Phase::Gone } else { Phase::Fading(t as f32 / CLEAR_MS as f32) };
        }
        let Some(released) = s.released_at else {
            return Phase::Drawing;
        };
        let since = now.saturating_sub(released);
        if since < APPEAR_MS {
            return Phase::Appearing(since as f32 / APPEAR_MS as f32);
        }
        if self.pinned {
            return Phase::Holding;
        }
        let fade_start = s.hold_from + self.hold_ms;
        if now < fade_start {
            Phase::Holding
        } else if now < fade_start + FADE_MS {
            Phase::Fading((now - fade_start) as f32 / FADE_MS as f32)
        } else {
            Phase::Gone
        }
    }

    pub fn opacity(&self, s: &Shape, now: u64) -> f32 {
        match self.phase(s, now) {
            Phase::Fading(p) => 1.0 - p,
            Phase::Gone => 0.0,
            _ => 1.0,
        }
    }

    /// Supprime les formes disparues. Renvoie vrai tant qu'une forme est animée.
    pub fn tick(&mut self, now: u64) -> bool {
        let phases: Vec<Phase> = self.shapes.iter().map(|s| self.phase(s, now)).collect();
        let mut it = phases.iter();
        self.shapes.retain(|_| !matches!(it.next(), Some(Phase::Gone)));
        phases.iter().any(|p| matches!(p, Phase::Drawing | Phase::Appearing(_) | Phase::Fading(_)))
    }

    /// Instant où la prochaine forme au repos commencera son fondu.
    pub fn next_deadline(&self, now: u64) -> Option<u64> {
        if self.pinned {
            return None;
        }
        self.shapes
            .iter()
            .filter(|s| matches!(self.phase(s, now), Phase::Holding))
            .map(|s| s.hold_from + self.hold_ms)
            .min()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scene() -> Scene {
        Scene::new(5000)
    }

    fn draw(s: &mut Scene, tool: Tool, a: Pt, b: Pt, t: u64) {
        s.begin(tool, a, 0, 5.0, 1.0, t);
        s.drag(b, false);
        s.release(t);
    }

    #[test]
    fn lifecycle_timings() {
        let mut s = scene();
        draw(&mut s, Tool::Arrow, Pt::new(0., 0.), Pt::new(100., 0.), 1000);
        let sh = s.shapes()[0].clone();
        assert!(matches!(s.phase(&sh, 1075), Phase::Appearing(p) if (p - 0.5).abs() < 0.01));
        assert!(matches!(s.phase(&sh, 1150), Phase::Holding));
        assert!(matches!(s.phase(&sh, 5999), Phase::Holding));
        assert!(matches!(s.phase(&sh, 6300), Phase::Fading(p) if (p - 0.5).abs() < 0.01));
        assert!((s.opacity(&sh, 6300) - 0.5).abs() < 0.01);
        assert_eq!(s.opacity(&sh, 3000), 1.0);
        assert!(matches!(s.phase(&sh, 6600), Phase::Gone));
        assert!(!s.tick(6600));
        assert!(s.shapes().is_empty());
    }

    #[test]
    fn drawing_phase_until_release() {
        let mut s = scene();
        s.begin(Tool::Arrow, Pt::new(0., 0.), 0, 5.0, 1.0, 0);
        s.drag(Pt::new(100., 0.), false);
        let sh = s.shapes()[0].clone();
        assert!(matches!(s.phase(&sh, 99_999), Phase::Drawing));
        assert!(s.tick(99_999));
    }

    #[test]
    fn tick_reports_animation_only_when_needed() {
        let mut s = scene();
        draw(&mut s, Tool::Rect, Pt::new(0., 0.), Pt::new(50., 50.), 0);
        assert!(s.tick(100)); // apparition
        assert!(!s.tick(3000)); // maintien : rien ne bouge
        assert!(s.tick(5100)); // fondu
    }

    #[test]
    fn next_deadline_is_fade_start() {
        let mut s = scene();
        assert_eq!(s.next_deadline(0), None);
        draw(&mut s, Tool::Rect, Pt::new(0., 0.), Pt::new(50., 50.), 1000);
        assert_eq!(s.next_deadline(2000), Some(6000));
    }

    #[test]
    fn hold_duration_is_adjustable() {
        let mut s = scene();
        draw(&mut s, Tool::Rect, Pt::new(0., 0.), Pt::new(50., 50.), 0);
        s.set_hold_ms(2000);
        let sh = s.shapes()[0].clone();
        assert!(matches!(s.phase(&sh, 2100), Phase::Fading(_)));
    }

    #[test]
    fn tiny_shapes_are_dropped() {
        let mut s = scene();
        draw(&mut s, Tool::Arrow, Pt::new(0., 0.), Pt::new(11., 0.), 0);
        draw(&mut s, Tool::Rect, Pt::new(0., 0.), Pt::new(5., 5.), 0);
        s.begin(Tool::Ellipse, Pt::new(9., 9.), 0, 5.0, 1.0, 0);
        s.drag(Pt::new(9., 9.), true);
        s.release(0);
        s.begin(Tool::Arrow, Pt::new(3., 3.), 0, 5.0, 1.0, 0); // clic sans glisser
        s.release(0);
        assert!(s.shapes().is_empty());
    }

    #[test]
    fn min_size_scales_with_dpi() {
        // 12 px logiques = 24 px physiques à 200 %
        let mut s = scene();
        s.begin(Tool::Arrow, Pt::new(0., 0.), 0, 5.0, 2.0, 0);
        s.drag(Pt::new(20., 0.), false);
        s.release(0);
        assert!(s.shapes().is_empty());
        s.begin(Tool::Arrow, Pt::new(0., 0.), 0, 5.0, 2.0, 0);
        s.drag(Pt::new(30., 0.), false);
        s.release(0);
        assert_eq!(s.shapes().len(), 1);
    }

    #[test]
    fn shift_constraints() {
        let mut s = scene();
        s.begin(Tool::Rect, Pt::new(0., 0.), 0, 5.0, 1.0, 0);
        s.drag(Pt::new(30., -10.), true);
        s.release(0);
        assert!(matches!(s.shapes()[0].kind, ShapeKind::Rect { b, .. } if b == Pt::new(30., -30.)));
        s.begin(Tool::Highlighter, Pt::new(0., 0.), 0, 5.0, 1.0, 0);
        s.drag(Pt::new(20., 3.), true);
        s.drag(Pt::new(80., 9.), true);
        s.release(0);
        assert!(
            matches!(&s.shapes()[1].kind, ShapeKind::Highlighter { pts } if pts.len() == 2 && pts[1] == Pt::new(80., 0.))
        );
        s.begin(Tool::Arrow, Pt::new(0., 0.), 0, 5.0, 1.0, 0);
        s.drag(Pt::new(100., 10.), true);
        s.release(0);
        assert!(matches!(s.shapes()[2].kind, ShapeKind::Arrow { to, .. } if to.y.abs() < 1e-3));
    }

    #[test]
    fn pin_suspends_and_unpin_restarts() {
        let mut s = scene();
        draw(&mut s, Tool::Arrow, Pt::new(0., 0.), Pt::new(100., 0.), 0);
        s.set_pinned(true, 100);
        assert!(s.pinned());
        let sh = s.shapes()[0].clone();
        assert!(matches!(s.phase(&sh, 60_000), Phase::Holding));
        assert_eq!(s.next_deadline(60_000), None);
        s.set_pinned(false, 60_000);
        let sh = s.shapes()[0].clone();
        assert!(matches!(s.phase(&sh, 64_999), Phase::Holding));
        assert!(matches!(s.phase(&sh, 65_300), Phase::Fading(_)));
    }

    #[test]
    fn clear_all_fades_in_200ms() {
        let mut s = scene();
        draw(&mut s, Tool::Arrow, Pt::new(0., 0.), Pt::new(100., 0.), 0);
        s.set_pinned(true, 0);
        s.clear_all(1000);
        let sh = s.shapes()[0].clone();
        assert!((s.opacity(&sh, 1100) - 0.5).abs() < 0.01);
        s.tick(1200);
        assert!(s.shapes().is_empty());
    }

    #[test]
    fn undo_removes_latest_visible() {
        let mut s = scene();
        draw(&mut s, Tool::Arrow, Pt::new(0., 0.), Pt::new(100., 0.), 0);
        draw(&mut s, Tool::Rect, Pt::new(0., 0.), Pt::new(50., 50.), 10);
        s.undo();
        assert_eq!(s.shapes().len(), 1);
        assert!(matches!(s.shapes()[0].kind, ShapeKind::Arrow { .. }));
        s.undo();
        s.undo();
        assert!(s.shapes().is_empty());
    }

    #[test]
    fn undo_skips_shapes_already_clearing() {
        let mut s = scene();
        draw(&mut s, Tool::Arrow, Pt::new(0., 0.), Pt::new(100., 0.), 0);
        s.clear_all(100);
        draw(&mut s, Tool::Rect, Pt::new(0., 0.), Pt::new(50., 50.), 110);
        s.undo();
        s.undo();
        assert_eq!(s.shapes().len(), 1);
        assert!(matches!(s.shapes()[0].kind, ShapeKind::Arrow { .. }));
    }

    #[test]
    fn cancel_drops_shape_in_progress_only() {
        let mut s = scene();
        draw(&mut s, Tool::Arrow, Pt::new(0., 0.), Pt::new(100., 0.), 0);
        s.begin(Tool::Rect, Pt::new(0., 0.), 0, 5.0, 1.0, 10);
        s.drag(Pt::new(80., 80.), false);
        s.cancel();
        assert_eq!(s.shapes().len(), 1);
        s.cancel();
        assert_eq!(s.shapes().len(), 1);
    }

    #[test]
    fn numbers_count_and_reset() {
        let mut s = scene();
        let n = |s: &Scene, i: usize| match s.shapes()[i].kind {
            ShapeKind::Number { n, .. } => n,
            _ => 0,
        };
        for i in 0..3 {
            s.begin(Tool::Number, Pt::new(i as f32 * 50., 0.), 0, 5.0, 1.0, 0);
            s.release(0);
        }
        assert_eq!((n(&s, 0), n(&s, 1), n(&s, 2)), (1, 2, 3));
        s.tick(10_000);
        assert!(s.shapes().is_empty()); // toutes effacées : le compteur repart à 1
        s.begin(Tool::Number, Pt::new(0., 0.), 0, 5.0, 1.0, 10_000);
        s.release(10_000);
        assert_eq!(n(&s, 0), 1);
        s.clear_all(10_001);
        s.begin(Tool::Number, Pt::new(0., 0.), 0, 5.0, 1.0, 10_002);
        s.release(10_002);
        assert_eq!(n(&s, s.shapes().len() - 1), 1); // Tout effacer remet aussi à 1
    }

    #[test]
    fn undone_number_is_reused() {
        let mut s = scene();
        for _ in 0..2 {
            s.begin(Tool::Number, Pt::new(0., 0.), 0, 5.0, 1.0, 0);
            s.release(0);
        }
        s.undo();
        s.begin(Tool::Number, Pt::new(0., 0.), 0, 5.0, 1.0, 0);
        s.release(0);
        assert!(matches!(s.shapes()[1].kind, ShapeKind::Number { n: 2, .. }));
    }

    #[test]
    fn free_stroke_is_capped() {
        let mut s = scene();
        s.begin(Tool::Free, Pt::new(0., 0.), 0, 5.0, 1.0, 0);
        for i in 1..20_000 {
            s.drag(Pt::new(i as f32 * 4.0, (i % 7) as f32 * 4.0), false);
        }
        assert!(matches!(&s.shapes()[0].kind, ShapeKind::Free { pts } if pts.len() == stroke::MAX_POINTS));
    }

    #[test]
    fn free_stroke_single_click_is_kept_as_dot() {
        let mut s = scene();
        s.begin(Tool::Free, Pt::new(5., 5.), 0, 5.0, 1.0, 0);
        s.release(0);
        assert_eq!(s.shapes().len(), 1);
    }

    #[test]
    fn begin_while_drawing_finishes_previous_shape() {
        let mut s = scene();
        s.begin(Tool::Arrow, Pt::new(0., 0.), 0, 5.0, 1.0, 0);
        s.drag(Pt::new(100., 0.), false);
        s.begin(Tool::Rect, Pt::new(0., 0.), 0, 5.0, 1.0, 50);
        let first = s.shapes()[0].clone();
        assert!(!matches!(s.phase(&first, 50), Phase::Drawing));
        assert_eq!(s.shapes().len(), 2);
    }

    #[test]
    fn single_click_highlighter_is_dropped() {
        // Un point seul ne se dessine pas : il ne doit ni vivre 5 s ni absorber un Ctrl+Z.
        let mut s = scene();
        draw(&mut s, Tool::Arrow, Pt::new(0., 0.), Pt::new(100., 0.), 0);
        s.begin(Tool::Highlighter, Pt::new(5., 5.), 0, 5.0, 1.0, 0);
        s.release(0);
        assert_eq!(s.shapes().len(), 1);
        s.undo();
        assert!(s.shapes().is_empty());
    }
}
