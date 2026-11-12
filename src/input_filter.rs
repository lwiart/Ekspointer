//! Filtre d'entrées : décide quelles touches et quels clics intercepter, et les
//! traduit en événements pour l'application. Logique pure, appelée par l'écoute bas niveau.

use crate::geom::{Pt, RectF};
use crate::pointer::Button;
use crate::shortcuts::{normalize_vk, Action, Bindings, KeyState, Mods, Shortcut};
use std::collections::HashSet;

pub const VK_ESCAPE: u32 = 0x1B;
pub const VK_LSHIFT: u32 = 0xA0;
pub const VK_RSHIFT: u32 = 0xA1;
pub const VK_LCONTROL: u32 = 0xA2;
pub const VK_RCONTROL: u32 = 0xA3;
pub const VK_LMENU: u32 = 0xA4;
pub const VK_RMENU: u32 = 0xA5;
pub const VK_LWIN: u32 = 0x5B;
pub const VK_RWIN: u32 = 0x5C;

/// Événement transmis à l'application.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum InputEvent {
    /// Un raccourci ponctuel a été déclenché.
    Action(Action),
    /// La combinaison de maintien vient d'être tenue.
    HoldStart,
    /// La combinaison de maintien n'est plus tenue.
    HoldEnd,
    /// État de la touche Maj.
    Shift(bool),
    PointerMove(Pt),
    Click(Pt, Button),
    StrokeBegin(Pt),
    StrokeMove(Pt),
    StrokeEnd,
    /// Raccourci saisi dans les réglages.
    Captured(Shortcut),
    CaptureCancelled,
}

/// Événement de souris brut.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MouseKind {
    Move,
    LeftDown,
    LeftUp,
    RightDown,
    RightUp,
}

/// Réponse du filtre : faut-il intercepter l'événement, et quoi transmettre.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct Decision {
    pub swallow: bool,
    pub events: Vec<InputEvent>,
}

/// Filtre d'entrées.
#[derive(Debug)]
pub struct Filter {
    bindings: Bindings,
    draw_mode: bool,
    exempt: Vec<RectF>,
    capturing: bool,
    /// Plus grande combinaison de touches de modification vue pendant la saisie.
    capture_mods: Mods,
    keys: KeyState,
    /// Touches (hors modification) actuellement enfoncées.
    down: HashSet<u32>,
    /// Touches dont l'appui a été intercepté : leur relâchement le sera aussi.
    swallowed: HashSet<u32>,
    hold_active: bool,
    stroke_active: bool,
    /// L'appui du bouton gauche a été intercepté : son relâchement le sera aussi.
    swallow_left_up: bool,
}

fn mod_count(m: Mods) -> u8 {
    m.ctrl as u8 + m.alt as u8 + m.shift as u8 + m.win as u8
}

impl Filter {
    pub fn new(bindings: Bindings) -> Filter {
        Filter {
            bindings,
            draw_mode: false,
            exempt: Vec::new(),
            capturing: false,
            capture_mods: Mods::default(),
            keys: KeyState::default(),
            down: HashSet::new(),
            swallowed: HashSet::new(),
            hold_active: false,
            stroke_active: false,
            swallow_left_up: false,
        }
    }

    pub fn set_bindings(&mut self, bindings: Bindings) {
        self.bindings = bindings;
    }

    pub fn set_draw_mode(&mut self, on: bool) {
        self.draw_mode = on;
    }

    /// Rectangles des fenêtres d'Ekspointer : les clics n'y sont jamais interceptés.
    pub fn set_exempt_rects(&mut self, rects: Vec<RectF>) {
        self.exempt = rects;
    }

    /// Active la saisie d'un raccourci : la prochaine combinaison est capturée.
    pub fn set_capturing(&mut self, on: bool) {
        self.capturing = on;
        self.capture_mods = Mods::default();
    }

    /// Vrai tant que la combinaison de maintien est tenue.
    pub fn hold_active(&self) -> bool {
        self.hold_active
    }

    fn end_hold(&mut self, events: &mut Vec<InputEvent>) {
        if !self.hold_active {
            return;
        }
        if self.stroke_active {
            self.stroke_active = false;
            events.push(InputEvent::StrokeEnd);
        }
        self.hold_active = false;
        events.push(InputEvent::HoldEnd);
    }

    /// Vrai si les touches de modification tenues sont exactement celles du maintien.
    fn hold_matches(&self) -> bool {
        let hold = self.bindings.get(Action::HoldDraw);
        hold.key.is_none() && !hold.mods.is_empty() && self.keys.mods() == hold.mods && self.down.is_empty()
    }

