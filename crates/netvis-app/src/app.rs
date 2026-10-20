use std::path::{Path, PathBuf};

use egui::{Key, KeyboardShortcut, Modifiers, RichText, ViewportCommand};
use egui_dock::{DockArea, DockState, NodeIndex};
use netvis_core::{Project, io};
use serde::{Deserialize, Serialize};

use crate::canvas;
use crate::file_dialog::{self, PathPurpose};
use crate::icons;
use crate::panels;
use crate::state::{Action, Document, Settings, State};
use crate::theme::{self, APP_NAME, BRAND_NAME, ThemeChoice};

const SETTINGS_KEY: &str = "netvis.settings";
const DOCK_KEY: &str = "netvis.dock.v2";
const AUTOSAVE_SECS: f64 = 90.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Tab {
    Toolbox,
    Outline,
    Canvas,
    Inspector,
    Issues,
    IpPlan,
    Vlans,
}

impl Tab {
    fn title(self) -> String {
        let (icon, name) = match self {
            Tab::Toolbox => (icons::TOOLBOX, "Toolbox"),
            Tab::Outline => (icons::TREE_VIEW, "Outline"),
            Tab::Canvas => (icons::MAP_TRIFOLD, "Topology"),
            Tab::Inspector => (icons::SLIDERS_HORIZONTAL, "Inspector"),
            Tab::Issues => (icons::LIST_CHECKS, "Issues"),
            Tab::IpPlan => (icons::TREE_STRUCTURE, "IP Plan"),
            Tab::Vlans => (icons::TAG, "VLANs & Wi-Fi"),
        };
        format!("{icon}  {name}")
    }
}

fn default_layout() -> DockState<Tab> {
    let mut dock = DockState::new(vec![Tab::Canvas]);
    let surface = dock.main_surface_mut();
    let [canvas, _inspector] = surface.split_right(NodeIndex::root(), 0.77, vec![Tab::Inspector]);
    let [canvas, _left] = surface.split_left(canvas, 0.22, vec![Tab::Toolbox, Tab::Outline]);
    surface.split_below(canvas, 0.68, vec![Tab::IpPlan, Tab::Vlans, Tab::Issues]);
    dock
}

/// What to do once the user has dealt with unsaved changes.
#[derive(Debug, Clone)]
enum Pending {
    New,
    Open,
    OpenPath(PathBuf),
    Quit,
}

enum Dialog {
    Unsaved { then: Pending },
    PathPrompt { purpose: PathPurpose, text: String, then: Option<Pending> },
    Recover { path: PathBuf },
    Error(String),
    About,
}

/// Deferred work triggered by a dialog button, run once the dialog is closed.
type AfterDialog = Box<dyn FnOnce(&mut NetVisApp)>;

pub struct NetVisApp {
    st: State,
    dock: DockState<Tab>,
    dialog: Option<Dialog>,
    allow_close: bool,
    applied_theme: Option<ThemeChoice>,
    title: String,
    autosave_path: Option<PathBuf>,
    last_autosave: (f64, u64),
    #[cfg(debug_assertions)]
    capture: Option<crate::debug_capture::Capture>,
}

impl NetVisApp {
    pub fn new(cc: &eframe::CreationContext<'_>, open: Option<PathBuf>) -> Self {
        let settings: Settings = cc.storage.and_then(|s| eframe::get_value(s, SETTINGS_KEY)).unwrap_or_default();
        let dock = cc.storage.and_then(|s| eframe::get_value(s, DOCK_KEY)).unwrap_or_else(default_layout);
        theme::install(&cc.egui_ctx, settings.theme);

        let autosave_path = directories::ProjectDirs::from("com", BRAND_NAME, APP_NAME)
            .map(|d| d.data_dir().join(format!("autosave.{}", io::FILE_EXTENSION)));

        let mut app = Self {
            st: State::new(settings),
            dock,
            dialog: None,
            allow_close: false,
            applied_theme: None,
            title: String::new(),
            autosave_path,
            last_autosave: (0.0, 0),
            #[cfg(debug_assertions)]
            capture: crate::debug_capture::Capture::from_env(),
        };
        if let Some(path) = open {
            app.open_path(&path);
        } else if let Some(p) = app.autosave_path.clone().filter(|p| p.exists()) {
            app.dialog = Some(Dialog::Recover { path: p });
        }
        #[cfg(debug_assertions)]
        app.apply_debug_env(&cc.egui_ctx);
        app
    }

