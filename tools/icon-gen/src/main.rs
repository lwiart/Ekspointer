//! Génère l'icône de l'application et les images de l'assistant d'installation
//! à partir de `assets/logo.svg` et `assets/logo-small.svg`.
//!
//! À lancer depuis la racine du dépôt : `cargo run -p icon-gen`.

use resvg::tiny_skia::{Color, Pixmap, Transform};
use resvg::usvg::{Options, Tree};
use std::error::Error;
use std::fs;
use std::io::BufWriter;

type Res<T> = Result<T, Box<dyn Error>>;

fn load(path: &str) -> Res<Tree> {
    Ok(Tree::from_data(&fs::read(path)?, &Options::default())?)
}

/// Rend un SVG carré dans une image de `size` pixels de côté.
fn render(tree: &Tree, size: u32) -> Res<Pixmap> {
    let mut pixmap = Pixmap::new(size, size).ok_or("taille d'image invalide")?;
    let scale = size as f32 / tree.size().width();
    resvg::render(tree, Transform::from_scale(scale, scale), &mut pixmap.as_mut());
    Ok(pixmap)
}

/// Octets RGBA non prémultipliés d'une image.
fn rgba(pixmap: &Pixmap) -> Vec<u8> {
    pixmap.pixels().iter().flat_map(|p| {
        let c = p.demultiply();
        [c.red(), c.green(), c.blue(), c.alpha()]
    }).collect()
}

fn write_ico(full: &Tree, small: &Tree, path: &str) -> Res<()> {
    let mut dir = ico::IconDir::new(ico::ResourceType::Icon);
    for size in [16u32, 24, 32, 48, 64, 256] {
        // En petit, le logo complet devient illisible : version simplifiée.
        let tree = if size <= 24 { small } else { full };
        let image = ico::IconImage::from_rgba_data(size, size, rgba(&render(tree, size)?));
        dir.add_entry(ico::IconDirEntry::encode(&image)?);
    }
    dir.write(BufWriter::new(fs::File::create(path)?))?;
    Ok(())
}

/// Écrit une image BMP 24 bits : le logo de côté `logo` centré à l'ordonnée `top`, sur un fond uni.
fn write_bmp(tree: &Tree, path: &str, w: u32, h: u32, background: (u8, u8, u8), logo: u32, top: u32) -> Res<()> {
    let mut canvas = Pixmap::new(w, h).ok_or("taille d'image invalide")?;
    canvas.fill(Color::from_rgba8(background.0, background.1, background.2, 255));
    let scale = logo as f32 / tree.size().width();
    let transform = Transform::from_scale(scale, scale).post_translate((w as f32 - logo as f32) / 2.0, top as f32);
    resvg::render(tree, transform, &mut canvas.as_mut());

    let row = (w * 3).div_ceil(4) * 4;
    let size = 54 + row * h;
    let mut out = Vec::with_capacity(size as usize);
    out.extend_from_slice(b"BM");
    out.extend_from_slice(&size.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&54u32.to_le_bytes());
    out.extend_from_slice(&40u32.to_le_bytes());
    out.extend_from_slice(&(w as i32).to_le_bytes());
    out.extend_from_slice(&(h as i32).to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&24u16.to_le_bytes());
    out.extend_from_slice(&[0u8; 24]);
    let pixels = canvas.pixels();
    for y in (0..h).rev() {
        for x in 0..w {
            let c = pixels[(y * w + x) as usize].demultiply();
            out.extend_from_slice(&[c.blue(), c.green(), c.red()]);
        }
        out.extend(std::iter::repeat(0u8).take((row - w * 3) as usize));
    }
    fs::write(path, out)?;
    Ok(())
}

fn main() -> Res<()> {
    let full = load("assets/logo.svg")?;
    let small = load("assets/logo-small.svg")?;
    write_ico(&full, &small, "assets/ekspointer.ico")?;
    // Assistant d'installation : petite image en haut à droite, grande image des pages d'accueil et de fin.
    write_bmp(&full, "assets/wizard-small.bmp", 55, 58, (0xFC, 0xFD, 0xFE), 50, 4)?;
    write_bmp(&full, "assets/wizard-large.bmp", 164, 314, (0xE3, 0xF1, 0xF7), 128, 40)?;
    // Aperçu pour les vérifications visuelles.
    render(&full, 256)?.save_png("assets/logo-preview.png")?;
    render(&small, 64)?.save_png("assets/logo-small-preview.png")?;
    println!("icône et images de l'assistant générées dans assets/");
    Ok(())
}
