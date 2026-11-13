//! Tests d'intégration de l'exécutable : ils lancent le vrai `ekspointer.exe`, copié dans un
//! dossier temporaire pour ne pas toucher aux réglages du poste.
//!
//! Ils ouvrent de vraies fenêtres : ils sont donc ignorés par défaut et se lancent un à la
//! fois. Chaque test isole ses instances par `EKSPOINTER_INSTANCE`, si bien qu'un Ekspointer
//! déjà lancé sur le poste n'est ni vu ni fermé :
//!
//! ```text
//! cargo test --test process -- --ignored --test-threads=1
//! ```

use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

/// Copie de l'exécutable dans un dossier temporaire neuf.
struct Sandbox {
    dir: PathBuf,
    exe: PathBuf,
    /// Suffixe d'instance : isole les Ekspointer lancés par ce test de celui de l'utilisateur.
    instance: String,
    children: Vec<Child>,
}

impl Sandbox {
    fn new() -> Sandbox {
        static N: AtomicU32 = AtomicU32::new(0);
        let dir = std::env::temp_dir().join(format!(
            "ekspointer-proc-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::SeqCst)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let exe = dir.join("ekspointer.exe");
        std::fs::copy(env!("CARGO_BIN_EXE_ekspointer"), &exe).unwrap();
        let instance = dir.file_name().unwrap().to_string_lossy().into_owned();
        Sandbox { dir, exe, instance, children: Vec::new() }
    }

    /// Commande qui lance l'exécutable du test, isolé de tout autre Ekspointer du poste.
    fn command(&self) -> Command {
        let mut command = Command::new(&self.exe);
        command.env("EKSPOINTER_INSTANCE", &self.instance);
        command
    }

    fn file(&self, name: &str) -> PathBuf {
        self.dir.join(name)
    }

    /// Lance une instance et lui laisse le temps de démarrer.
    fn start(&mut self) -> usize {
        let child = self.command().arg("--capturable").spawn().unwrap();
        self.children.push(child);
        std::thread::sleep(Duration::from_millis(1500));
        self.children.len() - 1
    }

    /// Lance l'exécutable avec des arguments et attend sa fin, dans la limite donnée.
    fn run(&self, args: &[&str], limit: Duration) -> Option<ExitStatus> {
        let mut child = self.command().args(args).spawn().unwrap();
        wait(&mut child, limit).or_else(|| {
            let _ = child.kill();
            None
        })
    }

    fn alive(&mut self, i: usize) -> bool {
        self.children[i].try_wait().unwrap().is_none()
    }

    fn wait(&mut self, i: usize, limit: Duration) -> Option<ExitStatus> {
        wait(&mut self.children[i], limit)
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        for child in &mut self.children {
            let _ = child.kill();
            let _ = child.wait();
        }
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn wait(child: &mut Child, limit: Duration) -> Option<ExitStatus> {
    let start = Instant::now();
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            return Some(status);
        }
        if start.elapsed() > limit {
            return None;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
}

fn json(path: &Path) -> serde_json::Value {
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

const QUICK: Duration = Duration::from_secs(2);
const SHUTDOWN: Duration = Duration::from_secs(3);

#[test]
#[ignore = "lance le vrai exécutable"]
fn starts_and_quits_cleanly() {
    let mut s = Sandbox::new();
    let first = s.start();
    assert!(s.alive(first), "l'application s'est arrêtée toute seule au démarrage");

    let quit = s.run(&["--quit"], QUICK).expect("--quit ne rend pas la main");
    assert_eq!(quit.code(), Some(0));
    let status = s.wait(first, SHUTDOWN).expect("l'instance ne s'est pas fermée");
    assert_eq!(status.code(), Some(0));
    assert!(!s.file("ekspointer.log").exists(), "une erreur a été journalisée : {:?}", std::fs::read_to_string(s.file("ekspointer.log")));
}

#[test]
#[ignore = "lance le vrai exécutable"]
fn second_instance_exits_immediately() {
    let mut s = Sandbox::new();
    let first = s.start();
    let second = s.run(&["--capturable"], QUICK).expect("le second lancement ne s'est pas terminé");
    assert_eq!(second.code(), Some(0));
    assert!(s.alive(first), "le second lancement a fermé la première instance");
    s.run(&["--quit"], QUICK);
    assert!(s.wait(first, SHUTDOWN).is_some());
}

#[test]
#[ignore = "lance le vrai exécutable"]
fn writes_config_next_to_executable_on_quit() {
    let mut s = Sandbox::new();
    assert!(!s.file("config.json").exists());
    let first = s.start();
    s.run(&["--quit"], QUICK);
    assert!(s.wait(first, SHUTDOWN).is_some());

    let config = json(&s.file("config.json"));
    assert_eq!(config["version"], 2);
    assert_eq!(config["hold_seconds"], 1);
    assert_eq!(config["halo_on"], true);
    assert_eq!(config["ripple_on"], true);
    assert_eq!(config["hide_from_capture"], false);
    assert_eq!(config["bindings"]["ToggleDraw"], "Ctrl+Alt+D");
    assert!(!s.file("config.json.tmp").exists());
    // Rien n'est écrit ailleurs que dans le dossier de l'exécutable.
    if let Some(appdata) = std::env::var_os("APPDATA") {
        assert!(!Path::new(&appdata).join("Ekspointer").exists());
    }
}

#[test]
#[ignore = "lance le vrai exécutable"]
fn keeps_existing_settings_across_a_run() {
    let mut s = Sandbox::new();
    std::fs::write(s.file("config.json"), r#"{"hold_seconds":9,"color":3,"bindings":{"ToggleDraw":"2×Tab"}}"#).unwrap();
    let first = s.start();
    s.run(&["--quit"], QUICK);
    assert!(s.wait(first, SHUTDOWN).is_some());

    let config = json(&s.file("config.json"));
    assert_eq!(config["hold_seconds"], 9);
    assert_eq!(config["color"], 3);
    assert_eq!(config["bindings"]["ToggleDraw"], "2×Tab");
}

#[test]
#[ignore = "lance le vrai exécutable"]
fn invalid_config_is_backed_up_at_startup() {
    let mut s = Sandbox::new();
    std::fs::write(s.file("config.json"), "{").unwrap();
    let first = s.start();
    assert!(s.alive(first), "un fichier de réglages illisible a empêché le démarrage");
    assert_eq!(std::fs::read_to_string(s.file("config.invalide.json")).unwrap(), "{");
    s.run(&["--quit"], QUICK);
    assert!(s.wait(first, SHUTDOWN).is_some());
    // Les réglages par défaut ont remplacé le fichier abîmé.
    assert_eq!(json(&s.file("config.json"))["hold_seconds"], 1);
}

#[test]
#[ignore = "lance le vrai exécutable"]
fn quit_without_running_instance_is_a_noop() {
    let s = Sandbox::new();
    let status = s.run(&["--quit"], QUICK).expect("--quit seul ne rend pas la main");
    assert_eq!(status.code(), Some(0));
    assert!(!s.file("config.json").exists());
}

#[test]
#[ignore = "lance le vrai exécutable"]
fn demo_mode_never_writes_settings() {
    let mut s = Sandbox::new();
    let child = s.command().args(["--demo", "--capturable"]).spawn().unwrap();
    s.children.push(child);
    std::thread::sleep(Duration::from_millis(1500));
    s.run(&["--quit"], QUICK);
    assert!(s.wait(0, SHUTDOWN).is_some());
    assert!(!s.file("config.json").exists());
}

/// Envoie un appui ou un relâchement de touche au système, comme le ferait le clavier.
fn send_key(vk: u16, down: bool) {
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP, VIRTUAL_KEY,
    };
    let flags = if down { KEYBD_EVENT_FLAGS(0) } else { KEYEVENTF_KEYUP };
    let input = INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 { ki: KEYBDINPUT { wVk: VIRTUAL_KEY(vk), dwFlags: flags, ..Default::default() } },
    };
    unsafe {
        SendInput(&[input], std::mem::size_of::<INPUT>() as i32);
    }
    std::thread::sleep(Duration::from_millis(30));
}

#[test]
#[ignore = "lance le vrai exécutable et envoie Ctrl+Alt+H au système"]
fn global_shortcut_goes_through_the_real_keyboard_hook() {
    // De bout en bout : écoute bas niveau sur son fil, filtre, application, enregistrement.
    let mut s = Sandbox::new();
    let first = s.start();
    for (vk, down) in [(0xA2, true), (0xA4, true), (0x48, true), (0x48, false), (0xA4, false), (0xA2, false)] {
        send_key(vk, down);
    }
    std::thread::sleep(Duration::from_millis(300));
    // Le halo est actif par défaut : le raccourci le coupe.
    assert_eq!(json(&s.file("config.json"))["halo_on"], false, "Ctrl+Alt+H n'a pas atteint l'application");

    // Un second appui le rallume : l'écoute fonctionne toujours après une première interception.
    for (vk, down) in [(0xA2, true), (0xA4, true), (0x48, true), (0x48, false), (0xA4, false), (0xA2, false)] {
        send_key(vk, down);
    }
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(json(&s.file("config.json"))["halo_on"], true);

    s.run(&["--quit"], QUICK);
    assert_eq!(s.wait(first, SHUTDOWN).expect("l'instance ne s'est pas fermée").code(), Some(0));
}

#[test]
#[ignore = "lance le vrai exécutable"]
fn quit_button_of_the_toolbar_closes_the_application() {
    // `--demo-ui=quit` clique sur la croix de la barre, par le même chemin que la souris.
    let mut s = Sandbox::new();
    let child = s.command().args(["--demo-ui=quit", "--capturable"]).spawn().unwrap();
    s.children.push(child);
    let status = s.wait(0, Duration::from_secs(5)).expect("la croix n'a pas fermé l'application");
    assert_eq!(status.code(), Some(0));
    assert!(!s.file("ekspointer.log").exists(), "une erreur a été journalisée");
}

#[test]
#[ignore = "lance le vrai exécutable"]
fn test_instances_are_isolated_from_each_other() {
    // Deux bacs à sable : chacun a son verrou et sa fenêtre, comme le test et l'Ekspointer
    // de l'utilisateur. Fermer l'un ne ferme pas l'autre.
    let (mut a, mut b) = (Sandbox::new(), Sandbox::new());
    let (first, second) = (a.start(), b.start());
    assert!(a.alive(first) && b.alive(second), "la seconde instance a été prise pour un second lancement");
    a.run(&["--quit"], QUICK);
    assert!(a.wait(first, SHUTDOWN).is_some());
    assert!(b.alive(second), "--quit a fermé une instance qui n'était pas la sienne");
    b.run(&["--quit"], QUICK);
    assert!(b.wait(second, SHUTDOWN).is_some());
}
