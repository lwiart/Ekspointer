//! Raccourcis clavier : modèle, analyse, affichage, détection de conflits.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::BTreeMap;
use std::fmt;
use std::str::FromStr;

// IMPL

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
