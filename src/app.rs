//! État de l'application et transitions. Seul module qui modifie l'état.

use crate::config::Config;
use crate::geom::{Pt, Rgba, PALETTE, WIDTHS};
use crate::input_filter::InputEvent;
use crate::pointer::Pointer;
use crate::shapes::{Scene, Tool};
use crate::shortcuts::{Action, BindError, Bindings, Shortcut};
use crate::ui::theme;

/// Commande venue de l'interface (barre d'outils, réglages).
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum UiCommand {
    /// `Some(outil)` entre en mode dessin avec cet outil ; `None` (Curseur) en sort.
    SelectTool(Option<Tool>),
    SetColor(u8),
    SetWidth(u8),
    ToggleHalo,
    ToggleRipple,
    ToggleTrail,
    TogglePin,
    ClearAll,
    SetHoldSeconds(u8),
    SetHaloSize(u32),
    SetHaloFollowsColor(bool),
    SetBinding(Action, Shortcut),
    ResetBindings,
    SetHideFromCapture(bool),
    SetToolbarPos(i32, i32),
    SetToolbarCollapsed(bool),
}

/// Ce que la couche Windows doit faire après une transition.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Effect {
    /// Les fenêtres de dessin doivent capter (ou laisser passer) la souris.
    OverlayCapture(bool),
    /// Le filtre d'entrées doit être resynchronisé (mode dessin, raccourcis).
    FilterChanged,
    /// Les réglages doivent être enregistrés.
    ConfigChanged,
    /// La barre d'outils et les réglages doivent être redessinés.
    UiChanged,
    /// Le masquage au partage d'écran a changé.
    CaptureAffinityChanged,
    /// Le raccourci demandé est déjà pris par cette action.
    BindingConflict(Action),
    /// Le raccourci demandé ne convient pas à l'action ; le texte explique pourquoi.
    BindingRejected(&'static str),
}

/// État complet de l'application.
#[derive(Debug)]
pub struct App {
    pub cfg: Config,
    pub scene: Scene,
    pub pointer: Pointer,
    /// Outil courant, utilisé en mode dessin comme en touche maintenue.
    pub tool: Tool,
    /// Mode dessin (bascule) actif.
    pub draw_mode: bool,
    /// Combinaison de maintien tenue.
    pub hold: bool,
    pub shift: bool,
    /// Dernier point du tracé en cours, pour réappliquer la contrainte Maj.
    last_pt: Option<Pt>,
}

impl App {
    pub fn new(mut cfg: Config) -> App {
        cfg.sanitize();
        let mut scene = Scene::new(cfg.hold_seconds as u64 * 1000);
        scene.set_pinned(cfg.pinned, 0);
        App {
            tool: cfg.last_tool,
            cfg,
            scene,
            pointer: Pointer::new(),
            draw_mode: false,
            hold: false,
            shift: false,
            last_pt: None,
        }
    }

    /// Couleur du halo : couleur de dessin, ou ambre si le halo ne la suit pas.
    pub fn halo_color(&self) -> Rgba {
        if self.cfg.halo_follows_color {
            PALETTE[self.cfg.color as usize % PALETTE.len()]
        } else {
            theme::EMBER
        }
    }

    fn set_draw(&mut self, on: bool) -> Vec<Effect> {
        if on == self.draw_mode {
            return Vec::new();
        }
        if !on {
            self.scene.cancel();
            self.last_pt = None;
        }
        self.draw_mode = on;
        vec![Effect::OverlayCapture(on), Effect::FilterChanged, Effect::UiChanged]
    }

    fn select_tool(&mut self, tool: Tool) -> Vec<Effect> {
        self.tool = tool;
        self.cfg.last_tool = tool;
        vec![Effect::ConfigChanged, Effect::UiChanged]
    }

