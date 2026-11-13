//! Barre d'outils : disposition et libellés. Pur, en pixels logiques.

use crate::geom::{Pt, RectF};
use crate::i18n::Lang;
use crate::shapes::Tool;
use crate::shortcuts::{Action, Bindings};
use crate::ui::theme::{BAR_H, BTN};

/// Marge intérieure de la barre.
pub const PAD: f32 = 8.0;
/// Écart entre deux boutons voisins.
pub const GAP: f32 = 4.0;
/// Espace de part et d'autre d'un filet de séparation.
const SEP_SPACE: f32 = 8.0;
/// Hauteur d'un filet de séparation.
const SEP_H: f32 = 24.0;

/// Facteur de réduction de la barre pendant qu'on la déplace.
pub const DRAG_SHRINK: f32 = 0.5;

/// Coin haut-gauche de la barre pendant un déplacement : le point saisi (`grab`, mesuré à
/// taille normale depuis ce coin) reste sous le curseur, la barre étant réduite de `factor`.
pub fn drag_origin(cursor: (i32, i32), grab: (i32, i32), factor: f32) -> (i32, i32) {
    (cursor.0 - (grab.0 as f32 * factor).round() as i32, cursor.1 - (grab.1 as f32 * factor).round() as i32)
}

/// Élément de la barre d'outils.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Item {
    /// Poignée de déplacement ; double-clic pour replier.
    Logo,
    /// Outil de dessin ; `None` est le curseur (mode normal).
    Tool(Option<Tool>),
    Color,
    Width,
    Halo,
    Ripple,
    Trail,
    Pin,
    Clear,
    Settings,
    /// Ouvre la page d'aide.
    Help,
    /// Ferme Ekspointer.
    Quit,
    Separator,
}

/// Disposition de la barre : taille et rectangle de chaque élément.
#[derive(Clone, PartialEq, Debug)]
pub struct Layout {
    pub w: f32,
    pub h: f32,
    pub items: Vec<(Item, RectF)>,
}

impl Layout {
    /// Élément cliquable sous un point (les filets ne le sont pas).
    pub fn hit(&self, p: Pt) -> Option<Item> {
        self.items.iter().find(|(i, r)| *i != Item::Separator && r.contains(p)).map(|(i, _)| *i)
    }

    /// Vrai si un point de la barre sert de poignée de déplacement : tout sauf un bouton
    /// (le logo, les filets, les marges et les espaces entre boutons).
    pub fn is_handle(&self, p: Pt) -> bool {
        let inside = p.x >= 0.0 && p.x < self.w && p.y >= 0.0 && p.y < self.h;
        inside && matches!(self.hit(p), None | Some(Item::Logo))
    }

    pub fn rect(&self, item: Item) -> Option<RectF> {
        self.items.iter().find(|(i, _)| *i == item).map(|(_, r)| *r)
    }
}

/// Groupes d'éléments, séparés par des filets.
fn groups() -> Vec<Vec<Item>> {
    vec![
        vec![Item::Logo],
        vec![
            Item::Tool(None),
            Item::Tool(Some(Tool::Arrow)),
            Item::Tool(Some(Tool::Rect)),
            Item::Tool(Some(Tool::Ellipse)),
            Item::Tool(Some(Tool::Free)),
            Item::Tool(Some(Tool::Highlighter)),
            Item::Tool(Some(Tool::Number)),
        ],
        vec![Item::Color, Item::Width],
        vec![Item::Halo, Item::Ripple, Item::Trail],
        vec![Item::Pin, Item::Clear, Item::Settings, Item::Help],
        vec![Item::Quit],
    ]
}

/// Disposition de la barre, repliée (disque avec le logo) ou dépliée.
pub fn layout(collapsed: bool) -> Layout {
    let y = (BAR_H - BTN) / 2.0;
    if collapsed {
        return Layout { w: BAR_H, h: BAR_H, items: vec![(Item::Logo, RectF { x: PAD, y, w: BTN, h: BTN })] };
    }
    let mut items = Vec::new();
    let mut x = PAD;
    for (g, group) in groups().into_iter().enumerate() {
        if g > 0 {
            x += SEP_SPACE - GAP;
            items.push((Item::Separator, RectF { x, y: (BAR_H - SEP_H) / 2.0, w: 1.0, h: SEP_H }));
            x += 1.0 + SEP_SPACE;
        }
        for item in group {
            items.push((item, RectF { x, y, w: BTN, h: BTN }));
            x += BTN + GAP;
        }
    }
    Layout { w: x - GAP + PAD, h: BAR_H, items }
}

