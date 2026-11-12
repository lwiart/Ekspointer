//! Périphériques graphiques (Direct3D, Direct2D, DirectComposition, DirectWrite),
//! surface de rendu d'une fenêtre et petit pinceau au-dessus de Direct2D.

use ekspointer::geom::{Pt, RectF, Rgba};
use std::mem::ManuallyDrop;
use windows::core::{Interface, Result, PCWSTR};
use windows::Win32::Foundation::{E_FAIL, HMODULE, HWND};
use windows::Win32::Graphics::Direct2D::Common::*;
use windows::Win32::Graphics::Direct2D::*;
use windows::Win32::Graphics::Direct3D::*;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::DirectComposition::*;
use windows::Win32::Graphics::DirectWrite::*;
use windows::Win32::Graphics::Dxgi::Common::*;
use windows::Win32::Graphics::Dxgi::*;
use windows_numerics::{Matrix3x2, Vector2};

/// Famille de police de l'interface.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Family {
    /// Instrument Sans : libellés.
    Body,
    /// Noto Sans Mono : étiquettes, raccourcis, numéros.
    Mono,
}

/// Alignement horizontal d'un texte dans son rectangle.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Align {
    Left,
    Center,
    Right,
}

/// Périphériques partagés par toutes les fenêtres.
pub struct Gfx {
    pub d2d_factory: ID2D1Factory1,
    pub dwrite: IDWriteFactory,
    pub dxgi_device: IDXGIDevice,
    pub dxgi_factory: IDXGIFactory2,
    pub d2d_device: ID2D1Device,
    pub dcomp: IDCompositionDevice,
    /// Trait à extrémités et jointures rondes.
    pub round: ID2D1StrokeStyle1,
    /// Trait à extrémités plates et jointures rondes.
    pub flat: ID2D1StrokeStyle1,
    /// Collection des polices embarquées, si leur chargement a réussi.
    pub fonts: Option<IDWriteFontCollection>,
}

fn create_d3d() -> Result<ID3D11Device> {
    for driver in [D3D_DRIVER_TYPE_HARDWARE, D3D_DRIVER_TYPE_WARP] {
        let mut device = None;
        let created = unsafe {
            D3D11CreateDevice(
                None,
                driver,
                HMODULE::default(),
                D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                None,
                D3D11_SDK_VERSION,
                Some(&mut device),
                None,
                None,
            )
        };
        if created.is_ok() {
            if let Some(device) = device {
                return Ok(device);
            }
        }
    }
    Err(E_FAIL.into())
}

fn stroke_style(factory: &ID2D1Factory1, cap: D2D1_CAP_STYLE) -> Result<ID2D1StrokeStyle1> {
    let props = D2D1_STROKE_STYLE_PROPERTIES1 {
        startCap: cap,
        endCap: cap,
        dashCap: cap,
        lineJoin: D2D1_LINE_JOIN_ROUND,
        miterLimit: 10.0,
        dashStyle: D2D1_DASH_STYLE_SOLID,
        dashOffset: 0.0,
        transformType: D2D1_STROKE_TRANSFORM_TYPE_NORMAL,
    };
    unsafe { factory.CreateStrokeStyle(&props, None) }
}

