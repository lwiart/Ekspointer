//! Icône de la zone de notification, son menu et les notifications.

use super::WM_APP_TRAY;
use ekspointer::i18n::{t, Lang, Text};
use windows::core::{Result, PCWSTR};
use windows::Win32::Foundation::{HWND, POINT};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Shell::*;
use windows::Win32::UI::WindowsAndMessaging::*;

/// Commande venue de la zone de notification.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TrayCommand {
    /// Clic gauche sur l'icône : afficher la barre.
    ShowToolbar,
    /// Clic droit : ouvrir le menu.
    OpenMenu,
    /// L'Explorateur a redémarré : remettre l'icône.
    Recreate,
    ToggleToolbar,
    Settings,
    Help,
    Quit,
}

const ID_TOGGLE: usize = 1;
const ID_SETTINGS: usize = 2;
const ID_QUIT: usize = 3;
const ID_HELP: usize = 4;

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(Some(0)).collect()
}

fn copy_into(dst: &mut [u16], text: &str) {
    let wide: Vec<u16> = text.encode_utf16().collect();
    let n = wide.len().min(dst.len() - 1);
    dst[..n].copy_from_slice(&wide[..n]);
    dst[n] = 0;
}

/// Icône de l'application (ressource 1), ou icône générique à défaut.
pub fn app_icon() -> HICON {
    unsafe {
        GetModuleHandleW(None)
            .ok()
            .and_then(|m| LoadIconW(Some(m.into()), PCWSTR(1 as *const u16)).ok())
            .or_else(|| LoadIconW(None, IDI_APPLICATION).ok())
            .unwrap_or_default()
    }
}

/// Icône de la zone de notification ; retirée à la destruction.
pub struct Tray {
    hwnd: HWND,
}

impl Tray {
    fn data(&self) -> NOTIFYICONDATAW {
        let mut nid = NOTIFYICONDATAW {
            cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: self.hwnd,
            uID: 1,
            uFlags: NIF_ICON | NIF_MESSAGE | NIF_TIP,
            uCallbackMessage: WM_APP_TRAY,
            hIcon: app_icon(),
            ..Default::default()
        };
        copy_into(&mut nid.szTip, "Ekspointer");
        nid
    }

    pub fn new(hwnd: HWND) -> Result<Tray> {
        let tray = Tray { hwnd };
        tray.add();
        Ok(tray)
    }

    /// (Re)pose l'icône, par exemple après un redémarrage de l'Explorateur.
    pub fn add(&self) {
        unsafe {
            let _ = Shell_NotifyIconW(NIM_ADD, &self.data());
        }
    }

    /// Affiche une notification.
    pub fn notify(&self, title: &str, text: &str) {
        let mut nid = self.data();
        nid.uFlags |= NIF_INFO;
        copy_into(&mut nid.szInfoTitle, title);
        copy_into(&mut nid.szInfo, text);
        nid.dwInfoFlags = NIIF_INFO;
        unsafe {
            let _ = Shell_NotifyIconW(NIM_MODIFY, &nid);
        }
    }

    /// Ouvre le menu au curseur et renvoie la commande choisie.
    pub fn menu(&self, toolbar_visible: bool, lang: Lang) -> Option<TrayCommand> {
        unsafe {
            let menu = CreatePopupMenu().ok()?;
            let items = [
                (ID_TOGGLE, t(lang, if toolbar_visible { Text::TrayHide } else { Text::TrayShow })),
                (ID_SETTINGS, t(lang, Text::Settings)),
                (ID_HELP, t(lang, Text::Help)),
            ];
            for (id, text) in items {
                let text = wide(text);
                let _ = AppendMenuW(menu, MF_STRING, id, PCWSTR(text.as_ptr()));
            }
            let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
            let quit = wide(t(lang, Text::TrayQuit));
            let _ = AppendMenuW(menu, MF_STRING, ID_QUIT, PCWSTR(quit.as_ptr()));
            let mut p = POINT::default();
            let _ = GetCursorPos(&mut p);
            // Sans cela, le menu ne se referme pas quand on clique ailleurs.
            let _ = SetForegroundWindow(self.hwnd);
            let chosen = TrackPopupMenu(menu, TPM_RETURNCMD | TPM_RIGHTBUTTON | TPM_NONOTIFY, p.x, p.y, None, self.hwnd, None);
            let _ = DestroyMenu(menu);
            match chosen.0 as usize {
                ID_TOGGLE => Some(TrayCommand::ToggleToolbar),
                ID_SETTINGS => Some(TrayCommand::Settings),
                ID_HELP => Some(TrayCommand::Help),
                ID_QUIT => Some(TrayCommand::Quit),
                _ => None,
            }
        }
    }
}

impl Drop for Tray {
    fn drop(&mut self) {
        unsafe {
            let _ = Shell_NotifyIconW(NIM_DELETE, &self.data());
        }
    }
}
