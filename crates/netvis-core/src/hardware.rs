//! Physical attributes of devices: catalogue models, rack placement and
//! PoE power budgets.

use std::collections::HashSet;

use netvis_catalog::models::{self, DeviceModel, PoeSpec};
use serde::{Deserialize, Serialize};

use crate::area::AreaKind;
use crate::device::{Device, DeviceKind};
use crate::edit::Edit;
use crate::iface::{LogicalKind, RadioConfig, WifiStandard};
use crate::ids::{AreaId, DeviceId, PortId};
use crate::link::Endpoint;
use crate::port::{PoeRole, PoeStandard, Port, PortMedia};
use crate::project::Project;

/// Where a device sits in a rack elevation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RackMount {
    /// A [`crate::Area`] of kind `Rack`.
    pub rack: AreaId,
    /// Lowest rack unit occupied, 1-based from the bottom.
    pub u: u8,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Hardware {
    /// Catalogue model id the port layout came from.
    pub model: Option<String>,
    /// Height in rack units; 0 = not rack-mountable.
    pub rack_units: u8,
    pub rack: Option<RackMount>,
    /// Total PoE the device can source.
    pub poe_budget_w: Option<f32>,
    /// Typical draw from mains, excluding PoE delivered to others.
    pub power_w: Option<f32>,
    /// PoE draw at this device when it is powered over Ethernet.
    pub pd_draw_w: Option<f32>,
    /// Output capacity (UPS/PDU).
    pub capacity_w: Option<f32>,
}

impl DeviceKind {
    /// Stable string key (`"SwitchL2"`), as used by the model catalogue.
    pub fn key(self) -> String {
        match serde_json::to_value(self) {
            Ok(serde_json::Value::String(s)) => s,
            _ => unreachable!("DeviceKind serialises as a string"),
        }
    }

    pub fn from_key(key: &str) -> Option<DeviceKind> {
        serde_json::from_value(serde_json::Value::String(key.to_owned())).ok()
    }

    pub fn default_hardware(self) -> Hardware {
        use DeviceKind::*;
        let rack_units = match self {
            Router | SwitchL2 | SwitchL3 | Firewall | WirelessController | LoadBalancer | VpnGateway | Server
            | PatchPanel | Pdu => 1,
            VmHost | Storage | Ups => 2,
            _ => 0,
        };
        let poe_budget_w = match self {
            // Matches the default 24 × PoE+ template.
            SwitchL2 => Some(370.0),
            PoeInjector => Some(60.0),
            _ => None,
        };
        Hardware { rack_units, poe_budget_w, ..Hardware::default() }
    }

    pub fn models(self) -> impl Iterator<Item = &'static DeviceModel> {
        let key = self.key();
        models::models().iter().filter(move |m| m.kind == key)
    }
}

/// One powered device hanging off a PSE port.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PoeLoad {
    pub pse_port: PortId,
    pub pd: Endpoint,
    pub pse_class: PoeStandard,
    pub pd_class: PoeStandard,
    /// Power consumed at the PD.
    pub draw_w: f32,
    /// Power the PSE must budget, including worst-case cable loss.
    pub demand_w: f32,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct PoeReport {
    pub budget_w: Option<f32>,
    pub demand_w: f32,
    pub loads: Vec<PoeLoad>,
    /// PSE-capable ports in total.
    pub pse_ports: usize,
}

impl PoeReport {
    pub fn fraction(&self) -> Option<f32> {
        self.budget_w.filter(|b| *b > 0.0).map(|b| self.demand_w / b)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct RackPower {
    pub load_w: f32,
    pub ups_capacity_w: f32,
    /// Devices without a `power_w` figure (excluded from the load).
    pub unknown: usize,
}

/// A device placed in a rack elevation.
#[derive(Debug, Clone, Copy)]
pub struct Mounted<'a> {
    pub device: &'a Device,
    pub bottom: u8,
    pub height: u8,
}

impl Mounted<'_> {
    pub fn top(&self) -> u8 {
        self.bottom + self.height.max(1) - 1
    }
}

impl Project {
    // ---- models ----------------------------------------------------------

