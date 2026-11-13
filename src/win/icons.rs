//! Icônes de l'interface et logo, tracés en vectoriel sur une grille de 24.

use super::gfx::{Align, Family, Painter};
use ekspointer::geom::{Pt, RectF, Rgba};
use ekspointer::shapes::arrow::arrow_outline;
use ekspointer::shapes::stroke::catmull_rom_beziers;
use ekspointer::shapes::Tool;
use ekspointer::ui::theme;
use ekspointer::ui::toolbar::Item;

/// Repère d'une icône : place les coordonnées de la grille de 24 dans un rectangle.
struct Grid<'a> {
    p: &'a Painter<'a>,
    r: RectF,
    color: Rgba,
    stroke: f32,
}

impl Grid<'_> {
    fn k(&self) -> f32 {
        self.r.w / 24.0
    }

    fn at(&self, x: f32, y: f32) -> Pt {
        Pt::new(self.r.x + x * self.k(), self.r.y + y * self.k())
    }

    fn pts(&self, pts: &[(f32, f32)]) -> Vec<Pt> {
        pts.iter().map(|(x, y)| self.at(*x, *y)).collect()
    }

    fn line(&self, a: (f32, f32), b: (f32, f32)) {
        self.p.line(self.at(a.0, a.1), self.at(b.0, b.1), self.color, self.stroke, true);
    }

    fn poly(&self, pts: &[(f32, f32)], closed: bool) {
        self.p.stroke_poly(&self.pts(pts), closed, self.color, self.stroke, true);
    }

    fn curve(&self, pts: &[(f32, f32)]) {
        self.p.stroke_beziers(&catmull_rom_beziers(&self.pts(pts)), self.color, self.stroke, true);
    }

    fn ring(&self, c: (f32, f32), radius: f32, color: Rgba) {
        self.p.stroke_ellipse(self.at(c.0, c.1), radius * self.k(), radius * self.k(), color, self.stroke);
    }

    fn dot(&self, c: (f32, f32), radius: f32, color: Rgba) {
        self.p.fill_circle(self.at(c.0, c.1), radius * self.k(), color);
    }

    fn rrect(&self, x: f32, y: f32, w: f32, h: f32, radius: f32) {
        let a = self.at(x, y);
        let r = RectF { x: a.x, y: a.y, w: w * self.k(), h: h * self.k() };
        self.p.stroke_rrect(r, radius * self.k(), self.color, self.stroke);
    }
}

