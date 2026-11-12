//! Réglages : saisie d'un raccourci, durée d'effacement, épingle.

mod common;
use common::*;
use ekspointer::app::{Effect, UiCommand};
use ekspointer::input_filter::InputEvent;
use ekspointer::shapes::{Phase, Tool};
use ekspointer::shortcuts::Action;

const D: u32 = 0x44;
const F: u32 = 0x46;
const R: u32 = 0x52;
const X: u32 = 0x58;

/// Saisit un raccourci pour `action`, comme le fait le panneau de réglages.
fn capture(r: &mut Rig, action: Action, keys: &[u32]) {
    r.filter.set_capturing(true);
    r.captured.clear();
    r.chord(keys);
    if let Some(InputEvent::Captured(shortcut)) = r.captured.first().copied() {
        r.command(UiCommand::SetBinding(action, shortcut));
    }
}

#[test]
fn captured_shortcut_becomes_active() {
    let mut r = Rig::new();
    capture(&mut r, Action::ToggleDraw, &[LCTRL, LALT, X]);
    assert_eq!(r.app.cfg.bindings.get(Action::ToggleDraw).to_string(), "Ctrl+Alt+X");
    assert!(!r.app.draw_mode); // la saisie elle-même ne déclenche rien

    r.chord(&[LCTRL, LALT, D]); // l'ancien raccourci ne fait plus rien
    assert!(!r.app.draw_mode);
    r.chord(&[LCTRL, LALT, X]);
    assert!(r.app.draw_mode);
}

#[test]
fn conflicting_capture_keeps_old_binding() {
    let mut r = Rig::new();
    capture(&mut r, Action::ToolRect, &[F]);
    assert!(r.effects.contains(&Effect::BindingConflict(Action::ToolArrow)));

    r.chord(&[LCTRL, LALT, D]);
    r.tap(R);
    assert_eq!(r.app.tool, Tool::Rect);
    r.tap(F);
    assert_eq!(r.app.tool, Tool::Arrow);
}

#[test]
fn cancelled_capture_changes_nothing() {
    let mut r = Rig::new();
    capture(&mut r, Action::ToggleDraw, &[ESC]);
    assert_eq!(r.captured, vec![InputEvent::CaptureCancelled]);
    assert_eq!(r.app.cfg.bindings.get(Action::ToggleDraw).to_string(), "Ctrl+Alt+D");
}

#[test]
fn rebinding_the_hold_combination_works() {
    let mut r = Rig::new();
    capture(&mut r, Action::HoldDraw, &[LCTRL, LSHIFT]);
    assert_eq!(r.app.cfg.bindings.get(Action::HoldDraw).to_string(), "Ctrl+Maj");
    r.key(LCTRL, true);
    r.key(LSHIFT, true);
    assert!(r.app.hold);
}

#[test]
fn hold_duration_change_applies_to_next_shape() {
    let mut r = Rig::new();
    r.command(UiCommand::SetHoldSeconds(2));
    r.command(UiCommand::SelectTool(Some(Tool::Arrow)));
    r.overlay_stroke(0., 0., 200., 0.);
    r.advance(2100);
    assert!(matches!(r.phase(0), Phase::Fading(_)));
}

#[test]
fn pin_keeps_shapes_until_unpinned() {
    let mut r = Rig::new();
    r.command(UiCommand::TogglePin);
    r.command(UiCommand::SelectTool(Some(Tool::Arrow)));
    r.overlay_stroke(0., 0., 200., 0.);
    r.advance(60_000);
    assert_eq!(r.shape_count(), 1);
    assert!(matches!(r.phase(0), Phase::Holding));

    r.command(UiCommand::TogglePin);
    r.advance(4000);
    assert_eq!(r.shape_count(), 1);
    r.advance(1700);
    assert_eq!(r.shape_count(), 0);
}

#[test]
fn reset_bindings_restores_defaults_in_the_filter() {
    let mut r = Rig::new();
    capture(&mut r, Action::ToggleDraw, &[LCTRL, LALT, X]);
    r.command(UiCommand::ResetBindings);
    r.chord(&[LCTRL, LALT, D]);
    assert!(r.app.draw_mode);
}

const TAB: u32 = 0x09;

/// Saisit un double appui pour `action`, comme le fait le panneau de réglages.
fn capture_double(r: &mut Rig, action: Action, key: u32) {
    r.filter.set_capturing_double();
    r.captured.clear();
    r.tap(key);
    if let Some(InputEvent::Captured(shortcut)) = r.captured.first().copied() {
        r.command(UiCommand::SetBinding(action, shortcut));
    }
}

#[test]
fn double_tap_on_tab_toggles_draw_mode() {
    let mut r = Rig::new();
    capture_double(&mut r, Action::ToggleDraw, TAB);
    assert_eq!(r.app.cfg.bindings.get(Action::ToggleDraw).to_string(), "2×Tab");

    assert!(!r.tap_at(1000, TAB)); // le premier appui atteint l'application
    assert!(!r.app.draw_mode);
    assert!(r.tap_at(1200, TAB)); // le second est intercepté
    assert!(r.app.draw_mode);

    // Un Tab isolé ne fait rien ; deux Tab rapprochés ressortent du mode dessin.
    assert!(!r.tap_at(3000, TAB));
    assert!(r.app.draw_mode);
    r.tap_at(5000, TAB);
    r.tap_at(5150, TAB);
    assert!(!r.app.draw_mode);

    // L'ancien raccourci multi-touches ne fait plus rien.
    r.chord(&[LCTRL, LALT, D]);
    assert!(!r.app.draw_mode);
}

#[test]
fn double_tap_on_ctrl_toggles_and_altgr_does_not() {
    let mut r = Rig::new();
    capture_double(&mut r, Action::ToggleDraw, LCTRL);
    assert_eq!(r.app.cfg.bindings.get(Action::ToggleDraw).to_string(), "2×Ctrl");

    r.tap_at(1000, LCTRL);
    r.tap_at(1150, LCTRL);
    assert!(r.app.draw_mode);
    r.tap(ESC);
    assert!(!r.app.draw_mode);

    // AltGr tapé deux fois (Ctrl gauche + Alt droit) ne compte pas.
    for at in [3000u64, 3150] {
        r.filter.set_now(at);
        r.key(LCTRL, true);
        r.key(RALT, true);
        r.key(RALT, false);
        r.key(LCTRL, false);
    }
    assert!(!r.app.draw_mode);

    // Ctrl+C deux fois de suite non plus.
    for at in [5000u64, 5150] {
        r.filter.set_now(at);
        r.chord(&[LCTRL, 0x43]);
    }
    assert!(!r.app.draw_mode);
}

#[test]
fn double_tap_binding_survives_restart() {
    let mut r = Rig::new();
    capture_double(&mut r, Action::ToggleDraw, TAB);
    let json = serde_json::to_string(&r.app.cfg).unwrap();
    let cfg: ekspointer::config::Config = serde_json::from_str(&json).unwrap();
    let mut r2 = Rig::with_config(cfg);
    r2.tap_at(100, TAB);
    r2.tap_at(250, TAB);
    assert!(r2.app.draw_mode);
}
