//! Réglages : modèle, lecture tolérante, écriture atomique.

use crate::i18n::Lang;
use crate::shapes::Tool;
use crate::shortcuts::Bindings;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Version du format des réglages.
///
/// 2 (Ekspointer 0.1.1) : effacement après 1 s au lieu de 5, halo et onde au clic actifs,
/// barre d'outils visible dans le partage d'écran.
///
/// 3 (Ekspointer 0.3.0) : le maintien pour dessiner passe de Ctrl+Alt à Ctrl seul.
pub const CURRENT_VERSION: u32 = 3;

/// Réglages de l'application, enregistrés dans `config.json` à côté de l'exécutable.
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[serde(default)]
pub struct Config {
    /// Version du format du fichier.
    pub version: u32,
    /// Langue de l'interface.
    pub language: Lang,
    /// Durée avant effacement, en secondes (1 à 15).
    pub hold_seconds: u8,
    /// Indice de l'épaisseur dans `geom::WIDTHS` (0 à 2).
    pub width: u8,
    /// Indice de la couleur dans `geom::PALETTE` (0 à 9).
    pub color: u8,
    pub last_tool: Tool,
    pub halo_on: bool,
    /// Diamètre du halo en pixels logiques (40 à 120).
    pub halo_size: u32,
    /// Le halo prend la couleur de dessin ; sinon il est jaune fluo.
    pub halo_follows_color: bool,
    pub ripple_on: bool,
    pub trail_on: bool,
    pub pinned: bool,
    pub bindings: Bindings,
    /// Position de la barre d'outils dans le bureau virtuel.
    pub toolbar_pos: Option<(i32, i32)>,
    pub toolbar_collapsed: bool,
    /// Exclut la barre d'outils du partage d'écran.
    pub hide_from_capture: bool,
}

impl Default for Config {
    fn default() -> Config {
        Config {
            version: CURRENT_VERSION,
            language: Lang::Fr,
            hold_seconds: 1,
            width: 1,
            color: 0,
            last_tool: Tool::Arrow,
            halo_on: true,
            halo_size: 64,
            halo_follows_color: true,
            ripple_on: true,
            trail_on: false,
            pinned: false,
            bindings: Bindings::defaults(),
            toolbar_pos: None,
            toolbar_collapsed: false,
            hide_from_capture: false,
        }
    }
}

impl Config {
    /// Ramène chaque valeur dans ses bornes.
    pub fn sanitize(&mut self) {
        self.hold_seconds = self.hold_seconds.clamp(1, 15);
        self.width = self.width.min(2);
        self.color = self.color.min(crate::geom::PALETTE.len() as u8 - 1);
        self.halo_size = self.halo_size.clamp(40, 120);
    }
}

/// Ce qui s'est passé à la lecture des réglages.
#[derive(Debug)]
pub enum LoadOutcome {
    /// Fichier lu.
    Loaded,
    /// Pas de fichier : valeurs par défaut.
    Defaults,
    /// Fichier illisible : mis de côté dans `backup`, valeurs par défaut.
    Invalid { backup: PathBuf },
}

/// Emplacement du fichier de réglages : à côté de l'exécutable.
pub fn default_path() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf))
        .unwrap_or_else(|| PathBuf::from("."))
        .join("config.json")
}

