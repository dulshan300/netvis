//! Undo/redo via reversible edits.
//!
//! Each edit stores the full before/after value of the entity it touches, so
//! applying and reverting are symmetric and cannot drift.

use crate::area::Area;
use crate::device::Device;
use crate::ids::{AreaId, DeviceId, LinkId};
use crate::link::Link;
use crate::project::{Project, ProjectMeta};

#[derive(Debug, Clone, PartialEq)]
pub enum Edit {
    Device { id: DeviceId, before: Option<Device>, after: Option<Device> },
    Link { id: LinkId, before: Option<Link>, after: Option<Link> },
    Area { id: AreaId, before: Option<Area>, after: Option<Area> },
    Meta { before: Box<ProjectMeta>, after: Box<ProjectMeta> },
    Batch(Vec<Edit>),
}

impl Edit {
    pub fn apply(&self, p: &mut Project) {
        match self {
            Edit::Device { id, after, .. } => p.set_device(*id, after.clone()),
            Edit::Link { id, after, .. } => p.set_link(*id, after.clone()),
            Edit::Area { id, after, .. } => p.set_area(*id, after.clone()),
            Edit::Meta { after, .. } => p.meta = (**after).clone(),
            Edit::Batch(edits) => edits.iter().for_each(|e| e.apply(p)),
        }
    }

    pub fn inverted(&self) -> Edit {
        match self {
            Edit::Device { id, before, after } => {
                Edit::Device { id: *id, before: after.clone(), after: before.clone() }
            }
            Edit::Link { id, before, after } => {
                Edit::Link { id: *id, before: after.clone(), after: before.clone() }
            }
            Edit::Area { id, before, after } => {
                Edit::Area { id: *id, before: after.clone(), after: before.clone() }
            }
            Edit::Meta { before, after } => Edit::Meta { before: after.clone(), after: before.clone() },
            Edit::Batch(edits) => Edit::Batch(edits.iter().rev().map(Edit::inverted).collect()),
        }
    }

    pub fn is_noop(&self) -> bool {
        match self {
            Edit::Device { before, after, .. } => before == after,
            Edit::Link { before, after, .. } => before == after,
            Edit::Area { before, after, .. } => before == after,
            Edit::Meta { before, after } => before == after,
            Edit::Batch(edits) => edits.iter().all(Edit::is_noop),
        }
    }

    /// Fold `next` into `self`, keeping `self`'s "before" state.
    fn merge(self, next: Edit) -> Edit {
        match (self, next) {
            (Edit::Device { id, before, .. }, Edit::Device { id: id2, after, .. }) if id == id2 => {
                Edit::Device { id, before, after }
            }
            (Edit::Link { id, before, .. }, Edit::Link { id: id2, after, .. }) if id == id2 => {
                Edit::Link { id, before, after }
            }
            (Edit::Area { id, before, .. }, Edit::Area { id: id2, after, .. }) if id == id2 => {
                Edit::Area { id, before, after }
            }
            (Edit::Meta { before, .. }, Edit::Meta { after, .. }) => Edit::Meta { before, after },
            (first, next) => Edit::Batch(vec![first, next]),
        }
    }
}

#[derive(Debug)]
struct Entry {
    edit: Edit,
    label: String,
    coalesce_key: Option<String>,
}

#[derive(Debug)]
pub struct History {
    undo: Vec<Entry>,
    redo: Vec<Entry>,
    limit: usize,
    /// Bumped on every change to the project (including undo/redo).
    revision: u64,
}

impl Default for History {
    fn default() -> Self {
        Self { undo: Vec::new(), redo: Vec::new(), limit: 500, revision: 0 }
    }
}

impl History {
    /// Apply `edit` to the project and record it.
    pub fn apply(&mut self, p: &mut Project, edit: Edit, label: impl Into<String>) {
        edit.apply(p);
        self.record(edit, label.into(), None);
    }