    fn set_modifier(&mut self, vk: u32, down: bool) -> bool {
        let slot = match vk {
            VK_LCONTROL => &mut self.keys.lctrl,
            VK_RCONTROL => &mut self.keys.rctrl,
            VK_LMENU => &mut self.keys.lalt,
            VK_RMENU => &mut self.keys.ralt,
            VK_LSHIFT | VK_RSHIFT | 0x10 => &mut self.keys.shift,
            VK_LWIN | VK_RWIN => &mut self.keys.win,
            _ => return false,
        };
        *slot = down;
        true
    }

    pub fn on_key(&mut self, vk: u32, down: bool) -> Decision {
        let mut d = Decision::default();
        let shift_before = self.keys.shift;
        let mods_before = self.keys.mods();

        if self.set_modifier(vk, down) {
            if self.keys.shift != shift_before {
                d.events.push(InputEvent::Shift(self.keys.shift));
            }
            if self.capturing {
                let now = self.keys.mods();
                if mod_count(now) > mod_count(self.capture_mods) {
                    self.capture_mods = now;
                }
                if !down && !self.capture_mods.is_empty() {
                    d.events.push(InputEvent::Captured(Shortcut { mods: self.capture_mods, key: None }));
                    self.set_capturing(false);
                }
                return d;
            }
            if self.keys.mods() != mods_before {
                if self.hold_matches() {
                    if !self.hold_active {
                        self.hold_active = true;
                        d.events.push(InputEvent::HoldStart);
                    }
                } else {
                    self.end_hold(&mut d.events);
                }
            }
            return d;
        }

        let vk = normalize_vk(vk);
        if !down {
            self.down.remove(&vk);
            d.swallow = self.swallowed.remove(&vk);
            return d;
        }
        if !self.down.insert(vk) {
            // Répétition clavier : même sort que l'appui initial, sans nouvel événement.
            d.swallow = self.swallowed.contains(&vk);
            return d;
        }

        if self.capturing {
            let mods = self.keys.mods();
            d.events.push(if vk == VK_ESCAPE && mods.is_empty() {
                InputEvent::CaptureCancelled
            } else {
                InputEvent::Captured(Shortcut { mods, key: Some(vk) })
            });
            self.set_capturing(false);
            self.swallowed.insert(vk);
            d.swallow = true;
            return d;
        }

        self.end_hold(&mut d.events);
        let pressed = Shortcut { mods: self.keys.mods(), key: Some(vk) };
        if let Some(action) = self.bindings.action_for(pressed, self.draw_mode) {
            d.events.push(InputEvent::Action(action));
            self.swallowed.insert(vk);
            d.swallow = true;
        }
        d
    }

    pub fn on_mouse(&mut self, kind: MouseKind, p: Pt) -> Decision {
        let mut d = Decision::default();
        let exempt = self.exempt.iter().any(|r| r.contains(p));
        match kind {
            MouseKind::Move => {
                d.events.push(InputEvent::PointerMove(p));
                if self.stroke_active {
                    d.events.push(InputEvent::StrokeMove(p));
                }
            }
            MouseKind::LeftDown => {
                self.swallow_left_up = false;
                if exempt {
                    // Clic sur une fenêtre d'Ekspointer : ni tracé ni onde.
                } else if self.hold_active {
                    self.stroke_active = true;
                    self.swallow_left_up = true;
                    d.swallow = true;
                    d.events.push(InputEvent::StrokeBegin(p));
                } else {
                    d.events.push(InputEvent::Click(p, Button::Left));
                }
            }
            MouseKind::LeftUp => {
                if self.swallow_left_up {
                    self.swallow_left_up = false;
                    d.swallow = true;
                    if self.stroke_active {
                        self.stroke_active = false;
                        d.events.push(InputEvent::StrokeEnd);
                    }
                }
            }
            MouseKind::RightDown => {
                if !exempt {
                    d.events.push(InputEvent::Click(p, Button::Right));
                }
            }
            MouseKind::RightUp => {}
        }
        d
    }

