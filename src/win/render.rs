//! Dessin des formes et des effets de pointeur. Ne fait que lire l'état de l'application.

use super::gfx::{Align, Family, Painter};
use ekspointer::app::App;
use ekspointer::geom::{Pt, Rgba, PALETTE};
use ekspointer::pointer::{Button, Pointer};
use ekspointer::shapes::arrow::{arrow_geom, arrow_outline};
use ekspointer::shapes::stroke::catmull_rom_beziers;
use ekspointer::shapes::{Phase, Shape, ShapeKind};
use ekspointer::ui::theme;

/// Largeur du liseré de lisibilité, de chaque côté, en pixels logiques.
const OUTLINE: f32 = 1.5;
/// Largeur du surligneur.
const HIGHLIGHTER_W: f32 = 22.0;
/// Opacité du surligneur.
const HIGHLIGHTER_A: f32 = 0.35;
/// Diamètre d'une pastille de numéro.
const NUMBER_D: f32 = 32.0;

/// Une passe de dessin d'une forme : couleur, décalage et surépaisseur.
struct Pass {
    color: Rgba,
    dy: f32,
    grow: f32,
}

fn shift(p: Pt, origin: Pt, dy: f32) -> Pt {
    Pt::new(p.x - origin.x, p.y - origin.y + dy)
}

/// Dessine la géométrie d'une forme pour une passe donnée (coordonnées de la fenêtre).
fn draw_geometry(p: &Painter, s: &Shape, origin: Pt, pass: &Pass, appear: f32) {
    let k = s.scale;
    let w = s.width * k;
    let grow = pass.grow * k;
    let dy = pass.dy * k;
    match &s.kind {
        ShapeKind::Arrow { from, to } => {
            let (a, b) = (shift(*from, origin, dy), shift(*to, origin, dy));
            let mut o = arrow_outline(a, b, w);
            // Apparition : les ailes de la tête s'ouvrent de 60 % à 100 %.
            let open = 0.6 + 0.4 * appear;
            o[2] = o[1].lerp(o[2], open);
            o[4] = o[5].lerp(o[4], open);
            let radius = arrow_geom(a.dist(b), w).corner_radius;
            p.fill_poly(&o, pass.color);
            p.stroke_poly(&o, true, pass.color, radius + grow, true);
        }
        ShapeKind::Rect { a, b } => {
            let (a, b) = (shift(*a, origin, dy), shift(*b, origin, dy));
            let r = ekspointer::geom::RectF {
                x: a.x.min(b.x),
                y: a.y.min(b.y),
                w: (a.x - b.x).abs(),
                h: (a.y - b.y).abs(),
            };
            p.stroke_rrect(r, 6.0 * k, pass.color, w + grow);
        }
        ShapeKind::Ellipse { a, b } => {
            let (a, b) = (shift(*a, origin, dy), shift(*b, origin, dy));
            let center = a.lerp(b, 0.5);
            p.stroke_ellipse(center, (a.x - b.x).abs() / 2.0, (a.y - b.y).abs() / 2.0, pass.color, w + grow);
        }
        ShapeKind::Free { pts } => {
            let pts: Vec<Pt> = pts.iter().map(|q| shift(*q, origin, dy)).collect();
            if pts.len() == 1 {
                p.fill_circle(pts[0], (w + grow) / 2.0, pass.color);
            } else {
                p.stroke_beziers(&catmull_rom_beziers(&pts), pass.color, w + grow, true);
            }
        }
        ShapeKind::Highlighter { pts } => {
            let pts: Vec<Pt> = pts.iter().map(|q| shift(*q, origin, dy)).collect();
            if pts.len() >= 2 {
                p.stroke_beziers(&catmull_rom_beziers(&pts), pass.color, HIGHLIGHTER_W * k + grow, false);
            }
        }
        ShapeKind::Number { at, .. } => {
            let scale = 0.8 + 0.2 * appear;
            p.fill_circle(shift(*at, origin, dy), (NUMBER_D / 2.0 * k) * scale + grow / 2.0, pass.color);
        }
    }
}