    /// Like [`apply`](Self::apply), but consecutive edits sharing `key` merge into
    /// one undo step (used for typing into text fields).
    pub fn apply_coalescing(&mut self, p: &mut Project, edit: Edit, label: impl Into<String>, key: String) {
        edit.apply(p);
        if let Some(top) = self.undo.last_mut()
            && top.coalesce_key.as_deref() == Some(key.as_str())
        {
            let prev = std::mem::replace(&mut top.edit, Edit::Batch(Vec::new()));
            top.edit = prev.merge(edit);
            self.redo.clear();
            self.revision += 1;
            return;
        }
        self.record(edit, label.into(), Some(key));
    }

    /// Record an edit whose effect is already present in the project
    /// (e.g. a drag that moved devices live).
    pub fn record_applied(&mut self, edit: Edit, label: impl Into<String>) {
        self.record(edit, label.into(), None);
    }

    fn record(&mut self, edit: Edit, label: String, coalesce_key: Option<String>) {
        if edit.is_noop() {
            return;
        }
        self.undo.push(Entry { edit, label, coalesce_key });
        if self.undo.len() > self.limit {
            self.undo.remove(0);
        }
        self.redo.clear();
        self.revision += 1;
    }

    /// Stop merging into the current undo step.
    pub fn break_coalescing(&mut self) {
        if let Some(top) = self.undo.last_mut() {
            top.coalesce_key = None;
        }
    }

    pub fn undo(&mut self, p: &mut Project) -> Option<String> {
        let mut entry = self.undo.pop()?;
        entry.edit.inverted().apply(p);
        entry.coalesce_key = None;
        let label = entry.label.clone();
        self.redo.push(entry);
        self.revision += 1;
        Some(label)
    }

    pub fn redo(&mut self, p: &mut Project) -> Option<String> {
        let entry = self.redo.pop()?;
        entry.edit.apply(p);
        let label = entry.label.clone();
        self.undo.push(entry);
        self.revision += 1;
        Some(label)
    }

    pub fn undo_label(&self) -> Option<&str> {
        self.undo.last().map(|e| e.label.as_str())
    }

    pub fn redo_label(&self) -> Option<&str> {
        self.redo.last().map(|e| e.label.as_str())
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
        self.revision += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::device::DeviceKind;
    use crate::geom::Pos;

    #[test]
    fn undo_redo_roundtrip() {
        let mut p = Project::default();
        let mut h = History::default();
        let (id, e) = p.add_device_edit(DeviceKind::Router, Pos::default());
        h.apply(&mut p, e, "Add router");
        assert!(p.device(id).is_some());

        assert_eq!(h.undo(&mut p).as_deref(), Some("Add router"));
        assert!(p.devices.is_empty());
        assert_eq!(h.redo(&mut p).as_deref(), Some("Add router"));
        assert!(p.device(id).is_some());
        assert!(h.redo(&mut p).is_none());
    }

    #[test]
    fn coalescing_merges_typing_into_one_step() {
        let mut p = Project::default();
        let mut h = History::default();
        let (id, e) = p.add_device_edit(DeviceKind::Router, Pos::default());
        h.apply(&mut p, e, "Add");
        let original = p.device(id).unwrap().clone();

        for name in ["E", "Ed", "Edge"] {
            let before = p.device(id).cloned();
            let mut after = before.clone().unwrap();
            after.name = name.into();
            let edit = Edit::Device { id, before, after: Some(after) };
            h.apply_coalescing(&mut p, edit, "Rename", format!("{id:?}.name"));
        }
        assert_eq!(p.device(id).unwrap().name, "Edge");
        h.undo(&mut p);
        assert_eq!(p.device(id).unwrap(), &original);
        h.undo(&mut p);
        assert!(p.devices.is_empty());
    }

    #[test]
    fn new_edit_clears_redo_and_noops_are_ignored() {
        let mut p = Project::default();
        let mut h = History::default();
        let (_, e) = p.add_device_edit(DeviceKind::Pc, Pos::default());
        h.apply(&mut p, e, "a");
        h.undo(&mut p);
        let rev = h.revision();
        h.record_applied(Edit::Batch(vec![]), "noop");
        assert_eq!(h.revision(), rev);
        assert!(h.redo_label().is_some());
        let (_, e) = p.add_device_edit(DeviceKind::Pc, Pos::default());
        h.apply(&mut p, e, "b");
        assert!(h.redo_label().is_none());
    }
}
