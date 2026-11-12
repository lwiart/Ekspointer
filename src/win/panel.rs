//! Fenêtre flottante générique de l'interface : barre d'outils, volets, bulle d'aide, réglages.
//!
//! Chaque panneau est fait de deux fenêtres : le contenu, qui reçoit la souris, et en dessous
//! une fenêtre d'ombre plus large, que la souris traverse.

use super::gfx::{Gfx, Painter, Surface};
use super::{post, Msg};
use ekspointer::geom::{Pt, RectF};
use ekspointer::ui::theme;
use windows::core::{w, Result, PCWSTR};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    ReleaseCapture, SetCapture, TrackMouseEvent, TME_LEAVE, TRACKMOUSEEVENT,
};
use windows::Win32::UI::WindowsAndMessaging::*;

const CLASS: PCWSTR = w!("EkspointerPanel");
const SHADOW_CLASS: PCWSTR = w!("EkspointerShadow");
const WM_MOUSELEAVE: u32 = 0x02A3;

/// Marge de la fenêtre d'ombre autour du contenu, en pixels logiques.
const SHADOW_MARGIN: f32 = 48.0;

/// Identifiant d'un panneau, transporté par ses événements.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PanelId {
    Toolbar,
    Popover,
    Tooltip,
    Settings,
}

impl PanelId {
    fn from_raw(v: isize) -> Option<PanelId> {
        Some(match v {
            1 => PanelId::Toolbar,
            2 => PanelId::Popover,
            3 => PanelId::Tooltip,
            4 => PanelId::Settings,
            _ => return None,
        })
    }

    fn raw(self) -> isize {
        match self {
            PanelId::Toolbar => 1,
            PanelId::Popover => 2,
            PanelId::Tooltip => 3,
            PanelId::Settings => 4,
        }
    }
}

/// Événement de souris d'un panneau. Les points sont en pixels physiques, relatifs au contenu.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum PanelEvent {
    Move(Pt),
    Leave,
    Down(Pt),
    Up(Pt),
    DoubleClick(Pt),
    /// Crans de molette, positifs vers le haut.
    Wheel(f32),
}

