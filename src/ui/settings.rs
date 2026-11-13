//! Panneau de réglages : lignes, disposition et calculs des contrôles. Pur, en pixels logiques.

use crate::geom::{Pt, RectF};
use crate::i18n::Lang;
use crate::shortcuts::{Action, Shortcut};

/// Largeur du panneau.
pub const PANEL_W: f32 = 420.0;
/// Hauteur du panneau.
pub const PANEL_H: f32 = 560.0;
/// Hauteur de l'en-tête fixe (titre et bouton de fermeture).
pub const HEADER_H: f32 = 60.0;
/// Marge latérale du contenu.
pub const PAD: f32 = 20.0;
/// Hauteur d'un titre de section.
const SECTION_H: f32 = 44.0;
/// Hauteur d'une ligne de réglage.
const ROW_H: f32 = 44.0;
/// Hauteur d'une ligne de raccourci.
const BINDING_H: f32 = 38.0;
/// Largeur de la zone de contrôle, à droite d'une ligne.
pub const CONTROL_W: f32 = 170.0;

/// Ligne du panneau de réglages.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Row {
    /// Choix de la langue : premier réglage du panneau.
    Language,
    Header(&'static str),
    HoldSeconds,
    Width,
    HaloSize,
    HaloFollowsColor,
    Binding(Action),
    ResetBindings,
    Autostart,
    HideFromCapture,
}

impl Row {
    /// Libellé affiché à gauche de la ligne.
    pub fn label(self) -> &'static str {
        match self {
            Row::Language => "Langue",
            Row::Header(title) => title,
            Row::HoldSeconds => "Durée avant effacement",
            Row::Width => "Épaisseur du trait",
            Row::HaloSize => "Taille du halo",
            Row::HaloFollowsColor => "Halo de la couleur du dessin",
            Row::Binding(action) => action.label(),
            Row::ResetBindings => "Rétablir les raccourcis par défaut",
            Row::Autostart => "Lancer au démarrage de Windows",
            Row::HideFromCapture => "Masquer la barre du partage d'écran",
        }
    }

    /// Libellé dans une langue.
    pub fn label_in(self, lang: Lang) -> &'static str {
        if lang == Lang::Fr {
            return self.label();
        }
        match self {
            Row::Language => "Language",
            Row::Header("Dessin") => "Drawing",
            Row::Header("Pointeur") => "Pointer",
            Row::Header("Raccourcis") => "Shortcuts",
            Row::Header("Général") => "General",
            Row::Header(title) => title,
            Row::HoldSeconds => "Time before fade-out",
            Row::Width => "Stroke thickness",
            Row::HaloSize => "Halo size",
            Row::HaloFollowsColor => "Halo uses the drawing color",
            Row::Binding(action) => action.label_in(lang),
            Row::ResetBindings => "Restore default shortcuts",
            Row::Autostart => "Start with Windows",
            Row::HideFromCapture => "Hide toolbar from screen sharing",
        }
    }

    fn height(self) -> f32 {
        match self {
            Row::Header(_) => SECTION_H,
            Row::Binding(_) => BINDING_H,
            Row::ResetBindings => ROW_H + 8.0,
            _ => ROW_H,
        }
    }
}

/// Lignes du panneau, dans l'ordre d'affichage.
pub fn rows() -> Vec<Row> {
    let mut rows = vec![
        Row::Header("Général"),
        Row::Language,
        Row::Autostart,
        Row::HideFromCapture,
        Row::Header("Dessin"),
        Row::HoldSeconds,
        Row::Width,
        Row::Header("Pointeur"),
        Row::HaloSize,
        Row::HaloFollowsColor,
        Row::Header("Raccourcis"),
    ];
    rows.extend(Action::all().into_iter().map(Row::Binding));
    rows.push(Row::ResetBindings);
    rows
}

/// Rectangle de chaque ligne dans le panneau, pour un défilement donné.
pub fn layout(scroll: f32) -> Vec<(Row, RectF)> {
    let mut y = HEADER_H - scroll;
    rows()
        .into_iter()
        .map(|row| {
            let r = RectF { x: PAD, y, w: PANEL_W - 2.0 * PAD, h: row.height() };
            y += row.height();
            (row, r)
        })
        .collect()
}

