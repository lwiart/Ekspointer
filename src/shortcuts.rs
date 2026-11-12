//! Raccourcis clavier : modèle, analyse, affichage, détection de conflits.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::BTreeMap;
use std::fmt;
use std::str::FromStr;

/// Touches de modification d'un raccourci.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default)]
pub struct Mods {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub win: bool,
}

impl Mods {
    pub fn is_empty(&self) -> bool {
        !(self.ctrl || self.alt || self.shift || self.win)
    }
}

/// Raccourci : touches de modification et, éventuellement, une touche
/// (code de touche virtuelle Windows). Sans touche, c'est une combinaison à maintenir.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct Shortcut {
    pub mods: Mods,
    pub key: Option<u32>,
}

impl Shortcut {
    /// Forme canonique : chiffres du pavé numérique ramenés à la rangée du haut.
    pub fn normalized(self) -> Shortcut {
        Shortcut { mods: self.mods, key: self.key.map(normalize_vk) }
    }
}

/// Ramène les chiffres du pavé numérique (0x60–0x69) aux chiffres de la rangée du haut.
pub fn normalize_vk(vk: u32) -> u32 {
    if (0x60..=0x69).contains(&vk) {
        vk - 0x30
    } else {
        vk
    }
}

/// Touches nommées, hors lettres, chiffres et F1–F12.
const NAMED_KEYS: [(&str, u32); 9] = [
    ("Échap", 0x1B),
    ("Espace", 0x20),
    ("Entrée", 0x0D),
    ("Tab", 0x09),
    ("Suppr", 0x2E),
    ("Haut", 0x26),
    ("Bas", 0x28),
    ("Gauche", 0x25),
    ("Droite", 0x27),
];

fn key_name(vk: u32) -> Option<String> {
    let vk = normalize_vk(vk);
    match vk {
        0x41..=0x5A | 0x30..=0x39 => Some((vk as u8 as char).to_string()),
        0x70..=0x7B => Some(format!("F{}", vk - 0x6F)),
        _ => NAMED_KEYS.iter().find(|(_, v)| *v == vk).map(|(n, _)| n.to_string()),
    }
}

fn key_from_name(name: &str) -> Option<u32> {
    let lower = name.to_lowercase();
    let mut chars = name.chars();
    if let (Some(c), None) = (chars.next(), chars.next()) {
        let c = c.to_ascii_uppercase();
        if c.is_ascii_uppercase() || c.is_ascii_digit() {
            return Some(c as u32);
        }
    }
    if let Some(n) = lower.strip_prefix('f').and_then(|n| n.parse::<u32>().ok()) {
        if (1..=12).contains(&n) {
            return Some(0x6F + n);
        }
    }
    match lower.as_str() {
        "esc" | "echap" | "escape" => return Some(0x1B),
        "entree" | "enter" => return Some(0x0D),
        "space" => return Some(0x20),
        _ => {}
    }
    NAMED_KEYS.iter().find(|(n, _)| n.to_lowercase() == lower).map(|(_, v)| *v)
}

impl fmt::Display for Shortcut {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut parts: Vec<String> = Vec::new();
        if self.mods.ctrl {
            parts.push("Ctrl".into());
        }
        if self.mods.alt {
            parts.push("Alt".into());
        }
        if self.mods.shift {
            parts.push("Maj".into());
        }
        if self.mods.win {
            parts.push("Win".into());
        }
        if let Some(vk) = self.key {
            parts.push(key_name(vk).unwrap_or_else(|| format!("Touche{vk}")));
        }
        f.write_str(&parts.join("+"))
    }
}

impl FromStr for Shortcut {
    type Err = String;

    fn from_str(s: &str) -> Result<Shortcut, String> {
        let mut mods = Mods::default();
        let mut key = None;
        for part in s.split('+').map(str::trim) {
            match part.to_lowercase().as_str() {
                "" => return Err(format!("raccourci incomplet : « {s} »")),
                "ctrl" => mods.ctrl = true,
                "alt" => mods.alt = true,
                "maj" | "shift" => mods.shift = true,
                "win" => mods.win = true,
                _ => {
                    if key.is_some() {
                        return Err(format!("plusieurs touches dans « {s} »"));
                    }
                    key = Some(key_from_name(part).ok_or_else(|| format!("touche inconnue : « {part} »"))?);
                }
            }
        }
        if key.is_none() && mods.is_empty() {
            return Err("raccourci vide".into());
        }
        Ok(Shortcut { mods, key })
    }
}

