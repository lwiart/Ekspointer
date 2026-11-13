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
use ekspointer::i18n::{t, Lang, Text};
use ekspointer::ui::settings::{conflict_message_in, Tab};
use ekspointer::ui::toolbar::{self, Item};
use win::panel::{PanelEvent, PanelId};
use win::settings_view::{SettingsAction, SettingsView, CONFLICT_MS};
use win::tray::{Tray, TrayCommand};
use win::ui::{Ui, UiAction, TIP_DELAY_MS};
use win::{autostart, input, log, Msg};
use windows::core::{w, Result, PCWSTR};
use windows::Win32::Foundation::{GetLastError, ERROR_ALREADY_EXISTS, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Dwm::DwmFlush;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::ProcessStatus::K32EmptyWorkingSet;
use windows::Win32::System::Threading::{CreateMutexW, GetCurrentProcess};
use windows::Win32::UI::WindowsAndMessaging::*;

/// Nom, pour Windows, d'un objet propre à cette instance. La variable d'environnement
/// `EKSPOINTER_INSTANCE` y ajoute un suffixe : les tests s'en servent pour lancer des instances
/// qui ne voient ni ne ferment l'Ekspointer de l'utilisateur.
fn instance_name(base: &str) -> Vec<u16> {
    let name = match std::env::var("EKSPOINTER_INSTANCE") {
        Ok(suffix) if !suffix.is_empty() => format!("{base}.{suffix}"),
        _ => base.to_string(),
    };
    name.encode_utf16().chain(Some(0)).collect()
}

/// Classe de la fenêtre principale (cachée), par laquelle une autre instance nous joint.
fn main_class() -> PCWSTR {
    static NAME: std::sync::OnceLock<Vec<u16>> = std::sync::OnceLock::new();
    PCWSTR(NAME.get_or_init(|| instance_name("EkspointerMain")).as_ptr())
}

/// Verrou d'instance unique.
fn mutex_name() -> PCWSTR {
    static NAME: std::sync::OnceLock<Vec<u16>> = std::sync::OnceLock::new();
    PCWSTR(NAME.get_or_init(|| instance_name("Ekspointer.Instance")).as_ptr())
}

/// Minuteur armé sur l'instant où la prochaine forme commencera son fondu.
const TIMER_DEADLINE: usize = 1;
/// Minuteur de recalage du clavier pendant que la combinaison de maintien est tenue.
const TIMER_RESYNC: usize = 2;
/// Minuteur de la bulle d'aide de la barre d'outils.
const TIMER_TOOLTIP: usize = 3;

/// Minuteur unique, peu après le démarrage : rend au système la mémoire qui n'a servi qu'à l'initialisation.
const TIMER_TRIM: usize = 5;
/// Minuteur de fin de défilement à la molette en mode dessin.
const TIMER_WHEEL: usize = 6;
/// Délai sans cran de molette au bout duquel les fenêtres de dessin captent de nouveau la souris.
const WHEEL_PASS_MS: u32 = 200;
/// Délai de ce minuteur, en millisecondes.
const TRIM_DELAY_MS: u32 = 3000;
/// Minuteur d'effacement du message de conflit de raccourci.
const TIMER_CONFLICT: usize = 4;

/// Message diffusé par Windows quand l'Explorateur redémarre (0 tant qu'il n'est pas enregistré).
static TASKBAR_CREATED: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

/// Options de la ligne de commande.
#[derive(Default)]
struct Args {
    /// Affiche une scène de démonstration épinglée.
    demo: bool,
    /// Laisse la barre d'outils visible dans les captures d'écran, pour cette session.
    capturable: bool,
    /// Demande à l'instance en cours de se fermer.
    quit: bool,
    /// Met l'interface dans un état donné, pour les vérifications visuelles :
    /// `popover`, `tooltip`, `settings`, `help`, `conflict`, `collapsed`, `drawing` ou `quit`.
    demo_ui: Option<String>,
    /// Force la langue de l'interface (`fr` ou `en`), pour les vérifications visuelles.
    lang: Option<Lang>,
}

fn parse_args() -> Args {
    let mut args = Args::default();
    for a in std::env::args().skip(1) {
        match a.as_str() {
            "--demo" => args.demo = true,
            "--capturable" => args.capturable = true,
            "--quit" => args.quit = true,
            other => {
                if let Some(name) = other.strip_prefix("--demo-ui=") {
                    args.demo_ui = Some(name.to_string());
                }
                match other {
                    "--lang=en" => args.lang = Some(Lang::En),
                    "--lang=fr" => args.lang = Some(Lang::Fr),
                    _ => {}
                }
            }
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
            WM_SETTINGCHANGE => {
                win::post(Msg::SystemSettingsChanged);
                LRESULT(0)
            }
            win::WM_APP_SHOW => {
                win::post(Msg::ShowToolbar);
                LRESULT(0)
            }
            win::WM_APP_TRAY => {
                match (lparam.0 & 0xFFFF) as u32 {
                    WM_LBUTTONUP => win::post(Msg::Tray(TrayCommand::ShowToolbar)),
                    WM_RBUTTONUP | WM_CONTEXTMENU => win::post(Msg::Tray(TrayCommand::OpenMenu)),
                    _ => {}
                }
                LRESULT(0)
            }
            m if m != 0 && m == TASKBAR_CREATED.load(std::sync::atomic::Ordering::Relaxed) => {
                win::post(Msg::Tray(TrayCommand::Recreate));
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
            lpszClassName: main_class(),
            ..Default::default()
        };
        RegisterClassW(&class);
        CreateWindowExW(
            WINDOW_EX_STYLE(0),
            main_class(),
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
    ui: Ui,
    settings: SettingsView,
    tray: Tray,
    app: App,
    main: HWND,
    /// Option de test : l'interface reste visible dans les captures d'écran.
    capturable: bool,
    /// Instant de la dernière recréation des ressources graphiques (0 : jamais).
    last_recovery: u64,
    /// Les formes ont pu changer depuis la dernière image.
    shapes_dirty: bool,
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
                Effect::OverlayCapture(on) => {
                    self.overlays.set_capture(on);
                    self.ui.raise();
                    self.settings.panel.raise();
                }
                Effect::FilterChanged => {
                    let draw = self.app.draw_mode;
                    let bindings = self.app.cfg.bindings.clone();
                    input::with_filter(|f| {
                        f.set_draw_mode(draw);
                        f.set_bindings(bindings);
                    });
                }
                Effect::ConfigChanged => self.save(),
                Effect::UiChanged => {
                    self.ui.dirty = true;
                    self.settings.dirty = true;
                }
                Effect::CaptureAffinityChanged => self.apply_capture_affinity(),
                Effect::BindingConflict(_) | Effect::BindingRejected(_) => {}
                Effect::OpenSettings => self.toggle_settings(),
                Effect::OpenHelp => self.open_settings(Tab::Help),
            }
        }
    }

    fn apply_capture_affinity(&self) {
        let hidden = self.app.cfg.hide_from_capture && !self.capturable;
        self.ui.set_capture_hidden(hidden);
        self.settings.panel.set_capture_hidden(hidden);
    }

    fn close_settings(&mut self) {
        self.settings.close();
        input::with_filter(|f| f.set_capturing(false));
    }

    fn toggle_settings(&mut self) {
        if self.settings.visible() {
            self.close_settings();
        } else {
            self.open_settings(Tab::Settings);
        }
    }

    /// Ouvre le panneau sur un onglet (réglages ou aide), au centre de l'écran de la barre.
    fn open_settings(&mut self, tab: Tab) {
        let bar = self.ui.toolbar.rect();
        let center = Pt::new(bar.x + bar.w / 2.0, bar.y + bar.h / 2.0);
        let (monitor, scale) = self.overlays.monitor_at(center).unwrap_or(self.overlays.primary());
        let appear = self.animate.then(|| self.now());
        if let Err(e) = self.settings.open(&self.gfx, monitor, scale, autostart::is_enabled(), tab, appear) {
            log::error(&format!("ouverture des réglages : {e}"));
        }
    }

    fn settings_actions(&mut self, actions: Vec<SettingsAction>, now: u64) {
        for action in actions {
            match action {
                SettingsAction::Command(cmd) => self.command(cmd, now),
                SettingsAction::Capture { action, double } => {
                    self.settings.capturing = Some((action, double));
                    input::with_filter(|f| if double { f.set_capturing_double() } else { f.set_capturing(true) });
                }
                SettingsAction::CancelCapture => {
                    self.settings.capturing = None;
                    input::with_filter(|f| f.set_capturing(false));
                }
                SettingsAction::Autostart(on) => {
                    if let Err(e) = autostart::set_enabled(on) {
                        log::error(&format!("lancement au démarrage : {e}"));
                    }
                    self.settings.set_autostart(autostart::is_enabled());
                }
                SettingsAction::Close => self.close_settings(),
                SettingsAction::OpenUrl(url) => open_url(url),
            }
        }
        self.settings.dirty = true;
    }

    /// Un raccourci vient d'être saisi dans les réglages (ou la saisie a été annulée).
    fn capture_result(&mut self, ev: InputEvent, now: u64) {
        let Some((action, _)) = self.settings.capturing.take() else {
            return;
        };
        self.settings.dirty = true;
        let InputEvent::Captured(shortcut) = ev else {
            return;
        };
        let effects = self.app.command(UiCommand::SetBinding(action, shortcut), now);
        for e in &effects {
            let lang = self.app.cfg.language;
            let message = match e {
                Effect::BindingConflict(other) => conflict_message_in(lang, shortcut, *other),
                Effect::BindingRejected(reason) => reason.message(lang).to_string(),
                _ => continue,
            };
            self.settings.show_conflict(action, message, now + CONFLICT_MS);
            unsafe {
                SetTimer(Some(self.main), TIMER_CONFLICT, CONFLICT_MS as u32 + 50, None);
            }
        }
        self.apply(effects);
    }

    fn save(&mut self) {
        if !self.save_enabled {
            return;
        }
        if let Err(e) = config::save(&self.cfg_path, &self.app.cfg) {
            if !self.save_failed {
                self.save_failed = true;
                log::error(&format!("réglages non enregistrés : {e}"));
                self.tray.notify("Ekspointer", t(self.app.cfg.language, Text::SaveFailed));
            }
        }
    }

    /// Exécute une commande de l'interface.
    fn command(&mut self, cmd: UiCommand, now: u64) {
        let effects = self.app.command(cmd, now);
        self.apply(effects);
        if matches!(cmd, UiCommand::SetToolbarCollapsed(_)) {
            self.relayout_ui();
        }
    }

    fn relayout_ui(&mut self) {
        if let Err(e) = self.ui.place_toolbar(&self.gfx, &self.app, &self.overlays) {
            log::error(&format!("placement de la barre d'outils : {e}"));
        }
    }

    fn show_toolbar(&mut self, visible: bool) {
        if let Err(e) = self.ui.set_toolbar_visible(&self.gfx, &self.app, &self.overlays, visible) {
            log::error(&format!("affichage de la barre d'outils : {e}"));
        }
    }

    fn ui_actions(&mut self, actions: Vec<UiAction>, now: u64) {
        for action in actions {
            match action {
                UiAction::Command(cmd) => self.command(cmd, now),
                UiAction::ToggleSettings => self.toggle_settings(),
                UiAction::OpenHelp => self.open_settings(Tab::Help),
                UiAction::Quit => unsafe {
                    let _ = DestroyWindow(self.main);
                },
                UiAction::ArmTooltip => unsafe {
                    SetTimer(Some(self.main), TIMER_TOOLTIP, TIP_DELAY_MS as u32, None);
                },
            }
        }
    }

    /// Indique au filtre d'entrées où se trouvent les fenêtres de l'interface.
    fn sync_exempt_rects(&self) {
        let mut rects = self.ui.exempt_rects();
        if self.settings.visible() {
            rects.push(self.settings.panel.rect());
        }
        input::with_filter(|f| f.set_exempt_rects(rects));
    }

    fn handle(&mut self, msg: Msg, now: u64) {
        // Un simple mouvement ou clic de souris ne change pas les formes ; tout le reste le peut.
        if !matches!(msg, Msg::Input(InputEvent::PointerMove(_) | InputEvent::Click(..)) | Msg::WheelPass) {
            self.shapes_dirty = true;
        }
        match msg {
            Msg::SystemSettingsChanged => {
                self.animate = animations_enabled();
                self.ui.set_animate(self.animate);
            }
            Msg::Input(ev @ (InputEvent::Captured(_) | InputEvent::CaptureCancelled)) => self.capture_result(ev, now),
            Msg::Input(ev) => {
                let overlays = &self.overlays;
                let effects = self.app.input(ev, now, &|p| overlays.scale_at(p));
                self.apply(effects);
                if matches!(ev, InputEvent::Click(..) | InputEvent::StrokeBegin(_)) {
                    self.ui.on_click_outside();
                    // Un clic hors des réglages abandonne la saisie d'un raccourci : sinon la
                    // prochaine touche tapée ailleurs serait avalée et deviendrait le raccourci.
                    if self.settings.capturing.take().is_some() {
                        input::with_filter(|f| f.set_capturing(false));
                        self.settings.dirty = true;
                    }
                }
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
                self.close_settings();
                self.relayout_ui();
                self.ui.raise();
            }
            Msg::Panel(PanelId::Settings, ev) => {
                let actions = self.settings.on_event(&self.gfx, ev, &self.app);
                self.settings_actions(actions, now);
            }
            Msg::Timer(TIMER_CONFLICT) => self.settings.expire(now),
            Msg::WheelPass => unsafe {
                // Chaque cran repousse l'échéance : la capture revient quand le défilement s'arrête.
                SetTimer(Some(self.main), TIMER_WHEEL, WHEEL_PASS_MS, None);
            },
            Msg::Timer(TIMER_WHEEL) => {
                self.overlays.end_wheel_pass();
                self.ui.raise();
                self.settings.panel.raise();
            }
            Msg::Timer(TIMER_TRIM) => unsafe {
                // Les bibliothèques graphiques touchent beaucoup de mémoire à l'initialisation, qui ne
                // resservira pas : Windows la reprend, et ne recharge ensuite que ce qui est réellement utilisé.
                let _ = K32EmptyWorkingSet(GetCurrentProcess());
            },
            Msg::Panel(id, ev) => match self.ui.on_panel(&self.gfx, id, ev, &self.app, &self.overlays, now) {
                Ok(actions) => self.ui_actions(actions, now),
                Err(e) => log::error(&format!("interface : {e}")),
            },
            Msg::Timer(TIMER_TOOLTIP) => {
                if let Err(e) = self.ui.show_tooltip_if_due(&self.gfx, &self.app, &self.overlays, now) {
                    log::error(&format!("bulle d'aide : {e}"));
                }
                self.ui.dirty = true;
            }
            Msg::Tray(cmd) => match cmd {
                TrayCommand::ShowToolbar => self.show_toolbar(true),
                TrayCommand::OpenMenu => {
                    if let Some(chosen) = self.tray.menu(self.ui.toolbar_visible(), self.app.cfg.language) {
                        win::post(Msg::Tray(chosen));
                    }
                }
                TrayCommand::Recreate => self.tray.add(),
                TrayCommand::ToggleToolbar => self.show_toolbar(!self.ui.toolbar_visible()),
                TrayCommand::Settings => self.open_settings(Tab::Settings),
                TrayCommand::Help => self.open_settings(Tab::Help),
                TrayCommand::Quit => unsafe {
                    let _ = DestroyWindow(self.main);
                },
            },
            Msg::ShowToolbar => self.show_toolbar(true),
            Msg::Timer(TIMER_RESYNC) => {
                input::resync();
                if self.app.hold {
                    unsafe {
                        SetTimer(Some(self.main), TIMER_RESYNC, 250, None);
                    }
                }
            }
            Msg::Timer(_) => {}
        }
        self.sync_exempt_rects();
    }

    fn render(&mut self, now: u64) {
        let mut failure = None;
        let shapes_dirty = std::mem::take(&mut self.shapes_dirty);
        if let Err(e) = self.overlays.render(&self.gfx, &self.app, now, self.animate, shapes_dirty) {
            failure = Some(format!("rendu des formes : {e}"));
        }
        if let Err(e) = self.ui.draw(&self.gfx, &self.app, now) {
            failure = Some(format!("dessin de l'interface : {e}"));
        }
        if let Err(e) = self.settings.draw(&self.gfx, &self.app, now) {
            failure = Some(format!("dessin des réglages : {e}"));
        }
        if let Some(reason) = failure {
            self.recover(&reason, now);
        }
    }

    /// Périphérique graphique perdu (pilote mis à jour, changement de carte, veille) : recrée
    /// toutes les ressources de rendu, des fenêtres de dessin comme de l'interface. Les formes,
    /// la position de la barre et l'état des réglages sont conservés.
    fn recover(&mut self, reason: &str, now: u64) {
        // Au plus une tentative par seconde, pour ne pas tourner en boucle si le périphérique reste indisponible.
        if self.last_recovery != 0 && now.saturating_sub(self.last_recovery) < 1000 {
            return;
        }
        self.last_recovery = now.max(1);
        log::error(&format!("{reason} ; recréation des ressources graphiques"));
        let gfx = match Gfx::new() {
            Ok(gfx) => gfx,
            Err(e) => {
                log::error(&format!("périphérique graphique indisponible : {e}"));
                return;
            }
        };
        self.gfx = gfx;
        if let Err(e) = self.overlays.rebuild(&self.gfx) {
            log::error(&format!("recréation des fenêtres de dessin : {e}"));
        }
        self.overlays.set_capture(self.app.draw_mode);

        let toolbar_visible = self.ui.toolbar_visible();
        match Ui::new(&self.gfx, &self.app, &self.overlays, self.animate) {
            Ok(ui) => self.ui = ui,
            Err(e) => log::error(&format!("recréation de la barre d'outils : {e}")),
        }
        if !toolbar_visible {
            self.show_toolbar(false);
        }
        let settings_open = self.settings.visible();
        match SettingsView::new(&self.gfx) {
            Ok(settings) => {
                self.settings = settings;
                input::with_filter(|f| f.set_capturing(false));
                if settings_open {
                    self.open_settings(Tab::Settings);
                }
            }
            Err(e) => log::error(&format!("recréation des réglages : {e}")),
        }
        self.apply_capture_affinity();
        self.sync_exempt_rects();
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
    let (cw, ch) = (170.0 * scale, 72.0 * scale);
    let (x0, y0) = (rect.x + 120.0 * scale, rect.y + 110.0 * scale);
    for color in 0..ekspointer::geom::PALETTE.len() as u8 {
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

/// Met l'interface dans un état donné en lui envoyant les mêmes événements que la souris.
fn demo_ui(shell: &mut Shell, name: &str) {
    let scale = shell.ui.toolbar.scale;
    let at = |item: Item| {
        let r = toolbar::layout(false).rect(item).unwrap_or_default();
        Pt::new((r.x + r.w / 2.0) * scale, (r.y + r.h / 2.0) * scale)
    };
    let click = |item: Item| {
        win::post(Msg::Panel(PanelId::Toolbar, PanelEvent::Down(at(item))));
        win::post(Msg::Panel(PanelId::Toolbar, PanelEvent::Up(at(item))));
    };
    match name {
        "popover" => click(Item::Color),
        "width" => click(Item::Width),
        "tooltip" => win::post(Msg::Panel(PanelId::Toolbar, PanelEvent::Move(at(Item::Clear)))),
        "drawing" => {
            click(Item::Tool(Some(Tool::Ellipse)));
            click(Item::Halo);
            click(Item::Pin);
        }
        "collapsed" => shell.command(UiCommand::SetToolbarCollapsed(true), 0),
        "quit" => click(Item::Quit),
        "keytool" => {
            use ekspointer::shortcuts::Action;
            win::post(Msg::Input(InputEvent::Action(Action::ToggleDraw)));
            win::post(Msg::Input(InputEvent::Action(Action::ToolEllipse)));
        }
        "help" | "help-end" => {
            shell.open_settings(Tab::Help);
            if name == "help-end" {
                win::post(Msg::Panel(PanelId::Settings, PanelEvent::Wheel(-30.0)));
            }
        }
        "settings" | "settings-end" | "conflict" => {
            shell.toggle_settings();
            if name == "settings-end" {
                win::post(Msg::Panel(PanelId::Settings, PanelEvent::Wheel(-30.0)));
            }
            if name == "conflict" {
                win::post(Msg::Panel(PanelId::Settings, PanelEvent::Wheel(-4.0)));
                shell.settings.capturing = Some((ekspointer::shortcuts::Action::ToolRect, false));
                if let Ok(shortcut) = "F".parse() {
                    win::post(Msg::Input(InputEvent::Captured(shortcut)));
                }
            }
        }
        _ => {}
    }
}

/// Ouvre une adresse dans le navigateur par défaut.
fn open_url(url: &str) {
    use windows::Win32::UI::Shell::ShellExecuteW;
    let wide: Vec<u16> = url.encode_utf16().chain(Some(0)).collect();
    unsafe {
        ShellExecuteW(None, w!("open"), PCWSTR(wide.as_ptr()), None, None, SW_SHOWNORMAL);
    }
}

/// Langue de Windows pour l'utilisateur courant.
fn system_lang() -> Lang {
    let mut buffer = [0u16; 85];
    let len = unsafe { windows::Win32::Globalization::GetUserDefaultLocaleName(&mut buffer) };
    let len = (len.max(1) as usize - 1).min(buffer.len());
    Lang::from_locale(&String::from_utf16_lossy(&buffer[..len]))
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
    let demo = args.demo || args.demo_ui.is_some();
    let (mut cfg, outcome) = if demo { (Config::default(), LoadOutcome::Defaults) } else { config::load(&cfg_path) };
    if let LoadOutcome::Invalid { backup } = &outcome {
        log::error(&format!("réglages illisibles, mis de côté dans {}", backup.display()));
    }
    // Premier lancement : l'interface prend la langue de Windows (français, sinon anglais).
    if matches!(outcome, LoadOutcome::Defaults) && !demo {
        cfg.language = system_lang();
        cfg.bindings.relocalize(Lang::Fr, cfg.language);
    }
    if let Some(forced) = args.lang {
        cfg.bindings.relocalize(cfg.language, forced);
        cfg.language = forced;
    }
    let lang = cfg.language;
    cfg.sanitize();
    TASKBAR_CREATED.store(unsafe { RegisterWindowMessageW(w!("TaskbarCreated")) }, std::sync::atomic::Ordering::Relaxed);

    let gfx = Gfx::new()?;
    let overlays = Overlays::new(&gfx)?;
    let app = App::new(cfg);
    let filter = Filter::new(app.cfg.bindings.clone());
    let _hooks = match input::install(filter) {
        Ok(h) => h,
        Err(_) => fatal(t(lang, Text::HookFailed)),
    };

    let animate = animations_enabled();
    let ui = Ui::new(&gfx, &app, &overlays, animate)?;
    let settings = SettingsView::new(&gfx)?;
    let tray = Tray::new(main)?;
    if matches!(outcome, LoadOutcome::Invalid { .. }) {
        tray.notify("Ekspointer", t(lang, Text::InvalidConfig));
    }

    let mut shell = Shell {
        gfx,
        overlays,
        ui,
        settings,
        tray,
        app,
        main,
        capturable: args.capturable,
        last_recovery: 0,
        shapes_dirty: true,
        start: Instant::now(),
        cfg_path,
        save_enabled: !demo,
        save_failed: false,
        animate,
    };
    shell.apply_capture_affinity();
    shell.sync_exempt_rects();
    // Position de départ du pointeur, pour que le halo n'apparaisse pas dans un coin de l'écran.
    let mut cursor = windows::Win32::Foundation::POINT::default();
    if unsafe { GetCursorPos(&mut cursor) }.is_ok() {
        win::post(Msg::Input(InputEvent::PointerMove(Pt::new(cursor.x as f32, cursor.y as f32))));
    }
    unsafe {
        SetTimer(Some(main), TIMER_TRIM, TRIM_DELAY_MS, None);
    }
    if args.demo {
        demo_scene(&mut shell);
    }
    if let Some(name) = &args.demo_ui {
        demo_ui(&mut shell, name);
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
        animating = shell.app.tick(now) | shell.ui.animating(now) | shell.settings.panel.appearing(now);
        if animating {
            shell.shapes_dirty = true;
        }
        shell.render(now);
        // Le rendu peut lancer une animation (pastille de l'outil actif, apparition d'un volet).
        animating |= shell.ui.animating(now) | shell.settings.panel.appearing(now);
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
        let mutex = CreateMutexW(None, true, mutex_name());
        let already_running = GetLastError() == ERROR_ALREADY_EXISTS;
        if already_running || args.quit {
            if let Ok(existing) = FindWindowW(main_class(), None) {
                let message = if args.quit { WM_CLOSE } else { win::WM_APP_SHOW };
                let _ = PostMessageW(Some(existing), message, WPARAM(0), LPARAM(0));
            }
            return;
        }
        std::mem::forget(mutex);
    }

    if let Err(e) = run(args) {
        fatal(&format!("{} : {e}", t(system_lang(), Text::StartFailed)));
    }
}