    /// Debug-only overrides used for scripted visual checks.
    #[cfg(debug_assertions)]
    fn apply_debug_env(&mut self, ctx: &egui::Context) {
        match std::env::var("NETVIS_THEME").as_deref() {
            Ok("dark") => self.st.settings.theme = ThemeChoice::Dark,
            Ok("light") => self.st.settings.theme = ThemeChoice::Light,
            _ => {}
        }
        if let Ok(names) = std::env::var("NETVIS_SELECT") {
            use crate::state::Item;
            let p = &self.st.doc.project;
            for n in names.split(',') {
                let item = p
                    .devices
                    .iter()
                    .find(|d| d.name == n)
                    .map(|d| Item::Device(d.id))
                    .or_else(|| p.subnets.iter().find(|s| s.name == n).map(|s| Item::Subnet(s.id)))
                    .or_else(|| p.vlans.iter().find(|v| v.name == n).map(|v| Item::Vlan(v.id)));
                if let Some(item) = item {
                    self.st.sel.add(item);
                }
            }
        }
        if let Ok(tab) = std::env::var("NETVIS_DEVTAB") {
            panels::debug_set_device_tab(ctx, &tab);
        }
        if let Ok(tab) = std::env::var("NETVIS_BOTTOM") {
            let want = match tab.as_str() {
                "vlans" => Some(Tab::Vlans),
                "issues" => Some(Tab::Issues),
                _ => None,
            };
            if let Some(t) = want
                && let Some(path) = self.dock.find_tab(&t)
            {
                let _ = self.dock.set_active_tab(path);
            }
        }
    }

    // ---- file operations ----------------------------------------------

    fn guard_unsaved(&mut self, then: Pending) {
        if self.st.doc.is_dirty() {
            self.dialog = Some(Dialog::Unsaved { then });
        } else {
            self.run_pending(then);
        }
    }

    fn run_pending(&mut self, p: Pending) {
        match p {
            Pending::New => {
                self.st.replace_document(Document::new(Project::default(), None));
                self.remove_autosave();
                self.st.set_status("New project");
            }
            Pending::Open => self.request_path(PathPurpose::Open, None),
            Pending::OpenPath(path) => self.open_path(&path),
            Pending::Quit => {
                self.allow_close = true;
                self.st.actions.push(Action::Quit);
            }
        }
    }

    fn request_path(&mut self, purpose: PathPurpose, then: Option<Pending>) {
        let stem = file_dialog::file_stem_for(&self.st.doc.project.meta.name);
        let suggested = format!("{stem}.{}", io::FILE_EXTENSION);
        match file_dialog::native(purpose, &suggested) {
            Some(Some(path)) => self.on_path_chosen(purpose, path, then),
            Some(None) => {}
            None => {
                let base = self
                    .st
                    .doc
                    .path
                    .clone()
                    .unwrap_or_else(|| file_dialog::default_dir().join(&suggested));
                let text = match purpose {
                    PathPurpose::SaveAs => base.display().to_string(),
                    PathPurpose::Open => base.parent().map(|p| p.display().to_string() + "/").unwrap_or_default(),
                };
                self.dialog = Some(Dialog::PathPrompt { purpose, text, then });
            }
        }
    }

    fn on_path_chosen(&mut self, purpose: PathPurpose, path: PathBuf, then: Option<Pending>) {
        match purpose {
            PathPurpose::Open => self.open_path(&path),
            PathPurpose::SaveAs => {
                if self.save_to(file_dialog::with_extension(path))
                    && let Some(next) = then
                {
                    self.run_pending(next);
                }
            }
        }
    }

