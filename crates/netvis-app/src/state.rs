//! Application state shared by all panels.

use std::path::PathBuf;

pub use netvis_core::{Item, Selection};
use netvis_core::{Edit, History, Pos, Project};
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub theme: ThemeChoice,
    pub show_grid: bool,
    pub snap_to_grid: bool,
    pub show_minimap: bool,
    pub show_addresses: bool,
    pub recent: Vec<PathBuf>,
}

impl Default for Settings {
    fn default() -> Self {
        Self { theme: ThemeChoice::System, show_grid: true, snap_to_grid: true, show_minimap: true, show_addresses: true, recent: Vec::new() }
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
    ShowTools,
    /// Bring the topology tab to the front (after revealing an item).
    ShowCanvas,
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
    pub vlsm: Option<crate::panels::vlsm::VlsmPlanner>,
    pub tools: crate::panels::tools::ToolsState,
    /// Report from the most recent auto-assign run.
    pub last_assign: Option<(Vec<String>, Vec<String>)>,
    /// Validation results for `issues_rev`.
    pub issues: Vec<netvis_validate::Issue>,
    issues_rev: Option<u64>,
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
            vlsm: None,
            tools: Default::default(),
            last_assign: None,
            issues: Vec::new(),
            issues_rev: None,
        }
    }

    pub fn replace_document(&mut self, doc: Document) {
        self.doc = doc;
        self.issues_rev = None;
        self.sel.clear();
        self.canvas.request_fit();
    }

    /// Re-run validation if the project changed since the last run.
    pub fn refresh_issues(&mut self) {
        let rev = self.doc.history.revision();
        if self.issues_rev != Some(rev) && !self.canvas.is_dragging() {
            self.issues = netvis_validate::validate(&self.doc.project);
            self.issues_rev = Some(rev);
        }
    }

    pub fn issue_counts(&self) -> [usize; 3] {
        use netvis_validate::Severity::*;
        let n = |s| self.issues.iter().filter(|i| i.severity == s).count();
        [n(Error), n(Warning), n(Info)]
    }

    /// Apply an issue's fix as one undo step.
    pub fn apply_fix(&mut self, fix: &netvis_validate::Fix) {
        let label = fix.label(&self.doc.project);
        match fix.edit(&self.doc.project) {
            Some(e) => {
                self.apply(e, &format!("Fix: {label}"));
                self.set_status(format!("Fixed: {label}"));
            }
            None => self.set_status("That fix no longer applies"),
        }
    }

    /// Apply every fix for errors and warnings as one undo step.
    pub fn fix_all(&mut self) {
        use netvis_validate::Severity;
        let mut codes: Vec<&str> =
            self.issues.iter().filter(|i| i.fix.is_some() && i.severity != Severity::Info).map(|i| i.code).collect();
        codes.dedup();
        let (edit, n) = netvis_validate::fix_all(&self.doc.project, Some(&codes));
        self.apply(edit, &format!("Fix {n} issue(s)"));
        self.refresh_issues();
        let [e, w, _] = self.issue_counts();
        self.set_status(format!("Applied {n} fix(es); {e} error(s) and {w} warning(s) remain"));
    }

    /// Select an item and centre the canvas on it.
    pub fn reveal(&mut self, item: Item) {
        self.doc.history.break_coalescing();
        self.sel.set_single(item);
        let p = &self.doc.project;
        let at = match item {
            Item::Device(id) => p.device(id).map(|d| d.rect()),
            Item::Area(id) => p.area(id).map(|a| a.rect),
            Item::Link(id) => p.link(id).and_then(|l| {
                let (a, b) = (p.device(l.a.device)?.rect(), p.device(l.b.device)?.rect());
                Some(a.union(&b))
            }),
            _ => None,
        };
        if let Some(r) = at {
            self.canvas.focus(r.min.offset(r.size.x / 2.0, r.size.y / 2.0));
        }
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
        let edit = self.doc.project.remove_edit(&self.sel);
        self.apply(edit, &format!("Delete {n} item(s)"));
        self.sel.clear();
    }

    /// Auto-assign addresses to the selected devices, or to all devices if none are selected.
    pub fn auto_assign(&mut self) {
        let scope = (!self.sel.devices.is_empty()).then(|| self.sel.devices.clone());
        let run = self.doc.project.auto_assign_edit(scope.as_deref());
        let (n, skipped) = (run.assigned.len(), run.skipped.len());
        let where_ = if scope.is_some() { "selected devices" } else { "all devices" };
        self.apply(run.edit, "Auto-assign addresses");
        self.set_status(match (n, skipped) {
            (0, 0) => format!("Auto-assign ({where_}): nothing to do — every eligible interface already has an address"),
            (n, 0) => format!("Auto-assign ({where_}): {n} address(es) assigned"),
            (n, s) => format!("Auto-assign ({where_}): {n} assigned, {s} skipped — see IP Plan"),
        });
        self.last_assign = Some((run.assigned, run.skipped));
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
        if devs.len() < 2 {
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
        let mut gap = (last_end - first - total) / (devs.len() - 1) as f32;
        // Overlapping (stacked) cards have no room to share out: spread them
        // apart instead, a grid step between each, starting from the first.
        let rects: Vec<_> = devs.iter().map(|d| d.rect()).collect();
        let overlapping = rects.iter().enumerate().any(|(i, a)| rects[i + 1..].iter().any(|b| a.intersects(b)));
        if overlapping {
            gap = gap.max(crate::canvas::GRID);
        }

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

#[cfg(test)]
mod tests {
    use super::*;
    use netvis_core::{DeviceKind, Pos};

    fn state_with(xs: &[(f32, f32)]) -> (State, Vec<netvis_core::DeviceId>) {
        let mut st = State::new(Settings::default());
        let mut ids = Vec::new();
        for &(x, y) in xs {
            let (id, e) = st.doc.project.add_device_edit(DeviceKind::Pc, Pos::new(x, y));
            st.apply(e, "add");
            st.sel.add(Item::Device(id));
            ids.push(id);
        }
        (st, ids)
    }

    fn xs(st: &State, ids: &[netvis_core::DeviceId]) -> Vec<f32> {
        ids.iter().map(|id| st.doc.project.device(*id).unwrap().pos.x).collect()
    }

    #[test]
    fn distribute_spaces_evenly() {
        let (mut st, ids) = state_with(&[(0.0, 0.0), (300.0, 0.0), (1000.0, 0.0)]);
        st.distribute(Axis::Horizontal);
        assert_eq!(xs(&st, &ids), vec![0.0, 500.0, 1000.0]);
        // A neat row has nothing to distribute vertically.
        st.distribute(Axis::Vertical);
        assert_eq!(xs(&st, &ids), vec![0.0, 500.0, 1000.0]);
    }

    #[test]
    fn distribute_spreads_stacked_cards() {
        let (mut st, ids) = state_with(&[(0.0, 0.0), (40.0, 0.0), (80.0, 0.0)]);
        let w = st.doc.project.device(ids[0]).unwrap().size().x;
        st.distribute(Axis::Horizontal);
        let x = xs(&st, &ids);
        assert_eq!(x[0], 0.0);
        assert_eq!(x[1], w + crate::canvas::GRID);
        assert_eq!(x[2], 2.0 * (w + crate::canvas::GRID));
        st.undo();
        assert_eq!(xs(&st, &ids), vec![0.0, 40.0, 80.0], "one undo step");
    }

    #[test]
    fn align_and_spread_a_pair() {
        let (mut st, ids) = state_with(&[(0.0, 0.0), (300.0, 75.0)]);
        st.align(Align::Top);
        let ys: Vec<f32> = ids.iter().map(|id| st.doc.project.device(*id).unwrap().pos.y).collect();
        assert_eq!(ys, vec![0.0, 0.0]);
        st.align(Align::Left);
        assert_eq!(xs(&st, &ids), vec![0.0, 0.0], "now fully stacked");
        st.distribute(Axis::Horizontal);
        let w = st.doc.project.device(ids[0]).unwrap().size().x;
        assert_eq!(xs(&st, &ids), vec![0.0, w + crate::canvas::GRID]);
    }
}