    /// Re-shape a device after a catalogue model. Ports keep their identity,
    /// configuration and cabling when a port of the same name exists in the
    /// model; links on ports the model doesn't have are removed.
    /// Returns the edit and the number of links that will be removed.
    pub fn apply_model_edit(&self, device: DeviceId, model_id: &str) -> Option<(Edit, usize)> {
        let m = models::model(model_id)?;
        let d = self.device(device)?;
        let mut nd = d.clone();

        let mut ports = Vec::new();
        for g in &m.ports {
            for name in g.names() {
                let mut port = d.ports.iter().find(|p| p.name == name).cloned().unwrap_or_else(|| Port::new(name, g.media));
                port.media = g.media;
                port.speed_mbps = g.speed.unwrap_or_else(|| g.media.default_speed_mbps());
                port.poe = match g.poe {
                    Some(PoeSpec::Pse(s)) => PoeRole::Source(s),
                    Some(PoeSpec::Pd(s)) => PoeRole::Powered(s),
                    None => PoeRole::None,
                };
                if g.media == PortMedia::Wireless {
                    let radio = port.cfg.radio.get_or_insert_with(RadioConfig::default);
                    if let Some(std) = m.wifi.as_deref().and_then(wifi_from_key) {
                        radio.standard = std;
                        if !std.bands().contains(&radio.band) {
                            radio.band = std.bands()[0];
                            radio.channel = radio.band.default_channel();
                        }
                    }
                } else {
                    port.cfg.radio = None;
                }
                let fits = port.transceiver.as_deref().and_then(netvis_catalog::transceiver::transceiver).is_some_and(|t| t.fits(port.media));
                if !fits {
                    port.transceiver = None;
                }
                ports.push(port);
            }
        }
        nd.ports = ports;
        let kept: HashSet<PortId> = nd.ports.iter().map(|p| p.id).collect();
        for l in &mut nd.logical {
            if let LogicalKind::SubInterface { parent, .. } = &mut l.kind
                && parent.is_some_and(|p| !kept.contains(&p))
            {
                *parent = None;
            }
        }
        nd.model = m.name.clone();
        nd.hw = Hardware {
            model: Some(m.id.clone()),
            rack_units: m.rack_units,
            rack: d.hw.rack,
            poe_budget_w: m.poe_budget_w,
            power_w: m.power_w,
            pd_draw_w: m.pd_draw_w,
            capacity_w: m.capacity_w,
        };

        let mut edits: Vec<Edit> = self
            .links
            .iter()
            .filter(|l| [l.a, l.b].iter().any(|e| e.device == device && !kept.contains(&e.port)))
            .map(|l| Edit::Link { id: l.id, before: Some(l.clone()), after: None })
            .collect();
        let removed = edits.len();
        edits.push(Edit::Device { id: device, before: Some(d.clone()), after: Some(nd) });
        Some((Edit::Batch(edits), removed))
    }

    // ---- PoE -------------------------------------------------------------

    pub fn poe_report(&self, device: DeviceId) -> PoeReport {
        let Some(d) = self.device(device) else { return PoeReport::default() };
        let mut r = PoeReport { budget_w: d.hw.poe_budget_w, ..Default::default() };
        for port in &d.ports {
            let PoeRole::Source(pse_class) = port.poe else { continue };
            r.pse_ports += 1;
            let Some(link) = self.port_link(port.id) else { continue };
            let far = if link.a.port == port.id { link.b } else { link.a };
            let Some(fd) = self.device(far.device) else { continue };
            let Some(PoeRole::Powered(pd_class)) = fd.port(far.port).map(|p| p.poe) else { continue };
            // The port can't deliver more than the lower of the two classes.
            let class = pd_class.min(pse_class);
            let draw_w = fd.hw.pd_draw_w.unwrap_or(pd_class.pd_watts()).min(class.pd_watts());
            let demand_w = draw_w * class.pse_watts() / class.pd_watts();
            r.demand_w += demand_w;
            r.loads.push(PoeLoad { pse_port: port.id, pd: far, pse_class, pd_class, draw_w, demand_w });
        }
        r
    }

    // ---- racks -----------------------------------------------------------

    pub fn racks(&self) -> impl Iterator<Item = &crate::area::Area> {
        self.areas.iter().filter(|a| a.kind == AreaKind::Rack)
    }

