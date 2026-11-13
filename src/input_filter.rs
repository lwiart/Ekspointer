//! Filtre d'entrées : décide quelles touches et quels clics intercepter, et les
//! traduit en événements pour l'application. Logique pure, appelée par l'écoute bas niveau.

use crate::geom::{Pt, RectF};
use crate::pointer::Button;
use crate::shortcuts::{
    double_tap_vk, is_double_modifier, normalize_vk, Action, Bindings, KeyState, Mods, Shortcut, DOUBLE_TAP_MS,
};
use std::collections::HashSet;

/// Délai sans appui ni répétition au bout duquel une touche interceptée est jugée relâchée.
pub const STALE_SWALLOWED_MS: u64 = 1500;

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
    /// La saisie en cours attend une touche seule, pour un double appui.
    capture_double: bool,
    /// Instant courant, fourni par l'appelant avant chaque événement.
    now: u64,
    /// Toutes les touches physiquement enfoncées.
    held: HashSet<u32>,
    /// Instant du dernier appui (ou de la dernière répétition) de chaque touche interceptée.
    swallowed_at: std::collections::HashMap<u32, u64>,
    /// Appui isolé en cours : touche et instant de l'appui.
    tap_cur: Option<(u32, u64)>,
    /// Dernier appui isolé terminé : touche et instant de l'appui.
    tap_prev: Option<(u32, u64)>,
    /// Plus grande combinaison de touches de modification vue pendant la saisie.
    capture_mods: Mods,
    keys: KeyState,
    /// Les deux touches Maj, suivies séparément : en relâcher une ne lève pas la contrainte.
    lshift: bool,
    rshift: bool,
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
            capture_double: false,
            now: 0,
            held: HashSet::new(),
            swallowed_at: std::collections::HashMap::new(),
            tap_cur: None,
            tap_prev: None,
            capture_mods: Mods::default(),
            keys: KeyState::default(),
            lshift: false,
            rshift: false,
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
        self.capture_double = false;
        self.capture_mods = Mods::default();
    }

    /// Active la saisie d'une touche seule, enregistrée comme double appui.
    pub fn set_capturing_double(&mut self) {
        self.set_capturing(true);
        self.capture_double = true;
    }

    /// Fixe l'instant courant (millisecondes), à appeler avant chaque événement clavier.
    pub fn set_now(&mut self, now: u64) {
        self.now = now;
    }

    /// Suit les appuis isolés pour reconnaître un double appui. Renvoie l'action à déclencher.
    ///
    /// Un appui est isolé si aucune autre touche n'est enfoncée entre son appui et son
    /// relâchement. Une touche ordinaire déclenche au second appui ; une touche de
    /// modification au second relâchement, pour ne pas confondre avec un raccourci ou AltGr.
    fn track_tap(&mut self, raw: u32, down: bool) -> Option<Action> {
        let Some(vk) = double_tap_vk(raw) else {
            if down {
                self.held.insert(raw);
            } else {
                self.held.remove(&raw);
            }
            self.tap_cur = None;
            self.tap_prev = None;
            return None;
        };
        let is_mod = is_double_modifier(vk);
        if down {
            if !self.held.insert(vk) {
                return None; // répétition clavier
            }
            if self.held.len() > 1 {
                self.tap_cur = None;
                self.tap_prev = None;
                return None;
            }
            if let Some((prev, t0)) = self.tap_prev {
                if prev != vk {
                    self.tap_prev = None;
                } else if !is_mod && self.now.saturating_sub(t0) <= DOUBLE_TAP_MS {
                    if let Some(action) = self.bindings.double_action_for(vk, self.draw_mode) {
                        self.tap_prev = None;
                        self.tap_cur = None;
                        return Some(action);
                    }
                }
            }
            self.tap_cur = Some((vk, self.now));
            None
        } else {
            self.held.remove(&vk);
            match self.tap_cur.take() {
                Some((cur, t)) if cur == vk => {
                    if is_mod {
                        if let Some((prev, t0)) = self.tap_prev {
                            if prev == vk && t.saturating_sub(t0) <= DOUBLE_TAP_MS {
                                if let Some(action) = self.bindings.double_action_for(vk, self.draw_mode) {
                                    self.tap_prev = None;
                                    return Some(action);
                                }
                            }
                        }
                    }
                    self.tap_prev = Some((vk, t));
                    None
                }
                _ => {
                    self.tap_prev = None;
                    None
                }
            }
        }
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
        if matches!(vk, VK_LSHIFT | VK_RSHIFT | 0x10) {
            if vk == VK_RSHIFT {
                self.rshift = down;
            } else {
                self.lshift = down;
            }
            self.keys.shift = self.lshift || self.rshift;
            return true;
        }
        let slot = match vk {
            VK_LCONTROL => &mut self.keys.lctrl,
            VK_RCONTROL => &mut self.keys.rctrl,
            VK_LMENU => &mut self.keys.lalt,
            VK_RMENU => &mut self.keys.ralt,
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

        // Double appui : hors saisie d'un raccourci.
        let was_held = double_tap_vk(vk).is_some_and(|v| self.held.contains(&v));
        let double_action = if self.capturing { None } else { self.track_tap(vk, down) };

        if self.set_modifier(vk, down) {
            if self.keys.shift != shift_before {
                d.events.push(InputEvent::Shift(self.keys.shift));
            }
            if let Some(action) = double_action {
                d.events.push(InputEvent::Action(action));
            }
            if self.capturing && self.capture_double {
                if down && !was_held {
                    if let Some(key) = double_tap_vk(vk) {
                        let shortcut = Shortcut { mods: Mods::default(), key: Some(key), double: true };
                        d.events.push(InputEvent::Captured(shortcut));
                        self.set_capturing(false);
                    }
                }
                return d;
            }
            if self.capturing {
                let now = self.keys.mods();
                if mod_count(now) > mod_count(self.capture_mods) {
                    self.capture_mods = now;
                }
                if !down && !self.capture_mods.is_empty() {
                    d.events.push(InputEvent::Captured(Shortcut { mods: self.capture_mods, key: None, double: false }));
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
            self.swallowed_at.remove(&vk);
            d.swallow = self.swallowed.remove(&vk);
            return d;
        }
        if !self.down.insert(vk) {
            // Répétition clavier : même sort que l'appui initial, sans nouvel événement.
            d.swallow = self.swallowed.contains(&vk);
            if d.swallow {
                self.swallowed_at.insert(vk, self.now);
            }
            return d;
        }

        if let Some(action) = double_action {
            self.end_hold(&mut d.events);
            d.events.push(InputEvent::Action(action));
            self.swallowed.insert(vk);
            self.swallowed_at.insert(vk, self.now);
            d.swallow = true;
            return d;
        }

        if self.capturing {
            let mods = self.keys.mods();
            d.events.push(if vk == VK_ESCAPE && (mods.is_empty() || self.capture_double) {
                InputEvent::CaptureCancelled
            } else if self.capture_double {
                InputEvent::Captured(Shortcut { mods: Mods::default(), key: Some(vk), double: true })
            } else {
                InputEvent::Captured(Shortcut { mods, key: Some(vk), double: false })
            });
            self.set_capturing(false);
            self.swallowed.insert(vk);
            self.swallowed_at.insert(vk, self.now);
            d.swallow = true;
            return d;
        }

        self.end_hold(&mut d.events);
        let pressed = Shortcut { mods: self.keys.mods(), key: Some(vk), double: false };
        if let Some(action) = self.bindings.action_for(pressed, self.draw_mode) {
            d.events.push(InputEvent::Action(action));
            self.swallowed.insert(vk);
            self.swallowed_at.insert(vk, self.now);
            d.swallow = true;
        }
        d
    }

    pub fn on_mouse(&mut self, kind: MouseKind, p: Pt) -> Decision {
        let mut d = Decision::default();
        let exempt = self.exempt.iter().any(|r| r.contains(p));
        if kind != MouseKind::Move {
            // Ctrl+clic deux fois de suite n'est pas un double appui sur Ctrl.
            self.tap_cur = None;
            self.tap_prev = None;
        }
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

    /// Oublie les touches que le filtre croit enfoncées et qui ne le sont plus, d'après l'état
    /// réel du clavier (`is_down` reçoit un code de touche virtuelle). Rattrape un relâchement
    /// que l'écoute n'a pas vu : écran de verrouillage, fenêtre d'une application élevée.
    pub fn prune_keys(&mut self, is_down: &dyn Fn(u32) -> bool) {
        // Les codes mémorisés sont canoniques : un chiffre peut venir du pavé numérique,
        // et Maj de l'une ou l'autre touche.
        let (now, swallowed_at) = (self.now, &self.swallowed_at);
        let really_down = |vk: u32| {
            // Une touche interceptée n'apparaît jamais comme enfoncée dans l'état du clavier :
            // elle est jugée tenue tant que son dernier appui ou sa dernière répétition est récent.
            if let Some(at) = swallowed_at.get(&vk) {
                return now.saturating_sub(*at) < STALE_SWALLOWED_MS;
            }
            is_down(vk)
                || ((0x30..=0x39).contains(&vk) && is_down(vk + 0x30))
                || (vk == VK_LSHIFT && is_down(VK_RSHIFT))
        };
        let before = self.held.len() + self.down.len();
        self.down.retain(|vk| really_down(*vk));
        self.swallowed.retain(|vk| really_down(*vk));
        self.held.retain(|vk| really_down(*vk));
        let swallowed = &self.swallowed;
        self.swallowed_at.retain(|vk, _| swallowed.contains(vk));
        if self.held.len() + self.down.len() != before {
            self.tap_cur = None;
            self.tap_prev = None;
        }
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
            self.lshift = actual.shift;
            self.rshift = actual.shift;
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

    fn double_tab() -> Filter {
        let mut b = Bindings::defaults();
        b.set(Action::ToggleDraw, "2×Tab".parse().unwrap()).unwrap();
        Filter::new(b)
    }

    fn key_at(f: &mut Filter, t: u64, vk: u32, down: bool) -> Decision {
        f.set_now(t);
        f.on_key(vk, down)
    }

    const TAB: u32 = 0x09;

    #[test]
    fn double_tap_fires_on_second_press() {
        let mut f = double_tab();
        let d = key_at(&mut f, 0, TAB, true);
        assert!(!d.swallow && d.events.is_empty()); // le premier appui atteint l'application
        assert!(!key_at(&mut f, 50, TAB, false).swallow);
        let d = key_at(&mut f, 200, TAB, true);
        assert!(d.swallow && has(&d, InputEvent::Action(Action::ToggleDraw)));
        assert!(key_at(&mut f, 250, TAB, false).swallow);
        // Un troisième appui isolé ne déclenche rien.
        let d = key_at(&mut f, 300, TAB, true);
        assert!(!d.swallow && d.events.is_empty());
    }

    #[test]
    fn double_tap_too_slow_does_nothing() {
        let mut f = double_tab();
        key_at(&mut f, 0, TAB, true);
        key_at(&mut f, 50, TAB, false);
        let d = key_at(&mut f, 500, TAB, true);
        assert!(!d.swallow && d.events.is_empty());
        key_at(&mut f, 550, TAB, false);
        // Cet appui tardif compte comme le premier d'une nouvelle paire.
        assert!(has(&key_at(&mut f, 700, TAB, true), InputEvent::Action(Action::ToggleDraw)));
    }

    #[test]
    fn double_tap_is_cancelled_by_another_key() {
        let mut f = double_tab();
        key_at(&mut f, 0, TAB, true);
        key_at(&mut f, 30, TAB, false);
        key_at(&mut f, 60, 0x41, true);
        key_at(&mut f, 90, 0x41, false);
        assert!(key_at(&mut f, 120, TAB, true).events.is_empty());
    }

    #[test]
    fn double_tap_ignores_keyboard_repeat() {
        let mut f = double_tab();
        key_at(&mut f, 0, TAB, true);
        assert!(key_at(&mut f, 100, TAB, true).events.is_empty()); // touche tenue : répétition
        assert!(key_at(&mut f, 200, TAB, true).events.is_empty());
    }

    fn double_ctrl() -> Filter {
        let mut b = Bindings::defaults();
        b.set(Action::ToggleDraw, "2×Ctrl".parse().unwrap()).unwrap();
        Filter::new(b)
    }

    #[test]
    fn double_tap_on_modifier_fires_on_second_release() {
        let mut f = double_ctrl();
        key_at(&mut f, 0, 0xA2, true);
        key_at(&mut f, 40, 0xA2, false);
        assert!(key_at(&mut f, 150, 0xA2, true).events.is_empty());
        let d = key_at(&mut f, 190, 0xA2, false);
        assert!(!d.swallow && has(&d, InputEvent::Action(Action::ToggleDraw)));
    }

    #[test]
    fn altgr_twice_is_not_a_double_ctrl() {
        let mut f = double_ctrl();
        let mut t = 0;
        for _ in 0..2 {
            for (vk, down) in [(0xA2, true), (0xA5, true), (0xA5, false), (0xA2, false)] {
                t += 20;
                assert!(key_at(&mut f, t, vk, down).events.iter().all(|e| !matches!(e, InputEvent::Action(_))));
            }
        }
    }

    #[test]
    fn ctrl_used_in_shortcuts_is_not_a_double_ctrl() {
        let mut f = double_ctrl();
        let mut t = 0;
        for _ in 0..2 {
            for (vk, down) in [(0xA2, true), (0x43, true), (0x43, false), (0xA2, false)] {
                t += 20;
                assert!(key_at(&mut f, t, vk, down).events.iter().all(|e| !matches!(e, InputEvent::Action(_))));
            }
        }
    }

    #[test]
    fn capture_double_mode_records_a_single_key() {
        let mut f = f();
        f.set_capturing_double();
        let d = f.on_key(TAB, true);
        assert!(d.swallow && has(&d, InputEvent::Captured("2×Tab".parse().unwrap())));
        f.set_capturing_double();
        let d = f.on_key(0xA2, true);
        assert!(has(&d, InputEvent::Captured("2×Ctrl".parse().unwrap())));
        f.set_capturing_double();
        assert!(has(&f.on_key(0x1B, true), InputEvent::CaptureCancelled));
        f.set_capturing_double();
        assert!(f.on_key(0xA5, true).events.is_empty()); // AltGr n'est pas une touche valable
    }

    #[test]
    fn stale_key_blocks_hold_until_pruned() {
        // Win+L : le relâchement de L part vers l'écran de verrouillage et n'est jamais vu.
        let mut f = f();
        f.on_key(0x4C, true);
        f.on_key(0xA2, true);
        assert!(f.on_key(0xA4, true).events.is_empty(), "une touche restée « enfoncée » empêche le maintien");
        f.on_key(0xA4, false);
        f.on_key(0xA2, false);

        f.prune_keys(&|_| false); // l'état réel du clavier : plus aucune touche enfoncée
        f.on_key(0xA2, true);
        assert!(has(&f.on_key(0xA4, true), InputEvent::HoldStart));
    }

    #[test]
    fn stale_swallowed_key_fires_again_after_pruning() {
        let mut f = f();
        f.set_draw_mode(true);
        assert!(has(&f.on_key(0x1B, true), InputEvent::Action(Action::ExitDraw)));
        // Relâchement manqué : sans élagage, l'appui suivant serait pris pour une répétition.
        assert!(f.on_key(0x1B, true).events.is_empty());
        f.set_now(10_000); // plus aucun appui ni répétition depuis longtemps
        f.prune_keys(&|_| false);
        let d = f.on_key(0x1B, true);
        assert!(d.swallow && has(&d, InputEvent::Action(Action::ExitDraw)));
    }

    #[test]
    fn pruning_keeps_keys_that_are_really_down() {
        let mut f = f();
        f.set_draw_mode(true);
        assert!(has(&f.on_key(0x46, true), InputEvent::Action(Action::ToolArrow)));
        f.prune_keys(&|vk| vk == 0x46);
        let d = f.on_key(0x46, true); // répétition clavier d'une touche toujours tenue
        assert!(d.swallow && d.events.is_empty());
        // Chiffre du pavé numérique tenu : son code canonique est celui de la rangée du haut.
        assert!(has(&f.on_key(0x61, true), InputEvent::Action(Action::Color(0))));
        f.prune_keys(&|vk| vk == 0x61);
        assert!(f.on_key(0x61, true).events.is_empty());
    }

    #[test]
    fn swallowed_key_still_held_is_not_pruned_from_keyboard_state() {
        // Windows ne marque pas comme enfoncée une touche que l'écoute a interceptée : l'état du
        // clavier ne prouve donc rien pour elle. Tant qu'elle se répète, elle est tenue.
        let mut f = f();
        f.on_key(0xA2, true);
        f.on_key(0xA4, true);
        f.set_now(1000);
        assert!(has(&f.on_key(0x44, true), InputEvent::Action(Action::ToggleDraw)));
        for t in [1500u64, 1530, 1560, 1590] {
            f.set_now(t);
            f.prune_keys(&|vk| vk == 0xA2 || vk == 0xA4); // D absente de l'état du clavier
            let d = f.on_key(0x44, true); // répétition clavier
            assert!(d.swallow && d.events.is_empty(), "la bascule ne doit pas se redéclencher à {t} ms");
        }
        f.prune_keys(&|vk| vk == 0xA2 || vk == 0xA4);
        assert!(f.on_key(0x44, false).swallow);
    }

    #[test]
    fn pruning_a_stale_key_restores_double_tap() {
        let mut f = double_ctrl();
        key_at(&mut f, 0, 0x4C, true); // L resté « enfoncé »
        f.prune_keys(&|_| false);
        key_at(&mut f, 1000, 0xA2, true);
        key_at(&mut f, 1040, 0xA2, false);
        key_at(&mut f, 1150, 0xA2, true);
        assert!(has(&key_at(&mut f, 1190, 0xA2, false), InputEvent::Action(Action::ToggleDraw)));
    }

    #[test]
    fn mouse_click_cancels_a_double_tap() {
        // Deux Ctrl+clic rapprochés ne sont pas un double appui sur Ctrl.
        let mut f = double_ctrl();
        key_at(&mut f, 0, 0xA2, true);
        f.on_mouse(MouseKind::LeftDown, Pt::new(5., 5.));
        f.on_mouse(MouseKind::LeftUp, Pt::new(5., 5.));
        key_at(&mut f, 40, 0xA2, false);
        key_at(&mut f, 150, 0xA2, true);
        f.on_mouse(MouseKind::LeftDown, Pt::new(5., 5.));
        f.on_mouse(MouseKind::LeftUp, Pt::new(5., 5.));
        let d = key_at(&mut f, 190, 0xA2, false);
        assert!(d.events.iter().all(|e| !matches!(e, InputEvent::Action(_))));
    }

    #[test]
    fn releasing_one_shift_keeps_the_constraint_while_the_other_is_held() {
        let mut f = f();
        assert!(has(&f.on_key(0xA0, true), InputEvent::Shift(true)));
        assert!(f.on_key(0xA1, true).events.is_empty());
        assert!(f.on_key(0xA0, false).events.is_empty()); // Maj droite toujours tenue
        assert!(has(&f.on_key(0xA1, false), InputEvent::Shift(false)));
    }

    #[test]
    fn digit_zero_selects_the_tenth_color() {
        let mut f = f();
        f.set_draw_mode(true);
        assert!(has(&f.on_key(0x30, true), InputEvent::Action(Action::Color(9))));
        assert!(has(&f.on_key(0x39, true), InputEvent::Action(Action::Color(8))));
    }
}
