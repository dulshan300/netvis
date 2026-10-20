//! Application state shared by all panels.

use std::path::PathBuf;

use netvis_core::{AreaId, DeviceId, Edit, History, LinkId, Pos, Project};
use serde::{Deserialize, Serialize};

use crate::canvas::CanvasState;
use crate::theme::ThemeChoice;

pub struct Document {
    pub project: Project,
    pub history: History,
    pub path: Option<PathBuf>,
    saved_revision: u64,
}

impl Document {
    pub fn new(project: Project, path: Option<PathBuf>) -> Self {
        let history = History::default();
        let saved_revision = history.revision();
        Self { project, history, path, saved_revision }
    }

    /// A document restored from autosave: unsaved, with no file path.
    pub fn recovered(project: Project) -> Self {
        let mut doc = Self::new(project, None);
        doc.saved_revision = u64::MAX;
        doc
    }

    pub fn is_dirty(&self) -> bool {
        self.history.revision() != self.saved_revision
    }

    pub fn mark_saved(&mut self, path: PathBuf) {
        self.path = Some(path);
        self.saved_revision = self.history.revision();
    }

    pub fn display_name(&self) -> String {
        self.path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| self.project.meta.name.clone())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Item {
    Device(DeviceId),
    Link(LinkId),
    Area(AreaId),
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Selection {
    pub devices: Vec<DeviceId>,
    pub links: Vec<LinkId>,
    pub areas: Vec<AreaId>,
}

impl Selection {
    pub fn is_empty(&self) -> bool {
        self.devices.is_empty() && self.links.is_empty() && self.areas.is_empty()
    }

    pub fn len(&self) -> usize {
        self.devices.len() + self.links.len() + self.areas.len()
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn contains(&self, item: Item) -> bool {
        match item {
            Item::Device(id) => self.devices.contains(&id),
            Item::Link(id) => self.links.contains(&id),
            Item::Area(id) => self.areas.contains(&id),
        }
    }

    pub fn add(&mut self, item: Item) {
        if self.contains(item) {
            return;
        }
        match item {
            Item::Device(id) => self.devices.push(id),
            Item::Link(id) => self.links.push(id),
            Item::Area(id) => self.areas.push(id),
        }
    }

    pub fn toggle(&mut self, item: Item) {
        if !self.contains(item) {
            self.add(item);
            return;
        }
        match item {
            Item::Device(id) => self.devices.retain(|d| *d != id),
            Item::Link(id) => self.links.retain(|l| *l != id),
            Item::Area(id) => self.areas.retain(|a| *a != id),
        }
    }

    pub fn set_single(&mut self, item: Item) {
        self.clear();
        self.add(item);
    }

    /// Click semantics: shift toggles, plain click replaces.
    pub fn click(&mut self, item: Item, additive: bool) {
        if additive { self.toggle(item) } else { self.set_single(item) }
    }

    pub fn single(&self) -> Option<Item> {
        if self.len() != 1 {
            return None;
        }
        self.devices
            .first()
            .map(|d| Item::Device(*d))
            .or_else(|| self.links.first().map(|l| Item::Link(*l)))
            .or_else(|| self.areas.first().map(|a| Item::Area(*a)))
    }

    pub fn retain_existing(&mut self, p: &Project) {
        self.devices.retain(|id| p.device(*id).is_some());
        self.links.retain(|id| p.link(*id).is_some());
        self.areas.retain(|id| p.area(*id).is_some());
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub theme: ThemeChoice,
    pub show_grid: bool,
    pub snap_to_grid: bool,
    pub show_minimap: bool,
    pub recent: Vec<PathBuf>,
}

impl Default for Settings {
    fn default() -> Self {
        Self { theme: ThemeChoice::System, show_grid: true, snap_to_grid: true, show_minimap: true, recent: Vec::new() }
    }
}

impl Settings {
    pub const MAX_RECENT: usize = 8;

    pub fn push_recent(&mut self, path: PathBuf) {
        self.recent.retain(|p| p != &path);
        self.recent.insert(0, path);
        self.recent.truncate(Self::MAX_RECENT);
    }
}

/// Requests from panels that the app shell must handle (file I/O, dialogs…).
#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    New,
    Open,
    OpenPath(PathBuf),
    Save,
    SaveAs,
    Quit,
    About,
    ResetLayout,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Align {
    Left,
    CenterX,
    Right,
    Top,
    CenterY,
    Bottom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Axis {
    Horizontal,
    Vertical,
}

pub struct State {
    pub doc: Document,
    pub sel: Selection,
    pub canvas: CanvasState,
    pub settings: Settings,
    pub actions: Vec<Action>,
    status: Option<(String, f64)>,
    pub now: f64,
}

impl State {
    pub fn new(settings: Settings) -> Self {
        Self {
            doc: Document::new(Project::default(), None),
            sel: Selection::default(),
            canvas: CanvasState::default(),
            settings,
            actions: Vec::new(),
            status: None,
            now: 0.0,
        }
    }

    pub fn replace_document(&mut self, doc: Document) {
        self.doc = doc;
        self.sel.clear();
        self.canvas.request_fit();
    }

    pub fn set_status(&mut self, msg: impl Into<String>) {
        self.status = Some((msg.into(), self.now + 4.0));
    }

    pub fn status(&self) -> Option<&str> {
        self.status.as_ref().filter(|(_, until)| *until > self.now).map(|(m, _)| m.as_str())
    }

    // ---- history -------------------------------------------------------

    pub fn apply(&mut self, edit: Edit, label: &str) {
        self.doc.history.apply(&mut self.doc.project, edit, label);
        self.sel.retain_existing(&self.doc.project);
    }

    pub fn apply_coalescing(&mut self, edit: Edit, label: &str, key: String) {
        self.doc.history.apply_coalescing(&mut self.doc.project, edit, label, key);
    }

    pub fn record_applied(&mut self, edit: Edit, label: &str) {
        self.doc.history.record_applied(edit, label);
    }

    pub fn undo(&mut self) {
        match self.doc.history.undo(&mut self.doc.project) {
            Some(label) => self.set_status(format!("Undo: {label}")),
            None => self.set_status("Nothing to undo"),
        }
        self.sel.retain_existing(&self.doc.project);
    }

    pub fn redo(&mut self) {
        match self.doc.history.redo(&mut self.doc.project) {
            Some(label) => self.set_status(format!("Redo: {label}")),
            None => self.set_status("Nothing to redo"),
        }
        self.sel.retain_existing(&self.doc.project);
    }

    // ---- selection commands -------------------------------------------

    pub fn select_all(&mut self) {
        let p = &self.doc.project;
        self.sel.devices = p.devices.iter().map(|d| d.id).collect();
        self.sel.areas = p.areas.iter().map(|a| a.id).collect();
        self.sel.links = p.links.iter().map(|l| l.id).collect();
    }

    pub fn delete_selection(&mut self) {
        if self.sel.is_empty() {
            return;
        }
        let n = self.sel.len();
        let edit = self.doc.project.remove_edit(&self.sel.devices, &self.sel.links, &self.sel.areas);
        self.apply(edit, &format!("Delete {n} item(s)"));
        self.sel.clear();
    }

    pub fn duplicate_selection(&mut self) {
        if self.sel.devices.is_empty() {
            return;
        }
        let (ids, edit) = self.doc.project.duplicate_edit(&self.sel.devices, Pos::new(40.0, 40.0));
        self.apply(edit, "Duplicate");
        self.sel.clear();
        self.sel.devices = ids;
    }

    /// Move selected devices by a world-space delta (keyboard nudge).
    pub fn nudge_selection(&mut self, dx: f32, dy: f32) {
        let edits = self
            .doc
            .project
            .devices
            .iter()
            .filter(|d| self.sel.devices.contains(&d.id))
            .map(|d| {
                let mut after = d.clone();
                after.pos = d.pos.offset(dx, dy);
                Edit::Device { id: d.id, before: Some(d.clone()), after: Some(after) }
            })
            .collect::<Vec<_>>();
        if !edits.is_empty() {
            self.apply_coalescing(Edit::Batch(edits), "Nudge", "nudge".into());
        }
    }

    pub fn bring_to_front(&mut self) {
        let edits = self
            .doc
            .project
            .devices
            .iter()
            .filter(|d| self.sel.devices.contains(&d.id))
            .flat_map(|d| {
                // Remove + re-add moves the device to the end of the draw order.
                [
                    Edit::Device { id: d.id, before: Some(d.clone()), after: None },
                    Edit::Device { id: d.id, before: None, after: Some(d.clone()) },
                ]
            })
            .collect();
        self.apply(Edit::Batch(edits), "Bring to front");
    }

    pub fn align(&mut self, how: Align) {
        let devs: Vec<_> =
            self.doc.project.devices.iter().filter(|d| self.sel.devices.contains(&d.id)).cloned().collect();
        if devs.len() < 2 {
            return;
        }
        let rects: Vec<_> = devs.iter().map(|d| d.rect()).collect();
        let min_x = rects.iter().map(|r| r.min.x).fold(f32::MAX, f32::min);
        let max_x = rects.iter().map(|r| r.max().x).fold(f32::MIN, f32::max);
        let min_y = rects.iter().map(|r| r.min.y).fold(f32::MAX, f32::min);
        let max_y = rects.iter().map(|r| r.max().y).fold(f32::MIN, f32::max);
        let mid_x = (min_x + max_x) / 2.0;
        let mid_y = (min_y + max_y) / 2.0;

        let edits = devs
            .into_iter()
            .map(|d| {
                let s = d.size();
                let mut after = d.clone();
                match how {
                    Align::Left => after.pos.x = min_x,
                    Align::CenterX => after.pos.x = mid_x - s.x / 2.0,
                    Align::Right => after.pos.x = max_x - s.x,
                    Align::Top => after.pos.y = min_y,
                    Align::CenterY => after.pos.y = mid_y - s.y / 2.0,
                    Align::Bottom => after.pos.y = max_y - s.y,
                }
                Edit::Device { id: d.id, before: Some(d), after: Some(after) }
            })
            .collect();
        self.apply(Edit::Batch(edits), "Align");
    }

    /// Space selected devices so the gaps between them are equal.
    pub fn distribute(&mut self, axis: Axis) {
        let mut devs: Vec<_> =
            self.doc.project.devices.iter().filter(|d| self.sel.devices.contains(&d.id)).cloned().collect();
        if devs.len() < 3 {
            return;
        }
        let start = |d: &netvis_core::Device| match axis {
            Axis::Horizontal => d.pos.x,
            Axis::Vertical => d.pos.y,
        };
        let extent = |d: &netvis_core::Device| match axis {
            Axis::Horizontal => d.size().x,
            Axis::Vertical => d.size().y,
        };
        devs.sort_by(|a, b| start(a).total_cmp(&start(b)));
        let first = start(&devs[0]);
        let last_end = devs.iter().map(|d| start(d) + extent(d)).fold(f32::MIN, f32::max);
        let total: f32 = devs.iter().map(&extent).sum();
        let gap = (last_end - first - total) / (devs.len() - 1) as f32;

        let mut cursor = first;
        let edits = devs
            .into_iter()
            .map(|d| {
                let mut after = d.clone();
                match axis {
                    Axis::Horizontal => after.pos.x = cursor,
                    Axis::Vertical => after.pos.y = cursor,
                }
                cursor += extent(&d) + gap;
                Edit::Device { id: d.id, before: Some(d), after: Some(after) }
            })
            .collect();
        self.apply(Edit::Batch(edits), "Distribute");
    }
}