impl Serialize for Shortcut {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for Shortcut {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Shortcut, D::Error> {
        String::deserialize(d)?.parse().map_err(serde::de::Error::custom)
    }
}

/// Action déclenchable par un raccourci.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Action {
    ToggleDraw,
    HoldDraw,
    ClearAll,
    ToggleHalo,
    ToolArrow,
    ToolRect,
    ToolEllipse,
    ToolFree,
    ToolHighlighter,
    ToolNumber,
    /// Couleur d'indice 0 à 5.
    Color(u8),
    Undo,
    ExitDraw,
}

const COLOR_LABELS: [&str; 6] = ["Couleur 1", "Couleur 2", "Couleur 3", "Couleur 4", "Couleur 5", "Couleur 6"];
const COLOR_NAMES: [&str; 6] = ["Color1", "Color2", "Color3", "Color4", "Color5", "Color6"];

impl Action {
    /// Toutes les actions, dans l'ordre d'affichage des réglages.
    pub fn all() -> Vec<Action> {
        let mut v = vec![
            Action::ToggleDraw,
            Action::HoldDraw,
            Action::ClearAll,
            Action::ToggleHalo,
            Action::ToolArrow,
            Action::ToolRect,
            Action::ToolEllipse,
            Action::ToolFree,
            Action::ToolHighlighter,
            Action::ToolNumber,
        ];
        v.extend((0..6).map(Action::Color));
        v.extend([Action::Undo, Action::ExitDraw]);
        v
    }

    /// Vrai pour les raccourcis actifs dans tous les modes.
    pub fn is_global(self) -> bool {
        matches!(self, Action::ToggleDraw | Action::HoldDraw | Action::ClearAll | Action::ToggleHalo)
    }

    /// Libellé affiché dans les réglages.
    pub fn label(self) -> &'static str {
        match self {
            Action::ToggleDraw => "Activer ou quitter le dessin",
            Action::HoldDraw => "Dessiner tant que la touche est tenue",
            Action::ClearAll => "Tout effacer",
            Action::ToggleHalo => "Activer ou couper le halo",
            Action::ToolArrow => "Flèche",
            Action::ToolRect => "Rectangle",
            Action::ToolEllipse => "Ellipse",
            Action::ToolFree => "Trait libre",
            Action::ToolHighlighter => "Surligneur",
            Action::ToolNumber => "Numéros",
            Action::Color(i) => COLOR_LABELS[(i as usize).min(5)],
            Action::Undo => "Annuler la dernière forme",
            Action::ExitDraw => "Quitter le mode dessin",
        }
    }

    /// Nom stable utilisé dans le fichier de réglages.
    pub fn name(self) -> &'static str {
        match self {
            Action::ToggleDraw => "ToggleDraw",
            Action::HoldDraw => "HoldDraw",
            Action::ClearAll => "ClearAll",
            Action::ToggleHalo => "ToggleHalo",
            Action::ToolArrow => "ToolArrow",
            Action::ToolRect => "ToolRect",
            Action::ToolEllipse => "ToolEllipse",
            Action::ToolFree => "ToolFree",
            Action::ToolHighlighter => "ToolHighlighter",
            Action::ToolNumber => "ToolNumber",
            Action::Color(i) => COLOR_NAMES[(i as usize).min(5)],
            Action::Undo => "Undo",
            Action::ExitDraw => "ExitDraw",
        }
    }

    pub fn from_name(name: &str) -> Option<Action> {
        Action::all().into_iter().find(|a| a.name() == name)
    }

    fn default_shortcut(self) -> Shortcut {
        let s = match self {
            Action::ToggleDraw => "Ctrl+Alt+D",
            Action::HoldDraw => "Ctrl+Alt",
            Action::ClearAll => "Ctrl+Alt+E",
            Action::ToggleHalo => "Ctrl+Alt+H",
            Action::ToolArrow => "F",
            Action::ToolRect => "R",
            Action::ToolEllipse => "C",
            Action::ToolFree => "D",
            Action::ToolHighlighter => "S",
            Action::ToolNumber => "N",
            Action::Color(i) => ["1", "2", "3", "4", "5", "6"][(i as usize).min(5)],
            Action::Undo => "Ctrl+Z",
            Action::ExitDraw => "Échap",
        };
        s.parse().expect("raccourci par défaut valide")
    }
}

/// État physique des touches de modification.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct KeyState {
    pub lctrl: bool,
    pub rctrl: bool,
    pub lalt: bool,
    pub ralt: bool,
    pub shift: bool,
    pub win: bool,
}