    /// Devices placed in `rack`, top first.
    pub fn rack_contents(&self, rack: AreaId) -> Vec<Mounted<'_>> {
        let mut v: Vec<_> = self
            .devices
            .iter()
            .filter_map(|d| {
                let m = d.hw.rack.filter(|m| m.rack == rack)?;
                Some(Mounted { device: d, bottom: m.u, height: d.hw.rack_units })
            })
            .collect();
        v.sort_by_key(|m| std::cmp::Reverse(m.bottom));
        v
    }

    /// Power picture for a rack: equipment load (mains draw plus PoE it
    /// delivers), UPS capacity, and how many devices have no power figure.
    pub fn rack_power(&self, rack: AreaId) -> RackPower {
        let mut r = RackPower::default();
        for m in self.rack_contents(rack) {
            match m.device.kind {
                DeviceKind::Ups => r.ups_capacity_w += m.device.hw.capacity_w.unwrap_or(0.0),
                DeviceKind::Pdu | DeviceKind::PatchPanel => {}
                _ => {
                    let poe = self.poe_report(m.device.id).demand_w;
                    match m.device.hw.power_w {
                        Some(w) => r.load_w += w + poe,
                        None => {
                            r.load_w += poe;
                            r.unknown += 1;
                        }
                    }
                }
            }
        }
        r
    }

    /// Devices whose card centre lies inside the area on the canvas.
    pub fn devices_in_area(&self, area: AreaId) -> Vec<&Device> {
        let Some(a) = self.area(area) else { return Vec::new() };
        self.devices
            .iter()
            .filter(|d| {
                let r = d.rect();
                a.rect.contains(r.min.offset(r.size.x / 2.0, r.size.y / 2.0))
            })
            .collect()
    }

    /// Lowest (or highest, if `from_top`) free position for a device of
    /// `height` units, ignoring `ignore` (the device being moved).
    pub fn rack_free_slot(&self, rack: AreaId, height: u8, from_top: bool, ignore: Option<DeviceId>) -> Option<u8> {
        let units = self.area(rack)?.rack_units;
        let h = height.max(1);
        if h > units {
            return None;
        }
        let taken: HashSet<u8> = self
            .rack_contents(rack)
            .iter()
            .filter(|m| Some(m.device.id) != ignore)
            .flat_map(|m| m.bottom..=m.top())
            .collect();
        let fits = |b: u8| (b..b + h).all(|u| !taken.contains(&u));
        let mut candidates: Vec<u8> = (1..=units - h + 1).collect();
        if from_top {
            candidates.reverse();
        }
        candidates.into_iter().find(|b| fits(*b))
    }

    /// Mount every rack-mountable device drawn inside the rack area that has
    /// no position yet. Network gear fills from the top, heavy kit (servers,
    /// storage, UPS) from the bottom. Returns the edit, how many were placed
    /// and the names of devices that didn't fit.
    pub fn auto_mount_edit(&self, rack: AreaId) -> (Edit, usize, Vec<String>) {
        let mut work = self.clone();
        let mut edits = Vec::new();
        let mut no_room = Vec::new();
        let candidates: Vec<DeviceId> = self
            .devices_in_area(rack)
            .iter()
            .filter(|d| d.hw.rack.is_none() && d.hw.rack_units > 0)
            .map(|d| d.id)
            .collect();
        for id in candidates {
            let d = work.device(id).expect("exists").clone();
            let from_top = !matches!(d.kind, DeviceKind::Server | DeviceKind::VmHost | DeviceKind::Storage | DeviceKind::Ups);
            match work.rack_free_slot(rack, d.hw.rack_units, from_top, None) {
                Some(u) => {
                    let mut after = d.clone();
                    after.hw.rack = Some(RackMount { rack, u });
                    let e = Edit::Device { id, before: Some(d), after: Some(after) };
                    e.apply(&mut work);
                    edits.push(e);
                }
                None => no_room.push(d.name.clone()),
            }
        }
        let n = edits.len();
        (Edit::Batch(edits), n, no_room)
    }
}

