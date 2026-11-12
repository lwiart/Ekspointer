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

/// Longueur de la tête, en multiples de l'épaisseur.
const HEAD_LEN: f32 = 5.2;
/// Envergure de la tête, en multiples de l'épaisseur.
const HEAD_WIDTH: f32 = 5.5;
/// Largeur du corps au col, en multiples de l'épaisseur.
const NECK_WIDTH: f32 = 2.2;
/// Part de la longueur que la tête ne dépasse jamais.
const HEAD_MAX_SHARE: f32 = 0.45;

/// Proportions d'une flèche de longueur `len` et d'épaisseur `w`.
///
/// Le corps part en pointe de l'origine et s'élargit jusqu'au col ; la tête est large,
/// avec des ailes tirées vers l'arrière. Une flèche courte est réduite en proportion.
pub fn arrow_geom(len: f32, w: f32) -> ArrowGeom {
    let nominal = HEAD_LEN * w;
    let head_len = nominal.min(HEAD_MAX_SHARE * len.max(0.0));
    let f = if nominal > 0.0 { head_len / nominal } else { 0.0 };
    ArrowGeom {
        head_len,
        head_width: HEAD_WIDTH * w * f,
        tail_width: 0.0,
        neck_width: NECK_WIDTH * w * f,
        notch: 0.22 * head_len,
        corner_radius: 0.15 * w,
    }
}

/// Contour fermé de la flèche (la queue est une pointe) : queue gauche, col gauche, aile gauche, pointe,
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
        // L = 200, w = 5 : tête de 5,2 × w, envergure 5,5 × w, col 2,2 × w, queue en pointe
        let g = arrow_geom(200.0, 5.0);
        assert!(close(g.head_len, 26.0), "{g:?}");
        assert!(close(g.head_width, 27.5), "{g:?}");
        assert!(close(g.tail_width, 0.0), "{g:?}");
        assert!(close(g.neck_width, 11.0), "{g:?}");
        assert!(close(g.notch, 5.72), "{g:?}");
        assert!(close(g.corner_radius, 0.75), "{g:?}");
    }

    #[test]
    fn thickness_scales_the_whole_arrow() {
        let (thin, thick) = (arrow_geom(400.0, 3.0), arrow_geom(400.0, 8.0));
        assert!(close(thin.head_len, 15.6) && close(thin.neck_width, 6.6), "{thin:?}");
        assert!(close(thick.head_len, 41.6) && close(thick.head_width, 44.0) && close(thick.neck_width, 17.6), "{thick:?}");
    }

    #[test]
    fn short_arrow_shrinks_in_proportion() {
        // L = 40, w = 8 : tête plafonnée à 45 % de la longueur, le reste suit la même réduction
        let g = arrow_geom(40.0, 8.0);
        let f = 18.0 / 41.6;
        assert!(close(g.head_len, 18.0), "{g:?}");
        assert!(close(g.head_width, 44.0 * f) && close(g.neck_width, 17.6 * f) && close(g.notch, 0.22 * 18.0), "{g:?}");
    }

    #[test]
    fn outline_tip_and_symmetry() {
        let o = arrow_outline(Pt::new(0., 0.), Pt::new(200., 0.), 5.0);
        assert!(close(o[3].x, 200.0) && close(o[3].y, 0.0), "{o:?}");
        assert!(close(o[2].x, 174.0) && close(o[2].y, -13.75), "{o:?}");
        assert!(close(o[4].x, 174.0) && close(o[4].y, 13.75), "{o:?}");
        assert!(close(o[1].x, 179.72) && close(o[1].y, -5.5) && close(o[5].y, 5.5), "{o:?}");
        // La queue se termine en pointe : ses deux points sont confondus avec l'origine.
        assert!(close(o[0].x, 0.0) && close(o[0].y, 0.0) && close(o[6].x, 0.0) && close(o[6].y, 0.0), "{o:?}");
    }

    #[test]
    fn outline_follows_direction() {
        // Flèche vers le bas : la pointe est en bas, les ailes de part et d'autre en x.
        let o = arrow_outline(Pt::new(50., 0.), Pt::new(50., 200.), 5.0);
        assert!(close(o[3].x, 50.0) && close(o[3].y, 200.0), "{o:?}");
        assert!(close(o[2].y, 174.0) && close(o[4].y, 174.0), "{o:?}");
        assert!(close((o[2].x - 50.0).abs(), 13.75) && close(o[2].x + o[4].x, 100.0), "{o:?}");
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
