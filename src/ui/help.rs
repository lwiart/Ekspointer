//! Page d'aide : mode d'emploi court et crédits. Pur, en pixels logiques.

use crate::geom::{Pt, RectF};
use crate::i18n::Lang;
use crate::shapes::Tool;
use crate::shortcuts::{Action, Bindings};
use crate::ui::settings::{HEADER_H, PAD, PANEL_H, PANEL_W};
use crate::ui::toolbar::{self, Item};

/// Auteur d'Ekspointer.
pub const AUTHOR: &str = "Laurent Wiart";
/// Société de l'auteur.
pub const COMPANY: &str = "Eksplodo";
/// Site de la société.
pub const SITE: &str = "eksplodo.fr";
/// Adresse ouverte par le lien des crédits.
pub const SITE_URL: &str = "https://eksplodo.fr/";
/// Largeur de la zone cliquable d'un lien, centrée sur sa ligne.
const LINK_W: f32 = 150.0;

/// Ligne de la page d'aide.
#[derive(Clone, PartialEq, Debug)]
pub enum Line {
    Title(String),
    Text(String),
    /// Libellé d'une action et son raccourci actuel.
    Key(String, String),
    /// Ligne de crédit, centrée.
    Credit(String),
    /// Bouton de la barre d'outils : son icône, son nom et, s'il en a un, son raccourci.
    Button(Item, String, Option<String>),
    /// Lien cliquable, centré : texte affiché et adresse ouverte dans le navigateur.
    Link(String, &'static str),
    Gap,
}

impl Line {
    fn height(&self) -> f32 {
        match self {
            Line::Title(_) => 36.0,
            Line::Text(_) | Line::Credit(_) => 22.0,
            Line::Link(..) => 26.0,
            Line::Key(..) | Line::Button(..) => 28.0,
            Line::Gap => 10.0,
        }
    }
}

/// Contenu de la page d'aide : mode d'emploi court, raccourcis actuels et crédits.
pub fn lines(lang: Lang, bindings: &Bindings, version: &str) -> Vec<Line> {
    let fr = lang == Lang::Fr;
    let pick = |f: &str, e: &str| if fr { f.to_string() } else { e.to_string() };
    let title = |f: &str, e: &str| Line::Title(pick(f, e));
    let text = |f: &str, e: &str| Line::Text(pick(f, e));
    let credit = |f: &str, e: &str| Line::Credit(pick(f, e));
    let key = |a: Action| Line::Key(a.label_in(lang).to_string(), bindings.get(a).display(lang));

    let button = |item: Item| {
        let (name, shortcut) = toolbar::tooltip_in(item, bindings, lang).unwrap_or_default();
        Line::Button(item, name, shortcut)
    };

    let mut out = vec![
        title("Dessiner", "Drawing"),
        text("Cliquez sur un outil : la souris trace.", "Click a tool: the mouse draws."),
        text("Échap ou l'outil Curseur rend la main.", "Esc or the Cursor tool gives control back."),
        text("Ou maintenez le raccourci ci-dessous et glissez.", "Or hold the shortcut below and drag."),
        text("La molette fait défiler la page, même en mode dessin.", "The wheel scrolls the page, even in drawing mode."),
        key(Action::ToggleDraw),
        key(Action::HoldDraw),
        key(Action::ClearAll),
        Line::Gap,
        title("Outils", "Tools"),
        button(Item::Tool(None)),
    ];
    for tool in [Tool::Arrow, Tool::Rect, Tool::Ellipse, Tool::Free, Tool::Highlighter, Tool::Number] {
        out.push(button(Item::Tool(Some(tool))));
    }
    let colors = format!("{} … {}", bindings.get(Action::Color(0)).display(lang), bindings.get(Action::Color(9)).display(lang));
    out.extend([
        Line::Gap,
        title("Pendant le dessin", "While drawing"),
        Line::Key(pick("Couleurs 1 à 10", "Colors 1 to 10"), colors),
        key(Action::Undo),
        key(Action::ExitDraw),
        Line::Key(pick("Angle de 15°, carré, cercle", "15° angle, square, circle"), pick("Maj", "Shift")),
        Line::Gap,
        title("Barre d'outils", "Toolbar"),
    ]);
    for item in [
        Item::Color,
        Item::Width,
        Item::Halo,
        Item::Ripple,
        Item::Trail,
        Item::Pin,
        Item::Clear,
        Item::Settings,
        Item::Help,
        Item::Quit,
    ] {
        out.push(button(item));
    }
    out.extend([
        text("Glissez la barre par son logo ou un espace vide.", "Drag the bar by its logo or any empty space."),
        text("Double-clic sur le logo : replier ou déplier.", "Double-click the logo to collapse or expand."),
        Line::Gap,
        title("Partage d'écran", "Screen sharing"),
        text("Partagez l'écran entier pour montrer les tracés.", "Share the entire screen to show drawings."),
        text("Une fenêtre ou un onglet seul ne les transmet pas.", "A single window or tab does not carry them."),
        Line::Gap,
        title("Crédits", "Credits"),
        Line::Credit(format!("Ekspointer {version}")),
        credit(&format!("Conçu par {AUTHOR}"), &format!("Created by {AUTHOR}")),
        credit(
            &format!("{COMPANY}, formation et conseil en automatisation et IA"),
            &format!("{COMPANY}, training and consulting in automation and AI"),
        ),
        Line::Link(SITE.to_string(), SITE_URL),
        Line::Gap,
        text("Polices Instrument Sans et Noto Sans Mono (OFL).", "Fonts: Instrument Sans and Noto Sans Mono (OFL)."),
    ]);
    out
}

/// Marge au-dessus de la première ligne.
const TOP: f32 = 8.0;

/// Rectangle de chaque ligne dans le panneau, pour un défilement donné.
pub fn layout(lines: &[Line], scroll: f32) -> Vec<RectF> {
    let mut y = HEADER_H + TOP - scroll;
    lines
        .iter()
        .map(|line| {
            let r = RectF { x: PAD, y, w: PANEL_W - 2.0 * PAD, h: line.height() };
            y += line.height();
            r
        })
        .collect()
}

/// Zone cliquable d'un lien : le milieu de sa ligne, pas la ligne entière.
pub fn link_rect(line: RectF) -> RectF {
    RectF { x: line.x + (line.w - LINK_W) / 2.0, y: line.y, w: LINK_W, h: line.h }
}

/// Adresse du lien sous un point, s'il y en a un (et s'il n'est pas caché sous l'en-tête).
pub fn link_at(lines: &[Line], scroll: f32, p: Pt) -> Option<&'static str> {
    if p.y < HEADER_H || p.y >= PANEL_H {
        return None;
    }
    lines.iter().zip(layout(lines, scroll)).find_map(|(line, r)| match line {
        Line::Link(_, url) if link_rect(r).contains(p) => Some(*url),
        _ => None,
    })
}

