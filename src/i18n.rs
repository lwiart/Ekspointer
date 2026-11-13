//! Langue de l'interface : français ou anglais.

use serde::{Deserialize, Serialize};

/// Langue de l'interface.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
#[serde(rename_all = "lowercase")]
pub enum Lang {
    #[default]
    Fr,
    En,
}

impl Lang {
    pub fn all() -> [Lang; 2] {
        [Lang::Fr, Lang::En]
    }

    /// Nom de la langue, dans cette langue.
    pub fn name(self) -> &'static str {
        match self {
            Lang::Fr => "Français",
            Lang::En => "English",
        }
    }

    /// Langue de départ d'après la langue de Windows (« fr-FR », « en-US »…) :
    /// français pour un système en français, anglais pour tous les autres.
    pub fn from_locale(locale: &str) -> Lang {
        if locale.to_lowercase().starts_with("fr") {
            Lang::Fr
        } else {
            Lang::En
        }
    }
}

/// Textes de l'interface qui ne sont le libellé ni d'une action ni d'une ligne de réglage.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Text {
    Settings,
    Help,
    /// Sélecteur du type de raccourci : combinaison de touches.
    ModeKeys,
    /// Invite de saisie d'une combinaison.
    TypeShortcut,
    /// Invite de saisie d'une touche seule (double appui).
    TypeKey,
    TrayShow,
    TrayHide,
    TrayQuit,
    InvalidConfig,
    SaveFailed,
    HookFailed,
    StartFailed,
}

impl Text {
    pub fn all() -> [Text; 12] {
        use Text::*;
        [Settings, Help, ModeKeys, TypeShortcut, TypeKey, TrayShow, TrayHide, TrayQuit, InvalidConfig, SaveFailed, HookFailed, StartFailed]
    }
}

/// Texte de l'interface dans une langue.
pub fn t(lang: Lang, text: Text) -> &'static str {
    use Lang::*;
    match (text, lang) {
        (Text::Settings, Fr) => "Réglages",
        (Text::Settings, En) => "Settings",
        (Text::Help, Fr) => "Aide",
        (Text::Help, En) => "Help",
        (Text::ModeKeys, Fr) => "Touches",
        (Text::ModeKeys, En) => "Keys",
        (Text::TypeShortcut, Fr) => "Tapez…",
        (Text::TypeShortcut, En) => "Press keys…",
        (Text::TypeKey, Fr) => "Tapez la touche",
        (Text::TypeKey, En) => "Press a key",
        (Text::TrayShow, Fr) => "Afficher la barre",
        (Text::TrayShow, En) => "Show toolbar",
        (Text::TrayHide, Fr) => "Masquer la barre",
        (Text::TrayHide, En) => "Hide toolbar",
        (Text::TrayQuit, Fr) => "Quitter",
        (Text::TrayQuit, En) => "Quit",
        (Text::InvalidConfig, Fr) => {
            "Le fichier de réglages était illisible. Il a été renommé en config.invalide.json et les réglages par défaut sont utilisés."
        }
        (Text::InvalidConfig, En) => {
            "The settings file could not be read. It was renamed to config.invalide.json and default settings are used."
        }
        (Text::SaveFailed, Fr) => {
            "Les réglages ne peuvent pas être enregistrés dans ce dossier. Ils seront perdus à la fermeture."
        }
        (Text::SaveFailed, En) => "Settings cannot be saved in this folder. They will be lost when Ekspointer closes.",
        (Text::HookFailed, Fr) => "Ekspointer n'a pas pu démarrer l'écoute du clavier et de la souris.",
        (Text::HookFailed, En) => "Ekspointer could not start listening to the keyboard and mouse.",
        (Text::StartFailed, Fr) => "Ekspointer n'a pas pu démarrer",
        (Text::StartFailed, En) => "Ekspointer could not start",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn language_follows_the_system_locale() {
        assert_eq!(Lang::from_locale("fr-FR"), Lang::Fr);
        assert_eq!(Lang::from_locale("fr-CA"), Lang::Fr);
        assert_eq!(Lang::from_locale("FR"), Lang::Fr);
        assert_eq!(Lang::from_locale("en-US"), Lang::En);
        assert_eq!(Lang::from_locale("de-DE"), Lang::En); // toute autre langue : anglais
        assert_eq!(Lang::from_locale(""), Lang::En);
    }

    #[test]
    fn language_is_saved_as_a_short_code() {
        assert_eq!(serde_json::to_string(&Lang::Fr).unwrap(), "\"fr\"");
        assert_eq!(serde_json::from_str::<Lang>("\"en\"").unwrap(), Lang::En);
        assert_eq!(Lang::default(), Lang::Fr);
        assert_eq!((Lang::Fr.name(), Lang::En.name()), ("Français", "English"));
        assert_eq!(Lang::all(), [Lang::Fr, Lang::En]);
    }

    #[test]
    fn every_text_exists_in_both_languages() {
        for text in Text::all() {
            let (fr, en) = (t(Lang::Fr, text), t(Lang::En, text));
            assert!(!fr.is_empty() && !en.is_empty(), "{text:?}");
            assert_ne!(fr, en, "{text:?} n'est pas traduit");
        }
    }

    #[test]
    fn a_few_texts() {
        assert_eq!(t(Lang::Fr, Text::Settings), "Réglages");
        assert_eq!(t(Lang::En, Text::Settings), "Settings");
        assert_eq!(t(Lang::Fr, Text::Help), "Aide");
        assert_eq!(t(Lang::En, Text::Help), "Help");
        assert_eq!(t(Lang::En, Text::TrayQuit), "Quit");
        assert_eq!(t(Lang::En, Text::ModeKeys), "Keys");
    }
}
