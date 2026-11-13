//! Raccourcis clavier : modèle, analyse, affichage, détection de conflits.

use crate::i18n::Lang;
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
    /// Double appui rapide sur `key`, sans touche de modification (« 2×Tab », « 2×Ctrl »).
    pub double: bool,
}

impl Shortcut {
    /// Forme canonique : chiffres du pavé numérique ramenés à la rangée du haut.
    pub fn normalized(self) -> Shortcut {
        Shortcut { mods: self.mods, key: self.key.map(normalize_vk), double: self.double }
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

/// Délai maximal entre les deux appuis d'un double appui, en millisecondes.
pub const DOUBLE_TAP_MS: u64 = 400;

/// Touches de modification utilisables en double appui : nom et code.
const DOUBLE_MODS: [(&str, u32); 2] = [("Ctrl", 0xA2), ("Maj", 0xA0)];

/// Vrai pour une touche de modification utilisable en double appui.
pub fn is_double_modifier(vk: u32) -> bool {
    DOUBLE_MODS.iter().any(|(_, v)| *v == vk)
}

/// Code canonique d'une touche pour le double appui, ou `None` si elle n'y est pas
/// utilisable : Ctrl droit et Alt droit (AltGr) ne comptent jamais, ni Alt et Win, qui
/// ouvrent un menu quand on les tape seuls.
pub fn double_tap_vk(vk: u32) -> Option<u32> {
    match vk {
        0xA1 => Some(0xA0),
        0xA3 | 0xA4 | 0xA5 | 0x5B | 0x5C | 0x10 | 0x11 | 0x12 => None,
        _ => Some(normalize_vk(vk)),
    }
}

/// Touches nommées, hors lettres, chiffres et F1–F12.
const NAMED_KEYS: [(&str, u32); 15] = [
    ("Retour arrière", 0x08),
    ("Inser", 0x2D),
    ("Début", 0x24),
    ("Fin", 0x23),
    ("PgPréc", 0x21),
    ("PgSuiv", 0x22),
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

/// Noms anglais des touches nommées, dans le même ordre que `NAMED_KEYS`.
const NAMED_KEYS_EN: [&str; 15] = [
    "Backspace", "Ins", "Home", "End", "PgUp", "PgDn", "Esc", "Space", "Enter", "Tab", "Del", "Up", "Down", "Left", "Right",
];

/// Nom d'une touche dans une langue ; « Touche<code> » ou « Key<code> » si elle n'a pas de nom.
fn key_name_in(vk: u32, lang: Lang) -> String {
    let vk = normalize_vk(vk);
    if let Some((_, v)) = DOUBLE_MODS.iter().find(|(_, v)| *v == vk) {
        return match (lang, *v) {
            (Lang::En, 0xA0) => "Shift".to_string(),
            _ => DOUBLE_MODS.iter().find(|(_, d)| d == v).map(|(n, _)| n.to_string()).unwrap_or_default(),
        };
    }
    if lang == Lang::En {
        if let Some(i) = NAMED_KEYS.iter().position(|(_, v)| *v == vk) {
            return NAMED_KEYS_EN[i].to_string();
        }
    }
    key_name(vk).unwrap_or_else(|| match lang {
        Lang::Fr => format!("Touche{vk}"),
        Lang::En => format!("Key{vk}"),
    })
}

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
    if let Some(v) = NAMED_KEYS.iter().find(|(n, _)| n.to_lowercase() == lower).map(|(_, v)| *v) {
        return Some(v);
    }
    // Noms anglais, tels que l'interface les affiche en anglais.
    if let Some(i) = NAMED_KEYS_EN.iter().position(|n| n.to_lowercase() == lower) {
        return Some(NAMED_KEYS[i].1);
    }
    if let Some(v) = lower.strip_prefix("key").and_then(|n| n.parse::<u32>().ok()).filter(|v| (1..=0xFE).contains(v)) {
        return Some(normalize_vk(v));
    }
    // Touche sans nom, écrite « Touche<code> » : ponctuation, touche ², pavé numérique, etc.
    lower.strip_prefix("touche").and_then(|n| n.parse::<u32>().ok()).filter(|v| (1..=0xFE).contains(v)).map(normalize_vk)
}

impl Shortcut {
    /// Texte du raccourci dans une langue (« Ctrl+Maj+F5 » ou « Ctrl+Shift+F5 »).
    pub fn display(&self, lang: Lang) -> String {
        if self.double {
            return format!("2×{}", key_name_in(self.key.unwrap_or(0), lang));
        }
        let mut parts: Vec<String> = Vec::new();
        if self.mods.ctrl {
            parts.push("Ctrl".into());
        }
        if self.mods.alt {
            parts.push("Alt".into());
        }
        if self.mods.shift {
            parts.push(if lang == Lang::En { "Shift" } else { "Maj" }.into());
        }
        if self.mods.win {
            parts.push("Win".into());
        }
        if let Some(vk) = self.key {
            parts.push(key_name_in(vk, lang));
        }
        parts.join("+")
    }
}

/// Forme française : c'est celle du fichier de réglages, quelle que soit la langue de l'interface.
impl fmt::Display for Shortcut {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.display(Lang::Fr))
    }
}

