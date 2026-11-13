//! Barre d'outils, volets (couleur, épaisseur) et bulle d'aide : état d'affichage,
//! réactions à la souris et dessin.

use super::gfx::{Align, Family, Gfx, Painter};
use super::icons;
use super::overlay::Overlays;
use super::panel::{Panel, PanelEvent, PanelId};
use ekspointer::app::{App, UiCommand};
use ekspointer::geom::{Pt, RectF, Rgba, PALETTE, WIDTHS};
use ekspointer::ui::theme;
use ekspointer::ui::toolbar::{self, Item, Layout, Popover};
use windows::core::Result;
use windows::Win32::Foundation::POINT;
use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;

/// Fond discret des pastilles de raccourci.
pub const SURFACE: Rgba = Rgba::hex(0xF1F7F9);
/// Écart entre la barre et un volet ou une bulle, en pixels logiques.
const GAP_BELOW: f32 = 10.0;
/// Hauteur de la bulle d'aide.
const TIP_H: f32 = 32.0;
/// Délai de survol avant la bulle d'aide, en millisecondes.
pub const TIP_DELAY_MS: u64 = 500;

/// Ce que l'interface demande à la boucle principale.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum UiAction {
    Command(UiCommand),
    ToggleSettings,
    /// Ouvrir la page d'aide.
    OpenHelp,
    /// Le survol a changé : armer le minuteur de la bulle d'aide.
    ArmTooltip,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Pop {
    Color,
    Width,
}

struct Drag {
    /// Écart entre le curseur et le coin de la barre au début du glissement, à taille normale.
    offset: (i32, i32),
    start: (i32, i32),
    moved: bool,
    /// Échelle de la barre avant le glissement : elle est réduite de moitié pendant qu'on la déplace.
    base_scale: f32,
}

/// Position du curseur dans le bureau virtuel, en pixels physiques.
pub fn cursor() -> (i32, i32) {
    let mut p = POINT::default();
    unsafe {
        let _ = GetCursorPos(&mut p);
    }
    (p.x, p.y)
}

/// Interface flottante.
pub struct Ui {
    pub toolbar: Panel,
    popover: Panel,
    tooltip: Panel,
    layout: Layout,
    hover: Option<Item>,
    hover_since: u64,
    pressed: Option<Item>,
    mouse: Option<Pt>,
    drag: Option<Drag>,
    pop: Option<Pop>,
    pop_hover: Option<usize>,
    tip: Option<(String, Option<String>)>,
    /// Élément actuellement désigné par la pastille de l'outil actif.
    pill_item: Option<Item>,
    pill_from: f32,
    pill_to: f32,
    pill_start: u64,
    /// La barre a été masquée depuis la zone de notification.
    hidden: bool,
    /// Quelque chose a changé : la barre doit être redessinée.
    pub dirty: bool,
    animate: bool,
}

impl Ui {
    pub fn new(gfx: &Gfx, app: &App, overlays: &Overlays, animate: bool) -> Result<Ui> {
        super::panel::register()?;
        let layout = toolbar::layout(app.cfg.toolbar_collapsed);
        let mut ui = Ui {
            toolbar: Panel::new(gfx, PanelId::Toolbar, (layout.w, layout.h), theme::BAR_H / 2.0)?,
            popover: Panel::new(gfx, PanelId::Popover, (10.0, 10.0), theme::R_POPOVER)?,
            tooltip: Panel::new(gfx, PanelId::Tooltip, (10.0, TIP_H), TIP_H / 2.0)?,
            layout,
            hover: None,
            hover_since: 0,
            pressed: None,
            mouse: None,
            drag: None,
            pop: None,
            pop_hover: None,
            tip: None,
            pill_item: None,
            pill_from: 0.0,
            pill_to: 0.0,
            pill_start: 0,
            hidden: false,
            dirty: true,
            animate,
        };
        ui.place_toolbar(gfx, app, overlays)?;
        ui.set_capture_hidden(app.cfg.hide_from_capture);
        Ok(ui)
    }

