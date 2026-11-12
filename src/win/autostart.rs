//! Lancement au démarrage de Windows : valeur `Ekspointer` de la clé `Run` de l'utilisateur.

use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::ERROR_SUCCESS;
use windows::Win32::System::Registry::*;

const RUN_KEY: PCWSTR = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run");
const VALUE: PCWSTR = w!("Ekspointer");

/// Vrai si Ekspointer est lancé au démarrage de Windows.
pub fn is_enabled() -> bool {
    unsafe { RegGetValueW(HKEY_CURRENT_USER, RUN_KEY, VALUE, RRF_RT_REG_SZ, None, None, None) == ERROR_SUCCESS }
}

/// Active ou désactive le lancement au démarrage.
pub fn set_enabled(on: bool) -> std::io::Result<()> {
    let status = unsafe {
        if on {
            let exe = std::env::current_exe()?;
            let command: Vec<u16> = format!("\"{}\"", exe.display()).encode_utf16().chain(Some(0)).collect();
            RegSetKeyValueW(
                HKEY_CURRENT_USER,
                RUN_KEY,
                VALUE,
                REG_SZ.0,
                Some(command.as_ptr() as *const core::ffi::c_void),
                (command.len() * 2) as u32,
            )
        } else if is_enabled() {
            RegDeleteKeyValueW(HKEY_CURRENT_USER, RUN_KEY, VALUE)
        } else {
            ERROR_SUCCESS
        }
    };
    if status == ERROR_SUCCESS {
        Ok(())
    } else {
        Err(std::io::Error::from_raw_os_error(status.0 as i32))
    }
}