impl FromStr for Shortcut {
    type Err = String;

    fn from_str(s: &str) -> Result<Shortcut, String> {
        let trimmed = s.trim();
        if let Some(rest) =
            trimmed.strip_prefix("2×").or_else(|| trimmed.strip_prefix("2x")).or_else(|| trimmed.strip_prefix("2X"))
        {
            let name = rest.trim();
            let lower = name.to_lowercase();
            let alias = match lower.as_str() {
                "shift" => "maj",
                other => other,
            };
            let key = DOUBLE_MODS
                .iter()
                .find(|(n, _)| n.to_lowercase() == alias)
                .map(|(_, v)| *v)
                .or_else(|| if name.contains('+') { None } else { key_from_name(name) })
                .ok_or_else(|| format!("double appui invalide : « {s} »"))?;
            return Ok(Shortcut { mods: Mods::default(), key: Some(key), double: true });
        }
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
        Ok(Shortcut { mods, key, double: false })
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
    ToggleRipple,
    ToggleTrail,
    /// Garder les tracés à l'écran (épingle).
    TogglePin,
    OpenSettings,
    OpenHelp,
    ToolArrow,
    ToolRect,
    ToolEllipse,
    ToolFree,
    ToolHighlighter,
    ToolNumber,
    /// Couleur d'indice 0 à 9.
    Color(u8),
    Undo,
    ExitDraw,
}

const COLOR_LABELS: [&str; 10] = [
    "Couleur 1", "Couleur 2", "Couleur 3", "Couleur 4", "Couleur 5", "Couleur 6", "Couleur 7", "Couleur 8", "Couleur 9",
    "Couleur 10",
];
const COLOR_LABELS_EN: [&str; 10] =
    ["Color 1", "Color 2", "Color 3", "Color 4", "Color 5", "Color 6", "Color 7", "Color 8", "Color 9", "Color 10"];
const COLOR_NAMES: [&str; 10] =
    ["Color1", "Color2", "Color3", "Color4", "Color5", "Color6", "Color7", "Color8", "Color9", "Color10"];

impl Action {
    /// Toutes les actions, dans l'ordre d'affichage des réglages.
    pub fn all() -> Vec<Action> {
        let mut v = vec![
            Action::ToggleDraw,
            Action::HoldDraw,
            Action::ClearAll,
            Action::ToggleHalo,
            Action::ToggleRipple,
            Action::ToggleTrail,
            Action::TogglePin,
            Action::OpenSettings,
            Action::OpenHelp,
            Action::ToolArrow,
            Action::ToolRect,
            Action::ToolEllipse,
            Action::ToolFree,
            Action::ToolHighlighter,
            Action::ToolNumber,
        ];
        v.extend((0..10).map(Action::Color));
        v.extend([Action::Undo, Action::ExitDraw]);
        v
    }

    /// Vrai pour les raccourcis actifs dans tous les modes.
    pub fn is_global(self) -> bool {
        matches!(
            self,
            Action::ToggleDraw
                | Action::HoldDraw
                | Action::ClearAll
                | Action::ToggleHalo
                | Action::ToggleRipple
                | Action::ToggleTrail
                | Action::TogglePin
                | Action::OpenSettings
                | Action::OpenHelp
        )
    }

    /// Libellé affiché dans les réglages.
    pub fn label(self) -> &'static str {
        match self {
            Action::ToggleDraw => "Activer le dessin",
            Action::HoldDraw => "Dessiner en maintenant",
            Action::ClearAll => "Tout effacer",
            Action::ToggleHalo => "Afficher le halo",
            Action::ToggleRipple => "Onde au clic",
            Action::ToggleTrail => "Traînée laser",
            Action::TogglePin => "Garder les tracés",
            Action::OpenSettings => "Réglages",
            Action::OpenHelp => "Aide",
            Action::ToolArrow => "Flèche",
            Action::ToolRect => "Rectangle",
            Action::ToolEllipse => "Ellipse",
            Action::ToolFree => "Trait libre",
            Action::ToolHighlighter => "Surligneur",
            Action::ToolNumber => "Numéros",
            Action::Color(i) => COLOR_LABELS[(i as usize).min(9)],
            Action::Undo => "Annuler la dernière forme",
            Action::ExitDraw => "Quitter le mode dessin",
        }
    }

