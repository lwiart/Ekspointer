//! Mode dessin (bascule) : du raccourci clavier à la disparition de la forme.

mod common;
use common::*;
use ekspointer::app::Effect;
use ekspointer::shapes::{Phase, ShapeKind, Tool};

const D: u32 = 0x44;
const E: u32 = 0x45;
const F: u32 = 0x46;
const R: u32 = 0x52;
const Z: u32 = 0x5A;

#[test]
fn toggle_draw_arrow_then_fade() {
    let mut r = Rig::new();
    r.chord(&[LCTRL, LALT, D]);
    assert!(r.app.draw_mode);
    assert!(r.effects.contains(&Effect::OverlayCapture(true)));

    r.overlay_stroke(100., 100., 400., 100.);
    assert_eq!(r.shape_count(), 1);
    assert!(matches!(r.kind(0), ShapeKind::Arrow { .. }));

    assert!(r.advance(100)); // apparition
    assert!(!r.advance(2000)); // maintien : aucune animation
    r.advance(3000); // 5,1 s après le relâchement
    assert!(matches!(r.phase(0), Phase::Fading(_)));
    r.advance(700);
    assert_eq!(r.shape_count(), 0);
}

#[test]
fn keys_change_tool_and_color_only_in_draw_mode() {
    let mut r = Rig::new();
    assert!(!r.tap(R));
    assert_eq!(r.app.tool, Tool::Arrow);

    r.chord(&[LCTRL, LALT, D]);
    assert!(r.tap(R));
    assert!(r.tap(0x34)); // touche 4
    assert_eq!(r.app.tool, Tool::Rect);
    assert_eq!(r.app.cfg.color, 3);

    r.overlay_stroke(0., 0., 200., 120.);
    let s = &r.app.scene.shapes()[0];
    assert!(matches!(s.kind, ShapeKind::Rect { .. }));
    assert_eq!(s.color, 3);
}

#[test]
fn escape_returns_to_normal_and_letters_pass_again() {
    let mut r = Rig::new();
    r.chord(&[LCTRL, LALT, D]);
    assert!(r.tap(F));
    assert!(r.tap(ESC));
    assert!(!r.app.draw_mode);
    assert!(r.effects.contains(&Effect::OverlayCapture(false)));
    assert!(!r.tap(F));
    assert!(!r.tap(ESC));
}

#[test]
fn toggle_shortcut_also_leaves_draw_mode() {
    let mut r = Rig::new();
    r.chord(&[LCTRL, LALT, D]);
    r.chord(&[LCTRL, LALT, D]);
    assert!(!r.app.draw_mode);
    assert!(!r.tap(F));
}

#[test]
fn undo_and_clear_all_shortcuts() {
    let mut r = Rig::new();
    r.chord(&[LCTRL, LALT, D]);
    for i in 0..3 {
        r.overlay_stroke(0., i as f32 * 60., 300., i as f32 * 60.);
    }
    assert_eq!(r.shape_count(), 3);
    r.chord(&[LCTRL, Z]);
    assert_eq!(r.shape_count(), 2);
    r.chord(&[LCTRL, LALT, E]);
    r.advance(250);
    assert_eq!(r.shape_count(), 0);
}

#[test]
fn shift_key_constrains_arrow_angle() {
    let mut r = Rig::new();
    r.chord(&[LCTRL, LALT, D]);
    r.key(LSHIFT, true);
    r.overlay_stroke(0., 0., 300., 20.); // 3,8° ramené à 0°
    r.key(LSHIFT, false);
    assert!(matches!(r.kind(0), ShapeKind::Arrow { to, .. } if to.y.abs() < 1e-3));
}

#[test]
fn numbers_increment_across_clicks() {
    let mut r = Rig::new();
    r.chord(&[LCTRL, LALT, D]);
    r.tap(0x4E); // N
    for i in 0..3 {
        r.overlay_stroke(i as f32 * 80., 50., i as f32 * 80., 50.);
    }
    let ns: Vec<u32> = (0..3)
        .map(|i| match r.kind(i) {
            ShapeKind::Number { n, .. } => *n,
            _ => 0,
        })
        .collect();
    assert_eq!(ns, vec![1, 2, 3]);
}
