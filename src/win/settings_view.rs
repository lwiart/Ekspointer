//! Panneau de réglages et d'aide : état d'affichage, réactions à la souris et dessin.

use super::gfx::{Align, Family, Gfx, Painter};
use super::icons;
use super::panel::{Panel, PanelEvent, PanelId};
use super::ui::{cursor, SURFACE};
use ekspointer::app::{App, UiCommand};
use ekspointer::geom::{Pt, RectF, Rgba, PALETTE, WIDTHS};
use ekspointer::i18n::{t, Lang, Text};
use ekspointer::shortcuts::Action;
use ekspointer::ui::help::{self, Line};
use ekspointer::ui::settings::{self, HeaderPart, Row, Tab, HEADER_H, PANEL_H, PANEL_W};
use ekspointer::ui::theme;
use ekspointer::ui::toolbar::{self, Item};
use windows::core::Result;

/// Couleur des messages d'alerte (ambre foncé du design system, lisible en petit corps).
const WARN: Rgba = Rgba::hex(0xBB450D);
/// Durée d'affichage d'un message de conflit, en millisecondes.
pub const CONFLICT_MS: u64 = 4000;
/// Version affichée dans les crédits.
const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Ce que le panneau demande à la boucle principale.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum SettingsAction {
    Command(UiCommand),
    /// Commencer la saisie d'un raccourci pour cette action (double appui ou combinaison).
    Capture { action: Action, double: bool },
    CancelCapture,
    Autostart(bool),
    /// Ouvrir une adresse dans le navigateur.
    OpenUrl(&'static str),
    Close,
}

/// Panneau de réglages.
pub struct SettingsView {
    pub panel: Panel,
    tab: Tab,
    scroll: f32,
    help_scroll: f32,
    hover: Option<Row>,
    hover_header: Option<HeaderPart>,
    /// Le curseur survole le lien de la page d'aide.
    hover_link: bool,
    /// Curseur en cours de glissement.
    dragging: Option<Row>,
    /// Déplacement du panneau en cours.
    moving: Option<Move>,
    /// Action dont le raccourci est en cours de saisie, et type de saisie.
    pub capturing: Option<(Action, bool)>,
    /// Message de conflit : ligne concernée, texte, instant d'expiration.
    conflict: Option<(Action, String, u64)>,
    autostart: bool,
    pub dirty: bool,
}

/// Déplacement du panneau par son en-tête : il est réduit de moitié le temps du déplacement.
struct Move {
    /// Écart entre le curseur et le coin du panneau au début, à taille normale.
    grab: (i32, i32),
    start: (i32, i32),
    moved: bool,
    /// Échelle du panneau avant le déplacement.
    base_scale: f32,
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

/// Sélecteur de langue : deux segments, à droite de la ligne.
fn language_rect(row: RectF) -> RectF {
    let c = settings::control_rect(row);
    RectF { x: c.x + c.w - 164.0, y: c.y, w: 164.0, h: c.h }
}

fn reset_rect(row: RectF) -> RectF {
    RectF { x: row.x, y: row.y + 12.0, w: 262.0, h: 32.0 }
}

impl SettingsView {
    pub fn new(gfx: &Gfx) -> Result<SettingsView> {
        Ok(SettingsView {
            panel: Panel::new(gfx, PanelId::Settings, (PANEL_W, PANEL_H), theme::R_PANEL)?,
            tab: Tab::Settings,
            scroll: 0.0,
            help_scroll: 0.0,
            hover: None,
            hover_header: None,
            hover_link: false,
            dragging: None,
            moving: None,
            capturing: None,
            conflict: None,
            autostart: false,
            dirty: true,
        })
    }

    pub fn visible(&self) -> bool {
        self.panel.visible()
    }

    /// Ouvre le panneau au centre d'un écran, sur l'onglet donné.
    pub fn open(
        &mut self,
        gfx: &Gfx,
        monitor: RectF,
        scale: f32,
        autostart: bool,
        tab: Tab,
        appear: Option<u64>,
    ) -> Result<()> {
        self.autostart = autostart;
        self.tab = tab;
        self.hover = None;
        self.hover_header = None;
        self.dragging = None;
        self.moving = None;
        self.dirty = true;
        if let Some(now) = appear {
            self.panel.start_appear(now);
        }
        let x = monitor.x + (monitor.w - PANEL_W * scale) / 2.0;
        let y = monitor.y + ((monitor.h - PANEL_H * scale) / 2.0).max(0.0);
        self.panel.place(gfx, (x as i32, y as i32), (PANEL_W, PANEL_H), theme::R_PANEL, scale)
    }

    pub fn close(&mut self) {
        self.panel.hide();
        self.capturing = None;
        self.conflict = None;
        self.moving = None;
    }

    pub fn set_autostart(&mut self, on: bool) {
        self.autostart = on;
        self.dirty = true;
    }

    /// Affiche un message sous la ligne d'un raccourci (conflit ou raccourci refusé).
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
            Row::HoldSeconds => UiCommand::SetHoldSeconds(settings::slider_value(track, x, 1, 15) as u8),
            Row::HaloSize => UiCommand::SetHaloSize(settings::slider_value(track, x, 40, 120) as u32),
            _ => return None,
        }))
    }

    /// Traite un événement de souris du panneau.
    pub fn on_event(&mut self, gfx: &Gfx, ev: PanelEvent, app: &App) -> Vec<SettingsAction> {
        let mut out = Vec::new();
        self.dirty = true;
        match ev {
            PanelEvent::Wheel(delta) => match self.tab {
                Tab::Settings => self.scroll = settings::clamp_scroll(self.scroll - delta * 54.0),
                Tab::Help => {
                    let lines = help::lines(app.cfg.language, &app.cfg.bindings, VERSION);
                    self.help_scroll = help::clamp_scroll(&lines, self.help_scroll - delta * 54.0);
                }
            },
            PanelEvent::Leave => {
                self.hover = None;
                self.hover_header = None;
            }
            PanelEvent::Cancel => {
                self.dragging = None;
                if let Some(m) = self.moving.take() {
                    self.end_move(gfx, m);
                }
            }
            PanelEvent::Move(p) => {
                let p = self.panel.to_dip(p);
                if let Some(m) = &mut self.moving {
                    // Déplacement du panneau par son en-tête, à demi-taille sous le curseur.
                    let c = cursor();
                    let pos = toolbar::drag_origin(c, m.grab, toolbar::DRAG_SHRINK);
                    if m.moved {
                        self.panel.move_to(pos);
                    } else if (c.0 - m.start.0).abs() + (c.1 - m.start.1).abs() > 4 {
                        m.moved = true;
                        let scale = m.base_scale * toolbar::DRAG_SHRINK;
                        let _ = self.panel.place(gfx, pos, (PANEL_W, PANEL_H), theme::R_PANEL, scale);
                    }
                } else if let Some(row) = self.dragging {
                    out.extend(self.slider_command(row, p.x));
                } else {
                    self.hover_header = settings::header_hit(p);
                    self.hover = if self.tab == Tab::Settings { settings::hit(self.scroll, p) } else { None };
                    self.hover_link = self.tab == Tab::Help && self.help_link(app, p).is_some();
                }
            }
            PanelEvent::Down(p) => {
                let p = self.panel.to_dip(p);
                if settings::header_hit(p) == Some(HeaderPart::Drag) {
                    let c = cursor();
                    self.moving = Some(Move {
                        grab: (c.0 - self.panel.pos.0, c.1 - self.panel.pos.1),
                        start: c,
                        moved: false,
                        base_scale: self.panel.scale,
                    });
                } else if self.tab == Tab::Settings {
                    if let Some(row @ (Row::HoldSeconds | Row::HaloSize)) = settings::hit(self.scroll, p) {
                        self.dragging = Some(row);
                        out.extend(self.slider_command(row, p.x));
                    }
                }
            }
            PanelEvent::Up(p) => {
                let p = self.panel.to_dip(p);
                if let Some(m) = self.moving.take() {
                    self.end_move(gfx, m);
                    return out;
                }
                if self.dragging.take().is_some() {
                    return out;
                }
                match settings::header_hit(p) {
                    Some(HeaderPart::Close) => {
                        out.push(SettingsAction::Close);
                        return out;
                    }
                    Some(HeaderPart::Tab(tab)) => {
                        if tab != self.tab && self.capturing.is_some() {
                            out.push(SettingsAction::CancelCapture);
                        }
                        self.tab = tab;
                        return out;
                    }
                    Some(HeaderPart::Drag) => return out,
                    None => {}
                }
                match self.tab {
                    Tab::Settings => self.click_row(p, app, &mut out),
                    Tab::Help => out.extend(self.help_link(app, p).map(SettingsAction::OpenUrl)),
                }
            }
            PanelEvent::DoubleClick(_) => {}
        }
        out
    }

    /// Fin d'un déplacement : le panneau reprend sa taille, le point saisi sous le curseur.
    fn end_move(&mut self, gfx: &Gfx, m: Move) {
        if m.moved {
            let pos = toolbar::drag_origin(cursor(), m.grab, 1.0);
            let _ = self.panel.place(gfx, pos, (PANEL_W, PANEL_H), theme::R_PANEL, m.base_scale);
            self.dirty = true;
        }
    }

    /// Adresse du lien de la page d'aide sous un point.
    fn help_link(&self, app: &App, p: Pt) -> Option<&'static str> {
        let lines = help::lines(app.cfg.language, &app.cfg.bindings, VERSION);
        help::link_at(&lines, self.help_scroll, p)
    }

    fn click_row(&mut self, p: Pt, app: &App, out: &mut Vec<SettingsAction>) {
        let Some(row) = settings::hit(self.scroll, p) else {
            return;
        };
        let Some(r) = self.row_rect(row) else {
            return;
        };
        let cmd = SettingsAction::Command;
        match row {
            Row::Language => {
                if let Some(i) = settings::segment_at(language_rect(r), 2, p) {
                    out.push(cmd(UiCommand::SetLanguage(Lang::all()[i])));
                }
            }
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

    pub fn draw(&mut self, gfx: &Gfx, app: &App, now: u64) -> Result<()> {
        if !self.visible() || (!self.dirty && !self.panel.appearing(now)) {
            return Ok(());
        }
        self.dirty = false;
        let lang = app.cfg.language;
        let (tab, scroll, help_scroll) = (self.tab, self.scroll, self.help_scroll);
        let (hover, hover_header, hover_link) = (self.hover, self.hover_header, self.hover_link);
        let capturing = self.capturing;
        let conflict = self.conflict.clone();
        let autostart = self.autostart;
        self.panel.draw(gfx, now, |p| {
            p.fill_rrect(RectF { x: 0.0, y: 0.0, w: PANEL_W, h: PANEL_H }, theme::R_PANEL, theme::BG);
            let body = RectF { x: 0.0, y: HEADER_H, w: PANEL_W, h: PANEL_H - HEADER_H };
            let (position, max, content) = match tab {
                Tab::Settings => {
                    p.with_rrect_clip(body, 0.0, || {
                        for (i, (row, r)) in settings::layout(scroll).into_iter().enumerate() {
                            if r.y + r.h < HEADER_H || r.y > PANEL_H {
                                continue;
                            }
                            draw_row(p, app, row, r, i == 0, hover == Some(row), capturing, &conflict, autostart);
                        }
                    });
                    (scroll, settings::max_scroll(), settings::content_height())
                }
                Tab::Help => {
                    let lines = help::lines(lang, &app.cfg.bindings, VERSION);
                    p.with_rrect_clip(body, 0.0, || {
                        for (line, r) in lines.iter().zip(help::layout(&lines, help_scroll)) {
                            if r.y + r.h < HEADER_H || r.y > PANEL_H {
                                continue;
                            }
                            draw_help_line(p, line, r, hover_link, app.cfg.color);
                        }
                    });
                    (help_scroll, help::max_scroll(&lines), help::content_height(&lines))
                }
            };
            // Indicateur de défilement.
            if max > 0.0 {
                let track_h = PANEL_H - HEADER_H - 24.0;
                let thumb_h = (track_h * (PANEL_H - HEADER_H) / content).max(24.0);
                let y = HEADER_H + 8.0 + (track_h - thumb_h) * (position / max);
                p.fill_rrect(RectF { x: PANEL_W - 9.0, y, w: 4.0, h: thumb_h }, 2.0, theme::LINE);
            }
            draw_header(p, lang, tab, hover_header);
            let edge = RectF { x: 0.5, y: 0.5, w: PANEL_W - 1.0, h: PANEL_H - 1.0 };
            p.stroke_rrect(edge, theme::R_PANEL - 0.5, theme::LINE, 1.0);
        })
    }
}

/// En-tête fixe : les deux onglets, le bouton de fermeture, et une zone libre pour déplacer le panneau.
fn draw_header(p: &Painter, lang: Lang, tab: Tab, hover: Option<HeaderPart>) {
    // Fond de l'en-tête : il masque le contenu qui défile dessous, en respectant les coins arrondis.
    p.with_rrect_clip(RectF { x: 0.0, y: 0.0, w: PANEL_W, h: PANEL_H }, theme::R_PANEL, || {
        p.fill_rrect(RectF { x: 0.0, y: 0.0, w: PANEL_W, h: HEADER_H }, 0.0, theme::BG);
    });
    let tabs = [(Tab::Settings, Text::Settings), (Tab::Help, Text::Help)];
    for ((this, text), r) in tabs.into_iter().zip(settings::tab_rects()) {
        let selected = this == tab;
        if selected {
            p.fill_rrect(r, r.h / 2.0, theme::INK);
        } else if hover == Some(HeaderPart::Tab(this)) {
            p.fill_rrect(r, r.h / 2.0, theme::HOVER);
        }
        let color = if selected { theme::WHITE } else { theme::INK };
        p.text(t(lang, text), Family::Body, 15.0, 700, Align::Center, r, color);
    }
    let c = settings::close_rect();
    if hover == Some(HeaderPart::Close) {
        p.fill_rrect(c, c.h / 2.0, theme::HOVER);
    }
    let m = c.center();
    p.line(Pt::new(m.x - 5.5, m.y - 5.5), Pt::new(m.x + 5.5, m.y + 5.5), theme::INK, 1.75, true);
    p.line(Pt::new(m.x + 5.5, m.y - 5.5), Pt::new(m.x - 5.5, m.y + 5.5), theme::INK, 1.75, true);
    p.line(Pt::new(0.0, HEADER_H - 0.5), Pt::new(PANEL_W, HEADER_H - 0.5), theme::LINE, 1.0, false);
}

fn draw_help_line(p: &Painter, line: &Line, r: RectF, hover_link: bool, color: u8) {
    match line {
        Line::Button(item, label, key) => {
            // L'icône du bouton, telle qu'elle apparaît dans la barre, puis son nom et son raccourci.
            let icon = RectF { x: r.x + 2.0, y: r.y + (r.h - 20.0) / 2.0, w: 20.0, h: 20.0 };
            if *item == Item::Color {
                p.fill_circle(icon.center(), 8.0, PALETTE[color as usize % PALETTE.len()]);
                p.stroke_ellipse(icon.center(), 8.0, 8.0, theme::LINE, 1.5);
            } else {
                icons::draw(p, *item, icon, theme::INK);
            }
            p.text(label, Family::Body, 13.0, 500, Align::Left, RectF { x: r.x + 34.0, w: r.w - 34.0, ..r }, theme::INK);
            if let Some(key) = key {
                let w = p.gfx.text_width(key, Family::Mono, 11.0, 600) + 20.0;
                let pill = RectF { x: r.x + r.w - w, y: r.y + 3.0, w, h: r.h - 6.0 };
                p.fill_rrect(pill, pill.h / 2.0, SURFACE);
                p.text(key, Family::Mono, 11.0, 600, Align::Center, pill, theme::INK);
            }
        }
        Line::Link(label, _) => {
            // Lien : bleu et souligné, sur un fond clair au survol.
            let zone = help::link_rect(r);
            let w = p.gfx.text_width(label, Family::Body, 13.5, 600);
            if hover_link {
                let pill = RectF { x: zone.x + (zone.w - w) / 2.0 - 12.0, y: zone.y + 1.0, w: w + 24.0, h: zone.h - 2.0 };
                p.fill_rrect(pill, pill.h / 2.0, theme::HOVER);
            }
            p.text(label, Family::Body, 13.5, 600, Align::Center, zone, theme::BLUE);
            let (x, y) = (zone.x + (zone.w - w) / 2.0, zone.y + zone.h / 2.0 + 9.0);
            p.line(Pt::new(x, y), Pt::new(x + w, y), theme::BLUE, 1.0, false);
        }
        Line::Title(text) => {
            let upper = text.to_uppercase();
            p.text(&upper, Family::Mono, 11.0, 600, Align::Left, RectF { y: r.y + 10.0, h: r.h - 10.0, ..r }, theme::BLUE);
        }
        Line::Text(text) => p.text(text, Family::Body, 13.0, 400, Align::Left, r, theme::MUTED),
        Line::Credit(text) => {
            let weight = if text.starts_with("Ekspointer") { 700 } else { 500 };
            p.text(text, Family::Body, 13.5, weight, Align::Center, r, theme::INK);
        }
        Line::Key(label, key) => {
            p.text(label, Family::Body, 13.0, 500, Align::Left, r, theme::INK);
            let w = p.gfx.text_width(key, Family::Mono, 11.0, 600) + 20.0;
            let pill = RectF { x: r.x + r.w - w, y: r.y + 3.0, w, h: r.h - 6.0 };
            p.fill_rrect(pill, pill.h / 2.0, SURFACE);
            p.text(key, Family::Mono, 11.0, 600, Align::Center, pill, theme::INK);
        }
        Line::Gap => {}
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

/// Segment sélectionné d'un contrôle à plusieurs positions.
fn draw_selected_segment(p: &Painter, s: RectF) {
    p.fill_rrect(RectF { x: s.x + 2.0, y: s.y + 2.0, w: s.w - 4.0, h: s.h - 4.0 }, (s.h - 4.0) / 2.0, theme::INK);
}

#[allow(clippy::too_many_arguments)]
fn draw_row(
    p: &Painter,
    app: &App,
    row: Row,
    r: RectF,
    first: bool,
    hover: bool,
    capturing: Option<(Action, bool)>,
    conflict: &Option<(Action, String, u64)>,
    autostart: bool,
) {
    let lang = app.cfg.language;
    if matches!(row, Row::Header(_)) {
        let text = row.label_in(lang).to_uppercase();
        p.text(&text, Family::Mono, 11.0, 600, Align::Left, RectF { x: r.x, y: r.y + 14.0, w: r.w, h: r.h - 14.0 }, theme::BLUE);
        // Filet au-dessus de chaque section, sauf la première, déjà sous l'en-tête du panneau.
        if !first {
            p.line(Pt::new(r.x, r.y + 8.5), Pt::new(r.x + r.w, r.y + 8.5), theme::LINE, 1.0, false);
        }
        return;
    }
    if row == Row::ResetBindings {
        let b = reset_rect(r);
        p.fill_rrect(b, b.h / 2.0, if hover { theme::HOVER } else { theme::BG });
        p.stroke_rrect(RectF { x: b.x + 0.75, y: b.y + 0.75, w: b.w - 1.5, h: b.h - 1.5 }, b.h / 2.0, theme::LINE, 1.5);
        p.text(row.label_in(lang), Family::Body, 13.0, 600, Align::Center, b, theme::INK);
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
        None => {
            p.text(row.label_in(lang), Family::Body, 14.0, 500, Align::Left, RectF { w: label_w.max(120.0) + 80.0, ..r }, theme::INK)
        }
    }

    match row {
        Row::Language => {
            let c = language_rect(r);
            p.fill_rrect(c, c.h / 2.0, SURFACE);
            for (i, choice) in Lang::all().into_iter().enumerate() {
                let s = settings::segment_rect(c, 2, i);
                let selected = choice == lang;
                if selected {
                    draw_selected_segment(p, s);
                }
                let color = if selected { theme::WHITE } else { theme::INK };
                p.text(choice.name(), Family::Body, 12.5, 600, Align::Center, s, color);
            }
        }
        Row::HoldSeconds => {
            let v = app.cfg.hold_seconds as i32;
            draw_slider(p, r, v, 1, 15, &format!("{v} s"));
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
                    draw_selected_segment(p, s);
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
                for (i, text) in [t(lang, Text::ModeKeys), "2×"].iter().enumerate() {
                    let s = settings::segment_rect(m, 2, i);
                    let selected = (i == 1) == double;
                    if selected {
                        draw_selected_segment(p, s);
                    }
                    let color = if selected { theme::WHITE } else { theme::MUTED };
                    p.text(text, Family::Mono, 10.0, 600, Align::Center, s, color);
                }
            }
            p.fill_rrect(pill, pill.h / 2.0, if hover || active.is_some() { theme::HOVER } else { SURFACE });
            if let Some((_, double)) = active {
                p.stroke_rrect(RectF { x: pill.x + 0.75, y: pill.y + 0.75, w: pill.w - 1.5, h: pill.h - 1.5 }, pill.h / 2.0, theme::BLUE, 1.5);
                let prompt = t(lang, if double { Text::TypeKey } else { Text::TypeShortcut });
                p.text(prompt, Family::Body, 11.0, 600, Align::Center, pill, theme::BLUE);
            } else {
                p.text(&shortcut.display(lang), Family::Mono, 11.5, 600, Align::Center, pill, theme::INK);
            }
        }
        Row::Header(_) | Row::ResetBindings => {}
    }
}