    /// Libellé dans une langue.
    pub fn label_in(self, lang: Lang) -> &'static str {
        if lang == Lang::Fr {
            return self.label();
        }
        match self {
            Action::ToggleDraw => "Toggle drawing",
            Action::HoldDraw => "Draw while held",
            Action::ClearAll => "Clear all",
            Action::ToggleHalo => "Show halo",
            Action::ToggleRipple => "Click ripple",
            Action::ToggleTrail => "Laser trail",
            Action::TogglePin => "Keep drawings",
            Action::OpenSettings => "Settings",
            Action::OpenHelp => "Help",
            Action::ToolArrow => "Arrow",
            Action::ToolRect => "Rectangle",
            Action::ToolEllipse => "Ellipse",
            Action::ToolFree => "Freehand",
            Action::ToolHighlighter => "Highlighter",
            Action::ToolNumber => "Numbers",
            Action::Color(i) => COLOR_LABELS_EN[(i as usize).min(9)],
            Action::Undo => "Undo last shape",
            Action::ExitDraw => "Leave drawing mode",
        }
    }

    /// Nom stable utilisé dans le fichier de réglages.
    pub fn name(self) -> &'static str {
        match self {
            Action::ToggleDraw => "ToggleDraw",
            Action::HoldDraw => "HoldDraw",
            Action::ClearAll => "ClearAll",
            Action::ToggleHalo => "ToggleHalo",
            Action::ToggleRipple => "ToggleRipple",
            Action::ToggleTrail => "ToggleTrail",
            Action::TogglePin => "TogglePin",
            Action::OpenSettings => "OpenSettings",
            Action::OpenHelp => "OpenHelp",
            Action::ToolArrow => "ToolArrow",
            Action::ToolRect => "ToolRect",
            Action::ToolEllipse => "ToolEllipse",
            Action::ToolFree => "ToolFree",
            Action::ToolHighlighter => "ToolHighlighter",
            Action::ToolNumber => "ToolNumber",
            Action::Color(i) => COLOR_NAMES[(i as usize).min(9)],
            Action::Undo => "Undo",
            Action::ExitDraw => "ExitDraw",
        }
    }

    pub fn from_name(name: &str) -> Option<Action> {
        Action::all().into_iter().find(|a| a.name() == name)
    }

    /// Raccourci par défaut dans une langue : la lettre des outils est l'initiale de leur nom.
    fn default_shortcut_in(self, lang: Lang) -> Shortcut {
        let letter = match (lang, self) {
            (Lang::En, Action::ToolArrow) => "A",
            (Lang::En, Action::ToolEllipse) => "E",
            (Lang::En, Action::ToolFree) => "F",
            (Lang::En, Action::ToolHighlighter) => "H",
            (Lang::En, Action::ToggleRipple) => "Ctrl+Alt+R",
            (Lang::En, Action::TogglePin) => "Ctrl+Alt+K",
            (Lang::En, Action::OpenSettings) => "Ctrl+Alt+S",
            (Lang::En, Action::OpenHelp) => "Ctrl+Alt+F1",
            _ => return self.default_shortcut(),
        };
        letter.parse().expect("raccourci par défaut valide")
    }

    fn default_shortcut(self) -> Shortcut {
        let s = match self {
            Action::ToggleDraw => "Ctrl+Alt+D",
            Action::HoldDraw => "Ctrl",
            Action::ClearAll => "Ctrl+Alt+E",
            Action::ToggleHalo => "Ctrl+Alt+H",
            Action::ToggleRipple => "Ctrl+Alt+O",
            Action::ToggleTrail => "Ctrl+Alt+T",
            Action::TogglePin => "Ctrl+Alt+G",
            Action::OpenSettings => "Ctrl+Alt+R",
            Action::OpenHelp => "Ctrl+Alt+A",
            Action::ToolArrow => "F",
            Action::ToolRect => "R",
            Action::ToolEllipse => "C",
            Action::ToolFree => "D",
            Action::ToolHighlighter => "S",
            Action::ToolNumber => "N",
            Action::Color(i) => ["1", "2", "3", "4", "5", "6", "7", "8", "9", "0"][(i as usize).min(9)],
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

/// Raison du refus d'un raccourci.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BindError {
    /// Le raccourci est déjà pris par cette action.
    Conflict(Action),
    /// Le raccourci ne convient pas à l'action.
    Invalid(Reason),
}

/// Pourquoi un raccourci ne convient pas à une action.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Reason {
    /// Le raccourci de maintien : des touches de modification seules, ou une touche de fonction seule.
    HoldModifiersOnly,
    /// Il manque une touche.
    NeedsKey,
    /// Un raccourci global ne doit pas être une touche de frappe ordinaire.
    NeedsModifier,
    /// Un double appui global ne se fait pas sur une touche qui écrit un caractère.
    DoubleNotTyping,
}