impl Gfx {
    pub fn new() -> Result<Gfx> {
        unsafe {
            let d2d_factory: ID2D1Factory1 = D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)?;
            let dwrite: IDWriteFactory = DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)?;
            let d3d = create_d3d()?;
            let dxgi_device: IDXGIDevice = d3d.cast()?;
            let dxgi_factory: IDXGIFactory2 = CreateDXGIFactory2(DXGI_CREATE_FACTORY_FLAGS(0))?;
            let d2d_device = d2d_factory.CreateDevice(&dxgi_device)?;
            let dcomp: IDCompositionDevice = DCompositionCreateDevice(&dxgi_device)?;
            let round = stroke_style(&d2d_factory, D2D1_CAP_STYLE_ROUND)?;
            let flat = stroke_style(&d2d_factory, D2D1_CAP_STYLE_FLAT)?;
            let fonts = super::fonts::load(&dwrite);
            Ok(Gfx { d2d_factory, dwrite, dxgi_device, dxgi_factory, d2d_device, dcomp, round, flat, fonts })
        }
    }

    /// Largeur d'un texte sur une ligne.
    pub fn text_width(&self, s: &str, family: Family, size: f32, weight: i32) -> f32 {
        let Ok(format) = self.text_format(family, size, weight, Align::Left) else {
            return 0.0;
        };
        let wide: Vec<u16> = s.encode_utf16().collect();
        unsafe {
            let Ok(layout) = self.dwrite.CreateTextLayout(&wide, &format, 10_000.0, 100.0) else {
                return 0.0;
            };
            let mut m = DWRITE_TEXT_METRICS::default();
            if layout.GetMetrics(&mut m).is_err() {
                return 0.0;
            }
            m.widthIncludingTrailingWhitespace
        }
    }

    /// Format de texte. `size` est dans l'unité du contexte de dessin.
    pub fn text_format(&self, family: Family, size: f32, weight: i32, align: Align) -> Result<IDWriteTextFormat> {
        let (embedded, fallback) = match family {
            Family::Body => (super::fonts::BODY_FAMILY, "Segoe UI"),
            Family::Mono => (super::fonts::MONO_FAMILY, "Consolas"),
        };
        let name = if self.fonts.is_some() { embedded } else { fallback };
        let wide: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
        let locale: Vec<u16> = "fr-FR".encode_utf16().chain(Some(0)).collect();
        unsafe {
            let format = self.dwrite.CreateTextFormat(
                PCWSTR(wide.as_ptr()),
                self.fonts.as_ref(),
                DWRITE_FONT_WEIGHT(weight),
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                size,
                PCWSTR(locale.as_ptr()),
            )?;
            format.SetTextAlignment(match align {
                Align::Left => DWRITE_TEXT_ALIGNMENT_LEADING,
                Align::Center => DWRITE_TEXT_ALIGNMENT_CENTER,
                Align::Right => DWRITE_TEXT_ALIGNMENT_TRAILING,
            })?;
            format.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER)?;
            format.SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP)?;
            Ok(format)
        }
    }
}

/// Surface de rendu transparente attachée à une fenêtre.
pub struct Surface {
    swap: IDXGISwapChain1,
    ctx: ID2D1DeviceContext,
    brush: ID2D1SolidColorBrush,
    // Gardés en vie : la cible et le visuel portent le contenu à l'écran.
    _target: IDCompositionTarget,
    _visual: IDCompositionVisual,
    w: u32,
    h: u32,
}

impl Surface {
    pub fn new(gfx: &Gfx, hwnd: HWND, w: u32, h: u32) -> Result<Surface> {
        let (w, h) = (w.max(1), h.max(1));
        unsafe {
            let desc = DXGI_SWAP_CHAIN_DESC1 {
                Width: w,
                Height: h,
                Format: DXGI_FORMAT_B8G8R8A8_UNORM,
                SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
                BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
                BufferCount: 2,
                Scaling: DXGI_SCALING_STRETCH,
                SwapEffect: DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL,
                AlphaMode: DXGI_ALPHA_MODE_PREMULTIPLIED,
                ..Default::default()
            };
            let swap = gfx.dxgi_factory.CreateSwapChainForComposition(&gfx.dxgi_device, &desc, None)?;
            let ctx = gfx.d2d_device.CreateDeviceContext(D2D1_DEVICE_CONTEXT_OPTIONS_NONE)?;
            let brush = ctx.CreateSolidColorBrush(&D2D1_COLOR_F { r: 0.0, g: 0.0, b: 0.0, a: 1.0 }, None)?;
            let target = gfx.dcomp.CreateTargetForHwnd(hwnd, true)?;
            let visual = gfx.dcomp.CreateVisual()?;
            visual.SetContent(&swap)?;
            target.SetRoot(&visual)?;
            gfx.dcomp.Commit()?;
            let surface = Surface { swap, ctx, brush, _target: target, _visual: visual, w, h };
            surface.bind()?;
            Ok(surface)
        }
    }