fn draw_shape(p: &Painter, app: &App, s: &Shape, origin: Pt, now: u64, animate: bool) {
    let phase = app.scene.phase(s, now);
    let opacity = app.scene.opacity(s, now);
    // Animations désactivées dans Windows : la forme disparaît d'un coup, sans fondu.
    if opacity <= 0.0 || (!animate && matches!(phase, Phase::Fading(_))) {
        return;
    }
    let appear = match phase {
        Phase::Appearing(t) if animate => theme::ease(t),
        _ => 1.0,
    };
    let color = PALETTE[s.color as usize % PALETTE.len()];
    let is_white = s.color == 5;

    p.with_opacity(opacity, || {
        if matches!(s.kind, ShapeKind::Highlighter { .. }) {
            let pass = Pass { color: color.with_alpha(HIGHLIGHTER_A), dy: 0.0, grow: 0.0 };
            draw_geometry(p, s, origin, &pass, appear);
            return;
        }
        // Ombre douce teintée encre : deux passes larges et légères, décalées vers le bas.
        draw_geometry(p, s, origin, &Pass { color: theme::INK.with_alpha(0.07), dy: 2.5, grow: 9.0 }, appear);
        draw_geometry(p, s, origin, &Pass { color: theme::INK.with_alpha(0.12), dy: 1.5, grow: 5.0 }, appear);
        // Liseré de lisibilité : blanc, ou encre sous une forme blanche.
        let outline = if is_white { theme::INK.with_alpha(0.85) } else { theme::WHITE.with_alpha(0.85) };
        draw_geometry(p, s, origin, &Pass { color: outline, dy: 0.0, grow: 2.0 * OUTLINE }, appear);
        draw_geometry(p, s, origin, &Pass { color, dy: 0.0, grow: 0.0 }, appear);

        if let ShapeKind::Number { at, n } = &s.kind {
            let k = s.scale;
            let c = shift(*at, origin, 0.0);
            let half = NUMBER_D / 2.0 * k;
            let text_color = if s.color == 3 || is_white { theme::INK } else { theme::WHITE };
            let size = if *n >= 100 { 12.0 } else if *n >= 10 { 15.0 } else { 17.0 };
            p.text(
                &n.to_string(),
                Family::Mono,
                size * k * (0.8 + 0.2 * appear),
                600,
                Align::Center,
                ekspointer::geom::RectF { x: c.x - half, y: c.y - half, w: 2.0 * half, h: 2.0 * half },
                text_color,
            );
        }
    });
}

/// Dessine toutes les formes. `origin` est le coin haut-gauche de l'écran dans le bureau virtuel.
pub fn draw_scene(p: &Painter, origin: Pt, app: &App, now: u64, animate: bool) {
    for s in app.scene.shapes() {
        draw_shape(p, app, s, origin, now, animate);
    }
}

/// Dessine les effets de pointeur. `scale` est l'échelle de l'écran.
pub fn draw_pointer(p: &Painter, origin: Pt, scale: f32, app: &App, now: u64) {
    let pointer = &app.pointer;

    // Traînée laser : du plus ancien au plus récent, de plus en plus épaisse et opaque.
    let trail = pointer.trail(now);
    for pair in trail.windows(2) {
        let (a, age_a) = pair[0];
        let (b, age_b) = pair[1];
        let life = 1.0 - (age_a + age_b) / 2.0;
        let width = (1.0 + 5.0 * life) * scale;
        let (a, b) = (shift(a, origin, 0.0), shift(b, origin, 0.0));
        p.line(a, b, theme::EMBER.with_alpha(0.25 * life), width * 2.6, true);
        p.line(a, b, theme::EMBER.with_alpha(life), width, true);
    }

    // Halo.
    if app.cfg.halo_on {
        let radius = app.cfg.halo_size as f32 / 2.0 * scale;
        p.radial_glow(shift(pointer.pos(), origin, 0.0), radius, app.halo_color().with_alpha(0.35), 0.7);
    }

    // Ondes au clic.
    for r in pointer.ripples() {
        let (radius, opacity) = Pointer::ripple_state(r, now);
        let center = shift(r.at, origin, 0.0);
        let k = r.scale;
        match r.button {
            Button::Left => {
                let c = PALETTE[app.cfg.color as usize % PALETTE.len()];
                p.stroke_ellipse(center, radius * k, radius * k, c.with_alpha(opacity), 3.0 * k);
            }
            Button::Right => {
                let c = theme::BLUE.with_alpha(opacity);
                p.stroke_ellipse(center, radius * k, radius * k, c, 3.0 * k);
                p.stroke_ellipse(center, radius * 0.6 * k, radius * 0.6 * k, c, 3.0 * k);
            }
        }
    }
}
