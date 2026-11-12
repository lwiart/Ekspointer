//! Panneau de réglages : état d'affichage, réactions à la souris et dessin.

use super::gfx::{Align, Family, Gfx, Painter};
use super::panel::{Panel, PanelEvent, PanelId};
use super::ui::SURFACE;
use ekspointer::app::{App, UiCommand};
use ekspointer::geom::{Pt, RectF, Rgba, WIDTHS};
use ekspointer::shortcuts::Action;
use ekspointer::ui::settings::{self, Row, HEADER_H, PAD, PANEL_H, PANEL_W};
use ekspointer::ui::theme;
use windows::core::Result;

/// Couleur des messages d'alerte (ambre foncé du design system, lisible en petit corps).
const WARN: Rgba = Rgba::hex(0xBB450D);
/// Durée d'affichage d'un message de conflit, en millisecondes.
pub const CONFLICT_MS: u64 = 4000;

/// Ce que le panneau demande à la boucle principale.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum SettingsAction {
    Command(UiCommand),
    /// Commencer la saisie d'un raccourci pour cette action (double appui ou combinaison).
    Capture { action: Action, double: bool },
    CancelCapture,
    Autostart(bool),
    Close,
}

/// Panneau de réglages.
pub struct SettingsView {
    pub panel: Panel,
    scroll: f32,
    hover: Option<Row>,
    hover_close: bool,
    /// Curseur en cours de glissement.
    dragging: Option<Row>,
    /// Action dont le raccourci est en cours de saisie, et type de saisie.
    pub capturing: Option<(Action, bool)>,
    /// Message de conflit : ligne concernée, texte, instant d'expiration.
    conflict: Option<(Action, String, u64)>,
    autostart: bool,
    pub dirty: bool,
}

fn close_rect() -> RectF {
    RectF { x: PANEL_W - PAD - 30.0, y: (HEADER_H - 32.0) / 2.0, w: 32.0, h: 32.0 }
}

/// Glissière d'un curseur : la zone de contrôle moins la place de la valeur affichée.
fn track_rect(row: RectF) -> RectF {
    let c = settings::control_rect(row);
    RectF { x: c.x + 8.0, y: c.y, w: c.w - 62.0, h: c.h }
}

fn switch_rect(row: RectF) -> RectF {
    RectF { x: row.x + row.w - 40.0, y: row.y + (row.h - 22.0) / 2.0, w: 40.0, h: 22.0 }
}

fn width_rect(row: RectF) -> RectF {
    let c = settings::control_rect(row);
    RectF { x: c.x + c.w - 126.0, y: c.y, w: 126.0, h: c.h }
}

fn reset_rect(row: RectF) -> RectF {
    RectF { x: row.x, y: row.y + 12.0, w: 262.0, h: 32.0 }
}

impl SettingsView {
    pub fn new(gfx: &Gfx) -> Result<SettingsView> {
        Ok(SettingsView {
            panel: Panel::new(gfx, PanelId::Settings, (PANEL_W, PANEL_H), theme::R_PANEL)?,
            scroll: 0.0,
            hover: None,
            hover_close: false,
            dragging: None,
            capturing: None,
            conflict: None,
            autostart: false,
            dirty: true,
        })
    }

    pub fn visible(&self) -> bool {
        self.panel.visible()
    }

    /// Ouvre le panneau au centre d'un écran.
    pub fn open(&mut self, gfx: &Gfx, monitor: RectF, scale: f32, autostart: bool, appear: Option<u64>) -> Result<()> {
        self.autostart = autostart;
        if let Some(now) = appear {
            self.panel.start_appear(now);
        }
        self.hover = None;
        self.hover_close = false;
        self.dragging = None;
        self.dirty = true;
        let x = monitor.x + (monitor.w - PANEL_W * scale) / 2.0;
        let y = monitor.y + ((monitor.h - PANEL_H * scale) / 2.0).max(0.0);
        self.panel.place(gfx, (x as i32, y as i32), (PANEL_W, PANEL_H), theme::R_PANEL, scale)
    }

    pub fn close(&mut self) {
        self.panel.hide();
        self.capturing = None;
        self.conflict = None;
    }

    pub fn set_autostart(&mut self, on: bool) {
        self.autostart = on;
        self.dirty = true;
    }

    /// Affiche un message de conflit sous la ligne d'un raccourci.
    pub fn show_conflict(&mut self, action: Action, message: String, until: u64) {
        self.conflict = Some((action, message, until));
        self.dirty = true;
    }

