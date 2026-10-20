//! Oversubscription: edge capacity versus uplink capacity, by hand or for
//! each switch in a project.

use netvis_core::{Device, DeviceCategory, DeviceKind, Project};

/// `count` ports at `mbps` each.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PortGroup {
    pub count: u32,
    pub mbps: u32,
}

pub fn capacity_mbps(groups: &[PortGroup]) -> f64 {
    groups.iter().map(|g| g.count as f64 * g.mbps as f64).sum()
}

/// Downlink : uplink ratio, e.g. `3.0` for 3:1.
pub fn ratio(down: &[PortGroup], up: &[PortGroup]) -> Option<f64> {
    let u = capacity_mbps(up);
    (u > 0.0).then(|| capacity_mbps(down) / u)
}

/// Common design guidance: access 20:1, distribution 4:1, core 1:1.
pub fn guideline(kind: DeviceKind) -> f64 {
    match tier(kind) {
        1 => 20.0,
        2 => 4.0,
        _ => 1.0,
    }
}

/// Position in the hierarchy; a link to a higher tier is an uplink.
fn tier(kind: DeviceKind) -> u8 {
    match kind {
        DeviceKind::SwitchL2 => 1,
        DeviceKind::SwitchL3 => 2,
        k if k.category() == DeviceCategory::CloudWan => 4,
        DeviceKind::Router | DeviceKind::Firewall | DeviceKind::VpnGateway | DeviceKind::LoadBalancer | DeviceKind::Modem => 3,
        _ => 0,
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SwitchOversub<'a> {
    pub device: &'a Device,
    /// Every non-uplink port at its configured speed.
    pub edge_ports: u32,
    pub edge_mbps: f64,
    /// Only the non-uplink ports that are cabled.
    pub connected_ports: u32,
    pub connected_mbps: f64,
    pub uplink_ports: u32,
    pub uplink_mbps: f64,
}

impl SwitchOversub<'_> {
    /// Ratio if every edge port ran at line rate.
    pub fn ratio(&self) -> Option<f64> {
        (self.uplink_mbps > 0.0).then(|| self.edge_mbps / self.uplink_mbps)
    }

    pub fn connected_ratio(&self) -> Option<f64> {
        (self.uplink_mbps > 0.0).then(|| self.connected_mbps / self.uplink_mbps)
    }

    pub fn guideline(&self) -> f64 {
        guideline(self.device.kind)
    }
}

/// One entry per switch. Uplinks are links to a higher tier (L2 → L3 →
/// router/firewall → WAN). A switch with no higher neighbour treats links to
/// peer switches as uplinks, so a chain of L2 switches still gets a figure.
pub fn switch_report(p: &Project) -> Vec<SwitchOversub<'_>> {
    let mut out = Vec::new();
    for d in p.devices.iter().filter(|d| matches!(d.kind, DeviceKind::SwitchL2 | DeviceKind::SwitchL3)) {
        let mine = tier(d.kind);
        let neighbours: Vec<(netvis_core::PortId, u8)> = p
            .links_of(d.id)
            .filter_map(|l| {
                let (own, other) = if l.a.device == d.id { (l.a, l.b) } else { (l.b, l.a) };
                Some((own.port, tier(p.device(other.device)?.kind)))
            })
            .collect();
        let has_higher = neighbours.iter().any(|(_, t)| *t > mine);
        let is_uplink = |port| {
            neighbours.iter().any(|(pt, t)| *pt == port && (*t > mine || (!has_higher && *t == mine)))
        };
        let mut r = SwitchOversub {
            device: d,
            edge_ports: 0,
            edge_mbps: 0.0,
            connected_ports: 0,
            connected_mbps: 0.0,
            uplink_ports: 0,
            uplink_mbps: 0.0,
        };
        for port in &d.ports {
            let mbps = port.speed_mbps as f64;
            if is_uplink(port.id) {
                r.uplink_ports += 1;
                r.uplink_mbps += mbps;
            } else {
                r.edge_ports += 1;
                r.edge_mbps += mbps;
                if p.port_link(port.id).is_some() {
                    r.connected_ports += 1;
                    r.connected_mbps += mbps;
                }
            }
        }
        out.push(r);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use netvis_core::{CableKind, Endpoint, Pos};

    #[test]
    fn manual_ratio() {
        let down = [PortGroup { count: 48, mbps: 1000 }];
        let up = [PortGroup { count: 2, mbps: 10_000 }];
        assert_eq!(ratio(&down, &up), Some(2.4));
        assert_eq!(ratio(&down, &[]), None);
    }

    #[test]
    fn project_switches() {
        let mut p = Project::default();
        let mut add = |kind| {
            let (id, e) = p.add_device_edit(kind, Pos::new(0.0, 0.0));
            e.apply(&mut p);
            id
        };
        let core = add(DeviceKind::SwitchL3);
        let access = add(DeviceKind::SwitchL2);
        let pc = add(DeviceKind::Pc);
        let link = |p: &mut Project, a, b| {
            let (pa, pb) = (p.first_free_port(a, None).unwrap(), p.first_free_port(b, None).unwrap());
            let (_, e) = p.add_link_edit(Endpoint { device: a, port: pa }, Endpoint { device: b, port: pb }).unwrap();
            e.apply(p);
            p.links.last_mut().unwrap().cable = CableKind::Cat6;
        };
        link(&mut p, access, core);
        link(&mut p, access, pc);

        let r = switch_report(&p);
        let a = r.iter().find(|s| s.device.id == access).unwrap();
        assert_eq!(a.uplink_ports, 1);
        assert_eq!(a.connected_ports, 1);
        assert_eq!(a.edge_ports as usize, p.device(access).unwrap().ports.len() - 1);
        assert!(a.ratio().unwrap() > 1.0);
        // The core's only neighbour is lower-tier: no uplink, no ratio.
        let c = r.iter().find(|s| s.device.id == core).unwrap();
        assert_eq!(c.uplink_ports, 0);
        assert_eq!(c.ratio(), None);
    }
}
