//! Trait libre : échantillonnage, lissage, contraintes de la touche Maj.

use crate::geom::Pt;

/// Distance minimale entre deux points enregistrés, en pixels logiques.
pub const MIN_POINT_DIST: f32 = 3.0;
/// Nombre maximal de points d'un tracé : au-delà, le tracé cesse de s'allonger.
pub const MAX_POINTS: usize = 5000;

/// Ajoute `p` s'il est assez loin du dernier point et que le plafond n'est pas atteint.
pub fn push_point(pts: &mut Vec<Pt>, p: Pt, min_dist: f32) -> bool {
    if pts.len() >= MAX_POINTS {
        return false;
    }
    if let Some(last) = pts.last() {
        if last.dist(p) < min_dist {
            return false;
        }
    }
    pts.push(p);
    true
}

/// Lisse une suite de points : une Bézier cubique par paire de points consécutifs
/// (Catmull-Rom uniforme, extrémités dupliquées).
pub fn catmull_rom_beziers(pts: &[Pt]) -> Vec<[Pt; 4]> {
    if pts.len() < 2 {
        return Vec::new();
    }
    (0..pts.len() - 1)
        .map(|i| {
            let p0 = pts[i.saturating_sub(1)];
            let p1 = pts[i];
            let p2 = pts[i + 1];
            let p3 = pts[(i + 2).min(pts.len() - 1)];
            let c1 = Pt::new(p1.x + (p2.x - p0.x) / 6.0, p1.y + (p2.y - p0.y) / 6.0);
            let c2 = Pt::new(p2.x - (p3.x - p1.x) / 6.0, p2.y - (p3.y - p1.y) / 6.0);
            [p1, c1, c2, p2]
        })
        .collect()
}

/// Contraint `b` pour que le rectangle `a`-`b` soit un carré (côté = plus grande dimension).
pub fn constrain_square(a: Pt, b: Pt) -> Pt {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let side = dx.abs().max(dy.abs());
    Pt::new(a.x + side * dx.signum(), a.y + side * dy.signum())
}

/// Contraint `b` sur l'horizontale ou la verticale passant par `a`.
pub fn constrain_axis(a: Pt, b: Pt) -> Pt {
    if (b.x - a.x).abs() >= (b.y - a.y).abs() {
        Pt::new(b.x, a.y)
    } else {
        Pt::new(a.x, b.y)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_respects_min_distance() {
        let mut v = vec![Pt::new(0., 0.)];
        assert!(!push_point(&mut v, Pt::new(2., 0.), 3.0));
        assert!(push_point(&mut v, Pt::new(3., 0.), 3.0));
        assert_eq!(v.len(), 2);
    }

    #[test]
    fn push_into_empty_always_adds() {
        let mut v = Vec::new();
        assert!(push_point(&mut v, Pt::new(1., 1.), 3.0));
        assert_eq!(v, vec![Pt::new(1., 1.)]);
    }

    #[test]
    fn push_stops_at_max_points() {
        let mut v: Vec<Pt> = (0..MAX_POINTS).map(|i| Pt::new(i as f32 * 10.0, 0.)).collect();
        assert!(!push_point(&mut v, Pt::new(1e6, 0.), 3.0));
        assert_eq!(v.len(), MAX_POINTS);
        assert_eq!((MIN_POINT_DIST, MAX_POINTS), (3.0, 5000));
    }

    #[test]
    fn beziers_pass_through_points() {
        let pts = [Pt::new(0., 0.), Pt::new(10., 0.), Pt::new(20., 10.)];
        let b = catmull_rom_beziers(&pts);
        assert_eq!(b.len(), 2);
        assert_eq!(b[0][0], pts[0]);
        assert_eq!(b[0][3], pts[1]);
        assert_eq!(b[1][0], pts[1]);
        assert_eq!(b[1][3], pts[2]);
    }

    #[test]
    fn beziers_are_smooth_at_joints() {
        // La tangente sortante d'un segment prolonge la tangente entrante du suivant.
        let pts = [Pt::new(0., 0.), Pt::new(10., 0.), Pt::new(20., 10.), Pt::new(30., 10.)];
        let b = catmull_rom_beziers(&pts);
        let out = (b[0][3].x - b[0][2].x, b[0][3].y - b[0][2].y);
        let inn = (b[1][1].x - b[1][0].x, b[1][1].y - b[1][0].y);
        assert!((out.0 - inn.0).abs() < 1e-4 && (out.1 - inn.1).abs() < 1e-4);
    }

    #[test]
    fn beziers_degenerate_inputs() {
        assert!(catmull_rom_beziers(&[]).is_empty());
        assert!(catmull_rom_beziers(&[Pt::new(1., 1.)]).is_empty());
    }

    #[test]
    fn square_keeps_signs() {
        assert_eq!(constrain_square(Pt::new(0., 0.), Pt::new(30., -10.)), Pt::new(30., -30.));
        assert_eq!(constrain_square(Pt::new(0., 0.), Pt::new(-5., 20.)), Pt::new(-20., 20.));
        assert_eq!(constrain_square(Pt::new(9., 9.), Pt::new(9., 9.)), Pt::new(9., 9.));
    }

    #[test]
    fn axis_picks_dominant() {
        assert_eq!(constrain_axis(Pt::new(0., 0.), Pt::new(50., 8.)), Pt::new(50., 0.));
        assert_eq!(constrain_axis(Pt::new(0., 0.), Pt::new(4., -30.)), Pt::new(0., -30.));
    }
}
