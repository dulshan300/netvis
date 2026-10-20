use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::area::Area;
use crate::device::{Device, DeviceKind};
use crate::edit::Edit;
use crate::geom::{Pos, Rect};
use crate::ids::{AreaId, DeviceId, LinkId, PortId, SsidId, SubnetId, VlanId};
use crate::link::{CableKind, Endpoint, Link};
use crate::port::PortMedia;
use crate::routing::Services;
use crate::select::Selection;
use crate::subnet::Subnet;
use crate::vlan::{Ssid, Vlan};

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
    pub vlans: Vec<Vlan>,
    pub subnets: Vec<Subnet>,
    pub ssids: Vec<Ssid>,
    pub services: Services,
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

    pub fn vlan(&self, id: VlanId) -> Option<&Vlan> {
        self.vlans.iter().find(|v| v.id == id)
    }

    pub fn subnet(&self, id: SubnetId) -> Option<&Subnet> {
        self.subnets.iter().find(|s| s.id == id)
    }

    pub fn ssid(&self, id: SsidId) -> Option<&Ssid> {
        self.ssids.iter().find(|s| s.id == id)
    }

    /// `"20 · Staff"`, or a placeholder for dangling references.
    pub fn vlan_label(&self, id: Option<VlanId>) -> String {
        match id {
            None => "—".into(),
            Some(id) => self.vlan(id).map(Vlan::label).unwrap_or_else(|| "(deleted VLAN)".into()),
        }
    }

    /// Lowest unused multiple of 10 (10, 20, …), falling back to any free tag.
    pub fn next_vlan_tag(&self) -> u16 {
        let used = |t: u16| self.vlans.iter().any(|v| v.tag == t);
        (1..=409u16)
            .map(|n| n * 10)
            .find(|t| !used(*t))
            .or_else(|| (Vlan::MIN_TAG..=Vlan::MAX_TAG).find(|t| !used(*t)))
            .unwrap_or(1)
    }

    pub(crate) fn set_vlan(&mut self, id: VlanId, value: Option<Vlan>) {
        set_entity(&mut self.vlans, |v| v.id == id, value);
    }

    pub(crate) fn set_subnet(&mut self, id: SubnetId, value: Option<Subnet>) {
        set_entity(&mut self.subnets, |s| s.id == id, value);
    }

    pub(crate) fn set_ssid(&mut self, id: SsidId, value: Option<Ssid>) {
        set_entity(&mut self.ssids, |s| s.id == id, value);
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

    /// The (first) cable on a port. Patch-panel ports can have two; see
    /// [`Project::port_links`] and [`Project::peer`].
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
        // Unused ports first; then, on patch panels, ports with only one side cabled.
        let cap = if d.kind.is_passthrough() { 2 } else { 1 };
        let pick = |limit: usize| {
            let free = |p: &&crate::port::Port| self.port_links(p.id).count() < limit;
            prefer.and_then(|m| d.ports.iter().filter(free).find(|p| p.media == m)).or_else(|| d.ports.iter().find(free))
        };
        pick(1).or_else(|| pick(cap)).map(|p| p.id)
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
            if self.port_links(ep.port).count() >= self.port_capacity(ep) {
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

    /// Remove everything in `sel` in one undoable step. Links attached to
    /// removed devices go too, and references to removed VLANs are cleared.
    pub fn remove_edit(&self, sel: &Selection) -> Edit {
        let mut edits = Vec::new();
        let doomed_link = |l: &Link| sel.links.contains(&l.id) || sel.devices.iter().any(|d| l.touches_device(*d));
        for l in self.links.iter().filter(|l| doomed_link(l)) {
            edits.push(Edit::Link { id: l.id, before: Some(l.clone()), after: None });
        }
        for d in &self.devices {
            if sel.devices.contains(&d.id) {
                edits.push(Edit::Device { id: d.id, before: Some(d.clone()), after: None });
            } else {
                let mut after = d.clone();
                let mut changed = !sel.vlans.is_empty() && after.forget_vlans(&sel.vlans);
                if after.hw.rack.is_some_and(|m| sel.areas.contains(&m.rack)) {
                    after.hw.rack = None;
                    changed = true;
                }
                if changed {
                    edits.push(Edit::Device { id: d.id, before: Some(d.clone()), after: Some(after) });
                }
            }
        }
        for a in self.areas.iter().filter(|a| sel.areas.contains(&a.id)) {
            edits.push(Edit::Area { id: a.id, before: Some(a.clone()), after: None });
        }
        for s in &self.subnets {
            if sel.subnets.contains(&s.id) {
                edits.push(Edit::Subnet { id: s.id, before: Some(s.clone()), after: None });
            } else if s.vlan.is_some_and(|v| sel.vlans.contains(&v)) {
                let after = Subnet { vlan: None, ..s.clone() };
                edits.push(Edit::Subnet { id: s.id, before: Some(s.clone()), after: Some(after) });
            }
        }
        for s in &self.ssids {
            if sel.ssids.contains(&s.id) {
                edits.push(Edit::Ssid { id: s.id, before: Some(s.clone()), after: None });
            } else if s.vlan.is_some_and(|v| sel.vlans.contains(&v)) {
                let after = Ssid { vlan: None, ..s.clone() };
                edits.push(Edit::Ssid { id: s.id, before: Some(s.clone()), after: Some(after) });
            }
        }
        for v in self.vlans.iter().filter(|v| sel.vlans.contains(&v.id)) {
            edits.push(Edit::Vlan { id: v.id, before: Some(v.clone()), after: None });
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

/// What [`Project::copy_with_rename_edit`] made.
#[derive(Debug, Clone, Default)]
pub struct CopyResult {
    pub devices: Vec<DeviceId>,
    pub areas: Vec<AreaId>,
    /// Copies that couldn't take the replaced name (it was taken) and got
    /// a numbered one instead.
    pub renumbered: usize,
}

impl Project {
    /// Copy areas (with everything inside them) and devices, offset by
    /// `delta`, replacing `find` with `replace` in their names: copy the
    /// "L1" floor as "L2" and `SW-L1-SEC` becomes `SW-L2-SEC`. Cables
    /// between copied devices are copied too, and devices mounted in a
    /// copied rack keep their rack position in the copy.
    pub fn copy_with_rename_edit(&self, areas: &[AreaId], devices: &[DeviceId], delta: Pos, find: &str, replace: &str) -> (CopyResult, Edit) {
        // Everything inside the chosen areas comes along.
        let mut area_ids: Vec<AreaId> = Vec::new();
        for a in self.areas.iter().filter(|a| areas.contains(&a.id)) {
            for b in self.areas.iter().filter(|b| a.rect.contains_rect(&b.rect)) {
                if !area_ids.contains(&b.id) {
                    area_ids.push(b.id);
                }
            }
        }
        let mut dev_ids: Vec<DeviceId> = devices.to_vec();
        for a in &area_ids {
            for d in self.devices_in_area(*a) {
                if !dev_ids.contains(&d.id) {
                    dev_ids.push(d.id);
                }
            }
        }
        let rename = |name: &str| if find.is_empty() { name.to_owned() } else { name.replace(find, replace) };
        let mut out = CopyResult::default();
        let mut edits = Vec::new();

        let mut area_map = std::collections::HashMap::new();
        for a in self.areas.iter().filter(|a| area_ids.contains(&a.id)) {
            let mut copy = a.clone();
            copy.id = AreaId::new();
            copy.name = rename(&a.name);
            copy.rect.min = a.rect.min.offset(delta.x, delta.y);
            area_map.insert(a.id, copy.id);
            out.areas.push(copy.id);
            edits.push(Edit::Area { id: copy.id, before: None, after: Some(copy) });
        }

        let mut names: Vec<String> = self.devices.iter().map(|d| d.name.clone()).collect();
        let mut port_map = std::collections::HashMap::new();
        for d in self.devices.iter().filter(|d| dev_ids.contains(&d.id)) {
            let wanted = rename(&d.name);
            let name = if !names.iter().any(|e| e.eq_ignore_ascii_case(&wanted)) {
                wanted
            } else {
                out.renumbered += 1;
                let prefix = d.kind.name_prefix();
                (1..).map(|n| format!("{prefix}-{n:02}")).find(|n| !names.iter().any(|e| e.eq_ignore_ascii_case(n))).expect("unbounded range")
            };
            names.push(name.clone());
            let mut copy = d.duplicate(name, d.pos.offset(delta.x, delta.y));
            copy.hw.rack = d.hw.rack.and_then(|m| area_map.get(&m.rack).map(|r| crate::RackMount { rack: *r, u: m.u }));
            for (old, new) in d.ports.iter().zip(&copy.ports) {
                port_map.insert(old.id, Endpoint { device: copy.id, port: new.id });
            }
            out.devices.push(copy.id);
            edits.push(Edit::Device { id: copy.id, before: None, after: Some(copy) });
        }
        for l in &self.links {
            if let (Some(a), Some(b)) = (port_map.get(&l.a.port), port_map.get(&l.b.port)) {
                let mut copy = l.clone();
                copy.id = LinkId::new();
                copy.a = *a;
                copy.b = *b;
                copy.label = rename(&l.label);
                edits.push(Edit::Link { id: copy.id, before: None, after: Some(copy) });
            }
        }
        (out, Edit::Batch(edits))
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

        let e = p.remove_edit(&Selection { devices: vec![a], ..Default::default() });
        h.apply(&mut p, e, "delete");
        assert!(p.device(a).is_none());
        assert!(p.links.is_empty());

        h.undo(&mut p);
        assert_eq!(p.devices.len(), snapshot.devices.len());
        assert_eq!(p.links, snapshot.links);
        assert_eq!(p.device(a), snapshot.device(a));
    }

    #[test]
    fn removing_vlan_clears_references_and_undo_restores() {
        use crate::iface::{AllowedVlans, LogicalInterface, LogicalKind, SwitchportMode};
        let (mut p, mut h, a, _) = two_switches();
        let v10 = Vlan::new(10, "Mgmt");
        let v20 = Vlan::new(20, "Staff");
        let (i10, i20) = (v10.id, v20.id);
        for v in [v10, v20] {
            h.apply(&mut p, Edit::Vlan { id: v.id, before: None, after: Some(v) }, "vlan");
        }
        let subnet = Subnet { vlan: Some(i20), ..Subnet::new("Staff", "10.0.20.0/24".parse().unwrap()) };
        h.apply(&mut p, Edit::Subnet { id: subnet.id, before: None, after: Some(subnet) }, "subnet");
        let mut d = p.device(a).unwrap().clone();
        let before = d.clone();
        d.ports[0].cfg.mode = SwitchportMode::Access { vlan: Some(i20) };
        d.ports[1].cfg.mode = SwitchportMode::Trunk { native: Some(i10), allowed: AllowedVlans::List(vec![i10, i20]) };
        d.logical.push(LogicalInterface::new("Vlan20", LogicalKind::Svi { vlan: Some(i20) }));
        h.apply(&mut p, Edit::Device { id: a, before: Some(before), after: Some(d) }, "cfg");
        let snapshot = p.clone();

        let e = p.remove_edit(&Selection { vlans: vec![i20], ..Default::default() });
        h.apply(&mut p, e, "delete vlan");
        assert!(p.vlan(i20).is_none());
        let d = p.device(a).unwrap();
        assert_eq!(d.ports[0].cfg.mode, SwitchportMode::Access { vlan: None });
        assert_eq!(d.ports[1].cfg.mode, SwitchportMode::Trunk { native: Some(i10), allowed: AllowedVlans::List(vec![i10]) });
        assert_eq!(d.logical[0].kind, LogicalKind::Svi { vlan: None });
        assert_eq!(p.subnets[0].vlan, None);

        h.undo(&mut p);
        assert_eq!(p.device(a), snapshot.device(a));
        assert_eq!(p.subnets, snapshot.subnets);
        assert_eq!(p.vlans.len(), 2);
    }

    #[test]
    fn next_vlan_tag_fills_gaps() {
        let mut p = Project::default();
        assert_eq!(p.next_vlan_tag(), 10);
        p.vlans.push(Vlan::new(10, "a"));
        p.vlans.push(Vlan::new(30, "b"));
        assert_eq!(p.next_vlan_tag(), 20);
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
