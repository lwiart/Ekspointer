//! Dessin en touche maintenue : la souris ne trace que pendant que Ctrl+Alt est tenu.

mod common;
use common::*;
use ekspointer::app::UiCommand;
use ekspointer::input_filter::MouseKind;
use ekspointer::shapes::{Phase, ShapeKind, Tool};
use ekspointer::shortcuts::KeyState;

#[test]
fn hold_draws_without_entering_draw_mode() {
    let mut r = Rig::new();
    r.key(LCTRL, true);
    r.key(LALT, true);
    assert!(r.app.hold);

    assert!(r.mouse(MouseKind::LeftDown, 100., 100.));
    assert!(!r.mouse(MouseKind::Move, 250., 180.));
    assert!(r.mouse(MouseKind::LeftUp, 250., 180.));
    r.key(LALT, false);
    r.key(LCTRL, false);

    assert!(!r.app.draw_mode && !r.app.hold);
    assert_eq!(r.shape_count(), 1);
    assert!(matches!(r.kind(0), ShapeKind::Arrow { .. }));
    assert!(!matches!(r.phase(0), Phase::Drawing));
    // Après le maintien, un clic ordinaire n'est plus intercepté.
    assert!(!r.mouse(MouseKind::LeftDown, 10., 10.));
}

#[test]
fn altgr_typing_never_draws() {
    let mut r = Rig::new();
    r.key(LCTRL, true); // AltGr = Ctrl gauche + Alt droit
    r.key(RALT, true);
    assert!(!r.app.hold);
    assert!(!r.mouse(MouseKind::LeftDown, 100., 100.));
    r.mouse(MouseKind::Move, 300., 300.);
    assert!(!r.mouse(MouseKind::LeftUp, 300., 300.));
    assert!(!r.tap(0x30)); // AltGr+0 = @ : la touche atteint l'application
    r.key(RALT, false);
    r.key(LCTRL, false);
    assert_eq!(r.shape_count(), 0);
    assert!(!r.app.draw_mode);
}

#[test]
fn hold_uses_last_tool_chosen_in_toolbar() {
    let mut r = Rig::new();
    r.command(UiCommand::SelectTool(Some(Tool::Ellipse)));
    r.command(UiCommand::SelectTool(None));
    assert!(!r.app.draw_mode);

    r.key(LCTRL, true);
    r.key(LALT, true);
    r.mouse(MouseKind::LeftDown, 0., 0.);
    r.mouse(MouseKind::Move, 200., 100.);
    r.mouse(MouseKind::LeftUp, 200., 100.);
    assert!(matches!(r.kind(0), ShapeKind::Ellipse { .. }));
}

#[test]
fn releasing_keys_mid_stroke_finishes_the_shape() {
    let mut r = Rig::new();
    r.key(LCTRL, true);
    r.key(LALT, true);
    r.mouse(MouseKind::LeftDown, 0., 0.);
    r.mouse(MouseKind::Move, 200., 0.);
    r.key(LALT, false);
    assert_eq!(r.shape_count(), 1);
    assert!(!matches!(r.phase(0), Phase::Drawing));
    // Le relâchement du bouton, dont l'appui avait été intercepté, l'est aussi.
    assert!(r.mouse(MouseKind::LeftUp, 200., 0.));
    r.mouse(MouseKind::Move, 400., 400.);
    assert!(matches!(r.kind(0), ShapeKind::Arrow { to, .. } if to.x == 200.0));
}

#[test]
fn stuck_modifier_is_recovered() {
    let mut r = Rig::new();
    r.key(LCTRL, true);
    r.key(LALT, true);
    r.mouse(MouseKind::LeftDown, 0., 0.);
    r.mouse(MouseKind::Move, 150., 0.);

    // Les relâchements n'ont pas été vus : l'état réel du clavier fait foi.
    let events = r.filter.resync(KeyState::default());
    r.feed(events);

    assert!(!r.app.hold);
    assert_eq!(r.shape_count(), 1);
    assert!(!matches!(r.phase(0), Phase::Drawing));
    assert!(!r.mouse(MouseKind::LeftDown, 5., 5.));
}

#[test]
fn hold_shape_fades_like_any_other() {
    let mut r = Rig::new();
    r.key(LCTRL, true);
    r.key(LALT, true);
    r.mouse(MouseKind::LeftDown, 0., 0.);
    r.mouse(MouseKind::Move, 200., 0.);
    r.mouse(MouseKind::LeftUp, 200., 0.);
    r.key(LALT, false);
    r.key(LCTRL, false);
    r.advance(5700);
    assert_eq!(r.shape_count(), 0);
}