impl KeyState {
    /// Touches de modification retenues pour les raccourcis. Seuls Ctrl et Alt de gauche
    /// comptent : AltGr, que Windows envoie comme Ctrl gauche + Alt droit, ne compte jamais.
    pub fn mods(&self) -> Mods {
        Mods { ctrl: self.lctrl && !self.ralt, alt: self.lalt, shift: self.shift, win: self.win }
    }
}

/// Affectation des raccourcis aux actions.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Bindings(BTreeMap<Action, Shortcut>);

impl Default for Bindings {
    fn default() -> Bindings {
        Bindings::defaults()
    }
}

impl Bindings {
    pub fn defaults() -> Bindings {
        Bindings(Action::all().into_iter().map(|a| (a, a.default_shortcut())).collect())
    }

    pub fn get(&self, a: Action) -> Shortcut {
        self.0.get(&a).copied().unwrap_or_else(|| a.default_shortcut())
    }

    /// Affecte un raccourci. Refuse s'il est déjà pris, en nommant l'action en conflit.
    pub fn set(&mut self, a: Action, s: Shortcut) -> Result<(), Action> {
        let s = s.normalized();
        if let Some((other, _)) = self.0.iter().find(|(o, v)| **o != a && v.normalized() == s) {
            return Err(*other);
        }
        self.0.insert(a, s);
        Ok(())
    }

    /// Action ponctuelle déclenchée par `s`. Les actions propres au dessin exigent `draw_mode`.
    pub fn action_for(&self, s: Shortcut, draw_mode: bool) -> Option<Action> {
        let s = s.normalized();
        s.key?;
        self.0
            .iter()
            .find(|(a, v)| v.normalized() == s && **a != Action::HoldDraw && (a.is_global() || draw_mode))
            .map(|(a, _)| *a)
    }

    pub fn all(&self) -> Vec<(Action, Shortcut)> {
        Action::all().into_iter().map(|a| (a, self.get(a))).collect()
    }
}

impl Serialize for Bindings {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_map(self.all().into_iter().map(|(a, sc)| (a.name(), sc.to_string())))
    }
}

