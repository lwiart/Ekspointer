//! Réglages : modèle, lecture tolérante, écriture atomique.

use crate::shapes::Tool;
use crate::shortcuts::Bindings;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

// IMPL

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shortcuts::Action;
    use std::sync::atomic::{AtomicU32, Ordering};

    /// Chemin d'un fichier dans un dossier temporaire propre à chaque appel.
    fn tmp(name: &str) -> PathBuf {
        static N: AtomicU32 = AtomicU32::new(0);
        let dir = std::env::temp_dir().join(format!(
            "ekspointer-test-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::SeqCst)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir.join(name)
    }

    #[test]
    fn defaults_match_spec() {
        let c = Config::default();
        assert_eq!((c.version, c.hold_seconds, c.width, c.color), (1, 5, 1, 0));
        assert_eq!(c.last_tool, Tool::Arrow);
        assert_eq!((c.halo_on, c.halo_size, c.halo_follows_color), (false, 64, true));
        assert_eq!((c.ripple_on, c.trail_on, c.pinned), (false, false, false));
        assert_eq!((c.toolbar_pos, c.toolbar_collapsed, c.hide_from_capture), (None, false, true));
        assert_eq!(c.bindings, Bindings::defaults());
    }

    #[test]
    fn missing_file_gives_defaults() {
        let (c, o) = load(&tmp("absent.json"));
        assert!(matches!(o, LoadOutcome::Defaults));
        assert_eq!(c.hold_seconds, 5);
        assert!(c.hide_from_capture);
    }

    #[test]
    fn roundtrip() {
        let p = tmp("rt.json");
        let mut c = Config::default();
        c.hold_seconds = 9;
        c.color = 3;
        c.last_tool = Tool::Highlighter;
        c.toolbar_pos = Some((120, -40));
        c.bindings.set(Action::ToggleDraw, "Ctrl+Alt+X".parse().unwrap()).unwrap();
        save(&p, &c).unwrap();
        let (d, o) = load(&p);
        assert!(matches!(o, LoadOutcome::Loaded));
        assert_eq!(d, c);
        assert!(!p.with_extension("json.tmp").exists());
    }

    #[test]
    fn save_overwrites_existing_file() {
        let p = tmp("ow.json");
        let mut c = Config::default();
        save(&p, &c).unwrap();
        c.color = 4;
        save(&p, &c).unwrap();
        assert_eq!(load(&p).0.color, 4);
    }

    #[test]
    fn invalid_file_is_renamed() {
        let p = tmp("config.json");
        std::fs::write(&p, "{ pas du json").unwrap();
        let (c, o) = load(&p);
        assert_eq!(c.hold_seconds, 5);
        match o {
            LoadOutcome::Invalid { backup } => {
                assert_eq!(backup.file_name().unwrap(), "config.invalide.json");
                assert_eq!(std::fs::read_to_string(&backup).unwrap(), "{ pas du json");
                assert!(!p.exists());
            }
            other => panic!("attendu Invalid, obtenu {other:?}"),
        }
    }

    #[test]
    fn unknown_and_missing_fields_tolerated() {
        let p = tmp("partial.json");
        std::fs::write(&p, r#"{"hold_seconds":8,"champ_futur":true}"#).unwrap();
        let (c, o) = load(&p);
        assert!(matches!(o, LoadOutcome::Loaded));
        assert_eq!(c.hold_seconds, 8);
        assert_eq!(c.halo_size, 64);
    }

    #[test]
    fn sanitize_clamps_out_of_range() {
        let p = tmp("range.json");
        std::fs::write(&p, r#"{"hold_seconds":200,"width":7,"color":9,"halo_size":5}"#).unwrap();
        let (c, _) = load(&p);
        assert_eq!((c.hold_seconds, c.width, c.color, c.halo_size), (15, 2, 5, 40));
        std::fs::write(&p, r#"{"hold_seconds":0,"halo_size":100000}"#).unwrap();
        let (c, _) = load(&p);
        assert_eq!((c.hold_seconds, c.halo_size), (2, 120));
    }

    #[test]
    fn invalid_or_conflicting_bindings_fall_back() {
        let p = tmp("binds.json");
        std::fs::write(
            &p,
            r#"{"bindings":{"ToolArrow":"Bidule","ToolRect":"C","ToggleDraw":"Ctrl+Alt+X","ActionInconnue":"Q"}}"#,
        )
        .unwrap();
        let (c, o) = load(&p);
        assert!(matches!(o, LoadOutcome::Loaded));
        assert_eq!(c.bindings.get(Action::ToolArrow).to_string(), "F"); // illisible -> défaut
        assert_eq!(c.bindings.get(Action::ToolRect).to_string(), "R"); // conflit avec Ellipse -> défaut
        assert_eq!(c.bindings.get(Action::ToolEllipse).to_string(), "C");
        assert_eq!(c.bindings.get(Action::ToggleDraw).to_string(), "Ctrl+Alt+X");
    }

    #[test]
    fn default_path_is_next_to_executable() {
        let p = default_path();
        assert_eq!(p.file_name().unwrap(), "config.json");
        assert_eq!(p.parent().unwrap(), std::env::current_exe().unwrap().parent().unwrap());
    }
}
