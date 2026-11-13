//! Écoute bas niveau du clavier et de la souris.
//!
//! Les deux écoutes vivent sur un fil d'exécution qui ne fait rien d'autre : Windows attend la
//! réponse de l'écoute avant de livrer chaque frappe et chaque mouvement de souris au système,
//! elle ne doit donc jamais patienter derrière le rendu ou une écriture sur disque. Les
//! fonctions d'écoute consultent le filtre et déposent des événements pour la boucle principale.

use super::{post, Msg};
use ekspointer::geom::Pt;
use ekspointer::input_filter::{Decision, Filter, MouseKind};
use ekspointer::shortcuts::KeyState;
use std::sync::{mpsc, Mutex, MutexGuard};
use std::thread::JoinHandle;
use windows::core::Result;
use windows::Win32::Foundation::{E_FAIL, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::SystemInformation::GetTickCount;
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::WindowsAndMessaging::*;

/// Filtre d'entrées, partagé entre le fil des écoutes et la boucle principale. Chacun ne le
/// verrouille que le temps d'un appel, sans rien attendre pendant ce temps.
static FILTER: Mutex<Option<Filter>> = Mutex::new(None);

fn filter() -> MutexGuard<'static, Option<Filter>> {
    FILTER.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Écoutes installées ; les retire et arrête leur fil à la destruction.
pub struct Hooks {
    thread_id: u32,
    thread: Option<JoinHandle<()>>,
}

impl Drop for Hooks {
    fn drop(&mut self) {
        unsafe {
            let _ = PostThreadMessageW(self.thread_id, WM_QUIT, WPARAM(0), LPARAM(0));
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
        *filter() = None;
    }
}

/// Accès au filtre depuis la boucle principale (mode dessin, raccourcis, zones exemptées).
pub fn with_filter<R>(f: impl FnOnce(&mut Filter) -> R) -> Option<R> {
    filter().as_mut().map(f)
}

fn vk_down(vk: u32) -> bool {
    unsafe { GetAsyncKeyState(vk as i32) as u16 & 0x8000 != 0 }
}

/// État réel des touches de modification.
fn read_keys() -> KeyState {
    let down = |vk: VIRTUAL_KEY| vk_down(vk.0 as u32);
    KeyState {
        lctrl: down(VK_LCONTROL),
        rctrl: down(VK_RCONTROL),
        lalt: down(VK_LMENU),
        ralt: down(VK_RMENU),
        shift: down(VK_LSHIFT) || down(VK_RSHIFT),
        win: down(VK_LWIN) || down(VK_RWIN),
    }
}

/// Horloge du filtre : la même que l'horodatage des événements clavier de Windows.
fn tick() -> u64 {
    unsafe { GetTickCount() as u64 }
}

/// Recale le filtre sur l'état réel du clavier et transmet les événements qui en découlent.
pub fn resync() {
    let events = with_filter(|f| {
        f.set_now(tick());
        f.prune_keys(&vk_down);
        f.resync(read_keys())
    });
    for ev in events.unwrap_or_default() {
        post(Msg::Input(ev));
    }
}

fn dispatch(decision: Option<Decision>) -> bool {
    let Some(decision) = decision else {
        return false;
    };
    for ev in decision.events {
        post(Msg::Input(ev));
    }
    decision.swallow
}

unsafe extern "system" fn keyboard_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32 {
        let info = unsafe { &*(lparam.0 as *const KBDLLHOOKSTRUCT) };
        let down = matches!(wparam.0 as u32, WM_KEYDOWN | WM_SYSKEYDOWN);
        // L'état du clavier lu ici précède l'événement en cours : il rattrape un relâchement
        // que l'écoute n'a pas vu (écran de verrouillage, fenêtre d'une application élevée).
        let before = read_keys();
        let decision = with_filter(|f| {
            f.set_now(info.time as u64);
            f.prune_keys(&vk_down);
            let mut events = f.resync(before);
            let mut d = f.on_key(info.vkCode, down);
            events.append(&mut d.events);
            Decision { swallow: d.swallow, events }
        });
        if dispatch(decision) {
            return LRESULT(1);
        }
    }
    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}

unsafe extern "system" fn mouse_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32 {
        let info = unsafe { &*(lparam.0 as *const MSLLHOOKSTRUCT) };
        let kind = match wparam.0 as u32 {
            WM_MOUSEMOVE => Some(MouseKind::Move),
            WM_LBUTTONDOWN => Some(MouseKind::LeftDown),
            WM_LBUTTONUP => Some(MouseKind::LeftUp),
            WM_RBUTTONDOWN => Some(MouseKind::RightDown),
            WM_RBUTTONUP => Some(MouseKind::RightUp),
            _ => None,
        };
        // Molette en mode dessin : les fenêtres de dessin se laissent traverser avant que
        // Windows ne choisisse à qui livrer ce cran.
        if matches!(wparam.0 as u32, WM_MOUSEWHEEL | WM_MOUSEHWHEEL) && super::overlay::wheel_pass_begin() {
            post(Msg::WheelPass);
        }
        if let Some(kind) = kind {
            let p = Pt::new(info.pt.x as f32, info.pt.y as f32);
            if dispatch(with_filter(|f| f.on_mouse(kind, p))) {
                return LRESULT(1);
            }
        }
    }
    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}

/// Corps du fil des écoutes : les installe, signale le résultat, puis sert leurs appels.
fn hook_thread(ready: mpsc::Sender<Result<u32>>) {
    unsafe {
        let keyboard = match SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_proc), None, 0) {
            Ok(h) => h,
            Err(e) => {
                let _ = ready.send(Err(e));
                return;
            }
        };
        let mouse = match SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_proc), None, 0) {
            Ok(h) => h,
            Err(e) => {
                let _ = UnhookWindowsHookEx(keyboard);
                let _ = ready.send(Err(e));
                return;
            }
        };
        let _ = ready.send(Ok(GetCurrentThreadId()));
        // Windows appelle les écoutes pendant que ce fil attend ses messages.
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {}
        let _ = UnhookWindowsHookEx(keyboard);
        let _ = UnhookWindowsHookEx(mouse);
    }
}

/// Installe les deux écoutes sur un fil d'exécution dédié.
pub fn install(new_filter: Filter) -> Result<Hooks> {
    *filter() = Some(new_filter);
    let (ready, started) = mpsc::channel();
    let thread = std::thread::Builder::new()
        .name("ekspointer-ecoute".into())
        .spawn(move || hook_thread(ready))
        .map_err(|_| windows::core::Error::from(E_FAIL))?;
    match started.recv() {
        Ok(Ok(thread_id)) => Ok(Hooks { thread_id, thread: Some(thread) }),
        Ok(Err(e)) => {
            *filter() = None;
            Err(e)
        }
        Err(_) => {
            *filter() = None;
            Err(E_FAIL.into())
        }
    }
}