    fn set_color(&mut self, color: u8) -> Vec<Effect> {
        let color = color.min(PALETTE.len() as u8 - 1);
        if color == self.cfg.color {
            return Vec::new();
        }
        self.cfg.color = color;
        vec![Effect::ConfigChanged, Effect::UiChanged]
    }

    fn toggle_halo(&mut self) -> Vec<Effect> {
        self.cfg.halo_on = !self.cfg.halo_on;
        vec![Effect::ConfigChanged, Effect::UiChanged]
    }

    /// Traite un événement d'entrée. `scale_at` donne l'échelle de l'écran sous un point.
    pub fn input(&mut self, ev: InputEvent, now: u64, scale_at: &dyn Fn(Pt) -> f32) -> Vec<Effect> {
        match ev {
            InputEvent::Action(action) => match action {
                Action::ToggleDraw => self.set_draw(!self.draw_mode),
                Action::ExitDraw => self.set_draw(false),
                Action::ClearAll => {
                    self.scene.clear_all(now);
                    Vec::new()
                }
                Action::ToggleHalo => self.toggle_halo(),
                Action::ToolArrow => self.select_tool(Tool::Arrow),
                Action::ToolRect => self.select_tool(Tool::Rect),
                Action::ToolEllipse => self.select_tool(Tool::Ellipse),
                Action::ToolFree => self.select_tool(Tool::Free),
                Action::ToolHighlighter => self.select_tool(Tool::Highlighter),
                Action::ToolNumber => self.select_tool(Tool::Number),
                Action::Color(i) => self.set_color(i),
                Action::Undo => {
                    self.scene.undo();
                    Vec::new()
                }
                Action::HoldDraw => Vec::new(),
            },
            InputEvent::HoldStart => {
                self.hold = true;
                vec![Effect::UiChanged]
            }
            InputEvent::HoldEnd => {
                self.hold = false;
                if !self.draw_mode {
                    self.scene.release(now);
                    self.last_pt = None;
                }
                vec![Effect::UiChanged]
            }
            InputEvent::Shift(on) => {
                self.shift = on;
                if let Some(p) = self.last_pt {
                    self.scene.drag(p, on);
                }
                Vec::new()
            }
            InputEvent::PointerMove(p) => {
                self.pointer.on_move(p, now, self.cfg.trail_on);
                Vec::new()
            }
            InputEvent::Click(p, button) => {
                if self.cfg.ripple_on {
                    self.pointer.on_click(p, button, scale_at(p), now);
                }
                Vec::new()
            }
            InputEvent::StrokeBegin(p) => {
                if self.draw_mode || self.hold {
                    let width = WIDTHS[self.cfg.width as usize % WIDTHS.len()];
                    self.scene.begin(self.tool, p, self.cfg.color, width, scale_at(p), now);
                    self.last_pt = Some(p);
                }
                Vec::new()
            }
            InputEvent::StrokeMove(p) => {
                if self.last_pt.is_some() {
                    self.scene.drag(p, self.shift);
                    self.last_pt = Some(p);
                }
                Vec::new()
            }
            InputEvent::StrokeEnd => {
                self.scene.release(now);
                self.last_pt = None;
                Vec::new()
            }
            InputEvent::Captured(_) | InputEvent::CaptureCancelled => Vec::new(),
        }
    }