/// Construit les réglages à partir d'un objet JSON, champ par champ : une valeur absente,
/// d'un mauvais type ou illisible laisse la valeur par défaut de ce seul champ, et un nombre
/// hors bornes est ramené dans ses bornes.
fn lenient(value: &serde_json::Value) -> Option<Config> {
    let obj = value.as_object()?;
    let mut c = Config::default();
    let number = |key: &str, max: f64| obj.get(key).and_then(|v| v.as_f64()).map(|n| n.clamp(0.0, max));
    let flag = |key: &str, default: bool| obj.get(key).and_then(|v| v.as_bool()).unwrap_or(default);
    if let Some(n) = number("hold_seconds", 255.0) {
        c.hold_seconds = n as u8;
    }
    if let Some(n) = number("width", 255.0) {
        c.width = n as u8;
    }
    if let Some(n) = number("color", 255.0) {
        c.color = n as u8;
    }
    if let Some(n) = number("halo_size", 100_000.0) {
        c.halo_size = n as u32;
    }
    if let Some(language) = obj.get("language").and_then(|v| serde_json::from_value(v.clone()).ok()) {
        c.language = language;
    }
    if let Some(tool) = obj.get("last_tool").and_then(|v| serde_json::from_value(v.clone()).ok()) {
        c.last_tool = tool;
    }
    if let Some(bindings) = obj.get("bindings").and_then(|v| serde_json::from_value(v.clone()).ok()) {
        c.bindings = bindings;
    }
    if let Some(pos) = obj.get("toolbar_pos").and_then(|v| serde_json::from_value(v.clone()).ok()) {
        c.toolbar_pos = pos;
    }
    c.halo_on = flag("halo_on", c.halo_on);
    c.halo_follows_color = flag("halo_follows_color", c.halo_follows_color);
    c.ripple_on = flag("ripple_on", c.ripple_on);
    c.trail_on = flag("trail_on", c.trail_on);
    c.pinned = flag("pinned", c.pinned);
    c.toolbar_collapsed = flag("toolbar_collapsed", c.toolbar_collapsed);
    c.hide_from_capture = flag("hide_from_capture", c.hide_from_capture);
    // Fichier d'une version antérieure : il porte d'anciennes valeurs par défaut, que
    // l'utilisateur n'a pas choisies. Les réglages dont le défaut a changé prennent le nouveau.
    let version = obj.get("version").and_then(|v| v.as_u64()).unwrap_or(CURRENT_VERSION as u64);
    if version < 2 {
        // La palette de la version 1 (ambre, bleu, encre, jaune, vert, blanc) a été réordonnée.
        const FROM_V1: [u8; 6] = [6, 3, 7, 0, 2, 5];
        c.color = obj
            .get("color")
            .and_then(|v| v.as_u64())
            .and_then(|old| FROM_V1.get(old as usize).copied())
            .unwrap_or(0);
        let fresh = Config::default();
        c.hold_seconds = fresh.hold_seconds;
        c.halo_on = fresh.halo_on;
        c.ripple_on = fresh.ripple_on;
        c.hide_from_capture = fresh.hide_from_capture;
    }
    if version < 3 {
        // L'ancien maintien par défaut (Ctrl+Alt) devient Ctrl seul ; un maintien choisi est gardé.
        use crate::shortcuts::Action;
        if c.bindings.get(Action::HoldDraw).to_string() == "Ctrl+Alt" {
            if let Ok(ctrl) = "Ctrl".parse() {
                let _ = c.bindings.set(Action::HoldDraw, ctrl);
            }
        }
    }
    c.sanitize();
    Some(c)
}

/// Met de côté un fichier de réglages inutilisable et renvoie les valeurs par défaut.
fn set_aside(path: &Path) -> (Config, LoadOutcome) {
    let backup = path.with_file_name("config.invalide.json");
    let _ = std::fs::remove_file(&backup);
    let _ = std::fs::rename(path, &backup);
    (Config::default(), LoadOutcome::Invalid { backup })
}

/// Lit les réglages. Ne renvoie jamais d'erreur : au pire, les valeurs par défaut.
///
/// La lecture est tolérante : marque d'ordre d'octets, encodage non UTF-8 et valeurs
/// isolées incorrectes sont acceptés. Seul un fichier qui n'est pas un objet JSON est mis de côté.
pub fn load(path: &Path) -> (Config, LoadOutcome) {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return (Config::default(), LoadOutcome::Defaults),
        // Le fichier existe mais ne se lit pas : ne pas l'écraser en silence.
        Err(_) => return set_aside(path),
    };
    let text = String::from_utf8_lossy(&bytes);
    let text = text.trim_start_matches('\u{feff}');
    match serde_json::from_str::<serde_json::Value>(text).ok().as_ref().and_then(lenient) {
        Some(cfg) => (cfg, LoadOutcome::Loaded),
        None => set_aside(path),
    }
}