/// Hauteur totale du contenu défilable.
pub fn content_height() -> f32 {
    rows().into_iter().map(Row::height).sum::<f32>() + PAD
}

/// Défilement maximal.
pub fn max_scroll() -> f32 {
    (content_height() - (PANEL_H - HEADER_H)).max(0.0)
}

pub fn clamp_scroll(scroll: f32) -> f32 {
    scroll.clamp(0.0, max_scroll())
}

/// Ligne interactive sous un point (ni titre de section, ni ligne cachée par l'en-tête).
pub fn hit(scroll: f32, p: Pt) -> Option<Row> {
    if p.y < HEADER_H || p.y >= PANEL_H {
        return None;
    }
    layout(scroll).into_iter().find(|(row, r)| !matches!(row, Row::Header(_)) && r.contains(p)).map(|(row, _)| row)
}

/// Zone de contrôle d'une ligne : sa partie droite.
pub fn control_rect(row: RectF) -> RectF {
    RectF { x: row.x + row.w - CONTROL_W, y: row.y + (row.h - 28.0) / 2.0, w: CONTROL_W, h: 28.0 }
}

/// Valeur d'un curseur pour une abscisse donnée, arrondie au plus proche et bornée.
pub fn slider_value(track: RectF, x: f32, min: i32, max: i32) -> i32 {
    if track.w <= 0.0 || max <= min {
        return min;
    }
    let t = ((x - track.x) / track.w).clamp(0.0, 1.0);
    min + (t * (max - min) as f32).round() as i32
}

/// Abscisse du bouton d'un curseur pour une valeur donnée.
pub fn slider_x(track: RectF, value: i32, min: i32, max: i32) -> f32 {
    if max <= min {
        return track.x;
    }
    track.x + track.w * (value.clamp(min, max) - min) as f32 / (max - min) as f32
}

/// Rectangle du segment `i` d'un contrôle découpé en `n` parts égales.
pub fn segment_rect(control: RectF, n: usize, i: usize) -> RectF {
    let w = control.w / n as f32;
    RectF { x: control.x + i as f32 * w, y: control.y, w, h: control.h }
}

/// Segment sous un point.
pub fn segment_at(control: RectF, n: usize, p: Pt) -> Option<usize> {
    (0..n).find(|i| segment_rect(control, n, *i).contains(p))
}

/// Vrai si l'action peut être déclenchée par un double appui : les raccourcis globaux ponctuels.
pub fn supports_double(action: Action) -> bool {
    action.is_global() && action != Action::HoldDraw
}

/// Parties cliquables d'une ligne de raccourci : le sélecteur « combinaison / double appui »
/// (pour les actions qui le proposent) et la pastille du raccourci.
pub fn binding_parts(row: RectF, action: Action) -> (Option<RectF>, RectF) {
    const PILL_W: f32 = 100.0;
    const MODE_W: f32 = 104.0;
    const H: f32 = 26.0;
    let y = row.y + (row.h - H) / 2.0;
    let pill = RectF { x: row.x + row.w - PILL_W, y, w: PILL_W, h: H };
    let mode = supports_double(action).then(|| RectF { x: pill.x - 8.0 - MODE_W, y, w: MODE_W, h: H });
    (mode, pill)
}

/// Message affiché quand un raccourci est déjà pris.
pub fn conflict_message(wanted: Shortcut, with: Action) -> String {
    conflict_message_in(Lang::Fr, wanted, with)
}

/// Message de conflit dans une langue.
pub fn conflict_message_in(lang: Lang, wanted: Shortcut, with: Action) -> String {
    let (shortcut, action) = (wanted.display(lang), with.label_in(lang));
    match lang {
        Lang::Fr => format!("« {shortcut} » est déjà utilisé par « {action} »."),
        Lang::En => format!("\"{shortcut}\" is already used by \"{action}\"."),
    }
}

