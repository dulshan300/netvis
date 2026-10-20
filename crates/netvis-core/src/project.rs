use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::area::Area;
use crate::device::{Device, DeviceKind};
use crate::edit::Edit;
use crate::geom::{Pos, Rect};
use crate::ids::{AreaId, DeviceId, LinkId, PortId};
use crate::link::{CableKind, Endpoint, Link};
use crate::port::PortMedia;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ProjectMeta {
    pub name: String,
    pub client: String,
    pub author: String,
    pub company: String,
    pub revision: String,
    pub description: String,
}

impl Default for ProjectMeta {
    fn default() -> Self {
        Self {
            name: "Untitled network".to_owned(),
            client: String::new(),
            author: String::new(),
            company: "Corescale".to_owned(),
            revision: "A".to_owned(),
            description: String::new(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Project {
    pub meta: ProjectMeta,
    /// Draw order: later devices are on top.
    pub devices: Vec<Device>,
    pub links: Vec<Link>,
    /// Draw order: later areas are on top (all areas are behind devices).
    pub areas: Vec<Area>,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum LinkError {
    #[error("unknown device or port")]
    UnknownEndpoint,
    #[error("a port cannot be linked to itself")]
    SamePort,
    #[error("port {0} is already connected")]
    PortInUse(String),
}

impl Project {
    pub fn device(&self, id: DeviceId) -> Option<&Device> {
        self.devices.iter().find(|d| d.id == id)
    }

    pub fn device_mut(&mut self, id: DeviceId) -> Option<&mut Device> {
        self.devices.iter_mut().find(|d| d.id == id)
    }

    pub fn link(&self, id: LinkId) -> Option<&Link> {
        self.links.iter().find(|l| l.id == id)
    }

    pub fn area(&self, id: AreaId) -> Option<&Area> {
        self.areas.iter().find(|a| a.id == id)
    }

    pub fn area_mut(&mut self, id: AreaId) -> Option<&mut Area> {
        self.areas.iter_mut().find(|a| a.id == id)
    }

    pub(crate) fn set_device(&mut self, id: DeviceId, value: Option<Device>) {
        set_entity(&mut self.devices, |d| d.id == id, value);
    }

    pub(crate) fn set_link(&mut self, id: LinkId, value: Option<Link>) {
        set_entity(&mut self.links, |l| l.id == id, value);
    }

    pub(crate) fn set_area(&mut self, id: AreaId, value: Option<Area>) {
        set_entity(&mut self.areas, |a| a.id == id, value);
    }

    pub fn port_link(&self, port: PortId) -> Option<&Link> {
        self.links.iter().find(|l| l.touches_port(port))
    }

    pub fn links_of(&self, device: DeviceId) -> impl Iterator<Item = &Link> {
        self.links.iter().filter(move |l| l.touches_device(device))
    }

    pub fn endpoint_media(&self, ep: Endpoint) -> Option<PortMedia> {
        self.device(ep.device)?.port(ep.port).map(|p| p.media)
    }

    /// First unconnected port on `device`, preferring `prefer` media if given.
    pub fn first_free_port(&self, device: DeviceId, prefer: Option<PortMedia>) -> Option<PortId> {
        let d = self.device(device)?;
        let free = |p: &&crate::port::Port| self.port_link(p.id).is_none();
        prefer
            .and_then(|m| d.ports.iter().filter(free).find(|p| p.media == m))
            .or_else(|| d.ports.iter().find(free))
            .map(|p| p.id)
    }

    pub fn check_link(&self, a: Endpoint, b: Endpoint) -> Result<(), LinkError> {
        let port_of = |ep: Endpoint| self.device(ep.device).and_then(|d| d.port(ep.port));
        let (Some(pa), Some(pb)) = (port_of(a), port_of(b)) else {
            return Err(LinkError::UnknownEndpoint);
        };
        if a.port == b.port {
            return Err(LinkError::SamePort);
        }
        for (ep, port) in [(a, pa), (b, pb)] {
            if self.port_link(ep.port).is_some() {
                let dev = self.device(ep.device).map(|d| d.name.as_str()).unwrap_or("?");
                return Err(LinkError::PortInUse(format!("{dev}:{}", port.name)));
            }
        }
        Ok(())
    }

    /// Hostname like `SW-03` that is not yet used.
    pub fn next_device_name(&self, kind: DeviceKind) -> String {
        let prefix = kind.name_prefix();
        (1..)
            .map(|n| format!("{prefix}-{n:02}"))
            .find(|name| !self.devices.iter().any(|d| d.name.eq_ignore_ascii_case(name)))
            .expect("unbounded range")
    }

    /// Bounding box of all content, if any.
    pub fn bounds(&self) -> Option<Rect> {
        self.devices
            .iter()
            .map(Device::rect)
            .chain(self.areas.iter().map(|a| a.rect))
            .reduce(|a, b| a.union(&b))
    }

    // ---- Edit builders -------------------------------------------------
    // These describe a change without applying it; pass the result to
    // `History::apply` so it becomes undoable.

    pub fn add_device_edit(&self, kind: DeviceKind, pos: Pos) -> (DeviceId, Edit) {
        let d = Device::new(kind, self.next_device_name(kind), pos);
        (d.id, Edit::Device { id: d.id, before: None, after: Some(d) })
    }

    pub fn add_link_edit(&self, a: Endpoint, b: Endpoint) -> Result<(LinkId, Edit), LinkError> {
        self.check_link(a, b)?;
        let cable = match (self.endpoint_media(a), self.endpoint_media(b)) {
            (Some(ma), Some(mb)) => CableKind::suggest(ma, mb),
            _ => CableKind::Unspecified,
        };
        let link = Link::new(a, b, cable);
        Ok((link.id, Edit::Link { id: link.id, before: None, after: Some(link) }))
    }

    /// Remove devices (with their links), links and areas in one undoable step.
    pub fn remove_edit(&self, devices: &[DeviceId], links: &[LinkId], areas: &[AreaId]) -> Edit {
        let mut edits = Vec::new();
        let doomed_link = |l: &Link| links.contains(&l.id) || devices.iter().any(|d| l.touches_device(*d));
        for l in self.links.iter().filter(|l| doomed_link(l)) {
            edits.push(Edit::Link { id: l.id, before: Some(l.clone()), after: None });
        }
        for d in self.devices.iter().filter(|d| devices.contains(&d.id)) {
            edits.push(Edit::Device { id: d.id, before: Some(d.clone()), after: None });
        }
        for a in self.areas.iter().filter(|a| areas.contains(&a.id)) {
            edits.push(Edit::Area { id: a.id, before: Some(a.clone()), after: None });
        }
        Edit::Batch(edits)
    }

    /// Copies of `devices`, offset by `delta`, plus copies of links between them.
    /// Returns the new device ids.
    pub fn duplicate_edit(&self, devices: &[DeviceId], delta: Pos) -> (Vec<DeviceId>, Edit) {
        let mut names: Vec<String> = self.devices.iter().map(|d| d.name.clone()).collect();
        let mut port_map = std::collections::HashMap::new();
        let mut new_ids = Vec::new();
        let mut edits = Vec::new();
        for d in self.devices.iter().filter(|d| devices.contains(&d.id)) {
            let prefix = d.kind.name_prefix();
            let name = (1..)
                .map(|n| format!("{prefix}-{n:02}"))
                .find(|n| !names.iter().any(|e| e.eq_ignore_ascii_case(n)))
                .expect("unbounded range");
            names.push(name.clone());
            let copy = d.duplicate(name, d.pos.offset(delta.x, delta.y));
            for (old, new) in d.ports.iter().zip(&copy.ports) {
                port_map.insert(old.id, Endpoint { device: copy.id, port: new.id });
            }
            new_ids.push(copy.id);
            edits.push(Edit::Device { id: copy.id, before: None, after: Some(copy) });
        }
        for l in &self.links {
            if let (Some(a), Some(b)) = (port_map.get(&l.a.port), port_map.get(&l.b.port)) {
                let mut copy = l.clone();
                copy.id = LinkId::new();
                copy.a = *a;
                copy.b = *b;
                edits.push(Edit::Link { id: copy.id, before: None, after: Some(copy) });
            }
        }
        (new_ids, Edit::Batch(edits))
    }
}

fn set_entity<T>(items: &mut Vec<T>, matches: impl Fn(&T) -> bool, value: Option<T>) {
    match (items.iter().position(&matches), value) {
        (Some(i), Some(v)) => items[i] = v,
        (Some(i), None) => {
            items.remove(i);
        }
        (None, Some(v)) => items.push(v),
        (None, None) => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edit::History;

    fn two_switches() -> (Project, History, DeviceId, DeviceId) {
        let mut p = Project::default();
        let mut h = History::default();
        let (a, e) = p.add_device_edit(DeviceKind::SwitchL2, Pos::default());
        h.apply(&mut p, e, "add");
        let (b, e) = p.add_device_edit(DeviceKind::SwitchL2, Pos::new(300.0, 0.0));
        h.apply(&mut p, e, "add");
        (p, h, a, b)
    }

    #[test]
    fn names_are_unique_and_sequential() {
        let (p, _, a, b) = two_switches();
        assert_eq!(p.device(a).unwrap().name, "SW-01");
        assert_eq!(p.device(b).unwrap().name, "SW-02");
        assert_eq!(p.next_device_name(DeviceKind::SwitchL2), "SW-03");
    }

    #[test]
    fn link_rules() {
        let (mut p, mut h, a, b) = two_switches();
        let pa = p.first_free_port(a, Some(PortMedia::SfpPlus)).unwrap();
        let pb = p.first_free_port(b, Some(PortMedia::SfpPlus)).unwrap();
        let (ea, eb) = (Endpoint { device: a, port: pa }, Endpoint { device: b, port: pb });

        let (_, e) = p.add_link_edit(ea, eb).unwrap();
        h.apply(&mut p, e, "link");
        assert_eq!(p.links[0].cable, CableKind::Om4);
        assert!(matches!(p.add_link_edit(ea, eb), Err(LinkError::PortInUse(_))));
        assert_eq!(p.add_link_edit(ea, ea).unwrap_err(), LinkError::SamePort);
        assert_ne!(p.first_free_port(a, Some(PortMedia::SfpPlus)), Some(pa));
    }

    #[test]
    fn removing_device_removes_links_and_undo_restores() {
        let (mut p, mut h, a, b) = two_switches();
        let ea = Endpoint { device: a, port: p.first_free_port(a, None).unwrap() };
        let eb = Endpoint { device: b, port: p.first_free_port(b, None).unwrap() };
        let (_, e) = p.add_link_edit(ea, eb).unwrap();
        h.apply(&mut p, e, "link");
        let snapshot = p.clone();

        let e = p.remove_edit(&[a], &[], &[]);
        h.apply(&mut p, e, "delete");
        assert!(p.device(a).is_none());
        assert!(p.links.is_empty());

        h.undo(&mut p);
        assert_eq!(p.devices.len(), snapshot.devices.len());
        assert_eq!(p.links, snapshot.links);
        assert_eq!(p.device(a), snapshot.device(a));
    }

    #[test]
    fn duplicate_copies_internal_links_only() {
        let (mut p, mut h, a, b) = two_switches();
        let ea = Endpoint { device: a, port: p.first_free_port(a, None).unwrap() };
        let eb = Endpoint { device: b, port: p.first_free_port(b, None).unwrap() };
        let (_, e) = p.add_link_edit(ea, eb).unwrap();
        h.apply(&mut p, e, "link");

        let (ids, e) = p.duplicate_edit(&[a], Pos::new(0.0, 200.0));
        h.apply(&mut p, e, "dup");
        assert_eq!(ids.len(), 1);
        assert_eq!(p.links.len(), 1, "external link must not be copied");

        let (ids, e) = p.duplicate_edit(&[a, b], Pos::new(0.0, 400.0));
        h.apply(&mut p, e, "dup");
        assert_eq!(ids.len(), 2);
        assert_eq!(p.links.len(), 2, "internal link is copied");
        assert_eq!(p.device(ids[0]).unwrap().name, "SW-04");
    }
}