    /// Retire le message de conflit quand son temps est écoulé.
    pub fn expire(&mut self, now: u64) {
        if self.conflict.as_ref().is_some_and(|(_, _, until)| now >= *until) {
            self.conflict = None;
            self.dirty = true;
        }
    }

    fn row_rect(&self, row: Row) -> Option<RectF> {
        settings::layout(self.scroll).into_iter().find(|(r, _)| *r == row).map(|(_, r)| r)
    }

    fn slider_command(&self, row: Row, x: f32) -> Option<SettingsAction> {
        let track = track_rect(self.row_rect(row)?);
        Some(SettingsAction::Command(match row {
            Row::HoldSeconds => UiCommand::SetHoldSeconds(settings::slider_value(track, x, 2, 15) as u8),
            Row::HaloSize => UiCommand::SetHaloSize(settings::slider_value(track, x, 40, 120) as u32),
            _ => return None,
        }))
    }

    /// Traite un événement de souris du panneau.
    pub fn on_event(&mut self, ev: PanelEvent, app: &App) -> Vec<SettingsAction> {
        let mut out = Vec::new();
        self.dirty = true;
        match ev {
            PanelEvent::Wheel(delta) => self.scroll = settings::clamp_scroll(self.scroll - delta * 54.0),
            PanelEvent::Leave => {
                self.hover = None;
                self.hover_close = false;
            }
            PanelEvent::Move(p) => {
                let p = self.panel.to_dip(p);
                if let Some(row) = self.dragging {
                    out.extend(self.slider_command(row, p.x));
                } else {
                    self.hover = settings::hit(self.scroll, p);
                    self.hover_close = close_rect().contains(p);
                }
            }
            PanelEvent::Down(p) => {
                let p = self.panel.to_dip(p);
                if let Some(row @ (Row::HoldSeconds | Row::HaloSize)) = settings::hit(self.scroll, p) {
                    self.dragging = Some(row);
                    out.extend(self.slider_command(row, p.x));
                }
            }
            PanelEvent::Up(p) => {
                let p = self.panel.to_dip(p);
                if self.dragging.take().is_some() {
                    return out;
                }
                if close_rect().contains(p) {
                    out.push(SettingsAction::Close);
                    return out;
                }
                let Some(row) = settings::hit(self.scroll, p) else {
                    return out;
                };
                let Some(r) = self.row_rect(row) else {
                    return out;
                };
                let cmd = SettingsAction::Command;
                match row {
                    Row::Width => {
                        if let Some(i) = settings::segment_at(width_rect(r), 3, p) {
                            out.push(cmd(UiCommand::SetWidth(i as u8)));
                        }
                    }
                    Row::HaloFollowsColor => out.push(cmd(UiCommand::SetHaloFollowsColor(!app.cfg.halo_follows_color))),
                    Row::HideFromCapture => out.push(cmd(UiCommand::SetHideFromCapture(!app.cfg.hide_from_capture))),
                    Row::Autostart => out.push(SettingsAction::Autostart(!self.autostart)),
                    Row::ResetBindings => {
                        if reset_rect(r).contains(p) {
                            out.push(cmd(UiCommand::ResetBindings));
                        }
                    }
                    Row::Binding(action) => {
                        let (mode, pill) = settings::binding_parts(r, action);
                        let current = app.cfg.bindings.get(action);
                        if let Some(i) = mode.and_then(|m| settings::segment_at(m, 2, p)) {
                            out.push(SettingsAction::Capture { action, double: i == 1 });
                        } else if pill.contains(p) {
                            if self.capturing.map(|(a, _)| a) == Some(action) {
                                out.push(SettingsAction::CancelCapture);
                            } else {
                                out.push(SettingsAction::Capture { action, double: current.double });
                            }
                        }
                    }
                    Row::HoldSeconds | Row::HaloSize | Row::Header(_) => {}
                }
            }
            PanelEvent::DoubleClick(_) => {}
            PanelEvent::Cancel => self.dragging = None,
        }
        out
    }