    /// Relie le contexte Direct2D au tampon de la chaîne d'échange.
    fn bind(&self) -> Result<()> {
        unsafe {
            let buffer: IDXGISurface = self.swap.GetBuffer(0)?;
            let props = D2D1_BITMAP_PROPERTIES1 {
                pixelFormat: D2D1_PIXEL_FORMAT {
                    format: DXGI_FORMAT_B8G8R8A8_UNORM,
                    alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
                },
                dpiX: 96.0,
                dpiY: 96.0,
                bitmapOptions: D2D1_BITMAP_OPTIONS_TARGET | D2D1_BITMAP_OPTIONS_CANNOT_DRAW,
                colorContext: ManuallyDrop::new(None),
            };
            let bitmap = self.ctx.CreateBitmapFromDxgiSurface(&buffer, Some(&props))?;
            self.ctx.SetTarget(&bitmap);
            Ok(())
        }
    }

    pub fn size(&self) -> (u32, u32) {
        (self.w, self.h)
    }

    pub fn resize(&mut self, w: u32, h: u32) -> Result<()> {
        let (w, h) = (w.max(1), h.max(1));
        if (w, h) == (self.w, self.h) {
            return Ok(());
        }
        unsafe {
            self.ctx.SetTarget(None);
            self.swap.ResizeBuffers(0, w, h, DXGI_FORMAT_UNKNOWN, DXGI_SWAP_CHAIN_FLAG(0))?;
        }
        self.w = w;
        self.h = h;
        self.bind()
    }

    /// Efface en transparent, dessine avec `f` (unités multipliées par `scale`), présente.
    pub fn draw(&mut self, gfx: &Gfx, scale: f32, f: impl FnOnce(&Painter)) -> Result<()> {
        unsafe {
            self.ctx.BeginDraw();
            self.ctx.SetTransform(&Matrix3x2 { M11: scale, M12: 0.0, M21: 0.0, M22: scale, M31: 0.0, M32: 0.0 });
            self.ctx.Clear(Some(&D2D1_COLOR_F { r: 0.0, g: 0.0, b: 0.0, a: 0.0 }));
            f(&Painter { ctx: &self.ctx, gfx, brush: &self.brush });
            self.ctx.EndDraw(None, None)?;
            self.swap.Present(0, DXGI_PRESENT(0)).ok()?;
            Ok(())
        }
    }
}

fn v(p: Pt) -> Vector2 {
    Vector2 { X: p.x, Y: p.y }
}

fn rect(r: RectF) -> D2D_RECT_F {
    D2D_RECT_F { left: r.x, top: r.y, right: r.x + r.w, bottom: r.y + r.h }
}

/// Pinceau : primitives de dessin au-dessus d'un contexte Direct2D.
pub struct Painter<'a> {
    pub ctx: &'a ID2D1DeviceContext,
    pub gfx: &'a Gfx,
    brush: &'a ID2D1SolidColorBrush,
}

