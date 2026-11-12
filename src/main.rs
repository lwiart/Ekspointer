#![windows_subsystem = "windows"]
//! Ekspointer : point d'entrée, instance unique et boucle de messages.

mod win;

use ekspointer::app::{App, Effect, UiCommand};
use ekspointer::config::{self, Config, LoadOutcome};
use ekspointer::geom::Pt;
use ekspointer::input_filter::{Filter, InputEvent};
use ekspointer::shapes::Tool;
use std::path::PathBuf;
use std::time::Instant;
use win::gfx::Gfx;
use win::overlay::Overlays;
use win::{input, log, Msg};
use windows::core::{w, Result, PCWSTR};
use windows::Win32::Foundation::{GetLastError, ERROR_ALREADY_EXISTS, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Dwm::DwmFlush;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::CreateMutexW;
use windows::Win32::UI::WindowsAndMessaging::*;

const MAIN_CLASS: PCWSTR = w!("EkspointerMain");
const MUTEX_NAME: PCWSTR = w!("Ekspointer.Instance");

/// Minuteur armé sur l'instant où la prochaine forme commencera son fondu.
const TIMER_DEADLINE: usize = 1;
/// Minuteur de recalage du clavier pendant que la combinaison de maintien est tenue.
const TIMER_RESYNC: usize = 2;

/// Options de la ligne de commande.
#[derive(Default)]
struct Args {
    /// Affiche une scène de démonstration épinglée.
    demo: bool,
    /// Laisse la barre d'outils visible dans les captures d'écran, pour cette session.
    capturable: bool,
    /// Demande à l'instance en cours de se fermer.
    quit: bool,
}

fn parse_args() -> Args {
    let mut args = Args::default();
    for a in std::env::args().skip(1) {
        match a.as_str() {
            "--demo" => args.demo = true,
            "--capturable" => args.capturable = true,
            "--quit" => args.quit = true,
            _ => {}
        }
    }
    args
}

unsafe extern "system" fn main_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        match msg {
            WM_TIMER => {
                let _ = KillTimer(Some(hwnd), wparam.0);
                win::post(Msg::Timer(wparam.0));
                LRESULT(0)
            }
            WM_DISPLAYCHANGE | WM_DPICHANGED => {
                win::post(Msg::DisplayChanged);
                LRESULT(0)
            }
            win::WM_APP_SHOW => {
                win::post(Msg::ShowToolbar);
                LRESULT(0)
            }
            WM_CLOSE => {
                let _ = DestroyWindow(hwnd);
                LRESULT(0)
            }
            WM_DESTROY => {
                PostQuitMessage(0);
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}

fn create_main_window() -> Result<HWND> {
    unsafe {
        let instance = GetModuleHandleW(None)?;
        let class = WNDCLASSW {
            lpfnWndProc: Some(main_proc),
            hInstance: instance.into(),
            lpszClassName: MAIN_CLASS,
            ..Default::default()
        };
        RegisterClassW(&class);
        CreateWindowExW(
            WINDOW_EX_STYLE(0),
            MAIN_CLASS,
            w!("Ekspointer"),
            WS_POPUP,
            0,
            0,
            0,
            0,
            None,
            None,
            Some(instance.into()),
            None,
        )
    }
}

/// Tout ce qui vit pendant l'exécution.
struct Shell {
    gfx: Gfx,
    overlays: Overlays,
    app: App,
    main: HWND,
    start: Instant,
    cfg_path: PathBuf,
    /// Faux en démonstration : les réglages ne sont pas écrits.
    save_enabled: bool,
    /// Un échec d'écriture a déjà été signalé.
    save_failed: bool,
    /// Windows autorise les animations (réglage « Afficher les animations »).
    animate: bool,
}

fn animations_enabled() -> bool {
    let mut enabled = windows::core::BOOL(1);
    unsafe {
        let _ = SystemParametersInfoW(
            SPI_GETCLIENTAREAANIMATION,
            0,
            Some(&mut enabled as *mut _ as *mut core::ffi::c_void),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        );
    }
    enabled.as_bool()
}

impl Shell {
    fn now(&self) -> u64 {
        self.start.elapsed().as_millis() as u64
    }

    fn apply(&mut self, effects: Vec<Effect>) {
        for e in effects {
            match e {
                Effect::OverlayCapture(on) => self.overlays.set_capture(on),
                Effect::FilterChanged => {
                    let draw = self.app.draw_mode;
                    let bindings = self.app.cfg.bindings.clone();
                    input::with_filter(|f| {
                        f.set_draw_mode(draw);
                        f.set_bindings(bindings);
                    });
                }
                Effect::ConfigChanged => self.save(),
                Effect::UiChanged | Effect::CaptureAffinityChanged | Effect::BindingConflict(_) => {}
            }
        }
    }

    fn save(&mut self) {
        if !self.save_enabled {
            return;
        }
        if let Err(e) = config::save(&self.cfg_path, &self.app.cfg) {
            if !self.save_failed {
                self.save_failed = true;
                log::error(&format!("réglages non enregistrés : {e}"));
            }
        }
    }

    fn handle(&mut self, msg: Msg, now: u64) {
        match msg {
            Msg::Input(ev) => {
                let overlays = &self.overlays;
                let effects = self.app.input(ev, now, &|p| overlays.scale_at(p));
                self.apply(effects);
                if matches!(ev, InputEvent::HoldStart) {
                    unsafe {
                        SetTimer(Some(self.main), TIMER_RESYNC, 250, None);
                    }
                }
            }
            Msg::DisplayChanged => {
                self.app.scene.cancel();
                self.app.scene.clear_all(now);
                if let Err(e) = self.overlays.rebuild(&self.gfx) {
                    log::error(&format!("recréation des fenêtres de dessin : {e}"));
                }
                self.overlays.set_capture(self.app.draw_mode);
            }
            Msg::Timer(TIMER_RESYNC) => {
                input::resync();
                if self.app.hold {
                    unsafe {
                        SetTimer(Some(self.main), TIMER_RESYNC, 250, None);
                    }
                }
            }
            Msg::Timer(_) | Msg::ShowToolbar => {}
        }
    }

    fn render(&mut self, now: u64) {
        if let Err(e) = self.overlays.render(&self.gfx, &self.app, now, self.animate) {
            // Périphérique graphique perdu : tout recréer, en conservant les formes.
            log::error(&format!("rendu : {e} ; recréation du périphérique graphique"));
            match Gfx::new() {
                Ok(gfx) => {
                    self.gfx = gfx;
                    if let Err(e) = self.overlays.rebuild(&self.gfx) {
                        log::error(&format!("recréation des fenêtres de dessin : {e}"));
                    }
                    self.overlays.set_capture(self.app.draw_mode);
                }
                Err(e) => log::error(&format!("périphérique graphique indisponible : {e}")),
            }
        }
    }

    /// Arme le minuteur qui réveillera la boucle quand une forme devra commencer son fondu.
    fn arm_deadline(&self, now: u64) {
        if let Some(deadline) = self.app.scene.next_deadline(now) {
            let delay = deadline.saturating_sub(now).clamp(1, u32::MAX as u64) as u32;
            unsafe {
                SetTimer(Some(self.main), TIMER_DEADLINE, delay, None);
            }
        }
    }
}

/// Trace une forme de chaque outil dans chaque couleur, épinglée, pour les vérifications visuelles.
fn demo_scene(shell: &mut Shell) {
    let (rect, scale) = shell.overlays.primary();
    let app = &mut shell.app;
    if !app.cfg.pinned {
        app.command(UiCommand::TogglePin, 0);
    }
    let tools = [Tool::Arrow, Tool::Rect, Tool::Ellipse, Tool::Free, Tool::Highlighter, Tool::Number];
    let (cw, ch) = (170.0 * scale, 110.0 * scale);
    let (x0, y0) = (rect.x + 120.0 * scale, rect.y + 150.0 * scale);
    for color in 0..6u8 {
        app.command(UiCommand::SetColor(color), 0);
        app.command(UiCommand::SetWidth(color % 3), 0);
        for (col, tool) in tools.iter().enumerate() {
            app.command(UiCommand::SelectTool(Some(*tool)), 0);
            let a = Pt::new(x0 + col as f32 * cw, y0 + color as f32 * ch);
            let b = Pt::new(a.x + cw * 0.7, a.y + ch * 0.55);
            let scale_at = |_: Pt| scale;
            app.input(InputEvent::StrokeBegin(a), 0, &scale_at);
            if matches!(tool, Tool::Free | Tool::Highlighter) {
                for i in 1..=12 {
                    let t = i as f32 / 12.0;
                    let wave = (t * 9.0).sin() * ch * 0.18;
                    app.input(InputEvent::StrokeMove(Pt::new(a.x + (b.x - a.x) * t, a.y + ch * 0.3 + wave)), 0, &scale_at);
                }
            } else if *tool != Tool::Number {
                app.input(InputEvent::StrokeMove(b), 0, &scale_at);
            }
            app.input(InputEvent::StrokeEnd, 0, &scale_at);
        }
    }
    app.command(UiCommand::SelectTool(None), 0);
}

fn fatal(text: &str) -> ! {
    log::error(text);
    let wide: Vec<u16> = text.encode_utf16().chain(Some(0)).collect();
    unsafe {
        MessageBoxW(None, PCWSTR(wide.as_ptr()), w!("Ekspointer"), MB_OK | MB_ICONERROR);
    }
    std::process::exit(1);
}

fn run(args: Args) -> Result<()> {
    let main = create_main_window()?;
    win::set_notify(main);

    let cfg_path = config::default_path();
    let (mut cfg, outcome) = if args.demo { (Config::default(), LoadOutcome::Defaults) } else { config::load(&cfg_path) };
    if let LoadOutcome::Invalid { backup } = &outcome {
        log::error(&format!("réglages illisibles, mis de côté dans {}", backup.display()));
    }
    if args.capturable {
        cfg.hide_from_capture = false;
    }

    let gfx = Gfx::new()?;
    let overlays = Overlays::new(&gfx)?;
    let app = App::new(cfg);
    let filter = Filter::new(app.cfg.bindings.clone());
    let _hooks = match input::install(filter) {
        Ok(h) => h,
        Err(_) => fatal("Ekspointer n'a pas pu démarrer l'écoute du clavier et de la souris."),
    };

    let mut shell = Shell {
        gfx,
        overlays,
        app,
        main,
        start: Instant::now(),
        cfg_path,
        save_enabled: !args.demo && !args.capturable,
        save_failed: false,
        animate: animations_enabled(),
    };
    if args.demo {
        demo_scene(&mut shell);
    }

    let mut dirty = true;
    let mut animating = false;
    loop {
        unsafe {
            let mut msg = MSG::default();
            if !dirty && !animating {
                if !GetMessageW(&mut msg, None, 0, 0).as_bool() {
                    break;
                }
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
            let mut quit = false;
            while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                if msg.message == WM_QUIT {
                    quit = true;
                    break;
                }
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
            if quit {
                break;
            }
        }

        let now = shell.now();
        for m in win::drain() {
            shell.handle(m, now);
        }
        animating = shell.app.tick(now);
        shell.render(now);
        dirty = false;
        if animating {
            unsafe {
                let _ = DwmFlush();
            }
        } else {
            shell.arm_deadline(now);
        }
    }
    shell.save();
    Ok(())
}

fn main() {
    std::panic::set_hook(Box::new(|info| {
        log::error(&format!("erreur inattendue : {info}"));
        std::process::exit(101);
    }));

    let args = parse_args();
    unsafe {
        // Instance unique : le mutex vit jusqu'à la fin du processus.
        let mutex = CreateMutexW(None, true, MUTEX_NAME);
        let already_running = GetLastError() == ERROR_ALREADY_EXISTS;
        if already_running || args.quit {
            if let Ok(existing) = FindWindowW(MAIN_CLASS, None) {
                let message = if args.quit { WM_CLOSE } else { win::WM_APP_SHOW };
                let _ = PostMessageW(Some(existing), message, WPARAM(0), LPARAM(0));
            }
            return;
        }
        std::mem::forget(mutex);
    }

    if let Err(e) = run(args) {
        fatal(&format!("Ekspointer n'a pas pu démarrer : {e}"));
    }
}