    pub fn draw(&mut self, gfx: &Gfx, app: &App, now: u64) -> Result<()> {
        if !self.visible() || (!self.dirty && !self.panel.appearing(now)) {
            return Ok(());
        }
        self.dirty = false;
        let view = &*self;
        let (scroll, hover, hover_close) = (view.scroll, view.hover, view.hover_close);
        let capturing = view.capturing;
        let conflict = view.conflict.clone();
        let autostart = view.autostart;
        self.panel.draw(gfx, now, |p| {
            p.fill_rrect(RectF { x: 0.0, y: 0.0, w: PANEL_W, h: PANEL_H }, theme::R_PANEL, theme::BG);
            let body = RectF { x: 0.0, y: HEADER_H, w: PANEL_W, h: PANEL_H - HEADER_H };
            p.with_rrect_clip(body, 0.0, || {
                for (row, r) in settings::layout(scroll) {
                    if r.y + r.h < HEADER_H || r.y > PANEL_H {
                        continue;
                    }
                    draw_row(p, app, row, r, hover == Some(row), capturing, &conflict, autostart);
                }
            });
            // Indicateur de défilement.
            let max = settings::max_scroll();
            if max > 0.0 {
                let track_h = PANEL_H - HEADER_H - 24.0;
                let thumb_h = (track_h * (PANEL_H - HEADER_H) / settings::content_height()).max(24.0);
                let y = HEADER_H + 8.0 + (track_h - thumb_h) * (scroll / max);
                p.fill_rrect(RectF { x: PANEL_W - 9.0, y, w: 4.0, h: thumb_h }, 2.0, theme::LINE);
            }
            // En-tête fixe.
            p.text(
                "Réglages",
                Family::Body,
                20.0,
                700,
                Align::Left,
                RectF { x: PAD, y: 0.0, w: 200.0, h: HEADER_H },
                theme::INK,
            );
            let c = close_rect();
            if hover_close {
                p.fill_rrect(c, c.h / 2.0, theme::HOVER);
            }
            let m = c.center();
            p.line(Pt::new(m.x - 5.5, m.y - 5.5), Pt::new(m.x + 5.5, m.y + 5.5), theme::INK, 1.75, true);
            p.line(Pt::new(m.x + 5.5, m.y - 5.5), Pt::new(m.x - 5.5, m.y + 5.5), theme::INK, 1.75, true);
            p.line(Pt::new(0.0, HEADER_H - 0.5), Pt::new(PANEL_W, HEADER_H - 0.5), theme::LINE, 1.0, false);
            let edge = RectF { x: 0.5, y: 0.5, w: PANEL_W - 1.0, h: PANEL_H - 1.0 };
            p.stroke_rrect(edge, theme::R_PANEL - 0.5, theme::LINE, 1.0);
        })
    }
}

fn draw_switch(p: &Painter, r: RectF, on: bool) {
    p.fill_rrect(r, r.h / 2.0, if on { theme::BLUE } else { theme::LINE });
    let x = if on { r.x + r.w - r.h / 2.0 } else { r.x + r.h / 2.0 };
    p.fill_circle(Pt::new(x, r.y + r.h / 2.0), r.h / 2.0 - 3.0, theme::WHITE);
}

fn draw_slider(p: &Painter, row: RectF, value: i32, min: i32, max: i32, text: &str) {
    let track = track_rect(row);
    let y = track.y + track.h / 2.0;
    let x = settings::slider_x(track, value, min, max);
    p.line(Pt::new(track.x, y), Pt::new(track.x + track.w, y), theme::LINE, 4.0, true);
    p.line(Pt::new(track.x, y), Pt::new(x, y), theme::BLUE, 4.0, true);
    p.fill_circle(Pt::new(x, y), 8.0, theme::BG);
    p.stroke_ellipse(Pt::new(x, y), 8.0, 8.0, theme::INK, 1.75);
    let value_rect = RectF { x: track.x + track.w + 8.0, y: row.y, w: 46.0, h: row.h };
    p.text(text, Family::Mono, 12.0, 500, Align::Right, value_rect, theme::MUTED);
}