    /// Traite une commande de l'interface.
    pub fn command(&mut self, cmd: UiCommand, now: u64) -> Vec<Effect> {
        let saved = vec![Effect::ConfigChanged, Effect::UiChanged];
        match cmd {
            UiCommand::SelectTool(Some(tool)) => {
                let mut e = self.select_tool(tool);
                e.extend(self.set_draw(true));
                e
            }
            UiCommand::SelectTool(None) => self.set_draw(false),
            UiCommand::SetColor(c) => self.set_color(c),
            UiCommand::SetWidth(w) => {
                let w = w.min(WIDTHS.len() as u8 - 1);
                if w == self.cfg.width {
                    return Vec::new();
                }
                self.cfg.width = w;
                saved
            }
            UiCommand::ToggleHalo => self.toggle_halo(),
            UiCommand::ToggleRipple => {
                self.cfg.ripple_on = !self.cfg.ripple_on;
                saved
            }
            UiCommand::ToggleTrail => {
                self.cfg.trail_on = !self.cfg.trail_on;
                saved
            }
            UiCommand::TogglePin => {
                self.cfg.pinned = !self.cfg.pinned;
                self.scene.set_pinned(self.cfg.pinned, now);
                saved
            }
            UiCommand::ClearAll => {
                self.scene.clear_all(now);
                Vec::new()
            }
            UiCommand::SetHoldSeconds(s) => {
                // Une valeur inchangée ne réécrit pas le fichier (glissement d'un curseur).
                if s.clamp(2, 15) == self.cfg.hold_seconds {
                    return Vec::new();
                }
                self.cfg.hold_seconds = s.clamp(2, 15);
                self.scene.set_hold_ms(self.cfg.hold_seconds as u64 * 1000);
                saved
            }
            UiCommand::SetHaloSize(s) => {
                if s.clamp(40, 120) == self.cfg.halo_size {
                    return Vec::new();
                }
                self.cfg.halo_size = s.clamp(40, 120);
                saved
            }
            UiCommand::SetHaloFollowsColor(on) => {
                self.cfg.halo_follows_color = on;
                saved
            }
            UiCommand::SetBinding(action, shortcut) => match self.cfg.bindings.set(action, shortcut) {
                Ok(()) => vec![Effect::FilterChanged, Effect::ConfigChanged, Effect::UiChanged],
                Err(BindError::Conflict(other)) => vec![Effect::BindingConflict(other)],
                Err(BindError::Invalid(reason)) => vec![Effect::BindingRejected(reason)],
            },
            UiCommand::ResetBindings => {
                self.cfg.bindings = Bindings::defaults();
                vec![Effect::FilterChanged, Effect::ConfigChanged, Effect::UiChanged]
            }
            UiCommand::SetHideFromCapture(on) => {
                self.cfg.hide_from_capture = on;
                vec![Effect::ConfigChanged, Effect::UiChanged, Effect::CaptureAffinityChanged]
            }
            UiCommand::SetToolbarPos(x, y) => {
                self.cfg.toolbar_pos = Some((x, y));
                vec![Effect::ConfigChanged]
            }
            UiCommand::SetToolbarCollapsed(on) => {
                self.cfg.toolbar_collapsed = on;
                saved
            }
        }
    }