/// Nom d'un outil dans l'interface.
pub fn tool_label(tool: Tool) -> &'static str {
    match tool {
        Tool::Arrow => "Flèche",
        Tool::Rect => "Rectangle",
        Tool::Ellipse => "Ellipse",
        Tool::Free => "Trait libre",
        Tool::Highlighter => "Surligneur",
        Tool::Number => "Numéros",
    }
}

/// Raccourci qui sélectionne un outil.
pub fn tool_action(tool: Tool) -> Action {
    match tool {
        Tool::Arrow => Action::ToolArrow,
        Tool::Rect => Action::ToolRect,
        Tool::Ellipse => Action::ToolEllipse,
        Tool::Free => Action::ToolFree,
        Tool::Highlighter => Action::ToolHighlighter,
        Tool::Number => Action::ToolNumber,
    }
}

/// Nom d'un outil dans une langue.
pub fn tool_label_in(tool: Tool, lang: Lang) -> &'static str {
    tool_action(tool).label_in(lang)
}

/// Bulle d'aide d'un élément : nom et, s'il y en a un, raccourci.
pub fn tooltip(item: Item, bindings: &Bindings) -> Option<(String, Option<String>)> {
    tooltip_in(item, bindings, Lang::Fr)
}

/// Bulle d'aide dans une langue.
pub fn tooltip_in(item: Item, bindings: &Bindings, lang: Lang) -> Option<(String, Option<String>)> {
    let key = |a: Action| Some(bindings.get(a).display(lang));
    let fr = lang == Lang::Fr;
    let (name, shortcut) = match item {
        Item::Logo => ("Ekspointer", None),
        Item::Tool(None) => (if fr { "Curseur" } else { "Cursor" }, key(Action::ExitDraw)),
        Item::Tool(Some(tool)) => (tool_label_in(tool, lang), key(tool_action(tool))),
        Item::Color => (if fr { "Couleur" } else { "Color" }, None),
        Item::Width => (if fr { "Épaisseur" } else { "Thickness" }, None),
        Item::Halo => ("Halo", key(Action::ToggleHalo)),
        Item::Ripple => (if fr { "Onde au clic" } else { "Click ripple" }, key(Action::ToggleRipple)),
        Item::Trail => (if fr { "Traînée laser" } else { "Laser trail" }, key(Action::ToggleTrail)),
        Item::Pin => (if fr { "Garder les tracés à l'écran" } else { "Keep drawings on screen" }, key(Action::TogglePin)),
        Item::Clear => (Action::ClearAll.label_in(lang), key(Action::ClearAll)),
        Item::Settings => (crate::i18n::t(lang, crate::i18n::Text::Settings), key(Action::OpenSettings)),
        Item::Help => (crate::i18n::t(lang, crate::i18n::Text::Help), key(Action::OpenHelp)),
        Item::Quit => (if fr { "Quitter Ekspointer" } else { "Quit Ekspointer" }, None),
        Item::Separator => return None,
    };
    Some((name.to_string(), shortcut))
}

/// Disposition d'un volet : taille et cellules cliquables.
#[derive(Clone, PartialEq, Debug)]
pub struct Popover {
    pub w: f32,
    pub h: f32,
    pub cells: Vec<RectF>,
}

impl Popover {
    /// Indice de la cellule sous un point.
    pub fn hit(&self, p: Pt) -> Option<usize> {
        self.cells.iter().position(|r| r.contains(p))
    }
}

fn popover(count: usize, cell: f32) -> Popover {
    let cells = (0..count).map(|i| RectF { x: PAD + i as f32 * (cell + GAP), y: PAD, w: cell, h: cell }).collect();
    Popover { w: 2.0 * PAD + count as f32 * cell + (count as f32 - 1.0) * GAP, h: 2.0 * PAD + cell, cells }
}

/// Volet des couleurs.
pub fn color_popover() -> Popover {
    popover(crate::geom::PALETTE.len(), 36.0)
}