    fn open_path(&mut self, path: &Path) {
        match io::load(path) {
            Ok(project) => {
                self.st.replace_document(Document::new(project, Some(path.to_path_buf())));
                self.st.settings.push_recent(path.to_path_buf());
                self.remove_autosave();
                self.st.set_status(format!("Opened {}", path.display()));
            }
            Err(e) => {
                self.st.settings.recent.retain(|p| p != path);
                self.dialog = Some(Dialog::Error(format!("Could not open {}:\n{e}", path.display())));
            }
        }
    }

    /// Save to the current path, prompting for one if needed. Runs `then` on success.
    fn save(&mut self, then: Option<Pending>) {
        match self.st.doc.path.clone() {
            Some(path) => {
                if self.save_to(path)
                    && let Some(next) = then
                {
                    self.run_pending(next);
                }
            }
            None => self.request_path(PathPurpose::SaveAs, then),
        }
    }

    fn save_to(&mut self, path: PathBuf) -> bool {
        match io::save(&self.st.doc.project, &path) {
            Ok(()) => {
                self.st.set_status(format!("Saved {}", path.display()));
                self.st.settings.push_recent(path.clone());
                self.st.doc.mark_saved(path);
                self.remove_autosave();
                true
            }
            Err(e) => {
                self.dialog = Some(Dialog::Error(format!("Could not save {}:\n{e}", path.display())));
                false
            }
        }
    }

    fn autosave(&mut self) {
        let rev = self.st.doc.history.revision();
        if !self.st.doc.is_dirty() || rev == self.last_autosave.1 || self.st.now - self.last_autosave.0 < AUTOSAVE_SECS {
            return;
        }
        self.last_autosave = (self.st.now, rev);
        let Some(path) = &self.autosave_path else { return };
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if io::save(&self.st.doc.project, path).is_ok() {
            self.st.set_status("Autosaved");
        }
    }

    fn remove_autosave(&mut self) {
        if let Some(p) = &self.autosave_path {
            let _ = std::fs::remove_file(p);
        }
    }

    fn handle_action(&mut self, ctx: &egui::Context, a: Action) {
        match a {
            Action::New => self.guard_unsaved(Pending::New),
            Action::Open => self.guard_unsaved(Pending::Open),
            Action::OpenPath(p) => self.guard_unsaved(Pending::OpenPath(p)),
            Action::Save => self.save(None),
            Action::SaveAs => self.request_path(PathPurpose::SaveAs, None),
            Action::Quit => {
                if self.allow_close || !self.st.doc.is_dirty() {
                    self.allow_close = true;
                    ctx.send_viewport_cmd(ViewportCommand::Close);
                } else {
                    self.dialog = Some(Dialog::Unsaved { then: Pending::Quit });
                }
            }
            Action::About => self.dialog = Some(Dialog::About),
            Action::ResetLayout => self.dock = default_layout(),
        }
    }

    // ---- input ---------------------------------------------------------

