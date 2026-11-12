//! Polices embarquées, chargées en mémoire par DirectWrite.

use windows::Win32::Graphics::DirectWrite::{IDWriteFactory, IDWriteFontCollection};

/// Nom de famille de la police des libellés.
pub const BODY_FAMILY: &str = "Instrument Sans";
/// Nom de famille de la police des étiquettes et raccourcis.
pub const MONO_FAMILY: &str = "Noto Sans Mono";

/// Charge les polices embarquées. `None` : les polices du système servent de repli.
pub fn load(_dwrite: &IDWriteFactory) -> Option<IDWriteFontCollection> {
    None
}