    /// Place la barre : position mémorisée si elle tombe sur un écran, sinon haut centre de l'écran principal.
    pub fn place_toolbar(&mut self, gfx: &Gfx, app: &App, overlays: &Overlays) -> Result<()> {
        self.layout = toolbar::layout(app.cfg.toolbar_collapsed);
        let (w, h) = (self.layout.w, self.layout.h);
        let saved = app.cfg.toolbar_pos.and_then(|(x, y)| {
            overlays.monitor_at(Pt::new(x as f32 + 8.0, y as f32 + 8.0)).map(|(_, scale)| ((x, y), scale))
        });
        let (pos, scale) = saved.unwrap_or_else(|| {
            let (rect, scale) = overlays.primary();
            (((rect.x + (rect.w - w * scale) / 2.0) as i32, (rect.y + 24.0 * scale) as i32), scale)
        });
        self.close_floating();
        if self.hidden {
            self.toolbar.pos = pos;
            self.toolbar.size = (w, h);
            self.toolbar.scale = scale;
            return Ok(());
        }
        self.dirty = true;
        self.toolbar.place(gfx, pos, (w, h), theme::BAR_H / 2.0, scale)
    }

    pub fn toolbar_visible(&self) -> bool {
        !self.hidden
    }

    /// Affiche ou masque la barre (zone de notification).
    pub fn set_toolbar_visible(&mut self, gfx: &Gfx, app: &App, overlays: &Overlays, visible: bool) -> Result<()> {
        self.hidden = !visible;
        if visible {
            self.place_toolbar(gfx, app, overlays)
        } else {
            self.close_floating();
            self.toolbar.hide();
            Ok(())
        }
    }

    /// Exclut (ou non) l'interface du partage d'écran.
    pub fn set_capture_hidden(&self, hidden: bool) {
        for p in [&self.toolbar, &self.popover, &self.tooltip] {
            p.set_capture_hidden(hidden);
        }
    }

    /// Replace l'interface au-dessus des fenêtres de dessin.
    pub fn raise(&self) {
        self.toolbar.raise();
        self.popover.raise();
        self.tooltip.raise();
    }

    /// Rectangles où les clics ne doivent jamais être interceptés par le filtre d'entrées.
    pub fn exempt_rects(&self) -> Vec<RectF> {
        [&self.toolbar, &self.popover].iter().filter(|p| p.visible()).map(|p| p.rect()).collect()
    }

    fn close_floating(&mut self) {
        self.pop = None;
        self.pop_hover = None;
        self.popover.hide();
        self.tip = None;
        self.tooltip.hide();
    }

    /// Un clic a eu lieu hors de l'interface : referme le volet ouvert.
    pub fn on_click_outside(&mut self) {
        if self.pop.is_some() {
            self.close_floating();
            self.dirty = true;
        }
    }

    /// Place un panneau flottant sous un élément de la barre (au-dessus s'il n'y a pas la place).
    fn place_under(
        gfx: &Gfx,
        toolbar: &Panel,
        overlays: &Overlays,
        panel: &mut Panel,
        anchor: RectF,
        size: (f32, f32),
        radius: f32,
    ) -> Result<()> {
        let s = toolbar.scale;
        let bar = toolbar.rect();
        let center_x = bar.x + (anchor.x + anchor.w / 2.0) * s;
        let (monitor, _) = overlays.monitor_at(Pt::new(bar.x + bar.w / 2.0, bar.y + bar.h / 2.0)).unwrap_or(overlays.primary());
        let mut x = center_x - size.0 * s / 2.0;
        x = x.clamp(monitor.x + 4.0 * s, (monitor.x + monitor.w - size.0 * s - 4.0 * s).max(monitor.x));
        let below = bar.y + bar.h + GAP_BELOW * s;
        let y = if below + size.1 * s <= monitor.y + monitor.h { below } else { bar.y - (GAP_BELOW + size.1) * s };
        panel.place(gfx, (x as i32, y as i32), size, radius, s)
    }

    fn open_popover(&mut self, gfx: &Gfx, overlays: &Overlays, pop: Pop, now: u64) -> Result<()> {
        self.close_floating();
        if self.animate {
            self.popover.start_appear(now);
        }
        let (item, layout) = match pop {
            Pop::Color => (Item::Color, toolbar::color_popover()),
            Pop::Width => (Item::Width, toolbar::width_popover()),
        };
        let Some(anchor) = self.layout.rect(item) else {
            return Ok(());
        };
        self.pop = Some(pop);
        Self::place_under(gfx, &self.toolbar, overlays, &mut self.popover, anchor, (layout.w, layout.h), theme::R_POPOVER)
    }