    fn shortcuts(&mut self, ctx: &egui::Context) {
        if self.dialog.is_some() || self.st.vlsm.is_some() {
            return;
        }
        let typing = ctx.egui_wants_keyboard_input();
        let cmd = Modifiers::COMMAND;
        let cmd_shift = Modifiers::COMMAND | Modifiers::SHIFT;
        let sc = |m, k| KeyboardShortcut::new(m, k);
        let fire = |s: KeyboardShortcut| ctx.input_mut(|i| i.consume_shortcut(&s));

        if fire(sc(cmd, Key::N)) {
            self.st.actions.push(Action::New);
        }
        if fire(sc(cmd, Key::O)) {
            self.st.actions.push(Action::Open);
        }
        if fire(sc(cmd_shift, Key::S)) {
            self.st.actions.push(Action::SaveAs);
        } else if fire(sc(cmd, Key::S)) {
            self.st.actions.push(Action::Save);
        }
        if fire(sc(cmd, Key::Num0)) {
            self.st.canvas.request_fit();
        }
        if fire(sc(cmd, Key::Num1)) {
            self.st.canvas.reset_zoom();
        }
        if fire(sc(cmd, Key::Equals)) || fire(sc(cmd, Key::Plus)) {
            self.st.canvas.zoom_step(1.2);
        }
        if fire(sc(cmd, Key::Minus)) {
            self.st.canvas.zoom_step(1.0 / 1.2);
        }
        if typing {
            return;
        }
        if fire(sc(cmd_shift, Key::Z)) || fire(sc(cmd, Key::Y)) {
            self.st.redo();
        } else if fire(sc(cmd, Key::Z)) {
            self.st.undo();
        }
        if fire(sc(cmd, Key::A)) {
            self.st.select_all();
        }
        if fire(sc(cmd, Key::D)) {
            self.st.duplicate_selection();
        }
        if fire(sc(Modifiers::NONE, Key::Delete)) || fire(sc(Modifiers::NONE, Key::Backspace)) {
            self.st.delete_selection();
        }
        if fire(sc(Modifiers::NONE, Key::Escape)) {
            self.st.sel.clear();
        }
        for (key, dx, dy) in [(Key::ArrowLeft, -1.0, 0.0), (Key::ArrowRight, 1.0, 0.0), (Key::ArrowUp, 0.0, -1.0), (Key::ArrowDown, 0.0, 1.0)] {
            let step = if fire(sc(Modifiers::SHIFT, key)) {
                1.0
            } else if fire(sc(Modifiers::NONE, key)) {
                canvas::GRID
            } else {
                continue;
            };
            self.st.nudge_selection(dx * step, dy * step);
        }
    }

    // ---- chrome --------------------------------------------------------

