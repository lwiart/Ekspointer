//! Couche Windows : fenêtres, rendu, écoute des entrées. Tout vit sur un seul fil d'exécution.
//!
//! Les procédures de fenêtre et les écoutes ne touchent jamais à l'état de l'application :
//! elles déposent un message dans une file, que la boucle principale vide ensuite.

pub mod fonts;
pub mod gfx;
pub mod input;
pub mod log;
pub mod overlay;
pub mod render;

use ekspointer::input_filter::InputEvent;
use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_APP};

/// Message sans contenu, posté pour réveiller la boucle principale.
pub const WM_APP_WAKE: u32 = WM_APP + 1;
/// Demande d'affichage de la barre, envoyée par un second lancement.
pub const WM_APP_SHOW: u32 = WM_APP + 2;
/// Notification de l'icône de la zone de notification.
pub const WM_APP_TRAY: u32 = WM_APP + 3;

/// Message interne à destination de la boucle principale.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Msg {
    /// Événement d'entrée (écoute bas niveau ou fenêtre de dessin).
    Input(InputEvent),
    /// Les écrans ont changé (branchement, résolution, échelle).
    DisplayChanged,
    /// Un minuteur de la fenêtre principale a expiré.
    Timer(usize),
    /// Un second lancement demande d'afficher la barre.
    ShowToolbar,
}

thread_local! {
    static QUEUE: RefCell<VecDeque<Msg>> = const { RefCell::new(VecDeque::new()) };
    static NOTIFY: Cell<HWND> = const { Cell::new(HWND(std::ptr::null_mut())) };
}

/// Désigne la fenêtre à réveiller quand un message est déposé.
pub fn set_notify(hwnd: HWND) {
    NOTIFY.with(|n| n.set(hwnd));
}

/// Dépose un message pour la boucle principale et la réveille si elle dort.
pub fn post(msg: Msg) {
    let was_empty = QUEUE.with(|q| {
        let mut q = q.borrow_mut();
        let empty = q.is_empty();
        q.push_back(msg);
        empty
    });
    if was_empty {
        let hwnd = NOTIFY.with(|n| n.get());
        if !hwnd.0.is_null() {
            unsafe {
                let _ = PostMessageW(Some(hwnd), WM_APP_WAKE, WPARAM(0), LPARAM(0));
            }
        }
    }
}

/// Retire tous les messages en attente.
pub fn drain() -> Vec<Msg> {
    QUEUE.with(|q| q.borrow_mut().drain(..).collect())
}