    fn popover_layout(&self) -> Option<Popover> {
        self.pop.map(|p| match p {
            Pop::Color => toolbar::color_popover(),
            Pop::Width => toolbar::width_popover(),
        })
    }

    /// Affiche la bulle d'aide si le survol dure depuis assez longtemps.
    pub fn show_tooltip_if_due(&mut self, gfx: &Gfx, app: &App, overlays: &Overlays, now: u64) -> Result<()> {
        let Some(item) = self.hover else {
            return Ok(());
        };
        if self.pop.is_some() || self.drag.is_some() || self.pressed.is_some() || self.tip.is_some() {
            return Ok(());
        }
        if now.saturating_sub(self.hover_since) + 20 < TIP_DELAY_MS {
            return Ok(());
        }
        let tip = toolbar::tooltip_in(item, &app.cfg.bindings, app.cfg.language);
        let (Some(text), Some(anchor)) = (tip, self.layout.rect(item)) else {
            return Ok(());
        };
        let mut w = 14.0 + gfx.text_width(&text.0, Family::Body, 13.0, 500) + 14.0;
        if let Some(key) = &text.1 {
            w += 2.0 + gfx.text_width(key, Family::Mono, 11.0, 500) + 16.0;
        }
        self.tip = Some(text);
        if self.animate {
            self.tooltip.start_appear(now);
        }
        Self::place_under(gfx, &self.toolbar, overlays, &mut self.tooltip, anchor, (w.ceil(), TIP_H), TIP_H / 2.0)
    }

    fn set_hover(&mut self, item: Option<Item>, now: u64, out: &mut Vec<UiAction>) {
        if item != self.hover {
            self.hover = item;
            self.hover_since = now;
            self.tip = None;
            self.tooltip.hide();
            if item.is_some() {
                out.push(UiAction::ArmTooltip);
            }
        }
    }

    /// Commande déclenchée par un clic sur un élément de la barre.
    fn activate(
        &mut self,
        gfx: &Gfx,
        app: &App,
        overlays: &Overlays,
        item: Item,
        now: u64,
        out: &mut Vec<UiAction>,
    ) -> Result<()> {
        let cmd = |c: UiCommand| UiAction::Command(c);
        match item {
            Item::Tool(None) => out.push(cmd(UiCommand::SelectTool(None))),
            Item::Tool(Some(tool)) => {
                // Recliquer sur l'outil actif revient au curseur.
                let again = app.draw_mode && app.tool == tool;
                out.push(cmd(UiCommand::SelectTool(if again { None } else { Some(tool) })));
            }
            Item::Color | Item::Width => {
                let pop = if item == Item::Color { Pop::Color } else { Pop::Width };
                if self.pop == Some(pop) {
                    self.close_floating();
                } else {
                    self.open_popover(gfx, overlays, pop, now)?;
                }
            }
            Item::Halo => out.push(cmd(UiCommand::ToggleHalo)),
            Item::Ripple => out.push(cmd(UiCommand::ToggleRipple)),
            Item::Trail => out.push(cmd(UiCommand::ToggleTrail)),
            Item::Pin => out.push(cmd(UiCommand::TogglePin)),
            Item::Clear => out.push(cmd(UiCommand::ClearAll)),
            Item::Settings => {
                self.close_floating();
                out.push(UiAction::ToggleSettings);
            }
            Item::Help => {
                self.close_floating();
                out.push(UiAction::OpenHelp);
            }
            Item::Logo | Item::Separator => {}
        }
        Ok(())
    }