    fn menu_bar(&mut self, ui: &mut egui::Ui) {
        egui::MenuBar::new().ui(ui, |ui| {
            let st = &mut self.st;
            ui.menu_button("File", |ui| {
                if ui.button(format!("{}  New            Ctrl+N", icons::FILE_PLUS)).clicked() {
                    st.actions.push(Action::New);
                }
                if ui.button(format!("{}  Open…          Ctrl+O", icons::FOLDER_OPEN)).clicked() {
                    st.actions.push(Action::Open);
                }
                ui.menu_button(format!("{}  Open recent", icons::CLOCK_COUNTER_CLOCKWISE), |ui| {
                    if st.settings.recent.is_empty() {
                        ui.label(RichText::new("No recent projects").weak());
                    }
                    for p in st.settings.recent.clone() {
                        let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                        if ui.button(name).on_hover_text(p.display().to_string()).clicked() {
                            st.actions.push(Action::OpenPath(p));
                        }
                    }
                    if !st.settings.recent.is_empty() {
                        ui.separator();
                        if ui.button("Clear list").clicked() {
                            st.settings.recent.clear();
                        }
                    }
                });
                ui.separator();
                if ui.button(format!("{}  Save           Ctrl+S", icons::FLOPPY_DISK)).clicked() {
                    st.actions.push(Action::Save);
                }
                if ui.button(format!("{}  Save as…       Ctrl+Shift+S", icons::FLOPPY_DISK_BACK)).clicked() {
                    st.actions.push(Action::SaveAs);
                }
                ui.separator();
                if ui.button(format!("{}  Exit", icons::SIGN_OUT)).clicked() {
                    st.actions.push(Action::Quit);
                }
            });
            ui.menu_button("Edit", |ui| {
                let undo = st.doc.history.undo_label().map(|l| format!("Undo {l}")).unwrap_or("Undo".into());
                let redo = st.doc.history.redo_label().map(|l| format!("Redo {l}")).unwrap_or("Redo".into());
                if ui
                    .add_enabled(st.doc.history.undo_label().is_some(), egui::Button::new(format!("{}  {undo}", icons::ARROW_COUNTER_CLOCKWISE)).shortcut_text("Ctrl+Z"))
                    .clicked()
                {
                    st.undo();
                }
                if ui
                    .add_enabled(st.doc.history.redo_label().is_some(), egui::Button::new(format!("{}  {redo}", icons::ARROW_CLOCKWISE)).shortcut_text("Ctrl+Y"))
                    .clicked()
                {
                    st.redo();
                }
                ui.separator();
                if ui.add(egui::Button::new(format!("{}  Select all", icons::SELECTION)).shortcut_text("Ctrl+A")).clicked() {
                    st.select_all();
                }
                let has_dev = !st.sel.devices.is_empty();
                if ui.add_enabled(has_dev, egui::Button::new(format!("{}  Duplicate", icons::COPY)).shortcut_text("Ctrl+D")).clicked() {
                    st.duplicate_selection();
                }
                if ui.add_enabled(!st.sel.is_empty(), egui::Button::new(format!("{}  Delete", icons::TRASH)).shortcut_text("Del")).clicked() {
                    st.delete_selection();
                }
            });
            ui.menu_button("View", |ui| {
                if ui.add(egui::Button::new(format!("{}  Zoom to fit", icons::CORNERS_OUT)).shortcut_text("Ctrl+0")).clicked() {
                    st.canvas.request_fit();
                }
                if ui.add(egui::Button::new(format!("{}  Actual size", icons::MAGNIFYING_GLASS)).shortcut_text("Ctrl+1")).clicked() {
                    st.canvas.reset_zoom();
                }
                ui.separator();
                ui.checkbox(&mut st.settings.show_grid, "Show grid");
                ui.checkbox(&mut st.settings.snap_to_grid, "Snap to grid");
                ui.checkbox(&mut st.settings.show_minimap, "Show minimap");
                ui.checkbox(&mut st.settings.show_addresses, "Show IP addresses on devices");
                ui.separator();
                ui.menu_button(format!("{}  Theme", icons::PALETTE), |ui| {
                    for t in [ThemeChoice::System, ThemeChoice::Light, ThemeChoice::Dark] {
                        ui.radio_value(&mut st.settings.theme, t, t.label());
                    }
                });
                if ui.button(format!("{}  Reset panel layout", icons::LAYOUT)).clicked() {
                    st.actions.push(Action::ResetLayout);
                }
            });
            ui.menu_button("Help", |ui| {
                if ui.button(format!("{}  Keyboard & mouse", icons::KEYBOARD)).clicked() {
                    st.actions.push(Action::About);
                }
                if ui.button(format!("{}  About {APP_NAME}", icons::INFO)).clicked() {
                    st.actions.push(Action::About);
                }
            });
        });
    }