/// Hauteur totale du contenu défilable.
pub fn content_height(lines: &[Line]) -> f32 {
    TOP + lines.iter().map(Line::height).sum::<f32>() + PAD
}

/// Défilement maximal.
pub fn max_scroll(lines: &[Line]) -> f32 {
    (content_height(lines) - (PANEL_H - HEADER_H)).max(0.0)
}

pub fn clamp_scroll(lines: &[Line], scroll: f32) -> f32 {
    scroll.clamp(0.0, max_scroll(lines))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all_text(lines: &[Line]) -> String {
        lines
            .iter()
            .map(|l| match l {
                Line::Title(s) | Line::Text(s) | Line::Credit(s) => s.clone(),
                Line::Link(label, _) => label.clone(),
                Line::Button(_, label, key) => format!("{label} {}", key.clone().unwrap_or_default()),
                Line::Key(label, key) => format!("{label} {key}"),
                Line::Gap => String::new(),
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn credits_name_the_author_and_the_company_in_both_languages() {
        for lang in Lang::all() {
            let text = all_text(&lines(lang, &Bindings::defaults(), "1.2.3"));
            assert!(text.contains("Laurent Wiart"), "{lang:?}");
            assert!(text.contains("Eksplodo"), "{lang:?}");
            assert!(text.contains("eksplodo.fr"), "{lang:?}");
            assert!(text.contains("Ekspointer 1.2.3"), "{lang:?}");
        }
        assert_eq!((AUTHOR, COMPANY, SITE), ("Laurent Wiart", "Eksplodo", "eksplodo.fr"));
    }

    #[test]
    fn page_is_translated() {
        let b = Bindings::defaults();
        let (fr, en) = (all_text(&lines(Lang::Fr, &b, "1")), all_text(&lines(Lang::En, &b, "1")));
        assert!(fr.contains("Crédits") && en.contains("Credits"));
        assert!(fr.contains("Dessiner") && en.contains("Drawing"));
        assert!(fr.contains("Partage d'écran") && en.contains("Screen sharing"));
        assert_ne!(fr, en);
    }

    #[test]
    fn shortcuts_shown_are_the_current_ones_in_the_current_language() {
        let mut b = Bindings::defaults();
        let key = |lines: &[Line], label: &str| {
            lines.iter().find_map(|l| match l {
                Line::Key(l, k) if l == label => Some(k.clone()),
                _ => None,
            })
        };
        let fr = lines(Lang::Fr, &b, "1");
        assert_eq!(key(&fr, "Activer le dessin").as_deref(), Some("Ctrl+Alt+D"));
        assert_eq!(key(&fr, "Quitter le mode dessin").as_deref(), Some("Échap"));
        let en = lines(Lang::En, &b, "1");
        assert_eq!(key(&en, "Toggle drawing").as_deref(), Some("Ctrl+Alt+D"));
        assert_eq!(key(&en, "Leave drawing mode").as_deref(), Some("Esc"));

        b.set(Action::ToggleDraw, "2×Ctrl".parse().unwrap()).unwrap();
        assert_eq!(key(&lines(Lang::Fr, &b, "1"), "Activer le dessin").as_deref(), Some("2×Ctrl"));
    }

    #[test]
    fn every_line_fits_on_one_line() {
        // Le texte n'est pas renvoyé à la ligne : chaque ligne doit tenir dans la largeur du panneau.
        for lang in Lang::all() {
            for line in lines(lang, &Bindings::defaults(), "0.2.0") {
                if let Line::Text(s) | Line::Credit(s) = &line {
                    assert!(s.chars().count() <= 54, "trop long ({}) : {s}", s.chars().count());
                }
            }
        }
    }

    #[test]
    fn layout_stacks_lines_and_scrolls() {
        let ls = lines(Lang::Fr, &Bindings::defaults(), "1");
        let l = layout(&ls, 0.0);
        assert_eq!(l.len(), ls.len());
        assert_eq!(l[0].y, HEADER_H + 8.0);
        for pair in l.windows(2) {
            assert_eq!(pair[0].y + pair[0].h, pair[1].y);
        }
        for r in &l {
            assert_eq!((r.x, r.w), (PAD, PANEL_W - 2.0 * PAD));
        }
        assert_eq!(layout(&ls, 50.0)[2].y, l[2].y - 50.0);
        assert!(max_scroll(&ls) > 0.0, "la page d'aide dépasse la hauteur du panneau");
        assert_eq!(clamp_scroll(&ls, -10.0), 0.0);
        assert_eq!(clamp_scroll(&ls, 1e6), max_scroll(&ls));
        let last = layout(&ls, max_scroll(&ls)).last().copied().unwrap();
        assert!(last.y + last.h <= PANEL_H);
    }

    #[test]
    fn credits_link_to_the_website() {
        assert_eq!(SITE_URL, "https://eksplodo.fr/");
        for lang in Lang::all() {
            let ls = lines(lang, &Bindings::defaults(), "1");
            let links: Vec<_> = ls.iter().filter(|l| matches!(l, Line::Link(..))).collect();
            assert_eq!(links, vec![&Line::Link("eksplodo.fr".to_string(), SITE_URL)], "{lang:?}");
        }
    }

    #[test]
    fn clicking_the_link_and_only_the_link_opens_the_website() {
        let ls = lines(Lang::Fr, &Bindings::defaults(), "1");
        let scroll = max_scroll(&ls); // les crédits sont en bas de page
        let i = ls.iter().position(|l| matches!(l, Line::Link(..))).unwrap();
        let r = link_rect(layout(&ls, scroll)[i]);
        assert!(r.w < PANEL_W - 2.0 * PAD, "la zone cliquable se limite au lien, pas à toute la ligne");
        let center = crate::geom::Pt::new(r.x + r.w / 2.0, r.y + r.h / 2.0);
        assert_eq!(link_at(&ls, scroll, center), Some(SITE_URL));
        // À côté du lien, sur une autre ligne, ou quand la ligne est cachée sous l'en-tête : rien.
        assert_eq!(link_at(&ls, scroll, crate::geom::Pt::new(PAD + 2.0, center.y)), None);
        assert_eq!(link_at(&ls, scroll, crate::geom::Pt::new(center.x, center.y - 60.0)), None);
        assert_eq!(link_at(&ls, 0.0, center), None);
        let hidden = layout(&ls, scroll)[i].y - HEADER_H + 30.0;
        assert_eq!(link_at(&ls, scroll + hidden, crate::geom::Pt::new(center.x, HEADER_H - 5.0)), None);
    }

    #[test]
    fn every_toolbar_button_is_explained_with_its_icon() {
        use crate::ui::toolbar::{self, Item};
        for lang in Lang::all() {
            let b = Bindings::defaults();
            let ls = lines(lang, &b, "1");
            for (item, _) in toolbar::layout(false).items {
                if matches!(item, Item::Logo | Item::Separator) {
                    continue;
                }
                let found: Vec<_> = ls.iter().filter(|l| matches!(l, Line::Button(i, ..) if *i == item)).collect();
                assert_eq!(found.len(), 1, "{item:?} en {lang:?}");
                // Même nom et même raccourci que la bulle du bouton.
                let (name, key) = toolbar::tooltip_in(item, &b, lang).unwrap();
                assert_eq!(found[0], &Line::Button(item, name, key));
            }
        }
    }

    #[test]
    fn help_mentions_the_wheel() {
        let b = Bindings::defaults();
        assert!(all_text(&lines(Lang::Fr, &b, "1")).contains("molette"));
        assert!(all_text(&lines(Lang::En, &b, "1")).contains("wheel"));
    }
}