    /// Traite un événement de souris de la barre, du volet ou de la bulle.
    pub fn on_panel(
        &mut self,
        gfx: &Gfx,
        id: PanelId,
        ev: PanelEvent,
        app: &App,
        overlays: &Overlays,
        now: u64,
    ) -> Result<Vec<UiAction>> {
        let mut out = Vec::new();
        self.dirty = true;
        match id {
            PanelId::Toolbar => match ev {
                PanelEvent::Move(p) => {
                    let p = self.toolbar.to_dip(p);
                    if let Some(drag) = &mut self.drag {
                        let c = cursor();
                        let pos = toolbar::drag_origin(c, drag.offset, toolbar::DRAG_SHRINK);
                        if drag.moved {
                            self.toolbar.move_to(pos);
                        } else if (c.0 - drag.start.0).abs() + (c.1 - drag.start.1).abs() > 4 {
                            // Le déplacement commence : la barre passe à demi-taille sous le curseur.
                            drag.moved = true;
                            let size = (self.layout.w, self.layout.h);
                            let scale = drag.base_scale * toolbar::DRAG_SHRINK;
                            self.toolbar.place(gfx, pos, size, theme::BAR_H / 2.0, scale)?;
                        }
                    } else {
                        self.mouse = Some(p);
                        let hit = self.layout.hit(p);
                        self.set_hover(hit, now, &mut out);
                    }
                }
                PanelEvent::Leave => {
                    if self.drag.is_none() {
                        self.mouse = None;
                        self.set_hover(None, now, &mut out);
                    }
                }
                PanelEvent::Down(p) => {
                    let p = self.toolbar.to_dip(p);
                    self.tip = None;
                    self.tooltip.hide();
                    self.pressed = self.layout.hit(p);
                    // Tout ce qui n'est pas un bouton sert de poignée : logo, filets, marges, espaces.
                    if self.layout.is_handle(p) {
                        let c = cursor();
                        self.close_floating();
                        self.drag = Some(Drag {
                            offset: (c.0 - self.toolbar.pos.0, c.1 - self.toolbar.pos.1),
                            start: c,
                            moved: false,
                            base_scale: self.toolbar.scale,
                        });
                    }
                }
                PanelEvent::Up(p) => {
                    let p = self.toolbar.to_dip(p);
                    let pressed = self.pressed.take();
                    if let Some(drag) = self.drag.take() {
                        self.end_drag(gfx, overlays, drag, &mut out)?;
                    } else if let Some(item) = pressed.filter(|i| self.layout.hit(p) == Some(*i)) {
                        self.activate(gfx, app, overlays, item, now, &mut out)?;
                    }
                }
                PanelEvent::DoubleClick(p) => {
                    if self.layout.hit(self.toolbar.to_dip(p)) == Some(Item::Logo) {
                        out.push(UiAction::Command(UiCommand::SetToolbarCollapsed(!app.cfg.toolbar_collapsed)));
                    }
                }
                PanelEvent::Wheel(_) => {}
                PanelEvent::Cancel => {
                    // La souris a été reprise (autre fenêtre, Alt+Tab) : terminer le glissement là où il est.
                    self.pressed = None;
                    if let Some(drag) = self.drag.take() {
                        self.end_drag(gfx, overlays, drag, &mut out)?;
                    }
                }
            },
            PanelId::Popover => {
                let Some(layout) = self.popover_layout() else {
                    return Ok(out);
                };
                match ev {
                    PanelEvent::Move(p) => self.pop_hover = layout.hit(self.popover.to_dip(p)),
                    PanelEvent::Leave => self.pop_hover = None,
                    PanelEvent::Up(p) => {
                        if let Some(i) = layout.hit(self.popover.to_dip(p)) {
                            out.push(UiAction::Command(match self.pop {
                                Some(Pop::Width) => UiCommand::SetWidth(i as u8),
                                _ => UiCommand::SetColor(i as u8),
                            }));
                            self.close_floating();
                        }
                    }
                    _ => {}
                }
            }
            PanelId::Tooltip | PanelId::Settings => {}
        }
        Ok(out)
    }

    /// Fin d'un déplacement : la barre reprend sa taille, le point saisi toujours sous le curseur.
    fn end_drag(&mut self, gfx: &Gfx, overlays: &Overlays, drag: Drag, out: &mut Vec<UiAction>) -> Result<()> {
        if !drag.moved {
            return Ok(());
        }
        let (x, y) = toolbar::drag_origin(cursor(), drag.offset, 1.0);
        // L'écran d'arrivée peut avoir une autre échelle que celui de départ.
        let center = Pt::new(x as f32 + 28.0, y as f32 + 28.0);
        let scale = overlays.monitor_at(center).map(|(_, s)| s).unwrap_or(drag.base_scale);
        self.toolbar.place(gfx, (x, y), (self.layout.w, self.layout.h), theme::BAR_H / 2.0, scale)?;
        out.push(UiAction::Command(UiCommand::SetToolbarPos(x, y)));
        self.dirty = true;
        Ok(())
    }