    fn toolbar(&mut self, ui: &mut egui::Ui) {
        let st = &mut self.st;
        ui.horizontal(|ui| {
            let icon_btn = |ui: &mut egui::Ui, icon: &str, tip: &str, enabled: bool| {
                ui.add_enabled(enabled, egui::Button::new(RichText::new(icon).size(16.0)).frame(false))
                    .on_hover_text(tip)
                    .clicked()
            };
            if icon_btn(ui, icons::FILE_PLUS, "New project (Ctrl+N)", true) {
                st.actions.push(Action::New);
            }
            if icon_btn(ui, icons::FOLDER_OPEN, "Open (Ctrl+O)", true) {
                st.actions.push(Action::Open);
            }
            if icon_btn(ui, icons::FLOPPY_DISK, "Save (Ctrl+S)", true) {
                st.actions.push(Action::Save);
            }
            ui.separator();
            if icon_btn(ui, icons::ARROW_COUNTER_CLOCKWISE, "Undo (Ctrl+Z)", st.doc.history.undo_label().is_some()) {
                st.undo();
            }
            if icon_btn(ui, icons::ARROW_CLOCKWISE, "Redo (Ctrl+Y)", st.doc.history.redo_label().is_some()) {
                st.redo();
            }
            ui.separator();
            if icon_btn(ui, icons::COPY, "Duplicate (Ctrl+D)", !st.sel.devices.is_empty()) {
                st.duplicate_selection();
            }
            if icon_btn(ui, icons::TRASH, "Delete (Del)", !st.sel.is_empty()) {
                st.delete_selection();
            }
            ui.separator();
            if icon_btn(ui, icons::MAGNIFYING_GLASS_MINUS, "Zoom out (Ctrl+-)", true) {
                st.canvas.zoom_step(1.0 / 1.2);
            }
            if ui
                .add(egui::Button::new(format!("{:>3.0}%", st.canvas.camera.zoom * 100.0)).frame(false))
                .on_hover_text("Reset to 100% (Ctrl+1)")
                .clicked()
            {
                st.canvas.reset_zoom();
            }
            if icon_btn(ui, icons::MAGNIFYING_GLASS_PLUS, "Zoom in (Ctrl+=)", true) {
                st.canvas.zoom_step(1.2);
            }
            if icon_btn(ui, icons::CORNERS_OUT, "Zoom to fit (Ctrl+0)", true) {
                st.canvas.request_fit();
            }
            ui.separator();
            ui.toggle_value(&mut st.settings.show_grid, RichText::new(icons::GRID_FOUR).size(16.0)).on_hover_text("Show grid");
            ui.toggle_value(&mut st.settings.snap_to_grid, RichText::new(icons::MAGNET).size(16.0)).on_hover_text("Snap to grid");
            ui.toggle_value(&mut st.settings.show_minimap, RichText::new(icons::MAP_TRIFOLD).size(16.0)).on_hover_text("Minimap");

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let dark = ui.visuals().dark_mode;
                let (icon, next) = if dark { (icons::SUN, ThemeChoice::Light) } else { (icons::MOON, ThemeChoice::Dark) };
                if icon_btn(ui, icon, "Toggle light/dark theme", true) {
                    st.settings.theme = next;
                }
                ui.label(RichText::new(format!("{APP_NAME} · {BRAND_NAME}")).weak().small());
            });
        });
    }

    fn status_bar(&mut self, ui: &mut egui::Ui) {
        let st = &self.st;
        ui.horizontal(|ui| {
            let msg = st.status().map(str::to_owned).unwrap_or_else(|| {
                "Drag from a port to another port to connect · Middle-drag or Space+drag to pan · Wheel to zoom".into()
            });
            ui.label(RichText::new(msg).small().weak());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let p = &st.doc.project;
                ui.label(RichText::new(format!("{} devices · {} links · {} areas", p.devices.len(), p.links.len(), p.areas.len())).small().weak());
                if let Some(w) = st.canvas.cursor_world {
                    ui.separator();
                    ui.label(RichText::new(format!("{:.0}, {:.0}", w.x, w.y)).small().weak().monospace());
                }
                if st.doc.is_dirty() {
                    ui.separator();
                    ui.label(RichText::new("● Unsaved").small().color(egui::Color32::from_rgb(217, 119, 6)));
                }
            });
        });
    }

    fn dialogs(&mut self, ctx: &egui::Context) {
        let Some(mut dialog) = self.dialog.take() else { return };
        let mut keep = true;
        let mut after: Option<AfterDialog> = None;

        let modal = egui::Modal::new(egui::Id::new("netvis-dialog"));
        let resp = match dialog {
            Dialog::Unsaved { ref then } => modal.show(ctx, |ui| {
                ui.set_width(380.0);
                ui.heading("Unsaved changes");
                ui.label(format!("Save changes to “{}” before continuing?", self.st.doc.display_name()));
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if ui.button(RichText::new(format!("{} Save", icons::FLOPPY_DISK)).strong()).clicked() {
                        let t = then.clone();
                        after = Some(Box::new(move |app| app.save(Some(t))));
                        keep = false;
                    }
                    if ui.button("Don't save").clicked() {
                        let t = then.clone();
                        after = Some(Box::new(move |app| {
                            app.remove_autosave();
                            app.allow_close = matches!(t, Pending::Quit);
                            app.run_pending(t);
                        }));
                        keep = false;
                    }
                    if ui.button("Cancel").clicked() {
                        keep = false;
                    }
                });
            }),
            Dialog::PathPrompt { purpose, ref mut text, ref then } => modal.show(ctx, |ui| {
                ui.set_width(520.0);
                ui.heading(match purpose {
                    PathPurpose::Open => "Open project",
                    PathPurpose::SaveAs => "Save project as",
                });
                ui.label(RichText::new("Enter the full file path:").weak());
                let r = ui.add(egui::TextEdit::singleline(text).desired_width(f32::INFINITY));
                r.request_focus();
                let enter = r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    let label = if purpose == PathPurpose::Open { "Open" } else { "Save" };
                    if (ui.button(RichText::new(label).strong()).clicked() || enter) && !text.trim().is_empty() {
                        let path = PathBuf::from(text.trim());
                        let t = then.clone();
                        after = Some(Box::new(move |app| app.on_path_chosen(purpose, path, t)));
                        keep = false;
                    }
                    if ui.button("Cancel").clicked() {
                        keep = false;
                    }
                });
            }),
            Dialog::Recover { ref path } => modal.show(ctx, |ui| {
                ui.set_width(400.0);
                ui.heading(format!("{} Recover unsaved work?", icons::LIFEBUOY));
                ui.label("NetVis found an autosave from a previous session that was not closed cleanly.");
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if ui.button(RichText::new("Recover").strong()).clicked() {
                        let p = path.clone();
                        after = Some(Box::new(move |app| match io::load(&p) {
                            Ok(project) => {
                                app.st.replace_document(Document::recovered(project));
                                app.st.set_status("Recovered autosave — remember to save");
                            }
                            Err(e) => app.dialog = Some(Dialog::Error(format!("Autosave is unreadable:\n{e}"))),
                        }));
                        keep = false;
                    }
                    if ui.button("Discard").clicked() {
                        after = Some(Box::new(|app| app.remove_autosave()));
                        keep = false;
                    }
                });
            }),
            Dialog::Error(ref msg) => modal.show(ctx, |ui| {
                ui.set_width(420.0);
                ui.heading(format!("{} Something went wrong", icons::WARNING));
                ui.label(msg);
                ui.add_space(10.0);
                if ui.button("OK").clicked() {
                    keep = false;
                }
            }),
            Dialog::About => modal.show(ctx, |ui| {
                ui.set_width(440.0);
                ui.heading(format!("{APP_NAME} {}", env!("CARGO_PKG_VERSION")));
                ui.label(RichText::new(format!("Network planning tool · {BRAND_NAME}")).weak());
                ui.add_space(8.0);
                egui::Grid::new("keys").num_columns(2).spacing([16.0, 4.0]).show(ui, |ui| {
                    for (k, v) in [
                        ("Drag device from Toolbox", "Place device"),
                        ("Drag port → port / device", "Create link"),
                        ("Drag on empty canvas", "Box select"),
                        ("Shift + click", "Add to / remove from selection"),
                        ("Middle-drag / Space + drag", "Pan"),
                        ("Mouse wheel / pinch", "Zoom at cursor"),
                        ("Arrow keys (Shift = 1px)", "Nudge selection"),
                        ("Ctrl+Z / Ctrl+Y", "Undo / Redo"),
                        ("Ctrl+D / Del", "Duplicate / Delete"),
                        ("Ctrl+0 / Ctrl+1", "Fit / 100%"),
                    ] {
                        ui.label(RichText::new(k).monospace());
                        ui.label(v);
                        ui.end_row();
                    }
                });
                ui.add_space(10.0);
                if ui.button("Close").clicked() {
                    keep = false;
                }
            }),
        };
        if resp.should_close() && !matches!(dialog, Dialog::Recover { .. }) {
            keep = false;
        }
        if keep {
            self.dialog = Some(dialog);
        }
        if let Some(f) = after {
            f(self);
        }
    }

    fn update_title(&mut self, ctx: &egui::Context) {
        let dirty = if self.st.doc.is_dirty() { " •" } else { "" };
        let title = format!("{}{dirty} — {APP_NAME}", self.st.doc.display_name());
        if title != self.title {
            ctx.send_viewport_cmd(ViewportCommand::Title(title.clone()));
            self.title = title;
        }
    }
}

