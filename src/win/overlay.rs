//! Fenêtres de dessin : une fenêtre transparente par écran, toujours au premier plan,
//! qui ne prend jamais le focus.

use super::gfx::{Gfx, Surface};
use super::render;
use super::{post, Msg};
use ekspointer::app::App;
use ekspointer::geom::{Pt, RectF};
use ekspointer::input_filter::InputEvent;
use windows::core::{w, Result, BOOL};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFO};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
use windows::Win32::UI::Input::KeyboardAndMouse::{ReleaseCapture, SetCapture};
use windows::Win32::UI::WindowsAndMessaging::*;

const CLASS: windows::core::PCWSTR = w!("EkspointerOverlay");

/// Un écran et sa fenêtre de dessin.
struct Overlay {
    hwnd: HWND,
    surface: Surface,
    /// Rectangle de l'écran dans le bureau virtuel, en pixels physiques.
    rect: RectF,
    scale: f32,
    /// Rien n'a été dessiné à la dernière image : inutile de redessiner tant que rien n'apparaît.
    blank: bool,
}

/// Toutes les fenêtres de dessin.
pub struct Overlays {
    list: Vec<Overlay>,
    capture: bool,
}

unsafe extern "system" fn enum_proc(hmon: HMONITOR, _: HDC, _: *mut RECT, data: LPARAM) -> BOOL {
    let list = unsafe { &mut *(data.0 as *mut Vec<(RectF, f32)>) };
    let mut info = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
    if unsafe { GetMonitorInfoW(hmon, &mut info) }.as_bool() {
        let r = info.rcMonitor;
        let (mut dx, mut dy) = (96u32, 96u32);
        let _ = unsafe { GetDpiForMonitor(hmon, MDT_EFFECTIVE_DPI, &mut dx, &mut dy) };
        list.push((
            RectF { x: r.left as f32, y: r.top as f32, w: (r.right - r.left) as f32, h: (r.bottom - r.top) as f32 },
            dx as f32 / 96.0,
        ));
    }
    true.into()
}

/// Écrans branchés : rectangle (pixels physiques du bureau virtuel) et échelle.
pub fn monitors() -> Vec<(RectF, f32)> {
    let mut list: Vec<(RectF, f32)> = Vec::new();
    unsafe {
        let _ = EnumDisplayMonitors(None, None, Some(enum_proc), LPARAM(&mut list as *mut _ as isize));
    }
    list
}