    /// Vrai tant qu'une animation de la barre est en cours.
    pub fn animating(&self, now: u64) -> bool {
        self.animate
            && (now < self.pill_start + theme::T_PILL_MS || self.popover.appearing(now) || self.tooltip.appearing(now))
    }

    /// Suit le réglage « Afficher les animations » de Windows.
    pub fn set_animate(&mut self, animate: bool) {
        self.animate = animate;
    }

    /// Redessine la barre, le volet et la bulle si nécessaire.
    pub fn draw(&mut self, gfx: &Gfx, app: &App, now: u64) -> Result<()> {
        // La pastille suit l'outil actif : curseur en mode normal, outil courant en mode dessin.
        let active = Item::Tool(if app.draw_mode { Some(app.tool) } else { None });
        if self.pill_item != Some(active) {
            let target = self.layout.rect(active).map(|r| r.x).unwrap_or(0.0);
            let current = self.pill_x(now);
            self.pill_from = if self.pill_item.is_some() && self.animate { current } else { target };
            self.pill_to = target;
            self.pill_start = now;
            self.pill_item = Some(active);
            self.dirty = true;
        }
        if !self.dirty && !self.animating(now) {
            return Ok(());
        }
        self.dirty = false;

        let pill_x = self.pill_x(now);
        let (layout, hover, pressed, mouse) = (&self.layout, self.hover, self.pressed, self.mouse);
        let collapsed = app.cfg.toolbar_collapsed;
        self.toolbar.draw(gfx, now, |p| draw_toolbar(p, app, layout, hover, pressed, mouse, pill_x, active, collapsed))?;

        if let Some(pop) = self.pop {
            let hover = self.pop_hover;
            self.popover.draw(gfx, now, |p| draw_popover(p, app, pop, hover))?;
        }
        if let Some(tip) = &self.tip {
            let w = self.tooltip.size.0;
            self.tooltip.draw(gfx, now, |p| draw_tooltip(p, tip, w))?;
        }
        Ok(())
    }

    fn pill_x(&self, now: u64) -> f32 {
        if !self.animate {
            return self.pill_to;
        }
        let t = now.saturating_sub(self.pill_start) as f32 / theme::T_PILL_MS as f32;
        self.pill_from + (self.pill_to - self.pill_from) * theme::ease(t)
    }
}

fn full(w: f32, h: f32) -> RectF {
    RectF { x: 0.0, y: 0.0, w, h }
}

fn inset(r: RectF, d: f32) -> RectF {
    RectF { x: r.x + d, y: r.y + d, w: r.w - 2.0 * d, h: r.h - 2.0 * d }
}

/// Fond d'un panneau : surface claire et filet.
fn panel_background(p: &Painter, w: f32, h: f32, radius: f32) {
    p.fill_rrect(full(w, h), radius, theme::BG);
    p.stroke_rrect(inset(full(w, h), 0.5), radius - 0.5, theme::LINE, 1.0);
}