/// Volet des trois épaisseurs.
pub fn width_popover() -> Popover {
    popover(3, BTN)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expanded_layout() {
        let l = layout(false);
        assert_eq!(l.h, 56.0);
        let tools: Vec<_> = l.items.iter().filter_map(|(i, _)| if let Item::Tool(t) = i { Some(*t) } else { None }).collect();
        assert_eq!(
            tools,
            vec![
                None,
                Some(Tool::Arrow),
                Some(Tool::Rect),
                Some(Tool::Ellipse),
                Some(Tool::Free),
                Some(Tool::Highlighter),
                Some(Tool::Number)
            ]
        );
        let order: Vec<_> = l.items.iter().map(|(i, _)| *i).filter(|i| !matches!(i, Item::Tool(_) | Item::Separator)).collect();
        assert_eq!(
            order,
            vec![
                Item::Logo,
                Item::Color,
                Item::Width,
                Item::Halo,
                Item::Ripple,
                Item::Trail,
                Item::Pin,
                Item::Clear,
                Item::Settings,
                Item::Help,
                Item::Quit
            ]
        );
        assert_eq!(l.items.iter().filter(|(i, _)| *i == Item::Separator).count(), 5);
        for (i, r) in &l.items {
            if !matches!(i, Item::Separator) {
                assert_eq!((r.w, r.h), (40.0, 40.0), "{i:?}");
                assert_eq!(r.y, 8.0, "{i:?}");
            }
        }
        for w in l.items.windows(2) {
            assert!(w[0].1.x + w[0].1.w <= w[1].1.x, "{:?} chevauche {:?}", w[0].0, w[1].0);
        }
        let last = l.items.last().unwrap().1;
        assert_eq!(last.x + last.w + 8.0, l.w);
    }

    #[test]
    fn collapsed_is_a_56_disc() {
        let l = layout(true);
        assert_eq!((l.w, l.h), (56.0, 56.0));
        assert_eq!(l.items.len(), 1);
        assert_eq!(l.items[0].0, Item::Logo);
        assert_eq!(l.items[0].1, RectF { x: 8.0, y: 8.0, w: 40.0, h: 40.0 });
    }

    #[test]
    fn hit_test() {
        let l = layout(false);
        let r = l.rect(Item::Clear).unwrap();
        assert_eq!(l.hit(Pt::new(r.x + 20.0, r.y + 20.0)), Some(Item::Clear));
        assert_eq!(l.hit(Pt::new(-5.0, 10.0)), None);
        let sep = l.items.iter().find(|(i, _)| *i == Item::Separator).unwrap().1;
        assert_eq!(l.hit(Pt::new(sep.x + 0.5, sep.y + 5.0)), None); // un filet n'est pas cliquable
        assert_eq!(l.rect(Item::Tool(Some(Tool::Rect))).map(|r| r.w), Some(40.0));
    }

    #[test]
    fn tooltips_show_french_name_and_shortcut() {
        let b = Bindings::defaults();
        let t = |i| tooltip(i, &b);
        assert_eq!(t(Item::Tool(Some(Tool::Arrow))), Some(("Flèche".into(), Some("F".into()))));
        assert_eq!(t(Item::Tool(Some(Tool::Ellipse))), Some(("Ellipse".into(), Some("C".into()))));
        assert_eq!(t(Item::Tool(None)), Some(("Curseur".into(), Some("Échap".into()))));
        assert_eq!(t(Item::Clear), Some(("Tout effacer".into(), Some("Ctrl+Alt+E".into()))));
        assert_eq!(t(Item::Halo), Some(("Halo".into(), Some("Ctrl+Alt+H".into()))));
        assert_eq!(t(Item::Pin), Some(("Garder les tracés à l'écran".into(), Some("Ctrl+Alt+G".into()))));
        assert_eq!(t(Item::Ripple), Some(("Onde au clic".into(), Some("Ctrl+Alt+O".into()))));
        assert_eq!(t(Item::Trail), Some(("Traînée laser".into(), Some("Ctrl+Alt+T".into()))));
        assert_eq!(t(Item::Color), Some(("Couleur".into(), None)));
        assert_eq!(t(Item::Width), Some(("Épaisseur".into(), None)));
        assert_eq!(t(Item::Settings), Some(("Réglages".into(), Some("Ctrl+Alt+R".into()))));
        assert_eq!(t(Item::Help), Some(("Aide".into(), Some("Ctrl+Alt+A".into()))));
        assert_eq!(t(Item::Separator), None);
    }

    #[test]
    fn tooltips_follow_rebinding() {
        let mut b = Bindings::defaults();
        b.set(Action::ToolArrow, "A".parse().unwrap()).unwrap();
        assert_eq!(tooltip(Item::Tool(Some(Tool::Arrow)), &b), Some(("Flèche".into(), Some("A".into()))));
    }

    #[test]
    fn popover_layouts() {
        let c = color_popover();
        assert_eq!(c.cells.len(), 10);
        let w = width_popover();
        assert_eq!(w.cells.len(), 3);
        for p in [&c, &w] {
            for pair in p.cells.windows(2) {
                assert!(pair[0].x + pair[0].w <= pair[1].x);
            }
            let last = p.cells.last().unwrap();
            assert!(last.x + last.w <= p.w && last.y + last.h <= p.h);
            assert_eq!(p.hit(Pt::new(p.cells[1].x + 2.0, p.cells[1].y + 2.0)), Some(1));
            assert_eq!(p.hit(Pt::new(-1.0, -1.0)), None);
        }
    }

    #[test]
    fn tooltips_in_english() {
        use crate::i18n::Lang;
        let b = Bindings::defaults();
        let t = |i| tooltip_in(i, &b, Lang::En);
        assert_eq!(t(Item::Tool(Some(Tool::Arrow))), Some(("Arrow".into(), Some("F".into()))));
        assert_eq!(t(Item::Tool(None)), Some(("Cursor".into(), Some("Esc".into()))));
        assert_eq!(t(Item::Tool(Some(Tool::Free))), Some(("Freehand".into(), Some("D".into()))));
        assert_eq!(t(Item::Clear), Some(("Clear all".into(), Some("Ctrl+Alt+E".into()))));
        assert_eq!(t(Item::Ripple), Some(("Click ripple".into(), Some("Ctrl+Alt+O".into()))));
        assert_eq!(t(Item::Trail), Some(("Laser trail".into(), Some("Ctrl+Alt+T".into()))));
        assert_eq!(t(Item::Pin), Some(("Keep drawings on screen".into(), Some("Ctrl+Alt+G".into()))));
        assert_eq!(t(Item::Width), Some(("Thickness".into(), None)));
        assert_eq!(t(Item::Settings), Some(("Settings".into(), Some("Ctrl+Alt+R".into()))));
        assert_eq!(t(Item::Help), Some(("Help".into(), Some("Ctrl+Alt+A".into()))));
        assert_eq!(tooltip_in(Item::Clear, &b, Lang::Fr), tooltip(Item::Clear, &b));
    }

    #[test]
    fn anything_but_a_button_moves_the_bar() {
        let l = layout(false);
        // Le logo, les filets, les marges et les espaces entre boutons servent de poignée.
        assert!(l.is_handle(Pt::new(l.rect(Item::Logo).unwrap().x + 5.0, 28.0)));
        let sep = l.items.iter().find(|(i, _)| *i == Item::Separator).unwrap().1;
        assert!(l.is_handle(Pt::new(sep.x, sep.y + 5.0)));
        assert!(l.is_handle(Pt::new(3.0, 3.0)));
        let arrow = l.rect(Item::Tool(Some(Tool::Arrow))).unwrap();
        assert!(l.is_handle(Pt::new(arrow.x + arrow.w + 2.0, 28.0))); // entre deux boutons
        assert!(l.is_handle(Pt::new(arrow.x + 20.0, 3.0))); // au-dessus d'un bouton
        // Un bouton, lui, se clique.
        assert!(!l.is_handle(Pt::new(arrow.x + 20.0, arrow.y + 20.0)));
        assert!(!l.is_handle(Pt::new(l.rect(Item::Settings).unwrap().x + 20.0, 28.0)));
        assert!(!l.is_handle(Pt::new(l.rect(Item::Help).unwrap().x + 20.0, 28.0)));
        // Repliée, la barre entière est une poignée.
        assert!(layout(true).is_handle(Pt::new(28.0, 28.0)));
    }

    #[test]
    fn bar_shrinks_by_half_while_dragged_and_stays_under_the_cursor() {
        assert_eq!(DRAG_SHRINK, 0.5);
        // Le point saisi reste sous le curseur, à taille normale comme à taille réduite.
        assert_eq!(drag_origin((1000, 500), (200, 40), 1.0), (800, 460));
        assert_eq!(drag_origin((1000, 500), (200, 40), DRAG_SHRINK), (900, 480));
        assert_eq!(drag_origin((-50, 10), (0, 0), DRAG_SHRINK), (-50, 10));
    }

    #[test]
    fn quit_button_closes_the_bar_at_its_far_end() {
        use crate::i18n::Lang;
        let l = layout(false);
        // Dernier élément, isolé des autres boutons par un filet pour éviter un clic par mégarde.
        let n = l.items.len();
        assert_eq!(l.items[n - 1].0, Item::Quit);
        assert_eq!(l.items[n - 2].0, Item::Separator);
        assert!(!l.is_handle(Pt::new(l.rect(Item::Quit).unwrap().x + 20.0, 28.0)));
        let b = Bindings::defaults();
        assert_eq!(tooltip(Item::Quit, &b), Some(("Quitter Ekspointer".into(), None)));
        assert_eq!(tooltip_in(Item::Quit, &b, Lang::En), Some(("Quit Ekspointer".into(), None)));
        // Repliée, la barre ne montre que son logo.
        assert!(layout(true).rect(Item::Quit).is_none());
    }
}
