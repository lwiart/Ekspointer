//! Polices embarquées (licence OFL), chargées en mémoire par DirectWrite.

use windows::core::{Interface, Result};
use windows::Win32::Graphics::DirectWrite::*;

/// Nom de famille de la police des libellés.
pub const BODY_FAMILY: &str = "Instrument Sans";
/// Nom de famille de la police des étiquettes et raccourcis.
pub const MONO_FAMILY: &str = "Noto Sans Mono";

static BODY: &[u8] = include_bytes!("../../assets/fonts/InstrumentSans.ttf");
static MONO: &[u8] = include_bytes!("../../assets/fonts/NotoSansMono.ttf");

fn try_load(dwrite: &IDWriteFactory) -> Result<IDWriteFontCollection> {
    unsafe {
        let factory: IDWriteFactory5 = dwrite.cast()?;
        let loader = factory.CreateInMemoryFontFileLoader()?;
        factory.RegisterFontFileLoader(&loader)?;
        let builder: IDWriteFontSetBuilder1 = factory.CreateFontSetBuilder()?.cast()?;
        for data in [BODY, MONO] {
            // Les octets sont statiques : aucun propriétaire à maintenir en vie.
            let file = loader.CreateInMemoryFontFileReference(
                &factory,
                data.as_ptr() as *const core::ffi::c_void,
                data.len() as u32,
                None,
            )?;
            builder.AddFontFile(&file)?;
        }
        let set = builder.CreateFontSet()?;
        let collection = factory.CreateFontCollectionFromFontSet(&set)?;
        collection.cast()
    }
}

/// Charge les polices embarquées. `None` : les polices du système servent de repli.
pub fn load(dwrite: &IDWriteFactory) -> Option<IDWriteFontCollection> {
    match try_load(dwrite) {
        Ok(collection) => Some(collection),
        Err(e) => {
            super::log::error(&format!("polices embarquées non chargées, repli sur les polices du système : {e}"));
            None
        }
    }
}