fn client_point(lparam: LPARAM) -> Pt {
    let x = (lparam.0 & 0xFFFF) as u16 as i16 as f32;
    let y = ((lparam.0 >> 16) & 0xFFFF) as u16 as i16 as f32;
    Pt::new(x, y)
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        let Some(id) = PanelId::from_raw(GetWindowLongPtrW(hwnd, GWLP_USERDATA)) else {
            return DefWindowProcW(hwnd, msg, wparam, lparam);
        };
        let send = |ev: PanelEvent| post(Msg::Panel(id, ev));
        match msg {
            WM_MOUSEACTIVATE => LRESULT(MA_NOACTIVATE as isize),
            WM_MOUSEMOVE => {
                let mut track = TRACKMOUSEEVENT {
                    cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                    dwFlags: TME_LEAVE,
                    hwndTrack: hwnd,
                    dwHoverTime: 0,
                };
                let _ = TrackMouseEvent(&mut track);
                send(PanelEvent::Move(client_point(lparam)));
                LRESULT(0)
            }
            WM_MOUSELEAVE => {
                send(PanelEvent::Leave);
                LRESULT(0)
            }
            WM_LBUTTONDOWN => {
                SetCapture(hwnd);
                send(PanelEvent::Down(client_point(lparam)));
                LRESULT(0)
            }
            WM_LBUTTONUP => {
                let _ = ReleaseCapture();
                send(PanelEvent::Up(client_point(lparam)));
                LRESULT(0)
            }
            WM_LBUTTONDBLCLK => {
                send(PanelEvent::DoubleClick(client_point(lparam)));
                LRESULT(0)
            }
            WM_MOUSEWHEEL => {
                let delta = ((wparam.0 >> 16) & 0xFFFF) as u16 as i16 as f32 / 120.0;
                send(PanelEvent::Wheel(delta));
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}

unsafe extern "system" fn shadow_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}

/// Enregistre les classes de fenêtre des panneaux. À appeler une fois.
pub fn register() -> Result<()> {
    unsafe {
        let instance = GetModuleHandleW(None)?;
        let class = WNDCLASSW {
            style: CS_DBLCLKS,
            lpfnWndProc: Some(wndproc),
            hInstance: instance.into(),
            hCursor: LoadCursorW(None, IDC_ARROW)?,
            lpszClassName: CLASS,
            ..Default::default()
        };
        RegisterClassW(&class);
        let shadow = WNDCLASSW {
            lpfnWndProc: Some(shadow_proc),
            hInstance: instance.into(),
            lpszClassName: SHADOW_CLASS,
            ..Default::default()
        };
        RegisterClassW(&shadow);
    }
    Ok(())
}

fn create(class: PCWSTR, ex: WINDOW_EX_STYLE) -> Result<HWND> {
    unsafe {
        CreateWindowExW(
            ex,
            class,
            w!("Ekspointer"),
            WS_POPUP,
            0,
            0,
            1,
            1,
            None,
            None,
            Some(GetModuleHandleW(None)?.into()),
            None,
        )
    }
}

/// Un panneau flottant.
pub struct Panel {
    hwnd: HWND,
    shadow: HWND,
    surface: Surface,
    shadow_surface: Surface,
    /// Échelle de l'écran où se trouve le panneau.
    pub scale: f32,
    /// Taille du contenu en pixels logiques.
    pub size: (f32, f32),
    /// Coin haut-gauche du contenu, en pixels physiques du bureau virtuel.
    pub pos: (i32, i32),
    /// Rayon des coins, en pixels logiques.
    radius: f32,
    visible: bool,
}

impl Panel {
    pub fn new(gfx: &Gfx, id: PanelId, size: (f32, f32), radius: f32) -> Result<Panel> {
        let base = WS_EX_NOREDIRECTIONBITMAP | WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE;
        let shadow = create(SHADOW_CLASS, base | WS_EX_LAYERED | WS_EX_TRANSPARENT)?;
        let hwnd = create(CLASS, base)?;
        unsafe {
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, id.raw());
        }
        let surface = Surface::new(gfx, hwnd, 1, 1)?;
        let shadow_surface = Surface::new(gfx, shadow, 1, 1)?;
        Ok(Panel { hwnd, shadow, surface, shadow_surface, scale: 1.0, size, pos: (0, 0), radius, visible: false })
    }

    pub fn visible(&self) -> bool {
        self.visible
    }

    /// Rectangle du contenu dans le bureau virtuel, en pixels physiques.
    pub fn rect(&self) -> RectF {
        RectF { x: self.pos.0 as f32, y: self.pos.1 as f32, w: self.size.0 * self.scale, h: self.size.1 * self.scale }
    }

    /// Convertit un point d'événement (pixels physiques du contenu) en pixels logiques.
    pub fn to_dip(&self, p: Pt) -> Pt {
        Pt::new(p.x / self.scale, p.y / self.scale)
    }

    /// Place, dimensionne et affiche le panneau, puis redessine son ombre.
    pub fn place(&mut self, gfx: &Gfx, pos: (i32, i32), size: (f32, f32), radius: f32, scale: f32) -> Result<()> {
        self.pos = pos;
        self.size = size;
        self.radius = radius;
        self.scale = scale;
        let (w, h) = ((size.0 * scale).ceil() as i32, (size.1 * scale).ceil() as i32);
        let m = (SHADOW_MARGIN * scale).ceil() as i32;
        let flags = SWP_NOACTIVATE | SWP_SHOWWINDOW;
        unsafe {
            let _ = SetWindowPos(self.shadow, Some(HWND_TOPMOST), pos.0 - m, pos.1 - m, w + 2 * m, h + 2 * m, flags);
            let _ = SetWindowPos(self.hwnd, Some(HWND_TOPMOST), pos.0, pos.1, w, h, flags);
        }
        self.surface.resize(w as u32, h as u32)?;
        self.shadow_surface.resize((w + 2 * m) as u32, (h + 2 * m) as u32)?;
        self.visible = true;
        self.draw_shadow(gfx)
    }

    /// Déplace le panneau sans le redimensionner (glissement de la barre).
    pub fn move_to(&mut self, pos: (i32, i32)) {
        self.pos = pos;
        let m = (SHADOW_MARGIN * self.scale).ceil() as i32;
        let flags = SWP_NOACTIVATE | SWP_NOSIZE | SWP_NOZORDER;
        unsafe {
            let _ = SetWindowPos(self.shadow, None, pos.0 - m, pos.1 - m, 0, 0, flags);
            let _ = SetWindowPos(self.hwnd, None, pos.0, pos.1, 0, 0, flags);
        }
    }

    pub fn hide(&mut self) {
        if !self.visible {
            return;
        }
        self.visible = false;
        unsafe {
            let _ = ShowWindow(self.hwnd, SW_HIDE);
            let _ = ShowWindow(self.shadow, SW_HIDE);
        }
    }

    /// Replace le panneau au-dessus des autres fenêtres toujours visibles (dont les fenêtres de dessin).
    pub fn raise(&self) {
        if !self.visible {
            return;
        }
        let flags = SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE;
        unsafe {
            let _ = SetWindowPos(self.shadow, Some(HWND_TOPMOST), 0, 0, 0, 0, flags);
            let _ = SetWindowPos(self.hwnd, Some(HWND_TOPMOST), 0, 0, 0, 0, flags);
        }
    }

    /// Exclut (ou non) le panneau des captures et du partage d'écran.
    pub fn set_capture_hidden(&self, hidden: bool) {
        let affinity = if hidden { WDA_EXCLUDEFROMCAPTURE } else { WDA_NONE };
        unsafe {
            let _ = SetWindowDisplayAffinity(self.hwnd, affinity);
            let _ = SetWindowDisplayAffinity(self.shadow, affinity);
        }
    }

    /// Dessine le contenu, en pixels logiques.
    pub fn draw(&mut self, gfx: &Gfx, f: impl FnOnce(&Painter)) -> Result<()> {
        if !self.visible {
            return Ok(());
        }
        self.surface.draw(gfx, self.scale, f)
    }

    /// Ombre « menu » du design system : `0 20px 44px -20px rgba(0,60,88,.45)`, approchée
    /// par des rectangles arrondis concentriques translucides.
    fn draw_shadow(&mut self, gfx: &Gfx) -> Result<()> {
        let (w, h) = self.size;
        let radius = self.radius.min(h / 2.0);
        const LAYERS: i32 = 22;
        let per_layer = 1.0 - (1.0f32 - 0.45).powf(1.0 / LAYERS as f32);
        self.shadow_surface.draw(gfx, self.scale, |p| {
            for i in 0..LAYERS {
                // De -22 à +22 autour du rectangle d'ombre (contenu rétréci de 20, descendu de 20).
                let d = -22.0 + 44.0 * (i as f32 + 0.5) / LAYERS as f32;
                let inset = 20.0 - d;
                let (rw, rh) = ((w - 2.0 * inset).max(1.0), (h - 2.0 * inset).max(1.0));
                let r = RectF {
                    x: SHADOW_MARGIN + (w - rw) / 2.0,
                    y: SHADOW_MARGIN + (h - rh) / 2.0 + 20.0,
                    w: rw,
                    h: rh,
                };
                p.fill_rrect(r, (radius - inset).max(2.0), theme::INK.with_alpha(per_layer));
            }
        })
    }
}

impl Drop for Panel {
    fn drop(&mut self) {
        unsafe {
            let _ = DestroyWindow(self.hwnd);
            let _ = DestroyWindow(self.shadow);
        }
    }
}