struct Tabs<'a> {
    st: &'a mut State,
}

impl egui_dock::TabViewer for Tabs<'_> {
    type Tab = Tab;

    fn id(&mut self, tab: &mut Tab) -> egui::Id {
        egui::Id::new(*tab)
    }

    fn title(&mut self, tab: &mut Tab) -> egui::WidgetText {
        tab.title().into()
    }

    fn ui(&mut self, ui: &mut egui::Ui, tab: &mut Tab) {
        if *tab == Tab::Canvas {
            canvas::show(ui, self.st);
            return;
        }
        egui::Frame::new().inner_margin(8.0).show(ui, |ui| match tab {
            Tab::Toolbox => panels::toolbox(ui, self.st),
            Tab::Outline => panels::outline(ui, self.st),
            Tab::Inspector => panels::inspector(ui, self.st),
            Tab::Issues => panels::issues(ui, self.st),
            Tab::IpPlan => panels::ip_plan(ui, self.st),
            Tab::Vlans => panels::vlans(ui, self.st),
            Tab::Canvas => unreachable!(),
        });
    }

    fn is_closeable(&self, _tab: &Tab) -> bool {
        false
    }

    fn scroll_bars(&self, _tab: &Tab) -> [bool; 2] {
        [false, false]
    }
}

impl eframe::App for NetVisApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.st.now = ctx.input(|i| i.time);

        if self.applied_theme != Some(self.st.settings.theme) {
            theme::apply_choice(&ctx, self.st.settings.theme);
            self.applied_theme = Some(self.st.settings.theme);
        }

        if ctx.input(|i| i.viewport().close_requested()) && !self.allow_close && self.st.doc.is_dirty() {
            ctx.send_viewport_cmd(ViewportCommand::CancelClose);
            if self.dialog.is_none() {
                self.dialog = Some(Dialog::Unsaved { then: Pending::Quit });
            }
        }

        self.shortcuts(&ctx);

        egui::Panel::top("menu").show(ui, |ui| self.menu_bar(ui));
        egui::Panel::top("toolbar").show(ui, |ui| self.toolbar(ui));
        egui::Panel::bottom("status").show(ui, |ui| self.status_bar(ui));
        egui::CentralPanel::default().frame(egui::Frame::NONE).show(ui, |ui| {
            let mut style = egui_dock::Style::from_egui(ui.style());
            style.tab.tab_body.inner_margin = egui::Margin::ZERO;
            DockArea::new(&mut self.dock)
                .style(style)
                .show_add_buttons(false)
                .show_leaf_close_all_buttons(false)
                .show_leaf_collapse_buttons(false)
                .show_inside(ui, &mut Tabs { st: &mut self.st });
        });

        panels::vlsm::show(&ctx, &mut self.st);
        self.dialogs(&ctx);

        for a in std::mem::take(&mut self.st.actions) {
            self.handle_action(&ctx, a);
        }

        self.autosave();
        self.update_title(&ctx);
        #[cfg(debug_assertions)]
        if let Some(c) = &mut self.capture {
            c.tick(&ctx);
        }
        // Keep the status-message timeout and autosave ticking.
        ctx.request_repaint_after(std::time::Duration::from_secs(1));
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, SETTINGS_KEY, &self.st.settings);
        eframe::set_value(storage, DOCK_KEY, &self.dock);
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        if !self.st.doc.is_dirty() || self.allow_close {
            self.remove_autosave();
        } else if let Some(path) = &self.autosave_path {
            if let Some(dir) = path.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            let _ = io::save(&self.st.doc.project, path);
        }
    }
}