#[allow(clippy::too_many_arguments)]
fn draw_toolbar(
    p: &Painter,
    app: &App,
    layout: &Layout,
    hover: Option<Item>,
    pressed: Option<Item>,
    mouse: Option<Pt>,
    pill_x: f32,
    active: Item,
    collapsed: bool,
) {
    let (w, h) = (layout.w, layout.h);
    let radius = h / 2.0;
    p.fill_rrect(full(w, h), radius, theme::BG);
    // Halo radial qui suit le curseur, limité à la forme de la barre.
    if let Some(m) = mouse {
        p.with_rrect_clip(full(w, h), radius, || p.radial_glow(m, 90.0, theme::HOVER.with_alpha(0.9), 0.0));
    }
    p.stroke_rrect(inset(full(w, h), 0.5), radius - 0.5, theme::LINE, 1.0);

    let drawing = app.draw_mode || app.hold;
    for (item, r) in &layout.items {
        let (item, r) = (*item, *r);
        match item {
            Item::Separator => {
                p.line(Pt::new(r.x + 0.5, r.y), Pt::new(r.x + 0.5, r.y + r.h), theme::LINE, 1.0, false);
                continue;
            }
            Item::Logo => {
                if hover == Some(Item::Logo) && !collapsed {
                    p.fill_rrect(r, r.h / 2.0, theme::HOVER);
                }
                icons::draw_logo(p, inset(r, 6.0));
                if drawing {
                    p.stroke_ellipse(r.center(), r.w / 2.0 - 1.0, r.h / 2.0 - 1.0, theme::EMBER, 2.0);
                }
                continue;
            }
            _ => {}
        }

        let is_active = item == active;
        let toggled = match item {
            Item::Halo => app.cfg.halo_on,
            Item::Ripple => app.cfg.ripple_on,
            Item::Trail => app.cfg.trail_on,
            Item::Pin => app.cfg.pinned,
            _ => false,
        };
        if is_active {
            // Dessinée à sa position animée, plus bas.
        } else if toggled || hover == Some(item) {
            let c = if pressed == Some(item) { theme::LINE } else { theme::HOVER };
            p.fill_rrect(r, r.h / 2.0, c);
        }
        if matches!(item, Item::Tool(_)) && is_active {
            continue;
        }
        let color = if toggled { theme::BLUE } else { theme::INK };
        draw_item(p, app, item, r, color);
    }

    // Pastille de l'outil actif, puis son icône en blanc par-dessus.
    if !collapsed {
        if let Some(r) = layout.rect(active) {
            let pill = RectF { x: pill_x, ..r };
            p.fill_rrect(pill, pill.h / 2.0, theme::INK);
            draw_item(p, app, active, pill, theme::WHITE);
        }
    }
}

fn draw_item(p: &Painter, app: &App, item: Item, r: RectF, color: Rgba) {
    if item == Item::Color {
        let c = PALETTE[app.cfg.color as usize % PALETTE.len()];
        p.fill_circle(r.center(), 11.0, c);
        p.stroke_ellipse(r.center(), 11.0, 11.0, theme::LINE, 1.5);
    } else {
        icons::draw(p, item, inset(r, 9.0), color);
    }
}

fn draw_popover(p: &Painter, app: &App, pop: Pop, hover: Option<usize>) {
    let layout = match pop {
        Pop::Color => toolbar::color_popover(),
        Pop::Width => toolbar::width_popover(),
    };
    panel_background(p, layout.w, layout.h, theme::R_POPOVER);
    for (i, r) in layout.cells.iter().enumerate() {
        let selected = match pop {
            Pop::Color => app.cfg.color as usize == i,
            Pop::Width => app.cfg.width as usize == i,
        };
        if hover == Some(i) || (selected && pop == Pop::Width) {
            p.fill_rrect(*r, r.h / 2.0, theme::HOVER);
        }
        match pop {
            Pop::Color => {
                p.fill_circle(r.center(), 12.0, PALETTE[i]);
                p.stroke_ellipse(r.center(), 12.0, 12.0, theme::LINE, 1.5);
                if selected {
                    p.stroke_ellipse(r.center(), 16.0, 16.0, theme::INK, 2.0);
                }
            }
            Pop::Width => {
                let c = r.center();
                let color = if selected { theme::BLUE } else { theme::INK };
                p.line(Pt::new(c.x - 11.0, c.y), Pt::new(c.x + 11.0, c.y), color, WIDTHS[i], true);
            }
        }
    }
}

fn draw_tooltip(p: &Painter, tip: &(String, Option<String>), w: f32) {
    panel_background(p, w, TIP_H, TIP_H / 2.0);
    let name_w = p.gfx.text_width(&tip.0, Family::Body, 13.0, 500);
    p.text(&tip.0, Family::Body, 13.0, 500, Align::Left, RectF { x: 14.0, y: 0.0, w: name_w + 4.0, h: TIP_H - 1.0 }, theme::INK);
    if let Some(key) = &tip.1 {
        let key_w = p.gfx.text_width(key, Family::Mono, 11.0, 500) + 16.0;
        let pill = RectF { x: w - key_w - 6.0, y: 6.0, w: key_w, h: TIP_H - 12.0 };
        p.fill_rrect(pill, pill.h / 2.0, SURFACE);
        p.text(key, Family::Mono, 11.0, 500, Align::Center, pill, theme::MUTED);
    }
}
