//! Types géométriques et couleurs partagés.

/// Point en pixels (physiques du bureau virtuel, ou logiques selon le contexte).
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct Pt {
    pub x: f32,
    pub y: f32,
}

impl Pt {
    pub const fn new(x: f32, y: f32) -> Pt {
        Pt { x, y }
    }

    pub fn dist(self, o: Pt) -> f32 {
        ((self.x - o.x).powi(2) + (self.y - o.y).powi(2)).sqrt()
    }

    pub fn lerp(self, o: Pt, t: f32) -> Pt {
        Pt::new(self.x + (o.x - self.x) * t, self.y + (o.y - self.y) * t)
    }
}

/// Rectangle : coin haut-gauche, largeur, hauteur.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct RectF {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl RectF {
    pub fn contains(&self, p: Pt) -> bool {
        p.x >= self.x && p.x < self.x + self.w && p.y >= self.y && p.y < self.y + self.h
    }

    pub fn center(&self) -> Pt {
        Pt::new(self.x + self.w / 2.0, self.y + self.h / 2.0)
    }
}

/// Couleur, composantes de 0 à 1, alpha non prémultiplié.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Rgba {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Rgba {
    pub const fn hex(rgb: u32) -> Rgba {
        Rgba {
            r: ((rgb >> 16) & 0xFF) as f32 / 255.0,
            g: ((rgb >> 8) & 0xFF) as f32 / 255.0,
            b: (rgb & 0xFF) as f32 / 255.0,
            a: 1.0,
        }
    }

    pub const fn with_alpha(self, a: f32) -> Rgba {
        Rgba { a, ..self }
    }
}

/// Couleurs de dessin, dans l'ordre des touches 1 à 9 puis 0 : les couleurs franches d'abord
/// (jaune par défaut), puis les autres.
pub const PALETTE: [Rgba; 10] = [
    Rgba::hex(0xFFC400), // jaune
    Rgba::hex(0xE5252A), // rouge
    Rgba::hex(0x1FA971), // vert
    Rgba::hex(0x0C659D), // bleu
    Rgba::hex(0x101418), // noir
    Rgba::hex(0xFFFFFF), // blanc
    Rgba::hex(0xF0631F), // ambre Eksplodo
    Rgba::hex(0x003C58), // encre Eksplodo
    Rgba::hex(0x8B3FD9), // violet
    Rgba::hex(0xF0489B), // rose
];

/// Indice du blanc dans la palette : il reçoit un liseré foncé au lieu d'un liseré clair.
pub const WHITE_INDEX: u8 = 5;

/// Vrai pour une couleur claire (jaune, blanc) : le texte posé dessus doit être foncé.
pub fn is_light(color: u8) -> bool {
    matches!(color, 0 | 5)
}

/// Épaisseurs de trait, en pixels logiques.
pub const WIDTHS: [f32; 3] = [3.0, 5.0, 8.0];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_parses_channels() {
        let c = Rgba::hex(0xF0631F);
        assert!((c.r - 240.0 / 255.0).abs() < 1e-6);
        assert!((c.g - 99.0 / 255.0).abs() < 1e-6);
        assert!((c.b - 31.0 / 255.0).abs() < 1e-6);
        assert_eq!(c.a, 1.0);
    }

    #[test]
    fn palette_order_matches_spec() {
        // Les couleurs franches d'abord : jaune, rouge, vert, bleu, noir, blanc.
        assert_eq!(PALETTE[0], Rgba::hex(0xFFC400));
        assert_eq!(PALETTE[1], Rgba::hex(0xE5252A));
        assert_eq!(PALETTE[2], Rgba::hex(0x1FA971));
        assert_eq!(PALETTE[3], Rgba::hex(0x0C659D));
        assert_eq!(PALETTE[4], Rgba::hex(0x101418));
        assert_eq!(PALETTE[5], Rgba::hex(0xFFFFFF));
        // Puis les autres : ambre et encre Eksplodo, violet, rose.
        assert_eq!(PALETTE[6], Rgba::hex(0xF0631F));
        assert_eq!(PALETTE[7], Rgba::hex(0x003C58));
        assert_eq!(PALETTE[8], Rgba::hex(0x8B3FD9));
        assert_eq!(PALETTE[9], Rgba::hex(0xF0489B));
        assert_eq!(PALETTE.len(), 10);
        // Couleurs claires : le texte posé dessus est foncé.
        let light: Vec<usize> = (0..PALETTE.len()).filter(|i| is_light(*i as u8)).collect();
        assert_eq!(light, vec![0, 5]);
        assert_eq!(WIDTHS, [3.0, 5.0, 8.0]);
    }

    #[test]
    fn dist_and_contains() {
        assert_eq!(Pt::new(0., 0.).dist(Pt::new(3., 4.)), 5.0);
        let r = RectF { x: 0., y: 0., w: 10., h: 10. };
        assert!(r.contains(Pt::new(5., 5.)));
        assert!(!r.contains(Pt::new(10.5, 5.)));
    }

    #[test]
    fn lerp_and_alpha() {
        assert_eq!(Pt::new(0., 0.).lerp(Pt::new(10., 20.), 0.5), Pt::new(5., 10.));
        assert_eq!(Rgba::hex(0xFFFFFF).with_alpha(0.35).a, 0.35);
    }
}
