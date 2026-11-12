//! Jetons du design system Eksplodo 2026.

use crate::geom::Rgba;

pub const BG: Rgba = Rgba::hex(0xFCFDFE);
pub const LINE: Rgba = Rgba::hex(0xCEDEE6);
pub const INK: Rgba = Rgba::hex(0x003C58);
pub const MUTED: Rgba = Rgba::hex(0x4E6F7E);
pub const HOVER: Rgba = Rgba::hex(0xE3F1F7);
pub const BLUE: Rgba = Rgba::hex(0x0C659D);
pub const EMBER: Rgba = Rgba::hex(0xF0631F);
pub const WHITE: Rgba = Rgba::hex(0xFFFFFF);

/// Hauteur de la barre d'outils, en pixels logiques.
pub const BAR_H: f32 = 56.0;
/// Côté d'un bouton de la barre.
pub const BTN: f32 = 40.0;
/// Épaisseur de trait des icônes.
pub const ICON_STROKE: f32 = 1.75;
/// Rayon du panneau de réglages.
pub const R_PANEL: f32 = 18.0;
/// Rayon des volets.
pub const R_POPOVER: f32 = 14.0;

/// Durée du glissement de la pastille de l'outil actif.
pub const T_PILL_MS: u64 = 220;
/// Durée d'apparition des volets et du panneau.
pub const T_POPOVER_MS: u64 = 180;

/// Courbe d'animation du design system : `cubic-bezier(.16,.84,.3,1)`.
pub fn ease(t: f32) -> f32 {
    if t <= 0.0 {
        return 0.0;
    }
    if t >= 1.0 {
        return 1.0;
    }
    const X1: f32 = 0.16;
    const Y1: f32 = 0.84;
    const X2: f32 = 0.3;
    const Y2: f32 = 1.0;
    let bez = |u: f32, a: f32, b: f32| {
        let v = 1.0 - u;
        3.0 * v * v * u * a + 3.0 * v * u * u * b + u * u * u
    };
    let dbez = |u: f32, a: f32, b: f32| {
        let v = 1.0 - u;
        3.0 * v * v * a + 6.0 * v * u * (b - a) + 3.0 * u * u * (1.0 - b)
    };
    // Résout x(u) = t par Newton, puis renvoie y(u).
    let mut u = t;
    for _ in 0..6 {
        let d = dbez(u, X1, X2);
        if d.abs() < 1e-6 {
            break;
        }
        u = (u - (bez(u, X1, X2) - t) / d).clamp(0.0, 1.0);
    }
    bez(u, Y1, Y2)
}

#[cfg(test)]
mod tests {
    use super::*;


    #[test]
    fn tokens_match_design_system() {
        assert_eq!(BG, Rgba::hex(0xFCFDFE));
        assert_eq!(LINE, Rgba::hex(0xCEDEE6));
        assert_eq!(INK, Rgba::hex(0x003C58));
        assert_eq!(MUTED, Rgba::hex(0x4E6F7E));
        assert_eq!(HOVER, Rgba::hex(0xE3F1F7));
        assert_eq!(BLUE, Rgba::hex(0x0C659D));
        assert_eq!(EMBER, Rgba::hex(0xF0631F));
        assert_eq!(WHITE, Rgba::hex(0xFFFFFF));
        assert_eq!((BAR_H, BTN, ICON_STROKE, R_PANEL, R_POPOVER), (56.0, 40.0, 1.75, 18.0, 14.0));
        assert_eq!((T_PILL_MS, T_POPOVER_MS), (220, 180));
    }

    #[test]
    fn ease_is_monotonic_and_bounded() {
        assert!(ease(0.0).abs() < 1e-3 && (ease(1.0) - 1.0).abs() < 1e-3);
        let mut prev = 0.0;
        for i in 1..=20 {
            let v = ease(i as f32 / 20.0);
            assert!(v >= prev);
            prev = v;
        }
        assert!(ease(0.25) > 0.5); // courbe très rapide au départ
    }

    #[test]
    fn ease_clamps_out_of_range() {
        assert_eq!(ease(-1.0), 0.0);
        assert_eq!(ease(2.0), 1.0);
    }
}