    /// Recale l'état des touches de modification sur l'état réel du clavier, pour
    /// rattraper un relâchement que l'écoute n'a pas vu. Ne démarre jamais un maintien.
    pub fn resync(&mut self, actual: KeyState) -> Vec<InputEvent> {
        let mut events = Vec::new();
        if actual == self.keys {
            return events;
        }
        if actual.shift != self.keys.shift {
            events.push(InputEvent::Shift(actual.shift));
        }
        self.keys = actual;
        if self.hold_active && !self.hold_matches() {
            self.end_hold(&mut events);
        }
        events
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn f() -> Filter {
        Filter::new(Bindings::defaults())
    }

    fn hold(f: &mut Filter) {
        f.on_key(0xA2, true);
        f.on_key(0xA4, true);
    }

    fn has(d: &Decision, e: InputEvent) -> bool {
        d.events.contains(&e)
    }

    #[test]
    fn hold_starts_and_ends() {
        let mut f = f();
        assert!(f.on_key(0xA2, true).events.is_empty());
        let d = f.on_key(0xA4, true);
        assert!(!d.swallow && has(&d, InputEvent::HoldStart));
        let d = f.on_key(0xA4, false);
        assert!(!d.swallow && has(&d, InputEvent::HoldEnd));
    }

    #[test]
    fn altgr_does_not_hold() {
        let mut f = f();
        f.on_key(0xA2, true);
        assert!(f.on_key(0xA5, true).events.is_empty());
    }

    #[test]
    fn extra_modifier_cancels_hold() {
        let mut f = f();
        hold(&mut f);
        assert!(has(&f.on_key(0xA0, true), InputEvent::HoldEnd)); // Ctrl+Alt+Maj n'est pas Ctrl+Alt
    }

    #[test]
    fn global_shortcut_fires_once_and_is_swallowed() {
        let mut f = f();
        hold(&mut f);
        let d = f.on_key(0x44, true);
        assert!(d.swallow && has(&d, InputEvent::Action(Action::ToggleDraw)) && has(&d, InputEvent::HoldEnd));
        let d = f.on_key(0x44, true); // répétition clavier
        assert!(d.swallow && d.events.is_empty());
        assert!(f.on_key(0x44, false).swallow); // relâchement avalé aussi
        assert!(!f.on_key(0x44, false).swallow);
    }

    #[test]
    fn hold_does_not_restart_after_a_shortcut_until_modifiers_change() {
        let mut f = f();
        hold(&mut f);
        f.on_key(0x44, true);
        assert!(f.on_key(0x44, false).events.is_empty());
        assert!(!f.on_mouse(MouseKind::LeftDown, Pt::new(1., 1.)).swallow);
    }

    #[test]
    fn letters_only_in_draw_mode() {
        let mut f = f();
        let d = f.on_key(0x46, true);
        assert!(!d.swallow && d.events.is_empty());
        f.on_key(0x46, false);
        f.set_draw_mode(true);
        let d = f.on_key(0x46, true);
        assert!(d.swallow && has(&d, InputEvent::Action(Action::ToolArrow)));
        let d = f.on_key(0x1B, true);
        assert!(d.swallow && has(&d, InputEvent::Action(Action::ExitDraw)));
        let d = f.on_key(0x61, true); // pavé numérique 1
        assert!(d.swallow && has(&d, InputEvent::Action(Action::Color(0))));
        let d = f.on_key(0x51, true); // touche sans action
        assert!(!d.swallow && d.events.is_empty());
    }

    #[test]
    fn shift_is_reported() {
        let mut f = f();
        assert!(has(&f.on_key(0xA0, true), InputEvent::Shift(true)));
        assert!(f.on_key(0xA0, true).events.is_empty()); // répétition
        assert!(has(&f.on_key(0xA0, false), InputEvent::Shift(false)));
    }

    #[test]
    fn hold_stroke_swallows_left_button_only() {
        let mut f = f();
        hold(&mut f);
        let d = f.on_mouse(MouseKind::LeftDown, Pt::new(10., 10.));
        assert!(d.swallow && has(&d, InputEvent::StrokeBegin(Pt::new(10., 10.))));
        assert!(!d.events.iter().any(|e| matches!(e, InputEvent::Click(..))));
        let d = f.on_mouse(MouseKind::Move, Pt::new(20., 10.));
        assert!(!d.swallow);
        assert!(has(&d, InputEvent::StrokeMove(Pt::new(20., 10.))) && has(&d, InputEvent::PointerMove(Pt::new(20., 10.))));
        let d = f.on_mouse(MouseKind::LeftUp, Pt::new(20., 10.));
        assert!(d.swallow && has(&d, InputEvent::StrokeEnd));
        let d = f.on_mouse(MouseKind::RightDown, Pt::new(20., 10.));
        assert!(!d.swallow && has(&d, InputEvent::Click(Pt::new(20., 10.), Button::Right)));
        assert!(!f.on_mouse(MouseKind::RightUp, Pt::new(20., 10.)).swallow);
    }

    #[test]
    fn releasing_hold_mid_stroke_ends_it_and_swallows_button_up() {
        let mut f = f();
        hold(&mut f);
        f.on_mouse(MouseKind::LeftDown, Pt::new(0., 0.));
        let d = f.on_key(0xA4, false);
        assert!(has(&d, InputEvent::StrokeEnd) && has(&d, InputEvent::HoldEnd));
        let d = f.on_mouse(MouseKind::Move, Pt::new(3., 3.));
        assert!(!d.events.iter().any(|e| matches!(e, InputEvent::StrokeMove(_))));
        assert!(f.on_mouse(MouseKind::LeftUp, Pt::new(5., 5.)).swallow);
        assert!(!f.on_mouse(MouseKind::LeftDown, Pt::new(5., 5.)).swallow);
    }

    #[test]
    fn clicks_pass_through_without_hold() {
        let mut f = f();
        let d = f.on_mouse(MouseKind::LeftDown, Pt::new(1., 1.));
        assert!(!d.swallow && has(&d, InputEvent::Click(Pt::new(1., 1.), Button::Left)));
        assert!(!f.on_mouse(MouseKind::LeftUp, Pt::new(1., 1.)).swallow);
        let d = f.on_mouse(MouseKind::Move, Pt::new(2., 2.));
        assert_eq!(d.events, vec![InputEvent::PointerMove(Pt::new(2., 2.))]);
    }

    #[test]
    fn exempt_rects_are_never_swallowed() {
        let mut f = f();
        f.set_exempt_rects(vec![RectF { x: 0., y: 0., w: 100., h: 56. }]);
        hold(&mut f);
        let d = f.on_mouse(MouseKind::LeftDown, Pt::new(50., 20.));
        assert!(!d.swallow && d.events.is_empty()); // ni tracé ni onde sur la barre d'outils
        assert!(!f.on_mouse(MouseKind::LeftUp, Pt::new(50., 20.)).swallow);
    }

    #[test]
    fn capture_mode_records_one_shortcut() {
        let mut f = f();
        f.set_capturing(true);
        assert!(f.on_key(0xA2, true).events.is_empty());
        assert!(f.on_key(0xA4, true).events.is_empty()); // pas de maintien pendant la saisie
        let d = f.on_key(0x58, true);
        assert!(d.swallow && has(&d, InputEvent::Captured("Ctrl+Alt+X".parse().unwrap())));
        assert!(f.on_key(0x58, false).swallow);
        f.on_key(0xA4, false);
        f.on_key(0xA2, false);
        // La saisie est terminée : un raccourci existant refonctionne.
        hold(&mut f);
        assert!(has(&f.on_key(0x44, true), InputEvent::Action(Action::ToggleDraw)));
    }

    #[test]
    fn capture_mode_escape_cancels() {
        let mut f = f();
        f.set_capturing(true);
        let d = f.on_key(0x1B, true);
        assert!(d.swallow && has(&d, InputEvent::CaptureCancelled));
    }

    #[test]
    fn capture_mode_swallows_existing_shortcuts_instead_of_firing_them() {
        let mut f = f();
        f.set_capturing(true);
        f.on_key(0xA2, true);
        f.on_key(0xA4, true);
        let d = f.on_key(0x44, true);
        assert!(d.swallow && !d.events.iter().any(|e| matches!(e, InputEvent::Action(_))));
    }

    #[test]
    fn capture_mode_accepts_modifier_only_on_release() {
        let mut f = f();
        f.set_capturing(true);
        f.on_key(0xA2, true);
        f.on_key(0xA0, true);
        let d = f.on_key(0xA0, false);
        assert!(has(&d, InputEvent::Captured("Ctrl+Maj".parse().unwrap())));
    }

    #[test]
    fn resync_clears_stuck_hold() {
        let mut f = f();
        hold(&mut f);
        f.on_mouse(MouseKind::LeftDown, Pt::new(0., 0.));
        let ev = f.resync(KeyState::default()); // relâchements manqués
        assert!(ev.contains(&InputEvent::StrokeEnd) && ev.contains(&InputEvent::HoldEnd));
        assert!(f.resync(KeyState::default()).is_empty());
        assert!(!f.on_mouse(MouseKind::LeftDown, Pt::new(1., 1.)).swallow);
    }

    #[test]
    fn resync_with_matching_state_changes_nothing() {
        let mut f = f();
        hold(&mut f);
        let ev = f.resync(KeyState { lctrl: true, lalt: true, ..Default::default() });
        assert!(ev.is_empty());
        assert!(f.on_mouse(MouseKind::LeftDown, Pt::new(1., 1.)).swallow);
    }

    #[test]
    fn rebinding_takes_effect() {
        let mut f = f();
        let mut b = Bindings::defaults();
        b.set(Action::HoldDraw, "Ctrl+Maj".parse().unwrap()).unwrap();
        f.set_bindings(b);
        f.on_key(0xA2, true);
        assert!(has(&f.on_key(0xA0, true), InputEvent::HoldStart));
    }
}