/// Écrit les réglages de façon atomique : fichier temporaire puis renommage.
pub fn save(path: &Path, cfg: &Config) -> std::io::Result<()> {
    let tmp = path.with_extension("json.tmp");
    let text = serde_json::to_string_pretty(cfg).map_err(std::io::Error::other)?;
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, path).inspect_err(|_| {
        let _ = std::fs::remove_file(&tmp);
    })
}

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
        assert_eq!((c.version, c.hold_seconds, c.width, c.color), (3, 1, 1, 0));
        assert_eq!(c.last_tool, Tool::Arrow);
        assert_eq!((c.halo_on, c.halo_size, c.halo_follows_color), (true, 64, true));
        assert_eq!((c.ripple_on, c.trail_on, c.pinned), (true, false, false));
        assert_eq!((c.toolbar_pos, c.toolbar_collapsed, c.hide_from_capture), (None, false, false));
        assert_eq!(c.bindings, Bindings::defaults());
    }

    #[test]
    fn missing_file_gives_defaults() {
        let (c, o) = load(&tmp("absent.json"));
        assert!(matches!(o, LoadOutcome::Defaults));
        assert_eq!(c.hold_seconds, 1);
        assert!(!c.hide_from_capture);
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
        assert_eq!(c.hold_seconds, 1);
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
        std::fs::write(&p, r#"{"hold_seconds":200,"width":7,"color":99,"halo_size":5}"#).unwrap();
        let (c, _) = load(&p);
        assert_eq!((c.hold_seconds, c.width, c.color, c.halo_size), (15, 2, 9, 40));
        std::fs::write(&p, r#"{"hold_seconds":0,"halo_size":100000}"#).unwrap();
        let (c, _) = load(&p);
        assert_eq!((c.hold_seconds, c.halo_size), (1, 120));
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

    #[test]
    fn one_bad_value_does_not_discard_the_whole_file() {
        // Valeurs hors du type attendu : seule la valeur fautive est remplacée, le reste est gardé.
        let p = tmp("config.json");
        std::fs::write(
            &p,
            r#"{"hold_seconds":300,"color":-1,"width":"gros","last_tool":"Bidule","halo_size":90,"halo_on":"oui",
                "toolbar_pos":"ici","bindings":{"ToggleDraw":"Ctrl+Alt+X"}}"#,
        )
        .unwrap();
        let (c, o) = load(&p);
        assert!(matches!(o, LoadOutcome::Loaded), "{o:?}");
        assert_eq!(c.hold_seconds, 15); // 300 ramené à la borne
        assert_eq!(c.color, 0); // -1 ramené à la borne
        assert_eq!(c.width, 1); // illisible : valeur par défaut
        assert_eq!(c.last_tool, Tool::Arrow);
        assert_eq!(c.halo_size, 90);
        assert!(c.halo_on); // illisible : valeur par défaut
        assert_eq!(c.toolbar_pos, None);
        assert_eq!(c.bindings.get(Action::ToggleDraw).to_string(), "Ctrl+Alt+X");
        assert!(p.exists() && !p.with_file_name("config.invalide.json").exists());
    }

    #[test]
    fn byte_order_mark_is_accepted() {
        // Le Bloc-notes de Windows peut enregistrer le fichier avec une marque d'ordre d'octets.
        let p = tmp("bom.json");
        std::fs::write(&p, "\u{feff}{\"hold_seconds\":7}").unwrap();
        let (c, o) = load(&p);
        assert!(matches!(o, LoadOutcome::Loaded), "{o:?}");
        assert_eq!(c.hold_seconds, 7);
    }

    #[test]
    fn non_utf8_file_keeps_what_can_be_read() {
        // Fichier enregistré en ANSI : « Échap » est illisible, le reste est conservé.
        let p = tmp("ansi.json");
        let mut bytes = b"{\"hold_seconds\":7,\"bindings\":{\"ExitDraw\":\"".to_vec();
        bytes.extend_from_slice(&[0xC9, b'c', b'h', b'a', b'p']);
        bytes.extend_from_slice(b"\",\"ToolArrow\":\"A\"}}");
        std::fs::write(&p, bytes).unwrap();
        let (c, o) = load(&p);
        assert!(matches!(o, LoadOutcome::Loaded), "{o:?}");
        assert_eq!(c.hold_seconds, 7);
        assert_eq!(c.bindings.get(Action::ToolArrow).to_string(), "A");
        assert_eq!(c.bindings.get(Action::ExitDraw).to_string(), "Échap");
    }

    #[test]
    fn json_that_is_not_an_object_is_invalid() {
        let p = tmp("config.json");
        std::fs::write(&p, "[1, 2, 3]").unwrap();
        assert!(matches!(load(&p).1, LoadOutcome::Invalid { .. }));
    }

    #[test]
    fn one_second_is_a_valid_duration() {
        let p = tmp("one.json");
        std::fs::write(&p, r#"{"version":2,"hold_seconds":1}"#).unwrap();
        assert_eq!(load(&p).0.hold_seconds, 1);
    }

    #[test]
    fn settings_from_version_1_take_the_new_defaults() {
        // Un fichier écrit par la version 0.1.0 porte les anciennes valeurs par défaut, que
        // l'utilisateur n'a pas choisies : les quatre réglages concernés passent aux nouvelles.
        let p = tmp("v1.json");
        std::fs::write(
            &p,
            r#"{"version":1,"hold_seconds":5,"halo_on":false,"ripple_on":false,"hide_from_capture":true,
                "color":3,"trail_on":true,"bindings":{"ToggleDraw":"2×Tab"}}"#,
        )
        .unwrap();
        let (c, o) = load(&p);
        assert!(matches!(o, LoadOutcome::Loaded));
        assert_eq!((c.version, c.hold_seconds, c.halo_on, c.ripple_on, c.hide_from_capture), (3, 1, true, true, false));
        // Le reste est conservé.
        assert_eq!((c.color, c.trail_on), (0, true)); // l'ancien jaune (3) est le nouveau jaune (0)
        assert_eq!(c.bindings.get(Action::ToggleDraw).to_string(), "2×Tab");
    }

    #[test]
    fn settings_from_the_current_version_are_kept() {
        let p = tmp("v2.json");
        std::fs::write(&p, r#"{"version":2,"hold_seconds":5,"halo_on":false,"ripple_on":false,"hide_from_capture":true}"#).unwrap();
        let (c, _) = load(&p);
        assert_eq!((c.hold_seconds, c.halo_on, c.ripple_on, c.hide_from_capture), (5, false, false, true));
    }

    #[test]
    fn language_is_remembered() {
        use crate::i18n::Lang;
        assert_eq!(Config::default().language, Lang::Fr);
        let p = tmp("lang.json");
        let mut c = Config::default();
        c.language = Lang::En;
        save(&p, &c).unwrap();
        assert_eq!(load(&p).0.language, Lang::En);
        std::fs::write(&p, r#"{"version":2,"language":"klingon"}"#).unwrap();
        assert_eq!(load(&p).0.language, Lang::Fr); // illisible : valeur par défaut
    }

    #[test]
    fn default_color_is_yellow() {
        use crate::geom::{Rgba, PALETTE};
        assert_eq!(PALETTE[Config::default().color as usize], Rgba::hex(0xFFC400));
    }

    #[test]
    fn colors_of_a_version_1_file_keep_their_hue() {
        // Ancien ordre : ambre, bleu, encre, jaune, vert, blanc.
        for (old, new) in [(0, 6), (1, 3), (2, 7), (3, 0), (4, 2), (5, 5)] {
            let p = tmp("v1color.json");
            std::fs::write(&p, format!(r#"{{"version":1,"color":{old}}}"#)).unwrap();
            assert_eq!(load(&p).0.color, new, "ancienne couleur {old}");
        }
    }

    #[test]
    fn old_default_hold_becomes_the_single_key() {
        // Avant la version 3 des réglages, le maintien par défaut était Ctrl+Alt.
        let hold = |json: &str| {
            let p = tmp("hold.json");
            std::fs::write(&p, json).unwrap();
            load(&p).0.bindings.get(Action::HoldDraw).to_string()
        };
        assert_eq!(hold(r#"{"version":2,"bindings":{"HoldDraw":"Ctrl+Alt"}}"#), "Ctrl");
        assert_eq!(hold(r#"{"version":1}"#), "Ctrl");
        // Un maintien choisi par l'utilisateur est conservé.
        assert_eq!(hold(r#"{"version":2,"bindings":{"HoldDraw":"Ctrl+Maj"}}"#), "Ctrl+Maj");
        // À partir de la version 3, Ctrl+Alt est un choix : il reste.
        assert_eq!(hold(r#"{"version":3,"bindings":{"HoldDraw":"Ctrl+Alt"}}"#), "Ctrl+Alt");
    }
}