impl Painter<'_> {
    fn set(&self, c: Rgba) {
        unsafe { self.brush.SetColor(&D2D1_COLOR_F { r: c.r, g: c.g, b: c.b, a: c.a }) };
    }

    fn style(&self, round: bool) -> &ID2D1StrokeStyle1 {
        if round {
            &self.gfx.round
        } else {
            &self.gfx.flat
        }
    }

    pub fn fill_rrect(&self, r: RectF, radius: f32, c: Rgba) {
        self.set(c);
        let rr = D2D1_ROUNDED_RECT { rect: rect(r), radiusX: radius, radiusY: radius };
        unsafe { self.ctx.FillRoundedRectangle(&rr, self.brush) };
    }

    pub fn stroke_rrect(&self, r: RectF, radius: f32, c: Rgba, width: f32) {
        self.set(c);
        let rr = D2D1_ROUNDED_RECT { rect: rect(r), radiusX: radius, radiusY: radius };
        unsafe { self.ctx.DrawRoundedRectangle(&rr, self.brush, width, &self.gfx.round) };
    }

    pub fn fill_circle(&self, center: Pt, radius: f32, c: Rgba) {
        self.set(c);
        let e = D2D1_ELLIPSE { point: v(center), radiusX: radius, radiusY: radius };
        unsafe { self.ctx.FillEllipse(&e, self.brush) };
    }

    pub fn stroke_ellipse(&self, center: Pt, rx: f32, ry: f32, c: Rgba, width: f32) {
        self.set(c);
        let e = D2D1_ELLIPSE { point: v(center), radiusX: rx, radiusY: ry };
        unsafe { self.ctx.DrawEllipse(&e, self.brush, width, &self.gfx.round) };
    }

    pub fn line(&self, a: Pt, b: Pt, c: Rgba, width: f32, round: bool) {
        self.set(c);
        unsafe { self.ctx.DrawLine(v(a), v(b), self.brush, width, self.style(round)) };
    }

    /// Construit un chemin : une suite de segments droits ou de Bézier depuis `start`.
    fn geometry(&self, start: Pt, lines: &[Pt], beziers: &[[Pt; 4]], closed: bool) -> Result<ID2D1PathGeometry1> {
        unsafe {
            let geom = self.gfx.d2d_factory.CreatePathGeometry()?;
            let sink = geom.Open()?;
            sink.BeginFigure(v(start), if closed { D2D1_FIGURE_BEGIN_FILLED } else { D2D1_FIGURE_BEGIN_HOLLOW });
            for p in lines {
                sink.AddLine(v(*p));
            }
            for b in beziers {
                sink.AddBezier(&D2D1_BEZIER_SEGMENT { point1: v(b[1]), point2: v(b[2]), point3: v(b[3]) });
            }
            sink.EndFigure(if closed { D2D1_FIGURE_END_CLOSED } else { D2D1_FIGURE_END_OPEN });
            sink.Close()?;
            Ok(geom)
        }
    }

    /// Remplit un polygone fermé.
    pub fn fill_poly(&self, pts: &[Pt], c: Rgba) {
        if pts.len() < 3 {
            return;
        }
        if let Ok(g) = self.geometry(pts[0], &pts[1..], &[], true) {
            self.set(c);
            unsafe { self.ctx.FillGeometry(&g, self.brush, None) };
        }
    }

    /// Trace une ligne brisée, fermée ou non, à jointures rondes.
    pub fn stroke_poly(&self, pts: &[Pt], closed: bool, c: Rgba, width: f32, round: bool) {
        if pts.len() < 2 {
            return;
        }
        if let Ok(g) = self.geometry(pts[0], &pts[1..], &[], closed) {
            self.set(c);
            unsafe { self.ctx.DrawGeometry(&g, self.brush, width, self.style(round)) };
        }
    }

    /// Trace une suite de Bézier cubiques jointives.
    pub fn stroke_beziers(&self, segs: &[[Pt; 4]], c: Rgba, width: f32, round: bool) {
        let Some(first) = segs.first() else {
            return;
        };
        if let Ok(g) = self.geometry(first[0], &[], segs, false) {
            self.set(c);
            unsafe { self.ctx.DrawGeometry(&g, self.brush, width, self.style(round)) };
        }
    }

    /// Disque en dégradé radial : `inner` au centre jusqu'à `solid` (fraction du rayon), puis transparent.
    pub fn radial_glow(&self, center: Pt, radius: f32, inner: Rgba, solid: f32) {
        let color = |a: f32| D2D1_COLOR_F { r: inner.r, g: inner.g, b: inner.b, a };
        let stops = [
            D2D1_GRADIENT_STOP { position: 0.0, color: color(inner.a) },
            D2D1_GRADIENT_STOP { position: solid.clamp(0.0, 1.0), color: color(inner.a) },
            D2D1_GRADIENT_STOP { position: 1.0, color: color(0.0) },
        ];
        unsafe {
            let Ok(collection) = self.ctx.CreateGradientStopCollection(
                &stops,
                D2D1_COLOR_SPACE_SRGB,
                D2D1_COLOR_SPACE_SRGB,
                D2D1_BUFFER_PRECISION_8BPC_UNORM,
                D2D1_EXTEND_MODE_CLAMP,
                D2D1_COLOR_INTERPOLATION_MODE_PREMULTIPLIED,
            ) else {
                return;
            };
            let props = D2D1_RADIAL_GRADIENT_BRUSH_PROPERTIES {
                center: v(center),
                gradientOriginOffset: Vector2 { X: 0.0, Y: 0.0 },
                radiusX: radius,
                radiusY: radius,
            };
            if let Ok(brush) = self.ctx.CreateRadialGradientBrush(&props, None, &collection) {
                let e = D2D1_ELLIPSE { point: v(center), radiusX: radius, radiusY: radius };
                self.ctx.FillEllipse(&e, &brush);
            }
        }
    }

    /// Texte sur une ligne, centré verticalement dans `r`.
    pub fn text(&self, s: &str, family: Family, size: f32, weight: i32, align: Align, r: RectF, c: Rgba) {
        let Ok(format) = self.gfx.text_format(family, size, weight, align) else {
            return;
        };
        let wide: Vec<u16> = s.encode_utf16().collect();
        self.set(c);
        unsafe {
            self.ctx.DrawText(
                &wide,
                &format,
                &rect(r),
                self.brush,
                D2D1_DRAW_TEXT_OPTIONS_NONE,
                DWRITE_MEASURING_MODE_NATURAL,
            );
        }
    }


    /// Dessine `f` en le limitant à un rectangle arrondi.
    pub fn with_rrect_clip(&self, r: RectF, radius: f32, f: impl FnOnce()) {
        let rr = D2D1_ROUNDED_RECT { rect: rect(r), radiusX: radius, radiusY: radius };
        let Ok(mask) = (unsafe { self.gfx.d2d_factory.CreateRoundedRectangleGeometry(&rr) }) else {
            f();
            return;
        };
        let params = D2D1_LAYER_PARAMETERS1 {
            contentBounds: rect(r),
            geometricMask: ManuallyDrop::new(mask.cast().ok()),
            maskAntialiasMode: D2D1_ANTIALIAS_MODE_PER_PRIMITIVE,
            maskTransform: Matrix3x2::identity(),
            opacity: 1.0,
            opacityBrush: ManuallyDrop::new(None),
            layerOptions: D2D1_LAYER_OPTIONS1_NONE,
        };
        unsafe {
            self.ctx.PushLayer(&params, None);
            f();
            self.ctx.PopLayer();
            drop(ManuallyDrop::into_inner(params.geometricMask));
        }
    }

    /// Dessine `f` avec une opacité globale (sans que les passes se voient l'une à travers l'autre).
    pub fn with_opacity(&self, opacity: f32, f: impl FnOnce()) {
        if opacity >= 0.999 {
            f();
            return;
        }
        let params = D2D1_LAYER_PARAMETERS1 {
            contentBounds: D2D_RECT_F { left: -1e7, top: -1e7, right: 1e7, bottom: 1e7 },
            geometricMask: ManuallyDrop::new(None),
            maskAntialiasMode: D2D1_ANTIALIAS_MODE_PER_PRIMITIVE,
            maskTransform: Matrix3x2::identity(),
            opacity: opacity.max(0.0),
            opacityBrush: ManuallyDrop::new(None),
            layerOptions: D2D1_LAYER_OPTIONS1_NONE,
        };
        unsafe {
            self.ctx.PushLayer(&params, None);
            f();
            self.ctx.PopLayer();
        }
    }
}