impl<'de> Deserialize<'de> for Bindings {
    /// Lecture tolérante : une entrée inconnue ou illisible est ignorée, et une action
    /// dont le raccourci entre en conflit avec une autre reprend sa valeur par défaut.
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Bindings, D::Error> {
        let raw = BTreeMap::<String, String>::deserialize(d)?;
        let mut b = Bindings::defaults();
        for (name, value) in &raw {
            if let (Some(a), Ok(s)) = (Action::from_name(name), value.parse::<Shortcut>()) {
                b.0.insert(a, s.normalized());
            }
        }
        loop {
            let clash: Vec<Action> = b
                .0
                .iter()
                .filter(|(a, s)| **s != a.default_shortcut() && b.0.iter().any(|(o, v)| o != *a && v == *s))
                .map(|(a, _)| *a)
                .collect();
            if clash.is_empty() {
                break;
            }
            for a in clash {
                b.0.insert(a, a.default_shortcut());
            }
        }
        Ok(b)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sc(s: &str) -> Shortcut {
        s.parse().unwrap()
    }

    #[test]
    fn parse_display_roundtrip() {
        for s in ["Ctrl+Alt+D", "Ctrl+Alt", "F", "Échap", "Ctrl+Z", "Ctrl+Maj+F5", "1", "Win+Espace", "Alt+Gauche"] {
            assert_eq!(sc(s).to_string(), s);
        }
    }

    #[test]
    fn parse_rejects_garbage() {
        assert!("Ctrl+".parse::<Shortcut>().is_err());
        assert!("Bidule".parse::<Shortcut>().is_err());
        assert!("".parse::<Shortcut>().is_err());
        assert!("A+B".parse::<Shortcut>().is_err());
    }

    #[test]
    fn parse_is_lenient_on_case_order_and_aliases() {
        assert_eq!(sc("alt+ctrl+d"), sc("Ctrl+Alt+D"));
        assert_eq!(sc("Shift+Esc"), sc("Maj+Échap"));
        assert_eq!(sc(" Ctrl + Z "), sc("Ctrl+Z"));
    }

    #[test]
    fn defaults_match_spec() {
        let b = Bindings::defaults();
        let g = |a| b.get(a).to_string();
        assert_eq!(g(Action::ToggleDraw), "Ctrl+Alt+D");
        assert_eq!(g(Action::HoldDraw), "Ctrl+Alt");
        assert_eq!(g(Action::ClearAll), "Ctrl+Alt+E");
        assert_eq!(g(Action::ToggleHalo), "Ctrl+Alt+H");
        assert_eq!(g(Action::ToolArrow), "F");
        assert_eq!(g(Action::ToolRect), "R");
        assert_eq!(g(Action::ToolEllipse), "C");
        assert_eq!(g(Action::ToolFree), "D");
        assert_eq!(g(Action::ToolHighlighter), "S");
        assert_eq!(g(Action::ToolNumber), "N");
        assert_eq!(g(Action::Color(0)), "1");
        assert_eq!(g(Action::Color(5)), "6");
        assert_eq!(g(Action::Undo), "Ctrl+Z");
        assert_eq!(g(Action::ExitDraw), "Échap");
        assert_eq!(b.all().len(), 18);
    }

    #[test]
    fn conflicts_are_refused_and_named() {
        let mut b = Bindings::defaults();
        assert_eq!(b.set(Action::ToolRect, sc("F")), Err(Action::ToolArrow));
        assert_eq!(b.get(Action::ToolRect).to_string(), "R");
        assert!(b.set(Action::ToolRect, sc("R")).is_ok()); // réaffecter sa propre valeur
        assert!(b.set(Action::ToolRect, sc("Ctrl+Alt+R")).is_ok());
        assert_eq!(b.get(Action::ToolRect).to_string(), "Ctrl+Alt+R");
    }

    #[test]
    fn numpad_and_top_row_digits_conflict() {
        let mut b = Bindings::defaults();
        let numpad1 = Shortcut { mods: Mods::default(), key: Some(0x61) };
        assert_eq!(b.set(Action::ToolRect, numpad1), Err(Action::Color(0)));
    }

    #[test]
    fn draw_only_actions_need_draw_mode() {
        let b = Bindings::defaults();
        assert_eq!(b.action_for(sc("F"), false), None);
        assert_eq!(b.action_for(sc("F"), true), Some(Action::ToolArrow));
        assert_eq!(b.action_for(sc("Ctrl+Alt+D"), false), Some(Action::ToggleDraw));
        assert_eq!(b.action_for(sc("Ctrl+Alt+D"), true), Some(Action::ToggleDraw));
        assert_eq!(b.action_for(sc("Ctrl+Alt"), true), None); // le maintien n'est pas une action ponctuelle
        assert_eq!(b.action_for(sc("Q"), true), None);
    }

    #[test]
    fn global_actions() {
        for a in [Action::ToggleDraw, Action::HoldDraw, Action::ClearAll, Action::ToggleHalo] {
            assert!(a.is_global());
        }
        for a in [Action::ToolArrow, Action::Color(2), Action::Undo, Action::ExitDraw] {
            assert!(!a.is_global());
        }
    }

    #[test]
    fn labels_are_french() {
        assert_eq!(Action::ToolArrow.label(), "Flèche");
        assert_eq!(Action::ClearAll.label(), "Tout effacer");
        assert_eq!(Action::Color(2).label(), "Couleur 3");
        assert_eq!(Action::HoldDraw.label(), "Dessiner tant que la touche est tenue");
    }

    #[test]
    fn altgr_never_counts() {
        // Ce que Windows envoie pour AltGr : Ctrl gauche + Alt droit.
        let altgr = KeyState { lctrl: true, ralt: true, ..Default::default() };
        assert_eq!(altgr.mods(), Mods::default());
        let right = KeyState { rctrl: true, lalt: true, ..Default::default() };
        assert!(!right.mods().ctrl && right.mods().alt);
        let left = KeyState { lctrl: true, lalt: true, ..Default::default() };
        assert!(left.mods().ctrl && left.mods().alt);
    }

    #[test]
    fn numpad_digits_normalize() {
        assert_eq!(normalize_vk(0x61), 0x31);
        assert_eq!(normalize_vk(0x60), 0x30);
        assert_eq!(normalize_vk(0x69), 0x39);
        assert_eq!(normalize_vk(0x31), 0x31);
        assert_eq!(normalize_vk(0x46), 0x46);
    }

    #[test]
    fn bindings_serialize_as_name_to_string_map() {
        let json = serde_json::to_value(Bindings::defaults()).unwrap();
        assert_eq!(json["ToggleDraw"], "Ctrl+Alt+D");
        assert_eq!(json["Color1"], "1");
        assert_eq!(json["ExitDraw"], "Échap");
        let back: Bindings = serde_json::from_value(json).unwrap();
        assert_eq!(back, Bindings::defaults());
    }

    #[test]
    fn swapped_bindings_survive_reload() {
        let b: Bindings = serde_json::from_str(r#"{"ToolArrow":"R","ToolRect":"F"}"#).unwrap();
        assert_eq!(b.get(Action::ToolArrow).to_string(), "R");
        assert_eq!(b.get(Action::ToolRect).to_string(), "F");
    }
}