fn screen_point(lparam_pos: u32) -> Pt {
    // GetMessagePos : coordonnées écran signées sur 16 bits.
    let x = (lparam_pos & 0xFFFF) as u16 as i16 as f32;
    let y = ((lparam_pos >> 16) & 0xFFFF) as u16 as i16 as f32;
    Pt::new(x, y)
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        match msg {
            WM_MOUSEACTIVATE => LRESULT(MA_NOACTIVATE as isize),
            WM_SETCURSOR => {
                if let Ok(cursor) = LoadCursorW(None, IDC_CROSS) {
                    SetCursor(Some(cursor));
                }
                LRESULT(1)
            }
            WM_LBUTTONDOWN => {
                SetCapture(hwnd);
                post(Msg::Input(InputEvent::StrokeBegin(screen_point(GetMessagePos()))));
                LRESULT(0)
            }
            WM_MOUSEMOVE => {
                if wparam.0 & 0x0001 != 0 {
                    post(Msg::Input(InputEvent::StrokeMove(screen_point(GetMessagePos()))));
                }
                LRESULT(0)
            }
            WM_LBUTTONUP => {
                let _ = ReleaseCapture();
                post(Msg::Input(InputEvent::StrokeEnd));
                LRESULT(0)
            }
            WM_CAPTURECHANGED => {
                post(Msg::Input(InputEvent::StrokeEnd));
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}

fn ex_style(capture: bool) -> WINDOW_EX_STYLE {
    let base = WS_EX_NOREDIRECTIONBITMAP | WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE;
    if capture {
        base
    } else {
        base | WS_EX_LAYERED | WS_EX_TRANSPARENT
    }
}

impl Overlays {
    pub fn new(gfx: &Gfx) -> Result<Overlays> {
        unsafe {
            let class = WNDCLASSW {
                lpfnWndProc: Some(wndproc),
                hInstance: GetModuleHandleW(None)?.into(),
                lpszClassName: CLASS,
                ..Default::default()
            };
            RegisterClassW(&class);
        }
        let mut overlays = Overlays { list: Vec::new(), capture: false };
        overlays.rebuild(gfx)?;
        Ok(overlays)
    }

    /// Recrée une fenêtre par écran branché.
    pub fn rebuild(&mut self, gfx: &Gfx) -> Result<()> {
        for o in self.list.drain(..) {
            unsafe {
                let _ = DestroyWindow(o.hwnd);
            }
        }
        for (rect, scale) in monitors() {
            unsafe {
                let hwnd = CreateWindowExW(
                    ex_style(self.capture),
                    CLASS,
                    w!("Ekspointer"),
                    WS_POPUP,
                    rect.x as i32,
                    rect.y as i32,
                    rect.w as i32,
                    rect.h as i32,
                    None,
                    None,
                    Some(GetModuleHandleW(None)?.into()),
                    None,
                )?;
                let surface = Surface::new(gfx, hwnd, rect.w as u32, rect.h as u32)?;
                let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
                self.list.push(Overlay { hwnd, surface, rect, scale, blank: false });
            }
        }
        Ok(())
    }

    /// Fait capter la souris aux fenêtres (mode dessin) ou la leur fait traverser.
    pub fn set_capture(&mut self, on: bool) {
        self.capture = on;
        for o in &self.list {
            unsafe {
                SetWindowLongPtrW(o.hwnd, GWL_EXSTYLE, ex_style(on).0 as isize);
                let _ = SetWindowPos(
                    o.hwnd,
                    Some(HWND_TOPMOST),
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_FRAMECHANGED,
                );
            }
        }
    }

    /// Fenêtres de dessin, pour replacer la barre d'outils au-dessus d'elles.
    pub fn hwnds(&self) -> Vec<HWND> {
        self.list.iter().map(|o| o.hwnd).collect()
    }

    /// Échelle de l'écran situé sous un point du bureau virtuel.
    pub fn scale_at(&self, p: Pt) -> f32 {
        self.list.iter().find(|o| o.rect.contains(p)).map(|o| o.scale).unwrap_or(1.0)
    }

    /// Rectangle et échelle de l'écran situé sous un point, ou de l'écran principal.
    pub fn monitor_at(&self, p: Pt) -> Option<(RectF, f32)> {
        self.list.iter().find(|o| o.rect.contains(p)).map(|o| (o.rect, o.scale))
    }

    /// Rectangle et échelle de l'écran principal (celui qui contient l'origine).
    pub fn primary(&self) -> (RectF, f32) {
        self.monitor_at(Pt::new(0.0, 0.0))
            .or_else(|| self.list.first().map(|o| (o.rect, o.scale)))
            .unwrap_or((RectF { x: 0.0, y: 0.0, w: 1920.0, h: 1080.0 }, 1.0))
    }

    /// Redessine les fenêtres qui ont quelque chose à montrer (ou à effacer).
    pub fn render(&mut self, gfx: &Gfx, app: &App, now: u64, animate: bool) -> Result<()> {
        let has_shapes = !app.scene.shapes().is_empty();
        let has_effects = !app.pointer.ripples().is_empty() || !app.pointer.trail(now).is_empty();
        let margin = app.cfg.halo_size as f32;
        for o in &mut self.list {
            let near = RectF {
                x: o.rect.x - margin,
                y: o.rect.y - margin,
                w: o.rect.w + 2.0 * margin,
                h: o.rect.h + 2.0 * margin,
            };
            let halo_here = app.cfg.halo_on && near.contains(app.pointer.pos());
            let content = has_shapes || has_effects || halo_here;
            if !content && o.blank {
                continue;
            }
            let origin = Pt::new(o.rect.x, o.rect.y);
            let scale = o.scale;
            o.surface.draw(gfx, 1.0, |p| {
                render::draw_scene(p, origin, app, now, animate);
                render::draw_pointer(p, origin, scale, app, now);
            })?;
            o.blank = !content;
        }
        Ok(())
    }
}

impl Drop for Overlays {
    fn drop(&mut self) {
        for o in self.list.drain(..) {
            unsafe {
                let _ = DestroyWindow(o.hwnd);
            }
        }
    }
}
