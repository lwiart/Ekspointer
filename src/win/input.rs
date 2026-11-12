//! Écoute bas niveau du clavier et de la souris. Les fonctions d'écoute ne font que
//! consulter le filtre et poster des événements : aucun autre travail, pour ne jamais
//! ralentir les entrées du système.

use super::{post, Msg};
use ekspointer::geom::Pt;
use ekspointer::input_filter::{Decision, Filter, MouseKind};
use ekspointer::shortcuts::KeyState;
use std::cell::RefCell;
use windows::core::Result;
use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::WindowsAndMessaging::*;

thread_local! {
    static FILTER: RefCell<Option<Filter>> = const { RefCell::new(None) };
}

/// Écoutes installées ; les retire à la destruction.
pub struct Hooks {
    keyboard: HHOOK,
    mouse: HHOOK,
}

impl Drop for Hooks {
    fn drop(&mut self) {
        unsafe {
            let _ = UnhookWindowsHookEx(self.keyboard);
            let _ = UnhookWindowsHookEx(self.mouse);
        }
        FILTER.with(|f| *f.borrow_mut() = None);
    }
}

/// Accès au filtre depuis la boucle principale (mode dessin, raccourcis, zones exemptées).
pub fn with_filter<R>(f: impl FnOnce(&mut Filter) -> R) -> Option<R> {
    FILTER.with(|cell| cell.try_borrow_mut().ok().and_then(|mut g| g.as_mut().map(f)))
}

fn key_down(vk: VIRTUAL_KEY) -> bool {
    unsafe { GetAsyncKeyState(vk.0 as i32) as u16 & 0x8000 != 0 }
}

/// État réel des touches de modification.
pub fn read_keys() -> KeyState {
    KeyState {
        lctrl: key_down(VK_LCONTROL),
        rctrl: key_down(VK_RCONTROL),
        lalt: key_down(VK_LMENU),
        ralt: key_down(VK_RMENU),
        shift: key_down(VK_LSHIFT) || key_down(VK_RSHIFT),
        win: key_down(VK_LWIN) || key_down(VK_RWIN),
    }
}

/// Recale le filtre sur l'état réel du clavier et transmet les événements qui en découlent.
pub fn resync() {
    if let Some(events) = with_filter(|f| f.resync(read_keys())) {
        for ev in events {
            post(Msg::Input(ev));
        }
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
        // L'état lu ici précède l'événement en cours : il rattrape un relâchement manqué.
        let before = read_keys();
        let decision = with_filter(|f| {
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
        if let Some(kind) = kind {
            let p = Pt::new(info.pt.x as f32, info.pt.y as f32);
            if dispatch(with_filter(|f| f.on_mouse(kind, p))) {
                return LRESULT(1);
            }
        }
    }
    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}

/// Installe les deux écoutes sur le fil d'exécution courant.
pub fn install(filter: Filter) -> Result<Hooks> {
    FILTER.with(|f| *f.borrow_mut() = Some(filter));
    unsafe {
        let keyboard = SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_proc), None, 0)?;
        let mouse = match SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_proc), None, 0) {
            Ok(h) => h,
            Err(e) => {
                let _ = UnhookWindowsHookEx(keyboard);
                return Err(e);
            }
        };
        Ok(Hooks { keyboard, mouse })
    }
}
