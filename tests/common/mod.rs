//! Banc d'essai des tests d'intégration : assemble le filtre d'entrées et l'application
//! comme le fait `main.rs`, sans fenêtre ni écoute système.
#![allow(dead_code)]

use ekspointer::app::{App, Effect, UiCommand};
use ekspointer::config::Config;
use ekspointer::geom::Pt;
use ekspointer::input_filter::{Filter, InputEvent, MouseKind};
use ekspointer::shapes::{Phase, ShapeKind};

pub const LCTRL: u32 = 0xA2;
pub const LALT: u32 = 0xA4;
pub const RALT: u32 = 0xA5;
pub const LSHIFT: u32 = 0xA0;
pub const ESC: u32 = 0x1B;

pub struct Rig {
    pub filter: Filter,
    pub app: App,
    pub now: u64,
    /// Tous les effets émis depuis le début, dans l'ordre.
    pub effects: Vec<Effect>,
    /// Événements de saisie de raccourci reçus (réglages).
    pub captured: Vec<InputEvent>,
}

impl Rig {
    pub fn new() -> Rig {
        Rig::with_config(Config::default())
    }

    pub fn with_config(cfg: Config) -> Rig {
        let app = App::new(cfg);
        let filter = Filter::new(app.cfg.bindings.clone());
        Rig { filter, app, now: 0, effects: Vec::new(), captured: Vec::new() }
    }

    fn apply(&mut self, effects: Vec<Effect>) {
        for e in &effects {
            if *e == Effect::FilterChanged {
                self.filter.set_draw_mode(self.app.draw_mode);
                self.filter.set_bindings(self.app.cfg.bindings.clone());
            }
        }
        self.effects.extend(effects);
    }

    /// Transmet des événements à l'application, comme la boucle principale.
    pub fn feed(&mut self, events: Vec<InputEvent>) {
        for ev in events {
            if matches!(ev, InputEvent::Captured(_) | InputEvent::CaptureCancelled) {
                self.captured.push(ev);
            }
            let effects = self.app.input(ev, self.now, &|_| 1.0);
            self.apply(effects);
        }
    }

    /// Appui ou relâchement d'une touche. Renvoie vrai si la touche est interceptée.
    pub fn key(&mut self, vk: u32, down: bool) -> bool {
        let d = self.filter.on_key(vk, down);
        self.feed(d.events);
        d.swallow
    }

    /// Appui puis relâchement d'une touche. Renvoie vrai si l'appui est intercepté.
    pub fn tap(&mut self, vk: u32) -> bool {
        let swallowed = self.key(vk, true);
        self.key(vk, false);
        swallowed
    }

    /// Enfonce les touches dans l'ordre puis les relâche dans l'ordre inverse.
    pub fn chord(&mut self, vks: &[u32]) {
        for vk in vks {
            self.key(*vk, true);
        }
        for vk in vks.iter().rev() {
            self.key(*vk, false);
        }
    }

    /// Événement de souris vu par l'écoute système. Renvoie vrai s'il est intercepté.
    pub fn mouse(&mut self, kind: MouseKind, x: f32, y: f32) -> bool {
        let d = self.filter.on_mouse(kind, Pt::new(x, y));
        self.feed(d.events);
        d.swallow
    }

    /// Tracé reçu par la fenêtre de dessin en mode bascule.
    pub fn overlay_stroke(&mut self, x0: f32, y0: f32, x1: f32, y1: f32) {
        self.feed(vec![
            InputEvent::StrokeBegin(Pt::new(x0, y0)),
            InputEvent::StrokeMove(Pt::new(x1, y1)),
            InputEvent::StrokeEnd,
        ]);
    }

    pub fn command(&mut self, cmd: UiCommand) {
        let effects = self.app.command(cmd, self.now);
        self.apply(effects);
    }

    /// Avance le temps. Renvoie vrai si une animation est en cours.
    pub fn advance(&mut self, ms: u64) -> bool {
        self.now += ms;
        self.app.tick(self.now)
    }

    pub fn shape_count(&self) -> usize {
        self.app.scene.shapes().len()
    }

    pub fn kind(&self, i: usize) -> &ShapeKind {
        &self.app.scene.shapes()[i].kind
    }

    pub fn phase(&self, i: usize) -> Phase {
        let s = &self.app.scene.shapes()[i];
        self.app.scene.phase(s, self.now)
    }
}