/// Onglet du panneau.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum Tab {
    #[default]
    Settings,
    Help,
}

/// Partie de l'en-tête du panneau.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum HeaderPart {
    Tab(Tab),
    Close,
    /// Zone libre : sert à déplacer le panneau.
    Drag,
}

/// Rectangles des deux onglets : Réglages, Aide.
pub fn tab_rects() -> [RectF; 2] {
    let y = (HEADER_H - 32.0) / 2.0;
    [RectF { x: PAD - 6.0, y, w: 112.0, h: 32.0 }, RectF { x: PAD - 6.0 + 116.0, y, w: 84.0, h: 32.0 }]
}

/// Rectangle du bouton de fermeture.
pub fn close_rect() -> RectF {
    RectF { x: PANEL_W - PAD - 30.0, y: (HEADER_H - 32.0) / 2.0, w: 32.0, h: 32.0 }
}

/// Partie de l'en-tête sous un point, ou `None` hors de l'en-tête.
pub fn header_hit(p: Pt) -> Option<HeaderPart> {
    if p.x < 0.0 || p.x >= PANEL_W || p.y < 0.0 || p.y >= HEADER_H {
        return None;
    }
    let [settings, help] = tab_rects();
    Some(if settings.contains(p) {
        HeaderPart::Tab(Tab::Settings)
    } else if help.contains(p) {
        HeaderPart::Tab(Tab::Help)
    } else if close_rect().contains(p) {
        HeaderPart::Close
    } else {
        HeaderPart::Drag
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_follow_spec_sections() {
        let r = rows();
        let headers: Vec<_> = r.iter().filter_map(|x| if let Row::Header(h) = x { Some(*h) } else { None }).collect();
        assert_eq!(headers, vec!["Général", "Dessin", "Pointeur", "Raccourcis"]);
        // 9 globaux + 6 outils + 10 couleurs + annuler + quitter
        assert_eq!(r.iter().filter(|x| matches!(x, Row::Binding(_))).count(), 27);
        for row in [Row::HoldSeconds, Row::Width, Row::HaloSize, Row::HaloFollowsColor, Row::ResetBindings, Row::Autostart, Row::HideFromCapture] {
            assert!(r.contains(&row), "{row:?}");
        }
        let pos = |row: Row| r.iter().position(|x| *x == row).unwrap();
        assert!(pos(Row::HoldSeconds) < pos(Row::Header("Pointeur")));
        assert!(pos(Row::HaloSize) < pos(Row::Header("Raccourcis")));
        assert!(pos(Row::Binding(Action::ToggleDraw)) < pos(Row::ResetBindings));
        // La section Général ouvre la page : langue, démarrage, masquage, dans cet ordre.
        assert_eq!(
            &r[..5],
            &[Row::Header("Général"), Row::Language, Row::Autostart, Row::HideFromCapture, Row::Header("Dessin")]
        );
        assert_eq!(r.last(), Some(&Row::ResetBindings));
    }

    #[test]
    fn labels_are_french() {
        assert_eq!(Row::HoldSeconds.label(), "Durée avant effacement");
        assert_eq!(Row::Width.label(), "Épaisseur du trait");
        assert_eq!(Row::HaloSize.label(), "Taille du halo");
        assert_eq!(Row::HaloFollowsColor.label(), "Halo de la couleur du dessin");
        assert_eq!(Row::Autostart.label(), "Lancer au démarrage de Windows");
        assert_eq!(Row::HideFromCapture.label(), "Masquer la barre du partage d'écran");
        assert_eq!(Row::ResetBindings.label(), "Rétablir les raccourcis par défaut");
        assert_eq!(Row::Binding(Action::ToolArrow).label(), "Flèche");
        assert_eq!(Row::Header("Dessin").label(), "Dessin");
    }

    #[test]
    fn conflict_message_names_the_action() {
        assert_eq!(conflict_message("F".parse().unwrap(), Action::ToolArrow), "« F » est déjà utilisé par « Flèche ».");
        assert_eq!(
            conflict_message("2×Tab".parse().unwrap(), Action::ClearAll),
            "« 2×Tab » est déjà utilisé par « Tout effacer »."
        );
    }

    #[test]
    fn layout_stacks_rows_inside_the_panel() {
        let l = layout(0.0);
        assert_eq!(l.len(), rows().len());
        assert_eq!(l[0].1.y, HEADER_H);
        for pair in l.windows(2) {
            assert_eq!(pair[0].1.y + pair[0].1.h, pair[1].1.y, "{:?} puis {:?}", pair[0].0, pair[1].0);
        }
        for (_, r) in &l {
            assert_eq!((r.x, r.w), (PAD, PANEL_W - 2.0 * PAD));
        }
        let last = l.last().unwrap().1;
        assert_eq!(content_height(), last.y + last.h - HEADER_H + PAD);
    }

    #[test]
    fn scrolling_shifts_rows_and_is_bounded() {
        let (a, b) = (layout(0.0), layout(100.0));
        assert_eq!(a[3].1.y - 100.0, b[3].1.y);
        assert_eq!(max_scroll(), content_height() - (PANEL_H - HEADER_H));
        assert!(max_scroll() > 0.0);
        assert_eq!(clamp_scroll(-50.0), 0.0);
        assert_eq!(clamp_scroll(1e6), max_scroll());
    }

    #[test]
    fn hit_ignores_headers_and_hidden_rows() {
        let l = layout(0.0);
        let hold = l.iter().find(|(r, _)| *r == Row::HoldSeconds).unwrap().1;
        assert_eq!(hit(0.0, Pt::new(hold.x + 5.0, hold.y + 5.0)), Some(Row::HoldSeconds));
        let header = l.iter().find(|(r, _)| matches!(r, Row::Header(_))).unwrap().1;
        assert_eq!(hit(0.0, Pt::new(header.x + 5.0, header.y + 5.0)), None);
        // Une ligne défilée sous l'en-tête du panneau n'est pas cliquable.
        assert_eq!(hit(60.0, Pt::new(PAD + 5.0, HEADER_H - 5.0)), None);
        assert_eq!(hit(0.0, Pt::new(PAD + 5.0, PANEL_H + 5.0)), None);
    }

    #[test]
    fn slider_maps_position_to_value() {
        let track = RectF { x: 100.0, y: 0.0, w: 130.0, h: 20.0 };
        assert_eq!(slider_value(track, 100.0, 2, 15), 2);
        assert_eq!(slider_value(track, 230.0, 2, 15), 15);
        assert_eq!(slider_value(track, 165.0, 2, 15), 9); // milieu, arrondi au plus proche
        assert_eq!(slider_value(track, -500.0, 2, 15), 2);
        assert_eq!(slider_value(track, 9999.0, 40, 120), 120);
        assert_eq!(slider_x(track, 2, 2, 15), 100.0);
        assert_eq!(slider_x(track, 15, 2, 15), 230.0);
        assert_eq!(slider_value(track, slider_x(track, 7, 2, 15), 2, 15), 7);
    }

    #[test]
    fn segments_split_a_control_evenly() {
        let c = RectF { x: 200.0, y: 10.0, w: 120.0, h: 28.0 };
        assert_eq!(segment_at(c, 3, Pt::new(205.0, 20.0)), Some(0));
        assert_eq!(segment_at(c, 3, Pt::new(260.0, 20.0)), Some(1));
        assert_eq!(segment_at(c, 3, Pt::new(319.0, 20.0)), Some(2));
        assert_eq!(segment_at(c, 3, Pt::new(199.0, 20.0)), None);
        assert_eq!(segment_at(c, 3, Pt::new(260.0, 50.0)), None);
        assert_eq!(segment_rect(c, 3, 1), RectF { x: 240.0, y: 10.0, w: 40.0, h: 28.0 });
    }

    #[test]
    fn only_punctual_global_shortcuts_offer_double_tap() {
        assert!(supports_double(Action::ToggleDraw));
        assert!(supports_double(Action::ClearAll));
        assert!(supports_double(Action::ToggleHalo));
        assert!(!supports_double(Action::HoldDraw)); // se maintient, ne se tape pas
        assert!(!supports_double(Action::ToolArrow));
        assert!(!supports_double(Action::ExitDraw));
    }

    #[test]
    fn binding_row_parts() {
        let row = RectF { x: 20.0, y: 100.0, w: 380.0, h: 40.0 };
        let (mode, pill) = binding_parts(row, Action::ToggleDraw);
        let mode = mode.unwrap();
        assert!(mode.x + mode.w <= pill.x);
        assert_eq!(pill.x + pill.w, row.x + row.w);
        assert!(pill.y >= row.y && pill.y + pill.h <= row.y + row.h);
        assert!(binding_parts(row, Action::ToolArrow).0.is_none());
    }

    #[test]
    fn language_is_the_first_setting() {
        // Première ligne de réglage, juste sous le titre de la section Général.
        assert_eq!(rows()[1], Row::Language);
        assert_eq!(Row::Language.label(), "Langue");
        let first = layout(0.0)[1];
        assert_eq!(first.0, Row::Language);
        assert_eq!(hit(0.0, Pt::new(first.1.x + 5.0, first.1.y + 5.0)), Some(Row::Language));
    }

    #[test]
    fn rows_are_translated() {
        use crate::i18n::Lang;
        assert_eq!(Row::Language.label_in(Lang::En), "Language");
        assert_eq!(Row::HoldSeconds.label_in(Lang::En), "Time before fade-out");
        assert_eq!(Row::Header("Dessin").label_in(Lang::En), "Drawing");
        assert_eq!(Row::Header("Raccourcis").label_in(Lang::En), "Shortcuts");
        assert_eq!(Row::Binding(Action::ToolArrow).label_in(Lang::En), "Arrow");
        for row in rows() {
            assert_eq!(row.label_in(Lang::Fr), row.label());
            assert!(!row.label_in(Lang::En).is_empty(), "{row:?}");
            if !matches!(row, Row::Binding(Action::ToolRect | Action::ToolEllipse)) {
                assert_ne!(row.label_in(Lang::En), row.label(), "{row:?} n'est pas traduit");
            }
        }
        assert_eq!(
            conflict_message_in(Lang::En, "F".parse().unwrap(), Action::ToolArrow),
            "\"F\" is already used by \"Arrow\"."
        );
        assert_eq!(
            conflict_message_in(Lang::Fr, "F".parse().unwrap(), Action::ToolArrow),
            conflict_message("F".parse().unwrap(), Action::ToolArrow)
        );
    }

    #[test]
    fn header_holds_two_tabs_a_close_button_and_a_drag_area() {
        let [tab_settings, tab_help] = tab_rects();
        let close = close_rect();
        for r in [tab_settings, tab_help, close] {
            assert!(r.x >= 0.0 && r.y >= 0.0 && r.x + r.w <= PANEL_W && r.y + r.h <= HEADER_H, "{r:?}");
        }
        assert!(tab_settings.x + tab_settings.w <= tab_help.x);
        assert!(tab_help.x + tab_help.w < close.x);
        // Le reste de l'en-tête sert à déplacer le panneau.
        assert_eq!(header_hit(Pt::new(tab_settings.x + 4.0, tab_settings.y + 4.0)), Some(HeaderPart::Tab(Tab::Settings)));
        assert_eq!(header_hit(Pt::new(tab_help.x + 4.0, tab_help.y + 4.0)), Some(HeaderPart::Tab(Tab::Help)));
        assert_eq!(header_hit(Pt::new(close.x + 4.0, close.y + 4.0)), Some(HeaderPart::Close));
        let gap = Pt::new((tab_help.x + tab_help.w + close.x) / 2.0, HEADER_H / 2.0);
        assert_eq!(header_hit(gap), Some(HeaderPart::Drag));
        assert_eq!(header_hit(Pt::new(4.0, 4.0)), Some(HeaderPart::Drag));
        assert_eq!(header_hit(Pt::new(50.0, HEADER_H + 5.0)), None); // sous l'en-tête
    }
}
