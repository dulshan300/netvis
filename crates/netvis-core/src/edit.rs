//! Undo/redo via reversible edits.
//!
//! Each edit stores the full before/after value of what it touches, so
//! applying and reverting are symmetric and cannot drift.

use crate::area::Area;
use crate::device::Device;
use crate::ids::{AreaId, DeviceId, LinkId, SsidId, SubnetId, VlanId};
use crate::link::Link;
use crate::project::{Project, ProjectMeta};
use crate::routing::Services;
use crate::subnet::Subnet;
use crate::vlan::{Ssid, Vlan};

/// Generates the `Edit` enum plus apply/invert/noop/merge for:
/// - entity variants: `{ id, before: Option<T>, after: Option<T> }`, applied via `Project::$setter`
/// - singleton variants: `{ before: Box<T>, after: Box<T> }`, applied by assigning `project.$field`
macro_rules! edits {
    (
        entities { $($ev:ident($eid:ty, $ety:ty) => $setter:ident),* $(,)? }
        singletons { $($sv:ident($sty:ty) => $field:ident),* $(,)? }
    ) => {
        // Edits hold whole entities by value; history is bounded (500 steps),
        // so the size gap between variants is not worth boxing every one.
        #[allow(clippy::large_enum_variant)]
        #[derive(Debug, Clone, PartialEq)]
        pub enum Edit {
            $($ev { id: $eid, before: Option<$ety>, after: Option<$ety> },)*
            $($sv { before: Box<$sty>, after: Box<$sty> },)*
            Batch(Vec<Edit>),
        }

        impl Edit {
            pub fn apply(&self, p: &mut Project) {
                match self {
                    $(Edit::$ev { id, after, .. } => p.$setter(*id, after.clone()),)*
                    $(Edit::$sv { after, .. } => p.$field = (**after).clone(),)*
                    Edit::Batch(edits) => edits.iter().for_each(|e| e.apply(p)),
                }
            }

            pub fn inverted(&self) -> Edit {
                match self {
                    $(Edit::$ev { id, before, after } => Edit::$ev { id: *id, before: after.clone(), after: before.clone() },)*
                    $(Edit::$sv { before, after } => Edit::$sv { before: after.clone(), after: before.clone() },)*
                    Edit::Batch(edits) => Edit::Batch(edits.iter().rev().map(Edit::inverted).collect()),
                }
            }

            pub fn is_noop(&self) -> bool {
                match self {
                    $(Edit::$ev { before, after, .. } => before == after,)*
                    $(Edit::$sv { before, after } => before == after,)*
                    Edit::Batch(edits) => edits.iter().all(Edit::is_noop),
                }
            }

            /// Fold `next` into `self`, keeping `self`'s "before" state.
            fn merge(self, next: Edit) -> Edit {
                match (self, next) {
                    $((Edit::$ev { id, before, .. }, Edit::$ev { id: id2, after, .. }) if id == id2 => {
                        Edit::$ev { id, before, after }
                    })*
                    $((Edit::$sv { before, .. }, Edit::$sv { after, .. }) => Edit::$sv { before, after },)*
                    (Edit::Batch(a), Edit::Batch(b)) if same_shape(&a, &b) => {
                        Edit::Batch(a.into_iter().zip(b).map(|(x, y)| x.merge(y)).collect())
                    }
                    (first, next) => Edit::Batch(vec![first, next]),
                }
            }

            /// Discriminant plus entity id, used to decide whether batches line up.
            fn target(&self) -> Option<(std::mem::Discriminant<Edit>, Option<uuid::Uuid>)> {
                match self {
                    $(Edit::$ev { id, .. } => Some((std::mem::discriminant(self), Some(id.0))),)*
                    $(Edit::$sv { .. } => Some((std::mem::discriminant(self), None)),)*
                    Edit::Batch(_) => None,
                }
            }
        }
    };
}

edits! {
    entities {
        Device(DeviceId, Device) => set_device,
        Link(LinkId, Link) => set_link,
        Area(AreaId, Area) => set_area,
        Vlan(VlanId, Vlan) => set_vlan,
        Subnet(SubnetId, Subnet) => set_subnet,
        Ssid(SsidId, Ssid) => set_ssid,
    }
    singletons {
        Meta(ProjectMeta) => meta,
        Services(Services) => services,
    }
}

fn same_shape(a: &[Edit], b: &[Edit]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.target().is_some() && x.target() == y.target())
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
    fn coalesced_batches_stay_flat() {
        let mut p = Project::default();
        let mut h = History::default();
        let (id, e) = p.add_device_edit(DeviceKind::Pc, Pos::default());
        h.apply(&mut p, e, "Add");
        for _ in 0..50 {
            let d = p.device(id).unwrap().clone();
            let mut moved = d.clone();
            moved.pos = d.pos.offset(1.0, 0.0);
            let edit = Edit::Batch(vec![Edit::Device { id, before: Some(d), after: Some(moved) }]);
            h.apply_coalescing(&mut p, edit, "Nudge", "nudge".into());
        }
        let top = &h.undo.last().unwrap().edit;
        assert!(matches!(top, Edit::Batch(v) if v.len() == 1), "merged batch must not nest");
        h.undo(&mut p);
        assert_eq!(p.device(id).unwrap().pos, Pos::default());
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
