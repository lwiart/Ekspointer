//! Réglages sur disque : l'état survit à un redémarrage, un fichier abîmé ne bloque rien.

mod common;
use common::*;
use ekspointer::app::UiCommand;
use ekspointer::config::{self, LoadOutcome};
use ekspointer::shapes::Tool;
use ekspointer::shortcuts::Action;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

/// Dossier temporaire propre à chaque test.
fn temp_dir() -> PathBuf {
    static N: AtomicU32 = AtomicU32::new(0);
    let dir = std::env::temp_dir().join(format!(
        "ekspointer-it-{}-{}",
        std::process::id(),
        N.fetch_add(1, Ordering::SeqCst)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn state_survives_restart() {
    let path = temp_dir().join("config.json");
    let mut r = Rig::new();
    r.command(UiCommand::SetColor(3));
    r.command(UiCommand::SetWidth(2));
    r.command(UiCommand::ToggleHalo);
    r.command(UiCommand::SetHaloSize(90));
    r.command(UiCommand::TogglePin);
    r.command(UiCommand::SelectTool(Some(Tool::Highlighter)));
    r.command(UiCommand::SetBinding(Action::ToggleDraw, "Ctrl+Alt+X".parse().unwrap()));
    r.command(UiCommand::SetToolbarPos(300, 40));
    config::save(&path, &r.app.cfg).unwrap();

    let (cfg, outcome) = config::load(&path);
    assert!(matches!(outcome, LoadOutcome::Loaded));
    let mut r2 = Rig::with_config(cfg);
    assert_eq!(r2.app.cfg.color, 3);
    assert_eq!(r2.app.cfg.width, 2);
    assert!(!r2.app.cfg.halo_on); // actif par défaut, coupé avant l'enregistrement
    assert_eq!(r2.app.cfg.halo_size, 90);
    assert!(r2.app.scene.pinned());
    assert_eq!(r2.app.tool, Tool::Highlighter);
    assert_eq!(r2.app.cfg.toolbar_pos, Some((300, 40)));
    assert!(!r2.app.draw_mode); // le mode dessin, lui, ne survit pas au redémarrage

    r2.chord(&[LCTRL, LALT, 0x58]);
    assert!(r2.app.draw_mode);
}

#[test]
fn corrupt_file_recovers_to_working_app() {
    let dir = temp_dir();
    let path = dir.join("config.json");
    std::fs::write(&path, "{\"hold_seconds\": ").unwrap();

    let (cfg, outcome) = config::load(&path);
    assert!(matches!(outcome, LoadOutcome::Invalid { .. }));
    assert!(dir.join("config.invalide.json").exists());
    assert!(!path.exists());

    let mut r = Rig::with_config(cfg);
    r.chord(&[LCTRL, LALT, 0x44]);
    assert!(r.app.draw_mode);

    // Les réglages suivants s'enregistrent normalement.
    r.command(UiCommand::SetColor(2));
    config::save(&path, &r.app.cfg).unwrap();
    assert_eq!(config::load(&path).0.color, 2);
}

#[test]
fn save_is_atomic() {
    let dir = temp_dir();
    let path = dir.join("config.json");
    let r = Rig::new();
    config::save(&path, &r.app.cfg).unwrap();
    config::save(&path, &r.app.cfg).unwrap();

    let names: Vec<String> =
        std::fs::read_dir(&dir).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
    assert_eq!(names, vec!["config.json".to_string()]);
    let json: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(json["version"], 3);
    assert_eq!(json["bindings"]["ToggleDraw"], "Ctrl+Alt+D");
}

#[test]
fn save_into_missing_folder_reports_an_error() {
    let path = temp_dir().join("absent").join("config.json");
    let r = Rig::new();
    assert!(config::save(&path, &r.app.cfg).is_err());
}

#[test]
fn hand_edited_file_is_sanitized_before_use() {
    let path = temp_dir().join("config.json");
    std::fs::write(&path, r#"{"hold_seconds":0,"color":9,"width":7,"last_tool":"Rect"}"#).unwrap();
    let (cfg, _) = config::load(&path);
    let mut r = Rig::with_config(cfg);
    r.command(UiCommand::SelectTool(Some(Tool::Rect)));
    r.overlay_stroke(0., 0., 100., 100.);
    let s = &r.app.scene.shapes()[0];
    assert_eq!((s.color, s.width), (9, 8.0));
    r.advance(1100); // durée ramenée à 1 s
    assert!(r.advance(1));
}