    /// Fait avancer les animations. Renvoie vrai tant qu'il faut continuer à dessiner.
    pub fn tick(&mut self, now: u64) -> bool {
        let scene = self.scene.tick(now);
        let pointer = self.pointer.tick(now);
        scene || pointer
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pointer::Button;
    use crate::shapes::{Phase, ShapeKind};

    fn app() -> App {
        App::new(Config::default())
    }

    fn one(_: Pt) -> f32 {
        1.0
    }

    #[test]
    fn toggle_enters_and_leaves_draw_mode() {
        let mut a = app();
        let e = a.input(InputEvent::Action(Action::ToggleDraw), 0, &one);
        assert!(a.draw_mode);
        assert!(e.contains(&Effect::OverlayCapture(true)) && e.contains(&Effect::FilterChanged) && e.contains(&Effect::UiChanged));
        let e = a.input(InputEvent::Action(Action::ExitDraw), 1, &one);
        assert!(!a.draw_mode && e.contains(&Effect::OverlayCapture(false)));
        assert!(a.input(InputEvent::Action(Action::ExitDraw), 2, &one).is_empty());
    }

    #[test]
    fn toolbar_tool_enters_draw_mode_and_cursor_leaves() {
        let mut a = app();
        let e = a.command(UiCommand::SelectTool(Some(Tool::Rect)), 0);
        assert!(a.draw_mode && a.tool == Tool::Rect && a.cfg.last_tool == Tool::Rect);
        assert!(e.contains(&Effect::OverlayCapture(true)) && e.contains(&Effect::ConfigChanged));
        a.command(UiCommand::SelectTool(None), 1);
        assert!(!a.draw_mode && a.tool == Tool::Rect);
    }

    #[test]
    fn tool_and_color_keys() {
        let mut a = app();
        a.input(InputEvent::Action(Action::ToggleDraw), 0, &one);
        a.input(InputEvent::Action(Action::ToolHighlighter), 1, &one);
        let e = a.input(InputEvent::Action(Action::Color(3)), 2, &one);
        assert_eq!(a.tool, Tool::Highlighter);
        assert_eq!(a.cfg.last_tool, Tool::Highlighter);
        assert_eq!(a.cfg.color, 3);
        assert!(e.contains(&Effect::ConfigChanged) && e.contains(&Effect::UiChanged));
    }

    #[test]
    fn stroke_uses_current_style_and_monitor_scale() {
        let mut a = app();
        a.command(UiCommand::SetColor(4), 0);
        a.command(UiCommand::SetWidth(2), 0);
        a.input(InputEvent::HoldStart, 0, &one);
        assert!(a.hold && !a.draw_mode);
        a.input(InputEvent::StrokeBegin(Pt::new(0., 0.)), 1, &|_| 1.5);
        a.input(InputEvent::StrokeMove(Pt::new(200., 0.)), 2, &one);
        a.input(InputEvent::StrokeEnd, 3, &one);
        let s = &a.scene.shapes()[0];
        assert_eq!((s.color, s.width, s.scale), (4, 8.0, 1.5));
        assert!(matches!(s.kind, ShapeKind::Arrow { .. }));
    }

    #[test]
    fn stroke_needs_draw_mode_or_hold() {
        let mut a = app();
        a.input(InputEvent::StrokeBegin(Pt::new(0., 0.)), 1, &one);
        a.input(InputEvent::StrokeMove(Pt::new(200., 0.)), 2, &one);
        a.input(InputEvent::StrokeEnd, 3, &one);
        assert!(a.scene.shapes().is_empty());
    }

    #[test]
    fn hold_end_finishes_stroke() {
        let mut a = app();
        a.input(InputEvent::HoldStart, 0, &one);
        a.input(InputEvent::StrokeBegin(Pt::new(0., 0.)), 1, &one);
        a.input(InputEvent::StrokeMove(Pt::new(100., 0.)), 2, &one);
        a.input(InputEvent::HoldEnd, 3, &one);
        assert!(!a.hold);
        let s = a.scene.shapes()[0].clone();
        assert!(!matches!(a.scene.phase(&s, 3), Phase::Drawing));
    }

    #[test]
    fn shift_constrains_current_stroke() {
        let mut a = app();
        a.input(InputEvent::Action(Action::ToggleDraw), 0, &one);
        a.input(InputEvent::Action(Action::ToolRect), 0, &one);
        a.input(InputEvent::Shift(true), 0, &one);
        a.input(InputEvent::StrokeBegin(Pt::new(0., 0.)), 1, &one);
        a.input(InputEvent::StrokeMove(Pt::new(40., 10.)), 2, &one);
        a.input(InputEvent::StrokeEnd, 3, &one);
        assert!(matches!(a.scene.shapes()[0].kind, ShapeKind::Rect { b, .. } if b == Pt::new(40., 40.)));
    }

    #[test]
    fn pressing_shift_mid_stroke_applies_at_once() {
        let mut a = app();
        a.input(InputEvent::Action(Action::ToggleDraw), 0, &one);
        a.input(InputEvent::Action(Action::ToolRect), 0, &one);
        a.input(InputEvent::StrokeBegin(Pt::new(0., 0.)), 1, &one);
        a.input(InputEvent::StrokeMove(Pt::new(40., 10.)), 2, &one);
        a.input(InputEvent::Shift(true), 3, &one);
        assert!(matches!(a.scene.shapes()[0].kind, ShapeKind::Rect { b, .. } if b == Pt::new(40., 40.)));
    }

    #[test]
    fn leaving_draw_mode_cancels_stroke_in_progress() {
        let mut a = app();
        a.input(InputEvent::Action(Action::ToggleDraw), 0, &one);
        a.input(InputEvent::StrokeBegin(Pt::new(0., 0.)), 1, &one);
        a.input(InputEvent::StrokeMove(Pt::new(100., 0.)), 2, &one);
        a.input(InputEvent::Action(Action::ExitDraw), 3, &one);
        assert!(a.scene.shapes().is_empty());
    }

    #[test]
    fn undo_and_clear_actions() {
        let mut a = app();
        a.input(InputEvent::Action(Action::ToggleDraw), 0, &one);
        for i in 0..2 {
            a.input(InputEvent::StrokeBegin(Pt::new(0., i as f32 * 50.)), 1, &one);
            a.input(InputEvent::StrokeMove(Pt::new(100., i as f32 * 50.)), 2, &one);
            a.input(InputEvent::StrokeEnd, 3, &one);
        }
        a.input(InputEvent::Action(Action::Undo), 4, &one);
        assert_eq!(a.scene.shapes().len(), 1);
        a.input(InputEvent::Action(Action::ClearAll), 5, &one);
        a.tick(300);
        assert!(a.scene.shapes().is_empty());
    }

    #[test]
    fn settings_commands_persist() {
        let mut a = app();
        let e = a.command(UiCommand::SetHoldSeconds(9), 0);
        assert!(e.contains(&Effect::ConfigChanged));
        assert_eq!(a.cfg.hold_seconds, 9);
        a.command(UiCommand::SetHoldSeconds(99), 0);
        assert_eq!(a.cfg.hold_seconds, 15);
        a.command(UiCommand::SetHaloSize(500), 0);
        assert_eq!(a.cfg.halo_size, 120);
        a.command(UiCommand::SetColor(42), 0);
        assert_eq!(a.cfg.color, 5);
        a.command(UiCommand::TogglePin, 0);
        assert!(a.cfg.pinned && a.scene.pinned());
        let e = a.command(UiCommand::ToggleHalo, 0);
        assert!(a.cfg.halo_on && e.contains(&Effect::ConfigChanged));
        let e = a.input(InputEvent::Action(Action::ToggleHalo), 0, &one);
        assert!(!a.cfg.halo_on && e.contains(&Effect::UiChanged) && e.contains(&Effect::ConfigChanged));
        let e = a.command(UiCommand::SetHideFromCapture(false), 0);
        assert!(!a.cfg.hide_from_capture && e.contains(&Effect::CaptureAffinityChanged));
        a.command(UiCommand::SetToolbarPos(300, 40), 0);
        a.command(UiCommand::SetToolbarCollapsed(true), 0);
        assert_eq!((a.cfg.toolbar_pos, a.cfg.toolbar_collapsed), (Some((300, 40)), true));
    }

    #[test]
    fn new_app_restores_state_from_config() {
        let mut c = Config::default();
        c.last_tool = Tool::Ellipse;
        c.pinned = true;
        c.hold_seconds = 3;
        let mut a = App::new(c);
        assert_eq!(a.tool, Tool::Ellipse);
        assert!(a.scene.pinned());
        a.command(UiCommand::TogglePin, 0);
        a.input(InputEvent::HoldStart, 0, &one);
        a.input(InputEvent::StrokeBegin(Pt::new(0., 0.)), 0, &one);
        a.input(InputEvent::StrokeMove(Pt::new(100., 100.)), 0, &one);
        a.input(InputEvent::StrokeEnd, 0, &one);
        let s = a.scene.shapes()[0].clone();
        assert!(matches!(a.scene.phase(&s, 3100), Phase::Fading(_)));
    }

    #[test]
    fn binding_conflict_is_reported_not_applied() {
        let mut a = app();
        let e = a.command(UiCommand::SetBinding(Action::ToolRect, "F".parse().unwrap()), 0);
        assert!(e.contains(&Effect::BindingConflict(Action::ToolArrow)));
        assert_eq!(a.cfg.bindings.get(Action::ToolRect).to_string(), "R");
        let e = a.command(UiCommand::SetBinding(Action::ToolRect, "X".parse().unwrap()), 0);
        assert!(e.contains(&Effect::FilterChanged) && e.contains(&Effect::ConfigChanged));
        let e = a.command(UiCommand::ResetBindings, 0);
        assert!(e.contains(&Effect::FilterChanged));
        assert_eq!(a.cfg.bindings, Bindings::defaults());
    }

    #[test]
    fn pointer_events_feed_effects_only_when_enabled() {
        let mut a = app();
        a.input(InputEvent::Click(Pt::new(1., 1.), Button::Left), 0, &one);
        assert!(a.pointer.ripples().is_empty());
        a.command(UiCommand::ToggleRipple, 0);
        a.input(InputEvent::Click(Pt::new(1., 1.), Button::Left), 1, &|_| 2.0);
        assert_eq!(a.pointer.ripples().len(), 1);
        assert_eq!(a.pointer.ripples()[0].scale, 2.0);
        a.input(InputEvent::PointerMove(Pt::new(7., 7.)), 2, &one);
        assert_eq!(a.pointer.pos(), Pt::new(7., 7.));
        assert!(a.pointer.trail(2).is_empty());
        a.command(UiCommand::ToggleTrail, 2);
        a.input(InputEvent::PointerMove(Pt::new(8., 8.)), 3, &one);
        assert_eq!(a.pointer.trail(3).len(), 1);
        assert!(a.tick(4));
    }

    #[test]
    fn halo_color_follows_setting() {
        let mut a = app();
        a.command(UiCommand::SetColor(1), 0);
        assert_eq!(a.halo_color(), PALETTE[1]);
        a.command(UiCommand::SetHaloFollowsColor(false), 0);
        assert_eq!(a.halo_color(), theme::EMBER);
    }

    #[test]
    fn invalid_binding_is_rejected_with_a_reason() {
        let mut a = app();
        let e = a.command(UiCommand::SetBinding(Action::ClearAll, "E".parse().unwrap()), 0);
        assert!(matches!(e.as_slice(), [Effect::BindingRejected(reason)] if !reason.is_empty()));
        assert_eq!(a.cfg.bindings.get(Action::ClearAll).to_string(), "Ctrl+Alt+E");
        let e = a.command(UiCommand::SetBinding(Action::HoldDraw, "Ctrl+Alt+X".parse().unwrap()), 0);
        assert!(matches!(e.as_slice(), [Effect::BindingRejected(_)]));
    }

    #[test]
    fn unchanged_setting_is_not_saved_again() {
        // Glisser un curseur sans changer de valeur ne doit pas réécrire le fichier à chaque mouvement.
        let mut a = app();
        assert!(a.command(UiCommand::SetHoldSeconds(5), 0).is_empty());
        assert!(a.command(UiCommand::SetHaloSize(64), 0).is_empty());
        assert!(a.command(UiCommand::SetWidth(1), 0).is_empty());
        assert!(a.command(UiCommand::SetColor(0), 0).is_empty());
        assert!(a.command(UiCommand::SetHoldSeconds(6), 0).contains(&Effect::ConfigChanged));
        assert!(a.command(UiCommand::SetHoldSeconds(99), 0).contains(&Effect::ConfigChanged));
        assert!(a.command(UiCommand::SetHoldSeconds(200), 0).is_empty()); // déjà à la borne
    }
}