#[allow(clippy::too_many_arguments)]
fn draw_row(
    p: &Painter,
    app: &App,
    row: Row,
    r: RectF,
    hover: bool,
    capturing: Option<(Action, bool)>,
    conflict: &Option<(Action, String, u64)>,
    autostart: bool,
) {
    if let Row::Header(title) = row {
        let text = title.to_uppercase();
        p.text(&text, Family::Mono, 11.0, 600, Align::Left, RectF { x: r.x, y: r.y + 14.0, w: r.w, h: r.h - 14.0 }, theme::BLUE);
        // Filet au-dessus de chaque section, sauf la première, déjà sous l'en-tête du panneau.
        if title != "Dessin" {
            p.line(Pt::new(r.x, r.y + 8.5), Pt::new(r.x + r.w, r.y + 8.5), theme::LINE, 1.0, false);
        }
        return;
    }
    if row == Row::ResetBindings {
        let b = reset_rect(r);
        p.fill_rrect(b, b.h / 2.0, if hover { theme::HOVER } else { theme::BG });
        p.stroke_rrect(RectF { x: b.x + 0.75, y: b.y + 0.75, w: b.w - 1.5, h: b.h - 1.5 }, b.h / 2.0, theme::LINE, 1.5);
        p.text(row.label(), Family::Body, 13.0, 600, Align::Center, b, theme::INK);
        return;
    }

    // Libellé, ou message de conflit pour la ligne concernée.
    let label_w = r.w - 200.0;
    let warning = match (row, conflict) {
        (Row::Binding(a), Some((b, message, _))) if a == *b => Some(message.as_str()),
        _ => None,
    };
    match warning {
        Some(message) => p.text(message, Family::Body, 12.0, 500, Align::Left, RectF { w: r.w - 112.0, ..r }, WARN),
        None => p.text(row.label(), Family::Body, 14.0, 500, Align::Left, RectF { w: label_w.max(120.0) + 80.0, ..r }, theme::INK),
    }

    match row {
        Row::HoldSeconds => {
            let v = app.cfg.hold_seconds as i32;
            draw_slider(p, r, v, 2, 15, &format!("{v} s"));
        }
        Row::HaloSize => {
            let v = app.cfg.halo_size as i32;
            draw_slider(p, r, v, 40, 120, &format!("{v} px"));
        }
        Row::Width => {
            let c = width_rect(r);
            p.fill_rrect(c, c.h / 2.0, SURFACE);
            for (i, w) in WIDTHS.iter().enumerate() {
                let s = settings::segment_rect(c, 3, i);
                let selected = app.cfg.width as usize == i;
                if selected {
                    p.fill_rrect(RectF { x: s.x + 2.0, y: s.y + 2.0, w: s.w - 4.0, h: s.h - 4.0 }, (s.h - 4.0) / 2.0, theme::INK);
                }
                let m = s.center();
                let color = if selected { theme::WHITE } else { theme::INK };
                p.line(Pt::new(m.x - 9.0, m.y), Pt::new(m.x + 9.0, m.y), color, *w * 0.8, true);
            }
        }
        Row::HaloFollowsColor => draw_switch(p, switch_rect(r), app.cfg.halo_follows_color),
        Row::HideFromCapture => draw_switch(p, switch_rect(r), app.cfg.hide_from_capture),
        Row::Autostart => draw_switch(p, switch_rect(r), autostart),
        Row::Binding(action) => {
            let (mode, pill) = settings::binding_parts(r, action);
            let shortcut = app.cfg.bindings.get(action);
            let active = capturing.filter(|(a, _)| *a == action);
            if let Some(m) = mode {
                let double = active.map(|(_, d)| d).unwrap_or(shortcut.double);
                p.fill_rrect(m, m.h / 2.0, SURFACE);
                for (i, text) in ["Touches", "2×"].iter().enumerate() {
                    let s = settings::segment_rect(m, 2, i);
                    let selected = (i == 1) == double;
                    if selected {
                        p.fill_rrect(RectF { x: s.x + 2.0, y: s.y + 2.0, w: s.w - 4.0, h: s.h - 4.0 }, (s.h - 4.0) / 2.0, theme::INK);
                    }
                    let color = if selected { theme::WHITE } else { theme::MUTED };
                    p.text(text, Family::Mono, 10.0, 600, Align::Center, s, color);
                }
            }
            p.fill_rrect(pill, pill.h / 2.0, if hover || active.is_some() { theme::HOVER } else { SURFACE });
            if active.is_some() {
                p.stroke_rrect(RectF { x: pill.x + 0.75, y: pill.y + 0.75, w: pill.w - 1.5, h: pill.h - 1.5 }, pill.h / 2.0, theme::BLUE, 1.5);
                let prompt = if active.is_some_and(|(_, d)| d) { "Tapez la touche" } else { "Tapez…" };
                p.text(prompt, Family::Body, 11.0, 600, Align::Center, pill, theme::BLUE);
            } else {
                p.text(&shortcut.to_string(), Family::Mono, 11.5, 600, Align::Center, pill, theme::INK);
            }
        }
        Row::Header(_) | Row::ResetBindings => {}
    }
}
