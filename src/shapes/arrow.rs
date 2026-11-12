//! Géométrie de la flèche (spec 5.4).

use crate::geom::Pt;

/// Dimensions d'une flèche, dans l'unité de `len` et `w`.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct ArrowGeom {
    /// Longueur de la tête, de la pointe à l'extrémité des ailes.
    pub head_len: f32,
    /// Envergure de la tête.
    pub head_width: f32,
    /// Largeur du corps à l'origine.
    pub tail_width: f32,
    /// Largeur du corps à la jonction avec la tête.
    pub neck_width: f32,
    /// Avancée du creux de la tête vers la pointe.
    pub notch: f32,
    /// Rayon d'arrondi des angles.
    pub corner_radius: f32,
}

/// Proportions d'une flèche de longueur `len` et d'épaisseur `w`.
pub fn arrow_geom(len: f32, w: f32) -> ArrowGeom {
    let head_len = (4.5 * w).max(16.0).min(0.4 * len.max(0.0));
    ArrowGeom {
        head_len,
        head_width: 0.8 * head_len,
        tail_width: 0.35 * w,
        neck_width: w,
        notch: 0.25 * head_len,
        corner_radius: 0.5 * w,
    }
}

/// Contour fermé de la flèche : queue gauche, col gauche, aile gauche, pointe,
/// aile droite, col droit, queue droite.
pub fn arrow_outline(from: Pt, to: Pt, w: f32) -> [Pt; 7] {
    let len = from.dist(to);
    let (dx, dy) = if len > f32::EPSILON {
        ((to.x - from.x) / len, (to.y - from.y) / len)
    } else {
        (1.0, 0.0)
    };
    // Normale « gauche » : vers le haut de l'écran pour une flèche pointant à droite.
    let (nx, ny) = (dy, -dx);
    let g = arrow_geom(len, w);
    let at = |along: f32, side: f32| Pt::new(to.x - dx * along + nx * side, to.y - dy * along + ny * side);
    let neck = g.head_len - g.notch;
    [
        at(len, g.tail_width / 2.0),
        at(neck, g.neck_width / 2.0),
        at(g.head_len, g.head_width / 2.0),
        to,
        at(g.head_len, -g.head_width / 2.0),
        at(neck, -g.neck_width / 2.0),
        at(len, -g.tail_width / 2.0),
    ]
}

/// Ramène la direction `from -> to` au multiple de `step_deg` le plus proche,
/// en conservant la longueur.
pub fn snap_angle(from: Pt, to: Pt, step_deg: f32) -> Pt {
    let len = from.dist(to);
    if len <= f32::EPSILON || step_deg <= 0.0 {
        return to;
    }
    let step = step_deg.to_radians();
    let angle = (to.y - from.y).atan2(to.x - from.x);
    let snapped = (angle / step).round() * step;
    Pt::new(from.x + len * snapped.cos(), from.y + len * snapped.sin())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-3
    }

    #[test]
    fn proportions_nominal() {
        // L = 200, w = 5
        let g = arrow_geom(200.0, 5.0);
        assert!(close(g.head_len, 22.5), "{g:?}");
        assert!(close(g.head_width, 18.0), "{g:?}");
        assert!(close(g.tail_width, 1.75), "{g:?}");
        assert!(close(g.neck_width, 5.0), "{g:?}");
        assert!(close(g.notch, 5.625), "{g:?}");
        assert!(close(g.corner_radius, 2.5), "{g:?}");
    }

    #[test]
    fn head_has_minimum_16() {
        // w = 3 : 4,5 × w = 13,5 < 16
        let g = arrow_geom(200.0, 3.0);
        assert!(close(g.head_len, 16.0) && close(g.head_width, 12.8), "{g:?}");
    }

    #[test]
    fn head_capped_at_40_percent() {
        // L = 40, w = 8 : 36 plafonné à 16
        let g = arrow_geom(40.0, 8.0);
        assert!(close(g.head_len, 16.0) && close(g.head_width, 12.8), "{g:?}");
    }

    #[test]
    fn outline_tip_and_symmetry() {
        let o = arrow_outline(Pt::new(0., 0.), Pt::new(200., 0.), 5.0);
        assert!(close(o[3].x, 200.0) && close(o[3].y, 0.0), "{o:?}");
        assert!(close(o[2].x, 177.5) && close(o[2].y, -9.0) && close(o[4].y, 9.0), "{o:?}");
        assert!(close(o[4].x, 177.5), "{o:?}");
        assert!(close(o[1].x, 183.125) && close(o[1].y, -2.5) && close(o[5].y, 2.5), "{o:?}");
        assert!(close(o[0].x, 0.0) && close(o[0].y, -0.875) && close(o[6].y, 0.875), "{o:?}");
    }

    #[test]
    fn outline_follows_direction() {
        // Flèche vers le bas : la pointe est en bas, les ailes de part et d'autre en x.
        let o = arrow_outline(Pt::new(50., 0.), Pt::new(50., 200.), 5.0);
        assert!(close(o[3].x, 50.0) && close(o[3].y, 200.0), "{o:?}");
        assert!(close(o[2].y, 177.5) && close(o[4].y, 177.5), "{o:?}");
        assert!(close((o[2].x - 50.0).abs(), 9.0) && close(o[2].x + o[4].x, 100.0), "{o:?}");
    }

    #[test]
    fn snap_to_15_degrees() {
        let p = snap_angle(Pt::new(0., 0.), Pt::new(100., 10.), 15.0); // 5,7° -> 0°
        assert!(close(p.y, 0.0) && close(p.x, (100.0f32 * 100.0 + 100.0).sqrt()), "{p:?}");
        let q = snap_angle(Pt::new(0., 0.), Pt::new(100., 20.), 15.0); // 11,3° -> 15°
        assert!(close(q.y / q.x, 15.0f32.to_radians().tan()), "{q:?}");
        let r = snap_angle(Pt::new(10., 10.), Pt::new(10., -90.), 15.0); // déjà à la verticale
        assert!(close(r.x, 10.0) && close(r.y, -90.0), "{r:?}");
    }

    #[test]
    fn zero_length_is_finite() {
        let p = snap_angle(Pt::new(5., 5.), Pt::new(5., 5.), 15.0);
        assert!(p.x.is_finite() && p.y.is_finite());
        for pt in arrow_outline(Pt::new(5., 5.), Pt::new(5., 5.), 5.0) {
            assert!(pt.x.is_finite() && pt.y.is_finite());
        }
    }
}