impl Reason {
    /// Explication affichée dans les réglages.
    pub fn message(self, lang: Lang) -> &'static str {
        match (self, lang) {
            (Reason::HoldModifiersOnly, Lang::Fr) => "Ce raccourci se maintient : Ctrl, Alt, Maj, Win, ou une touche F1 à F12.",
            (Reason::HoldModifiersOnly, Lang::En) => "This shortcut is held down: Ctrl, Alt, Shift, Win, or an F1 to F12 key.",
            (Reason::NeedsKey, Lang::Fr) => "Ajoutez une touche à ce raccourci.",
            (Reason::NeedsKey, Lang::En) => "Add a key to this shortcut.",
            (Reason::NeedsModifier, Lang::Fr) => "Ajoutez Ctrl, Alt ou Win, ou choisissez une touche de fonction.",
            (Reason::NeedsModifier, Lang::En) => "Add Ctrl, Alt or Win, or pick a function key.",
            (Reason::DoubleNotTyping, Lang::Fr) => "Double appui : choisissez Ctrl, Maj, Tab ou une touche F1 à F12.",
            (Reason::DoubleNotTyping, Lang::En) => "Double tap: pick Ctrl, Shift, Tab or an F1 to F12 key.",
        }
    }
}

/// Vrai pour une touche qui écrit un caractère : lettre, chiffre, pavé numérique, espace, signe.
/// Tab n'en fait pas partie : on ne le double pas en écrivant.
pub fn is_typing_key(vk: u32) -> bool {
    matches!(vk, 0x20 | 0x30..=0x39 | 0x41..=0x5A | 0x60..=0x6F | 0xBA..=0xC0 | 0xDB..=0xDF | 0xE2)
}

