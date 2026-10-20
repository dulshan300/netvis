//! Debug builds only: `NETVIS_SCREENSHOT=out.ppm netvis file.netvis` renders a
//! few frames, saves a screenshot and exits. Used for visual regression checks.

use std::path::PathBuf;

pub struct Capture {
    path: PathBuf,
    frame: u32,
    requested: bool,
}

impl Capture {
    pub fn from_env() -> Option<Self> {
        std::env::var_os("NETVIS_SCREENSHOT").map(|p| Self { path: p.into(), frame: 0, requested: false })
    }

    pub fn tick(&mut self, ctx: &egui::Context) {
        self.frame += 1;
        ctx.request_repaint();
        if self.frame == 20 && !self.requested {
            self.requested = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
        }
        let shot = ctx.input(|i| {
            i.raw.events.iter().find_map(|e| match e {
                egui::Event::Screenshot { image, .. } => Some(image.clone()),
                _ => None,
            })
        });
        if let Some(img) = shot {
            let [w, h] = img.size;
            let mut out = format!("P6\n{w} {h}\n255\n").into_bytes();
            for px in &img.pixels {
                out.extend_from_slice(&[px.r(), px.g(), px.b()]);
            }
            if let Err(e) = std::fs::write(&self.path, out) {
                eprintln!("screenshot failed: {e}");
            }
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
}