fn wifi_from_key(key: &str) -> Option<WifiStandard> {
    serde_json::from_value(serde_json::Value::String(key.to_owned())).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::area::Area;
    use crate::edit::History;
    use crate::geom::{Pos, Rect};

    fn add(p: &mut Project, h: &mut History, kind: DeviceKind, x: f32, y: f32) -> DeviceId {
        let (id, e) = p.add_device_edit(kind, Pos::new(x, y));
        h.apply(p, e, "");
        id
    }

    fn connect(p: &mut Project, h: &mut History, a: DeviceId, b: DeviceId) {
        let ea = Endpoint { device: a, port: p.first_free_port(a, Some(PortMedia::Rj45)).unwrap() };
        let eb = Endpoint { device: b, port: p.first_free_port(b, Some(PortMedia::Rj45)).unwrap() };
        let (_, e) = p.add_link_edit(ea, eb).unwrap();
        h.apply(p, e, "");
    }

    #[test]
    fn kind_keys_roundtrip() {
        for k in DeviceKind::ALL {
            assert_eq!(DeviceKind::from_key(&k.key()), Some(k));
        }
        assert!(DeviceKind::SwitchL2.models().count() >= 3);
        for m in models::models() {
            assert!(DeviceKind::from_key(&m.kind).is_some(), "model {} has unknown kind {}", m.id, m.kind);
        }
    }

    #[test]
    fn poe_budget_counts_class_and_measured_draw() {
        let (mut p, mut h) = (Project::default(), History::default());
        let sw = add(&mut p, &mut h, DeviceKind::SwitchL2, 0.0, 0.0);
        let ap = add(&mut p, &mut h, DeviceKind::AccessPoint, 0.0, 200.0);
        let cam = add(&mut p, &mut h, DeviceKind::IpCamera, 200.0, 200.0);
        connect(&mut p, &mut h, sw, ap);
        connect(&mut p, &mut h, sw, cam);
        // The camera reports its actual draw; the AP falls back to its class.
        p.device_mut(cam).unwrap().hw.pd_draw_w = Some(6.0);

        let r = p.poe_report(sw);
        assert_eq!(r.budget_w, Some(370.0));
        assert_eq!(r.loads.len(), 2);
        assert_eq!(r.pse_ports, 24);
        let ap_load = r.loads.iter().find(|l| l.pd.device == ap).unwrap();
        assert!((ap_load.demand_w - 30.0).abs() < 0.01, "at-class PD budgets the full 30 W");
        let cam_load = r.loads.iter().find(|l| l.pd.device == cam).unwrap();
        assert!((cam_load.demand_w - 6.0 * 15.4 / 12.95).abs() < 0.01);
    }

    #[test]
    fn applying_a_model_keeps_matching_ports_and_links() {
        let (mut p, mut h) = (Project::default(), History::default());
        let sw = add(&mut p, &mut h, DeviceKind::SwitchL2, 0.0, 0.0);
        let pc = add(&mut p, &mut h, DeviceKind::Pc, 0.0, 200.0);
        connect(&mut p, &mut h, sw, pc);
        let ge1 = p.device(sw).unwrap().ports[0].id;

        let (e, removed) = p.apply_model_edit(sw, "sw-access-48p-poe").unwrap();
        assert_eq!(removed, 0);
        h.apply(&mut p, e, "model");
        let d = p.device(sw).unwrap();
        assert_eq!(d.ports.len(), 52);
        assert_eq!(d.ports[0].id, ge1, "ge1 keeps its identity");
        assert_eq!(d.hw.poe_budget_w, Some(740.0));
        assert_eq!(p.links.len(), 1);

        // A model without ge* ports drops the link.
        let (e, removed) = p.apply_model_edit(sw, "sw-access-24p-mgig-bt").unwrap();
        assert_eq!(removed, 1);
        h.apply(&mut p, e, "model");
        assert!(p.links.is_empty());
        h.undo(&mut p);
        assert_eq!(p.links.len(), 1);
    }

    #[test]
    fn rack_slots_and_auto_mount() {
        let (mut p, mut h) = (Project::default(), History::default());
        let rack = Area::new(AreaKind::Rack, "R1", Rect::new(0.0, 0.0, 1000.0, 1000.0));
        let rid = rack.id;
        h.apply(&mut p, Edit::Area { id: rid, before: None, after: Some(rack) }, "");
        p.area_mut(rid).unwrap().rack_units = 6;

        let sw = add(&mut p, &mut h, DeviceKind::SwitchL2, 10.0, 10.0);
        let srv = add(&mut p, &mut h, DeviceKind::Storage, 200.0, 10.0);
        let ups = add(&mut p, &mut h, DeviceKind::Ups, 400.0, 10.0);
        let big = add(&mut p, &mut h, DeviceKind::VmHost, 600.0, 10.0);
        let outside = add(&mut p, &mut h, DeviceKind::Router, 2000.0, 10.0);

        let (e, n, no_room) = p.auto_mount_edit(rid);
        h.apply(&mut p, e, "mount");
        assert_eq!(n, 3);
        assert_eq!(no_room, vec!["HV-01".to_owned()]);
        assert_eq!(p.device(sw).unwrap().hw.rack, Some(RackMount { rack: rid, u: 6 }), "switch at the top");
        assert_eq!(p.device(srv).unwrap().hw.rack.unwrap().u, 1, "storage at the bottom");
        assert_eq!(p.device(ups).unwrap().hw.rack.unwrap().u, 3);
        assert!(p.device(big).unwrap().hw.rack.is_none());
        assert!(p.device(outside).unwrap().hw.rack.is_none());
        assert_eq!(p.rack_free_slot(rid, 1, false, None), Some(5));
        assert_eq!(p.rack_free_slot(rid, 2, false, None), None);
        assert_eq!(p.rack_contents(rid)[0].device.id, sw, "top first");

        // Deleting the rack clears mounts.
        let e = p.remove_edit(&crate::select::Selection { areas: vec![rid], ..Default::default() });
        h.apply(&mut p, e, "");
        assert!(p.devices.iter().all(|d| d.hw.rack.is_none()));
    }
}