/// Vérifie qu'un raccourci convient à une action, indépendamment des autres raccourcis.
pub fn validate(action: Action, s: Shortcut) -> Result<(), Reason> {
    if action == Action::HoldDraw {
        // Des touches de modification seules (Ctrl, Ctrl+Alt…), ou une touche de fonction seule.
        let modifiers_only = s.key.is_none() && !s.mods.is_empty();
        let function_key = s.mods.is_empty() && s.key.is_some_and(|k| (0x70..=0x7B).contains(&k));
        return if !s.double && (modifiers_only || function_key) { Ok(()) } else { Err(Reason::HoldModifiersOnly) };
    }
    let Some(key) = s.key else {
        return Err(Reason::NeedsKey);
    };
    if s.double {
        // Un double appui actif partout ne doit pas être une touche qui écrit un caractère :
        // deux lettres identiques tapées vite (« elle ») le déclencheraient en pleine frappe.
        return if action.is_global() && is_typing_key(key) { Err(Reason::DoubleNotTyping) } else { Ok(()) };
    }
    // Un raccourci actif partout ne doit pas être une touche de frappe ordinaire : il serait
    // intercepté dans toutes les applications.
    let function_key = (0x70..=0x7B).contains(&key);
    if action.is_global() && !(s.mods.ctrl || s.mods.alt || s.mods.win || function_key) {
        return Err(Reason::NeedsModifier);
    }
    Ok(())
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

    /// Raccourcis par défaut d'une langue.
    pub fn defaults_in(lang: Lang) -> Bindings {
        Bindings(Action::all().into_iter().map(|a| (a, a.default_shortcut_in(lang))).collect())
    }

    /// Changement de langue : les raccourcis restés à leur valeur par défaut dans l'ancienne
    /// langue prennent celle de la nouvelle. Un raccourci choisi par l'utilisateur est conservé,
    /// et un outil garde sa lettre si la nouvelle est déjà prise par un tel choix.
    pub fn relocalize(&mut self, from: Lang, to: Lang) {
        let moving: Vec<Action> =
            Action::all().into_iter().filter(|a| self.get(*a) == a.default_shortcut_in(from)).collect();
        let kept: Vec<Shortcut> =
            self.0.iter().filter(|(a, _)| !moving.contains(a)).map(|(_, s)| s.normalized()).collect();
        for a in moving {
            let wanted = a.default_shortcut_in(to);
            if !kept.contains(&wanted.normalized()) {
                self.0.insert(a, wanted);
            }
        }
    }

    pub fn get(&self, a: Action) -> Shortcut {
        self.0.get(&a).copied().unwrap_or_else(|| a.default_shortcut())
    }

    /// Affecte un raccourci. Refuse s'il ne convient pas à l'action, ou s'il est déjà pris
    /// (en nommant alors l'action en conflit).
    pub fn set(&mut self, a: Action, s: Shortcut) -> Result<(), BindError> {
        let s = s.normalized();
        validate(a, s).map_err(BindError::Invalid)?;
        if let Some((other, _)) = self.0.iter().find(|(o, v)| **o != a && v.normalized() == s) {
            return Err(BindError::Conflict(*other));
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

    /// Action déclenchée par un double appui sur la touche `vk` (code canonique).
    pub fn double_action_for(&self, vk: u32, draw_mode: bool) -> Option<Action> {
        self.0
            .iter()
            .find(|(a, s)| s.double && s.key == Some(vk) && **a != Action::HoldDraw && (a.is_global() || draw_mode))
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
                if validate(a, s).is_ok() {
                    b.0.insert(a, s.normalized());
                }
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
        assert_eq!(g(Action::HoldDraw), "Ctrl");
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
        assert_eq!(g(Action::Color(8)), "9");
        assert_eq!(g(Action::Color(9)), "0"); // la dixième couleur est sur la touche 0
        assert_eq!(g(Action::Undo), "Ctrl+Z");
        assert_eq!(g(Action::ExitDraw), "Échap");
        assert_eq!(b.all().len(), 27);
    }

    #[test]
    fn conflicts_are_refused_and_named() {
        let mut b = Bindings::defaults();
        assert_eq!(b.set(Action::ToolRect, sc("F")), Err(BindError::Conflict(Action::ToolArrow)));
        assert_eq!(b.get(Action::ToolRect).to_string(), "R");
        assert!(b.set(Action::ToolRect, sc("R")).is_ok()); // réaffecter sa propre valeur
        assert!(b.set(Action::ToolRect, sc("Ctrl+Alt+Y")).is_ok());
        assert_eq!(b.get(Action::ToolRect).to_string(), "Ctrl+Alt+Y");
    }

    #[test]
    fn numpad_and_top_row_digits_conflict() {
        let mut b = Bindings::defaults();
        let numpad1 = Shortcut { mods: Mods::default(), key: Some(0x61), double: false };
        assert_eq!(b.set(Action::ToolRect, numpad1), Err(BindError::Conflict(Action::Color(0))));
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
        assert_eq!(Action::HoldDraw.label(), "Dessiner en maintenant");
        assert_eq!(Action::ToggleDraw.label(), "Activer le dessin");
        assert_eq!(Action::ToggleHalo.label(), "Afficher le halo");
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
    fn double_tap_parse_display_roundtrip() {
        for s in ["2×Tab", "2×Ctrl", "2×Maj", "2×F", "2×Échap"] {
            assert_eq!(sc(s).to_string(), s);
            assert!(sc(s).double);
        }
        assert_eq!(sc("2xtab"), sc("2×Tab"));
        assert!(!sc("Tab").double);
        assert_ne!(sc("Tab"), sc("2×Tab"));
    }

    #[test]
    fn double_tap_rejects_combinations() {
        assert!("2×Ctrl+D".parse::<Shortcut>().is_err());
        assert!("2×".parse::<Shortcut>().is_err());
        assert!("Ctrl+2×D".parse::<Shortcut>().is_err());
    }

    #[test]
    fn global_double_tap_refuses_typing_keys() {
        // Lettre, chiffre, pavé numérique, espace, signe : un double appui global serait
        // intercepté en pleine frappe dans toutes les applications.
        for s in ["2×A", "2×5", "2×Espace"] {
            let mut b = Bindings::defaults();
            assert_eq!(b.set(Action::ToggleDraw, sc(s)), Err(BindError::Invalid(Reason::DoubleNotTyping)), "{s}");
        }
        for vk in [0x61, 0x6B, 0xBA, 0xE2] {
            let s = Shortcut { mods: Mods::default(), key: Some(vk), double: true };
            assert_eq!(validate(Action::ClearAll, s), Err(Reason::DoubleNotTyping), "{vk:#x}");
        }
        // Les touches qui n'écrivent rien restent permises, ainsi que Tab.
        for s in ["2×Tab", "2×Ctrl", "2×Maj", "2×F8", "2×Inser"] {
            let mut b = Bindings::defaults();
            assert!(b.set(Action::ToggleDraw, sc(s)).is_ok(), "{s}");
        }
        // En mode dessin le clavier est déjà réservé au dessin : une lettre y est permise.
        assert!(validate(Action::ToolRect, sc("2×R")).is_ok());
    }

    #[test]
    fn config_file_cannot_bind_a_global_double_tap_on_a_typing_key() {
        let b: Bindings =
            serde_json::from_str(r#"{"ToggleDraw":"2×E","ClearAll":"2×1","TogglePin":"2×Tab"}"#).unwrap();
        assert_eq!(b.get(Action::ToggleDraw), Action::ToggleDraw.default_shortcut());
        assert_eq!(b.get(Action::ClearAll), Action::ClearAll.default_shortcut());
        assert_eq!(b.get(Action::TogglePin).to_string(), "2×Tab");
        assert_eq!(b.double_action_for(0x45, false), None);
        assert_eq!(b.double_action_for(0x31, false), None);
    }

    #[test]
    fn double_tap_bindings() {
        let mut b = Bindings::defaults();
        assert!(b.set(Action::ToggleDraw, sc("2×Tab")).is_ok());
        assert_eq!(b.set(Action::ClearAll, sc("2×Tab")), Err(BindError::Conflict(Action::ToggleDraw)));
        assert_eq!(b.double_action_for(0x09, false), Some(Action::ToggleDraw));
        assert_eq!(b.double_action_for(0x41, false), None);
        assert_eq!(b.action_for(sc("Tab"), true), None); // un seul appui ne déclenche rien
        assert!(b.set(Action::ToolRect, sc("2×R")).is_ok());
        assert_eq!(b.double_action_for(0x52, false), None); // action propre au mode dessin
        assert_eq!(b.double_action_for(0x52, true), Some(Action::ToolRect));
        let json = serde_json::to_string(&b).unwrap();
        let back: Bindings = serde_json::from_str(&json).unwrap();
        assert_eq!(back.get(Action::ToggleDraw).to_string(), "2×Tab");
    }

    #[test]
    fn double_tap_key_normalization() {
        assert_eq!(double_tap_vk(0xA1), Some(0xA0)); // Maj droite -> Maj
        assert_eq!(double_tap_vk(0xA2), Some(0xA2)); // Ctrl gauche
        assert_eq!(double_tap_vk(0xA3), None); // Ctrl droit : ne compte jamais
        assert_eq!(double_tap_vk(0xA5), None); // AltGr : ne compte jamais
        // Alt et Win ouvrent des menus quand on les tape seuls : pas de double appui dessus.
        assert_eq!(double_tap_vk(0xA4), None);
        assert_eq!(double_tap_vk(0x5B), None);
        assert!("2×Alt".parse::<Shortcut>().is_err());
        assert!("2×Win".parse::<Shortcut>().is_err());
        assert_eq!(double_tap_vk(0x61), Some(0x31)); // pavé numérique
    }

    #[test]
    fn swapped_bindings_survive_reload() {
        let b: Bindings = serde_json::from_str(r#"{"ToolArrow":"R","ToolRect":"F"}"#).unwrap();
        assert_eq!(b.get(Action::ToolArrow).to_string(), "R");
        assert_eq!(b.get(Action::ToolRect).to_string(), "F");
    }

    fn invalid(r: Result<(), BindError>) -> bool {
        matches!(r, Err(BindError::Invalid(_)))
    }

    #[test]
    fn global_shortcuts_must_not_be_plain_typing_keys() {
        let mut b = Bindings::defaults();
        // Une lettre seule, ou Maj+lettre, avalerait la frappe dans toutes les applications.
        assert!(invalid(b.set(Action::ClearAll, sc("E"))));
        assert!(invalid(b.set(Action::ClearAll, sc("Maj+E"))));
        assert!(invalid(b.set(Action::ToggleDraw, sc("Espace"))));
        assert_eq!(b.get(Action::ClearAll).to_string(), "Ctrl+Alt+E");
        assert!(b.set(Action::ClearAll, sc("Ctrl+E")).is_ok());
        assert!(b.set(Action::ClearAll, sc("F8")).is_ok()); // une touche de fonction seule est admise
        assert!(b.set(Action::ToggleHalo, sc("Win+H")).is_ok());
    }

    #[test]
    fn one_shot_actions_need_a_key() {
        let mut b = Bindings::defaults();
        assert!(invalid(b.set(Action::ToggleDraw, sc("Ctrl"))));
        assert!(invalid(b.set(Action::ToolArrow, sc("Ctrl+Maj"))));
        assert!(b.set(Action::ToolArrow, sc("A")).is_ok()); // en mode dessin, une lettre seule convient
    }

    #[test]
    fn hold_action_takes_modifiers_only() {
        let mut b = Bindings::defaults();
        assert!(invalid(b.set(Action::HoldDraw, sc("Ctrl+Alt+X"))));
        assert!(invalid(b.set(Action::HoldDraw, sc("2×Ctrl"))));
        assert!(invalid(b.set(Action::HoldDraw, sc("X"))));
        assert!(invalid(b.set(Action::HoldDraw, sc("Ctrl+F8"))));
        assert_eq!(b.get(Action::HoldDraw).to_string(), "Ctrl");
        assert!(b.set(Action::HoldDraw, sc("Ctrl+Maj")).is_ok());
        assert!(b.set(Action::HoldDraw, sc("Ctrl+Alt")).is_ok());
        // Une touche de fonction seule convient aussi.
        assert!(b.set(Action::HoldDraw, sc("F8")).is_ok());
        assert_eq!(b.set(Action::ClearAll, sc("F8")), Err(BindError::Conflict(Action::HoldDraw)));
    }

    #[test]
    fn invalid_shapes_in_a_file_fall_back_to_defaults() {
        let b: Bindings =
            serde_json::from_str(r#"{"ClearAll":"E","ToggleDraw":"Ctrl","HoldDraw":"Ctrl+Alt+X","ToolRect":"X"}"#).unwrap();
        assert_eq!(b.get(Action::ClearAll).to_string(), "Ctrl+Alt+E");
        assert_eq!(b.get(Action::ToggleDraw).to_string(), "Ctrl+Alt+D");
        assert_eq!(b.get(Action::HoldDraw).to_string(), "Ctrl");
        assert_eq!(b.get(Action::ToolRect).to_string(), "X");
    }

    #[test]
    fn every_key_the_capture_can_emit_survives_a_restart() {
        // Toute touche, nommée ou non, doit pouvoir être relue depuis le fichier de réglages.
        for vk in 0x08u32..=0xFE {
            if matches!(vk, 0x10..=0x12 | 0x5B | 0x5C | 0xA0..=0xA5) {
                continue; // touches de modification
            }
            let s = Shortcut { mods: Mods { ctrl: true, ..Default::default() }, key: Some(vk), double: false }.normalized();
            let text = s.to_string();
            assert_eq!(text.parse::<Shortcut>().as_ref(), Ok(&s), "touche {vk:#x} affichée « {text} »");
        }
    }

    #[test]
    fn common_keys_have_french_names() {
        for (vk, name) in [(0x08, "Retour arrière"), (0x2D, "Inser"), (0x24, "Début"), (0x23, "Fin"), (0x21, "PgPréc"), (0x22, "PgSuiv")] {
            let s = Shortcut { mods: Mods { ctrl: true, ..Default::default() }, key: Some(vk), double: false };
            assert_eq!(s.to_string(), format!("Ctrl+{name}"));
        }
        assert_eq!(sc("Ctrl+Touche222").key, Some(222)); // touche ² du clavier français
    }

    #[test]
    fn shortcuts_display_in_english() {
        use crate::i18n::Lang;
        for (fr, en) in [
            ("Ctrl+Maj+F5", "Ctrl+Shift+F5"),
            ("Échap", "Esc"),
            ("Win+Espace", "Win+Space"),
            ("Alt+Gauche", "Alt+Left"),
            ("Ctrl+Suppr", "Ctrl+Del"),
            ("Ctrl+Retour arrière", "Ctrl+Backspace"),
            ("2×Maj", "2×Shift"),
            ("2×Tab", "2×Tab"),
            ("Ctrl+Touche222", "Ctrl+Key222"),
        ] {
            assert_eq!(sc(fr).display(Lang::En), en);
            assert_eq!(sc(fr).display(Lang::Fr), fr);
            // Le nom anglais se relit aussi, et le fichier de réglages garde la forme française.
            assert_eq!(sc(en), sc(fr), "{en}");
            assert_eq!(sc(en).to_string(), fr);
        }
    }

    #[test]
    fn labels_and_refusal_reasons_are_translated() {
        use crate::i18n::Lang;
        assert_eq!(Action::ToggleDraw.label_in(Lang::En), "Toggle drawing");
        assert_eq!(Action::HoldDraw.label_in(Lang::En), "Draw while held");
        assert_eq!(Action::ClearAll.label_in(Lang::En), "Clear all");
        assert_eq!(Action::ToolArrow.label_in(Lang::En), "Arrow");
        assert_eq!(Action::ToolHighlighter.label_in(Lang::En), "Highlighter");
        assert_eq!(Action::Color(2).label_in(Lang::En), "Color 3");
        assert_eq!(Action::ExitDraw.label_in(Lang::En), "Leave drawing mode");
        for a in Action::all() {
            assert_eq!(a.label_in(Lang::Fr), a.label());
            assert!(!a.label_in(Lang::En).is_empty());
        }
        for reason in [Reason::HoldModifiersOnly, Reason::NeedsKey, Reason::NeedsModifier, Reason::DoubleNotTyping] {
            assert_ne!(reason.message(Lang::Fr), reason.message(Lang::En));
        }
        let mut b = Bindings::defaults();
        assert_eq!(b.set(Action::ClearAll, sc("E")), Err(BindError::Invalid(Reason::NeedsModifier)));
        assert_eq!(b.set(Action::ToggleDraw, sc("Ctrl")), Err(BindError::Invalid(Reason::NeedsKey)));
        assert_eq!(b.set(Action::HoldDraw, sc("X")), Err(BindError::Invalid(Reason::HoldModifiersOnly)));
    }

    #[test]
    fn ten_colors_have_labels_and_names() {
        use crate::i18n::Lang;
        assert_eq!(Action::Color(9).label(), "Couleur 10");
        assert_eq!(Action::Color(9).label_in(Lang::En), "Color 10");
        assert_eq!(Action::Color(9).name(), "Color10");
        assert_eq!(Action::from_name("Color10"), Some(Action::Color(9)));
        assert_eq!(Action::all().iter().filter(|a| matches!(a, Action::Color(_))).count(), 10);
    }

    #[test]
    fn tool_shortcuts_follow_the_language() {
        use crate::i18n::Lang;
        let en = Bindings::defaults_in(Lang::En);
        let g = |a| en.get(a).to_string();
        // Première lettre du nom anglais de l'outil.
        assert_eq!(g(Action::ToolArrow), "A");
        assert_eq!(g(Action::ToolRect), "R");
        assert_eq!(g(Action::ToolEllipse), "E");
        assert_eq!(g(Action::ToolFree), "F");
        assert_eq!(g(Action::ToolHighlighter), "H");
        assert_eq!(g(Action::ToolNumber), "N");
        // Le reste ne dépend pas de la langue.
        assert_eq!(g(Action::ToggleDraw), "Ctrl+Alt+D");
        assert_eq!(g(Action::Color(0)), "1");
        assert_eq!(Bindings::defaults_in(Lang::Fr), Bindings::defaults());
    }

    #[test]
    fn switching_language_moves_default_tool_shortcuts_only() {
        use crate::i18n::Lang;
        let mut b = Bindings::defaults();
        b.relocalize(Lang::Fr, Lang::En);
        assert_eq!(b, Bindings::defaults_in(Lang::En));
        b.relocalize(Lang::En, Lang::Fr);
        assert_eq!(b, Bindings::defaults());

        // Un raccourci choisi par l'utilisateur est conservé.
        let mut b = Bindings::defaults();
        b.set(Action::ToolArrow, sc("X")).unwrap();
        b.relocalize(Lang::Fr, Lang::En);
        assert_eq!(b.get(Action::ToolArrow).to_string(), "X");
        assert_eq!(b.get(Action::ToolEllipse).to_string(), "E");

        // Si la lettre anglaise est déjà prise par un choix de l'utilisateur, l'outil garde la sienne.
        let mut b = Bindings::defaults();
        b.set(Action::Undo, sc("H")).unwrap();
        b.relocalize(Lang::Fr, Lang::En);
        assert_eq!(b.get(Action::ToolHighlighter).to_string(), "S");
        assert_eq!(b.get(Action::Undo).to_string(), "H");
        // Aucun doublon, dans tous les cas.
        let all = b.all();
        for (i, (_, s)) in all.iter().enumerate() {
            assert!(all[i + 1..].iter().all(|(_, o)| o != s), "doublon sur {s}");
        }
    }

    #[test]
    fn pointer_effects_settings_and_help_have_shortcuts() {
        use crate::i18n::Lang;
        let extra = [Action::ToggleRipple, Action::ToggleTrail, Action::TogglePin, Action::OpenSettings, Action::OpenHelp];
        let keys = |lang| {
            let b = Bindings::defaults_in(lang);
            extra.map(|a| b.get(a).to_string())
        };
        // Initiale du nom dans chaque langue : Onde, Traînée, Garder, Réglages, Aide.
        assert_eq!(keys(Lang::Fr), ["Ctrl+Alt+O", "Ctrl+Alt+T", "Ctrl+Alt+G", "Ctrl+Alt+R", "Ctrl+Alt+A"]);
        // Ripple, Trail, Keep, Settings ; Help prend F1, la lettre H étant celle du halo.
        assert_eq!(keys(Lang::En), ["Ctrl+Alt+R", "Ctrl+Alt+T", "Ctrl+Alt+K", "Ctrl+Alt+S", "Ctrl+Alt+F1"]);
        for a in extra {
            assert!(a.is_global(), "{a:?}");
            assert_eq!(Action::from_name(a.name()), Some(a));
            assert_ne!(a.label_in(Lang::En), a.label());
        }
        // Aucun doublon dans les raccourcis par défaut, dans les deux langues.
        for lang in Lang::all() {
            let all = Bindings::defaults_in(lang).all();
            for (i, (_, s)) in all.iter().enumerate() {
                assert!(all[i + 1..].iter().all(|(_, o)| o != s), "doublon sur {s} en {lang:?}");
            }
        }
    }
}
