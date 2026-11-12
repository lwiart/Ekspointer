//! État de l'application et transitions. Seul module qui modifie l'état.

use crate::config::Config;
use crate::geom::{Pt, Rgba, PALETTE, WIDTHS};
use crate::input_filter::InputEvent;
use crate::pointer::Pointer;
use crate::shapes::{Scene, Tool};
use crate::shortcuts::{Action, Bindings, Shortcut};
use crate::ui::theme;

// IMPL

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
}