/// Dessine l'icône d'un élément de la barre dans `r` (carré), au trait de couleur `color`.
pub fn draw(p: &Painter, item: Item, r: RectF, color: Rgba) {
    let g = Grid { p, r, color, stroke: theme::ICON_STROKE * r.w / 24.0 * (24.0 / 22.0) };
    match item {
        Item::Tool(None) => {
            g.poly(&[(6.5, 3.5), (6.5, 18.0), (10.2, 14.6), (12.8, 20.2), (15.0, 19.2), (12.4, 13.7), (17.6, 13.7)], true);
        }
        Item::Tool(Some(Tool::Arrow)) => {
            g.line((5.5, 18.5), (18.0, 6.0));
            g.poly(&[(10.0, 6.0), (18.0, 6.0), (18.0, 14.0)], false);
        }
        Item::Tool(Some(Tool::Rect)) => g.rrect(4.0, 6.0, 16.0, 12.0, 2.5),
        Item::Tool(Some(Tool::Ellipse)) => {
            p.stroke_ellipse(g.at(12.0, 12.0), 8.5 * g.k(), 6.5 * g.k(), color, g.stroke);
        }
        Item::Tool(Some(Tool::Free)) => {
            g.curve(&[(3.5, 15.0), (6.5, 8.0), (10.0, 13.5), (13.5, 17.0), (16.5, 9.0), (20.5, 11.5)]);
        }
        Item::Tool(Some(Tool::Highlighter)) => {
            g.poly(&[(14.5, 4.0), (20.0, 9.5), (11.0, 18.5), (5.5, 18.5), (5.5, 13.0)], true);
            g.line((11.0, 7.5), (16.5, 13.0));
            g.line((4.0, 21.5), (13.0, 21.5));
        }
        Item::Tool(Some(Tool::Number)) => {
            g.ring((12.0, 12.0), 8.5, color);
            p.text("1", Family::Mono, 11.5 * g.k(), 600, Align::Center, r, color);
        }
        Item::Width => {
            for (y, w) in [(6.5, 1.2), (11.5, 2.4), (17.5, 3.8)] {
                p.line(g.at(4.5, y), g.at(19.5, y), color, w * g.k(), true);
            }
        }
        Item::Halo => {
            g.dot((12.0, 12.0), 3.2, color);
            g.ring((12.0, 12.0), 8.5, color);
        }
        Item::Ripple => {
            g.dot((12.0, 12.0), 1.8, color);
            g.ring((12.0, 12.0), 5.0, color);
            g.ring((12.0, 12.0), 9.0, color.with_alpha(color.a * 0.45));
        }
        Item::Trail => {
            g.dot((17.5, 6.5), 2.8, color);
            let pts = g.pts(&[(4.0, 19.0), (8.0, 16.5), (11.5, 13.0), (14.5, 9.5)]);
            for (i, pair) in pts.windows(2).enumerate() {
                let t = (i as f32 + 1.0) / 3.0;
                p.line(pair[0], pair[1], color.with_alpha(color.a * (0.25 + 0.75 * t)), (0.9 + 1.5 * t) * g.k(), true);
            }
        }
        Item::Pin => {
            g.poly(&[(9.0, 4.0), (15.0, 4.0), (14.2, 10.0), (17.0, 13.5), (7.0, 13.5), (9.8, 10.0)], true);
            g.line((12.0, 13.5), (12.0, 20.5));
        }
        Item::Clear => {
            g.line((4.5, 7.0), (19.5, 7.0));
            g.poly(&[(6.5, 7.0), (7.5, 20.0), (16.5, 20.0), (17.5, 7.0)], false);
            g.poly(&[(9.5, 7.0), (9.5, 4.0), (14.5, 4.0), (14.5, 7.0)], false);
            g.line((10.3, 11.0), (10.3, 16.5));
            g.line((13.7, 11.0), (13.7, 16.5));
        }
        Item::Settings => {
            for (y, knob) in [(6.5, 15.0), (12.0, 8.5), (17.5, 13.5)] {
                g.line((4.0, y), (20.0, y));
                g.dot((knob, y), 2.6, theme::BG);
                g.ring((knob, y), 2.6, color);
            }
        }
        Item::Help => {
            g.ring((12.0, 12.0), 8.5, color);
            p.text("?", Family::Mono, 12.0 * g.k(), 700, Align::Center, r, color);
        }
        Item::Quit => {
            g.line((6.5, 6.5), (17.5, 17.5));
            g.line((17.5, 6.5), (6.5, 17.5));
        }
        Item::Logo | Item::Color | Item::Separator => {}
    }
}

/// Logo Ekspointer : l'écran du logo Eksplodo, d'où décolle une flèche-pointeur, et son point ambre.
pub fn draw_logo(p: &Painter, r: RectF) {
    let g = Grid { p, r, color: theme::INK, stroke: 1.9 * r.w / 24.0 };
    // Écran et pied.
    g.rrect(2.5, 8.5, 19.0, 11.0, 2.2);
    g.line((12.0, 19.5), (12.0, 22.0));
    g.line((8.0, 22.3), (16.0, 22.3));
    // Nuages du décollage, au bas de l'écran.
    for (x, y, radius) in [(5.6, 17.4, 1.3), (8.2, 17.8, 1.1), (10.8, 17.4, 1.4), (13.6, 17.8, 1.1), (16.2, 17.5, 1.3), (18.6, 17.7, 1.1)] {
        g.dot((x, y), radius, theme::INK);
    }
    // Flèche qui décolle de l'écran : un fond clair la détache du cadre, puis la flèche encre.
    let (from, to) = (g.at(7.5, 16.0), g.at(18.8, 2.2));
    let w = 2.4 * g.k();
    let outline = arrow_outline(from, to, w);
    p.stroke_poly(&outline, true, theme::BG, 2.6 * g.k(), true);
    p.fill_poly(&outline, theme::INK);
    p.stroke_poly(&outline, true, theme::INK, 0.5 * g.k(), true);
    // Point ambre : l'étincelle du décollage.
    g.dot((7.2, 16.3), 2.1, theme::BG);
    g.dot((7.2, 16.3), 1.5, theme::EMBER);
}
