//! Effets de pointeur : ondes au clic et traînée laser (spec 5.6).

use crate::geom::Pt;
use std::collections::VecDeque;

// IMPL

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
