//! Couche Windows : fenêtres, rendu, écoute des entrées. Tout vit sur un seul fil d'exécution.
//!
//! Les procédures de fenêtre et les écoutes ne touchent jamais à l'état de l'application :
//! elles déposent un message dans une file, que la boucle principale vide ensuite.

pub mod fonts;
pub mod gfx;
pub mod input;
pub mod log;
pub mod autostart;
pub mod icons;
pub mod overlay;
pub mod panel;
pub mod render;
pub mod settings_view;
pub mod tray;
pub mod ui;

use ekspointer::input_filter::InputEvent;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicIsize, Ordering};
use std::sync::{Mutex, MutexGuard};
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
    /// Un cran de molette traverse les fenêtres de dessin : il faudra rétablir leur capture.
    WheelPass,
    /// Un réglage de Windows a changé (par exemple « Afficher les animations »).
    SystemSettingsChanged,
    /// Événement de souris d'un panneau de l'interface.
    Panel(panel::PanelId, panel::PanelEvent),
    /// Commande du menu de la zone de notification.
    Tray(tray::TrayCommand),
}

/// File des messages en attente. Le fil des écoutes y dépose, la boucle principale la vide.
static QUEUE: Mutex<VecDeque<Msg>> = Mutex::new(VecDeque::new());
/// Fenêtre à réveiller quand un message est déposé (0 tant qu'elle n'existe pas).
static NOTIFY: AtomicIsize = AtomicIsize::new(0);

fn queue() -> MutexGuard<'static, VecDeque<Msg>> {
    QUEUE.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Désigne la fenêtre à réveiller quand un message est déposé.
pub fn set_notify(hwnd: HWND) {
    NOTIFY.store(hwnd.0 as isize, Ordering::SeqCst);
}

/// Dépose un message pour la boucle principale et la réveille si elle dort.
pub fn post(msg: Msg) {
    let was_empty = {
        let mut q = queue();
        let empty = q.is_empty();
        q.push_back(msg);
        empty
    };
    if was_empty {
        let hwnd = NOTIFY.load(Ordering::SeqCst);
        if hwnd != 0 {
            unsafe {
                let _ = PostMessageW(Some(HWND(hwnd as *mut core::ffi::c_void)), WM_APP_WAKE, WPARAM(0), LPARAM(0));
            }
        }
    }
}

/// Retire tous les messages en attente.
pub fn drain() -> Vec<Msg> {
    queue().drain(..).collect()
}
