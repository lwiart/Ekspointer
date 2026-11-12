//! Effets de pointeur : ondes au clic et traînée laser (spec 5.6).

use crate::geom::Pt;
use std::collections::VecDeque;

/// Durée de vie d'une onde au clic.
pub const RIPPLE_MS: u64 = 450;
/// Rayon de départ d'une onde, en pixels logiques.
pub const RIPPLE_R0: f32 = 8.0;
/// Rayon final d'une onde, en pixels logiques.
pub const RIPPLE_R1: f32 = 48.0;
/// Durée couverte par la traînée laser.
pub const TRAIL_MS: u64 = 700;

/// Bouton de souris.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Button {
    Left,
    Right,
}

/// Onde déclenchée par un clic.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Ripple {
    pub at: Pt,
    pub button: Button,
    pub start: u64,
    /// Échelle de l'écran où le clic a eu lieu.
    pub scale: f32,
}

/// État des effets de pointeur.
#[derive(Debug, Default)]
pub struct Pointer {
    pos: Pt,
    ripples: Vec<Ripple>,
    trail: VecDeque<(Pt, u64)>,
}

impl Pointer {
    pub fn new() -> Pointer {
        Pointer::default()
    }

    /// Dernière position connue du curseur.
    pub fn pos(&self) -> Pt {
        self.pos
    }

    /// Enregistre un déplacement ; la traînée n'est alimentée que si elle est activée.
    pub fn on_move(&mut self, p: Pt, now: u64, trail_on: bool) {
        self.pos = p;
        if trail_on {
            self.trail.push_back((p, now));
            self.prune_trail(now);
        } else {
            self.trail.clear();
        }
    }

    pub fn on_click(&mut self, p: Pt, button: Button, scale: f32, now: u64) {
        self.pos = p;
        self.ripples.push(Ripple { at: p, button, start: now, scale });
    }

    fn prune_trail(&mut self, now: u64) {
        while self.trail.front().is_some_and(|(_, t)| now.saturating_sub(*t) >= TRAIL_MS) {
            self.trail.pop_front();
        }
    }

    /// Retire les ondes et points expirés. Renvoie vrai tant qu'il reste quelque chose à animer.
    pub fn tick(&mut self, now: u64) -> bool {
        self.ripples.retain(|r| now.saturating_sub(r.start) < RIPPLE_MS);
        self.prune_trail(now);
        !self.ripples.is_empty() || !self.trail.is_empty()
    }

    pub fn ripples(&self) -> &[Ripple] {
        &self.ripples
    }

    /// Rayon (pixels logiques) et opacité d'une onde à l'instant `now`.
    pub fn ripple_state(r: &Ripple, now: u64) -> (f32, f32) {
        let t = (now.saturating_sub(r.start) as f32 / RIPPLE_MS as f32).clamp(0.0, 1.0);
        let eased = 1.0 - (1.0 - t).powi(3);
        (RIPPLE_R0 + (RIPPLE_R1 - RIPPLE_R0) * eased, 1.0 - t)
    }

    /// Points de la traînée, du plus ancien au plus récent, avec leur âge de 0 (récent) à 1.
    pub fn trail(&self, now: u64) -> Vec<(Pt, f32)> {
        self.trail
            .iter()
            .filter(|(_, t)| now.saturating_sub(*t) < TRAIL_MS)
            .map(|(p, t)| (*p, now.saturating_sub(*t) as f32 / TRAIL_MS as f32))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ripple_grows_and_fades() {
        let r = Ripple { at: Pt::new(0., 0.), button: Button::Left, start: 1000, scale: 1.0 };
        let (r0, o0) = Pointer::ripple_state(&r, 1000);
        assert!((r0 - 8.0).abs() < 0.01 && (o0 - 1.0).abs() < 0.01);
        let (r1, o1) = Pointer::ripple_state(&r, 1450);
        assert!((r1 - 48.0).abs() < 0.01 && o1.abs() < 0.01);
        let (rm, om) = Pointer::ripple_state(&r, 1225);
        assert!(rm > 28.0); // ralentit en fin de course
        assert!((om - 0.5).abs() < 0.01);
    }

    #[test]
    fn ripples_expire() {
        let mut p = Pointer::new();
        p.on_click(Pt::new(1., 1.), Button::Right, 1.0, 0);
        assert!(p.tick(449));
        assert_eq!(p.ripples().len(), 1);
        assert_eq!(p.ripples()[0].button, Button::Right);
        assert!(!p.tick(451));
        assert!(p.ripples().is_empty());
    }

    #[test]
    fn trail_prunes_after_700ms() {
        let mut p = Pointer::new();
        p.on_move(Pt::new(0., 0.), 0, true);
        p.on_move(Pt::new(10., 0.), 400, true);
        p.on_move(Pt::new(20., 0.), 800, true);
        let t = p.trail(800);
        assert_eq!(t.len(), 2);
        assert!((t[0].1 - 400.0 / 700.0).abs() < 0.01 && t[1].1.abs() < 0.01);
        assert!(p.tick(900));
        assert!(!p.tick(1501));
        assert!(p.trail(1501).is_empty());
    }

    #[test]
    fn trail_off_records_nothing() {
        let mut p = Pointer::new();
        p.on_move(Pt::new(5., 5.), 0, false);
        assert!(p.trail(0).is_empty());
        assert_eq!(p.pos(), Pt::new(5., 5.));
        assert!(!p.tick(0));
    }

    #[test]
    fn turning_trail_off_clears_it() {
        let mut p = Pointer::new();
        p.on_move(Pt::new(0., 0.), 0, true);
        p.on_move(Pt::new(9., 9.), 10, false);
        assert!(p.trail(10).is_empty());
    }

    #[test]
    fn constants_match_spec() {
        assert_eq!((RIPPLE_MS, RIPPLE_R0, RIPPLE_R1, TRAIL_MS), (450, 8.0, 48.0, 700));
    }
}
