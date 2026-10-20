//! IP address management: who uses what, how full each subnet is, and
//! automatic assignment of free addresses.

use std::collections::HashSet;
use std::net::IpAddr;

use netvis_ip as ip;

use crate::device::{DeviceCategory, DeviceKind};
use crate::edit::Edit;
use crate::iface::{AddrKind, IfAddr, IfaceKey, LogicalKind};
use crate::ids::{DeviceId, PortId, SubnetId, VlanId};
use crate::link::Endpoint;
use crate::port::PortMedia;
use crate::project::Project;
use crate::subnet::Subnet;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Assignment {
    pub addr: IfAddr,
    pub device: DeviceId,
    pub iface: IfaceKey,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Usage {
    pub usable: u128,
    /// Addresses delegated to child subnets.
    pub allocated: u128,
    /// Distinct addresses on interfaces, plus the gateway (outside child subnets).
    pub assigned: u128,
    pub dhcp_pool: u128,
    pub reserved: u128,
    pub free: u128,
}

impl Usage {
    pub fn used_fraction(&self) -> f32 {
        if self.usable == 0 {
            return 1.0;
        }
        (1.0 - self.free as f64 / self.usable as f64) as f32
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct VlanUsage {
    pub access_ports: usize,
    pub trunk_ports: usize,
    pub logical: usize,
    pub subnets: Vec<SubnetId>,
    pub ssids: usize,
}

/// Result of an auto-assign run: one undoable edit plus a human-readable report.
#[derive(Debug, Clone)]
pub struct AutoAssign {
    pub edit: Edit,
    pub assigned: Vec<String>,
    pub skipped: Vec<String>,
}

impl Project {
    /// Every address configured on any interface.
    pub fn assignments(&self) -> Vec<Assignment> {
        self.devices
            .iter()
            .flat_map(|d| {
                d.interfaces().flat_map(move |i| {
                    i.l3.addresses.iter().map(move |a| Assignment { addr: *a, device: d.id, iface: i.key })
                })
            })
            .collect()
    }

    /// Most specific subnet containing `addr`.
    pub fn subnet_containing(&self, addr: IpAddr) -> Option<&Subnet> {
        self.subnets.iter().filter(|s| s.net.contains(&addr)).max_by_key(|s| s.net.prefix_len())
    }

    pub fn assignments_in(&self, s: &Subnet) -> Vec<Assignment> {
        let mut v: Vec<_> = self.assignments().into_iter().filter(|a| s.net.contains(&a.addr.ip())).collect();
        v.sort_by_key(|a| ip::to_u128(a.addr.ip()));
        v
    }

    /// Subnets nested directly inside `s` (not inside another child).
    pub fn child_subnets(&self, s: &Subnet) -> Vec<&Subnet> {
        let inside = |outer: &Subnet, inner: &Subnet| {
            inner.id != outer.id && inner.net.prefix_len() > outer.net.prefix_len() && outer.net.contains(&inner.net.network())
        };
        let nested: Vec<&Subnet> = self.subnets.iter().filter(|c| inside(s, c)).collect();
        nested.iter().filter(|c| !nested.iter().any(|o| inside(o, c))).copied().collect()
    }

    pub fn subnet_usage(&self, s: &Subnet) -> Usage {
        let usable = ip::usable_count(&s.net);
        let children = self.child_subnets(s);
        let in_child = |a: IpAddr| children.iter().any(|c| c.net.contains(&a));
        let allocated: u128 = children.iter().map(|c| ip::total_addresses(&c.net)).sum::<u128>().min(usable);
        let dhcp_pool = s.dhcp.as_ref().map_or(0, |d| d.range().len()).min(usable);
        let reserved: u128 = s.reserved.iter().map(|r| r.len()).sum::<u128>().min(usable);
        let mut hosts: HashSet<IpAddr> = self
            .assignments()
            .into_iter()
            .map(|a| a.addr.ip())
            .filter(|a| ip::is_usable_host(&s.net, *a) && !s.is_blocked(*a) && !in_child(*a))
            .collect();
        if let Some(g) = s.gateway.filter(|g| ip::is_usable_host(&s.net, *g) && !in_child(*g)) {
            hosts.insert(g);
        }
        let assigned = hosts.len() as u128;
        let free = usable.saturating_sub(allocated + assigned + dhcp_pool + reserved);
        Usage { usable, allocated, assigned, dhcp_pool, reserved, free }
    }

    fn assigned_set(&self) -> HashSet<IpAddr> {
        self.assignments().into_iter().map(|a| a.addr.ip()).collect()
    }

    /// Lowest address that is not assigned, not the gateway, and outside the
    /// DHCP pool and reserved ranges.
    pub fn next_free_ip(&self, s: &Subnet) -> Option<IpAddr> {
        let used = self.assigned_set();
        ip::next_free(&s.net, |a| used.contains(&a) || s.is_blocked(a))
    }

    /// A free /24 for a new subnet: `10.0.<tag>.0/24` when a VLAN tag ≤ 255 is
    /// given and that block is unused, otherwise the first free `10.x.y.0/24`.
    pub fn suggest_subnet(&self, vlan_tag: Option<u16>) -> ip::IpNet {
        let free = |n: &ip::IpNet| !self.subnets.iter().any(|s| ip::overlaps(&s.net, n));
        let block = |b: u8, c: u8| ip::IpNet::new(IpAddr::from([10, b, c, 0]), 24).expect("valid");
        if let Some(tag) = vlan_tag.and_then(|t| u8::try_from(t).ok())
            && free(&block(0, tag))
        {
            return block(0, tag);
        }
        (0..=255u8)
            .flat_map(|b| (0..=255u8).map(move |c| block(b, c)))
            .find(free)
            .unwrap_or_else(|| block(0, 0))
    }

    /// Subnet bound to `vlan` (IPv4 preferred over IPv6).
    pub fn subnet_for_vlan(&self, vlan: VlanId) -> Option<&Subnet> {
        let mut bound = self.subnets.iter().filter(|s| s.vlan == Some(vlan));
        bound.clone().find(|s| s.net.addr().is_ipv4()).or_else(|| bound.next())
    }

    /// The untagged VLAN a host port lives in: its own access/native VLAN, or
    /// else the access/native VLAN of the switch port it is cabled to.
    pub fn untagged_vlan_at(&self, device: DeviceId, port: PortId) -> Option<VlanId> {
        let own = self.device(device)?.port(port)?;
        if let Some(v) = own.cfg.mode.untagged_vlan() {
            return Some(v);
        }
        let far = self.peer(Endpoint { device, port })?;
        self.device(far.device)?.port(far.port)?.cfg.mode.untagged_vlan()
    }

    pub fn vlan_usage(&self, vlan: VlanId) -> VlanUsage {
        use crate::iface::SwitchportMode as M;
        let mut u = VlanUsage::default();
        for d in &self.devices {
            for p in &d.ports {
                match &p.cfg.mode {
                    M::Access { vlan: Some(v) } if *v == vlan => u.access_ports += 1,
                    m @ M::Trunk { .. } if m.carries(vlan) => u.trunk_ports += 1,
                    _ => {}
                }
            }
            u.logical += d.logical.iter().filter(|l| l.kind.vlan() == Some(vlan)).count();
        }
        u.subnets = self.subnets.iter().filter(|s| s.vlan == Some(vlan)).map(|s| s.id).collect();
        u.ssids = self.ssids.iter().filter(|s| s.vlan == Some(vlan)).count();
        u
    }

    /// Give every unaddressed host port and VLAN interface the next free
    /// address in the subnet bound to its VLAN. SVIs take the subnet gateway
    /// when it is still free. Host ports also get the subnet gateway as their
    /// default gateway. `scope` limits the run to the given devices.
    pub fn auto_assign_edit(&self, scope: Option<&[DeviceId]>) -> AutoAssign {
        let mut used = self.assigned_set();
        let mut edits = Vec::new();
        let mut assigned = Vec::new();
        let mut skipped = Vec::new();

        for d in self.devices.iter().filter(|d| scope.is_none_or(|s| s.contains(&d.id))) {
            let mut nd = d.clone();

            for l in nd.logical.iter_mut().filter(|l| l.enabled && !l.l3.is_addressed()) {
                let Some(v) = l.kind.vlan() else { continue };
                let label = format!("{}:{}", d.name, l.name);
                let Some(s) = self.subnet_for_vlan(v) else {
                    skipped.push(format!("{label} — {} has no subnet", self.vlan_label(Some(v))));
                    continue;
                };
                let prefer = matches!(l.kind, LogicalKind::Svi { .. }).then_some(s.gateway).flatten();
                match take(s, prefer, &mut used) {
                    Some(a) => {
                        l.l3.addresses.push(IfAddr { net: host_net(a, s), kind: AddrKind::Static });
                        assigned.push(format!("{label} → {a}/{}", s.net.prefix_len()));
                    }
                    None => skipped.push(format!("{label} — {} is full", s.net)),
                }
            }

            if !assigns_host_ports(d.kind) {
                if nd != *d {
                    edits.push(Edit::Device { id: d.id, before: Some(d.clone()), after: Some(nd) });
                }
                continue;
            }
            for p in nd.ports.iter_mut() {
                let eligible = p.cfg.enabled
                    && !p.cfg.l3.is_addressed()
                    && !p.cfg.mode.is_switchport()
                    && p.cfg.mode != crate::iface::SwitchportMode::Routed
                    && !matches!(p.media, PortMedia::Wireless | PortMedia::Virtual | PortMedia::Console)
                    && self.port_link(p.id).is_some();
                if !eligible {
                    continue;
                }
                let label = format!("{}:{}", d.name, p.name);
                let Some(v) = self.untagged_vlan_at(d.id, p.id) else {
                    skipped.push(format!("{label} — connected port has no access VLAN"));
                    continue;
                };
                let Some(s) = self.subnet_for_vlan(v) else {
                    skipped.push(format!("{label} — {} has no subnet", self.vlan_label(Some(v))));
                    continue;
                };
                match take(s, None, &mut used) {
                    Some(a) => {
                        p.cfg.l3.addresses.push(IfAddr { net: host_net(a, s), kind: AddrKind::Static });
                        if p.cfg.l3.gateway.is_none() {
                            p.cfg.l3.gateway = s.gateway;
                        }
                        assigned.push(format!("{label} → {a}/{}", s.net.prefix_len()));
                    }
                    None => skipped.push(format!("{label} — {} is full", s.net)),
                }
            }
            if nd != *d {
                edits.push(Edit::Device { id: d.id, before: Some(d.clone()), after: Some(nd) });
            }
        }
        AutoAssign { edit: Edit::Batch(edits), assigned, skipped }
    }
}

/// Claim `prefer` if still free, else the next free address in `s`.
fn take(s: &Subnet, prefer: Option<IpAddr>, used: &mut HashSet<IpAddr>) -> Option<IpAddr> {
    let addr = prefer
        .filter(|g| !used.contains(g))
        .or_else(|| ip::next_free(&s.net, |a| used.contains(&a) || s.is_blocked(a)))?;
    used.insert(addr);
    Some(addr)
}

fn host_net(a: IpAddr, s: &Subnet) -> ip::IpNet {
    ip::IpNet::new(a, s.net.prefix_len()).expect("prefix from a valid subnet")
}

/// Kinds whose physical ports are host interfaces (switches, patch panels and
/// cloud objects address via SVIs or not at all).
fn assigns_host_ports(kind: DeviceKind) -> bool {
    !matches!(
        kind,
        DeviceKind::SwitchL2
            | DeviceKind::SwitchL3
            | DeviceKind::PatchPanel
            | DeviceKind::WallOutlet
            | DeviceKind::PoeInjector
            | DeviceKind::MediaConverter
    ) && kind.category() != DeviceCategory::CloudWan
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edit::History;
    use crate::geom::Pos;
    use crate::iface::{LogicalInterface, SwitchportMode};
    use crate::link::Endpoint;
    use crate::subnet::{AddrRange, DhcpScope};
    use crate::vlan::Vlan;

    struct Fixture {
        p: Project,
        h: History,
        sw: DeviceId,
        pc: DeviceId,
        core: DeviceId,
        staff: VlanId,
        subnet: SubnetId,
    }

    fn ip(s: &str) -> IpAddr {
        s.parse().unwrap()
    }

    /// core(L3, SVI Vlan20) —trunk— sw(L2) —access 20— pc
    fn fixture() -> Fixture {
        let mut p = Project::default();
        let mut h = History::default();
        let add = |p: &mut Project, h: &mut History, k| {
            let (id, e) = p.add_device_edit(k, Pos::default());
            h.apply(p, e, "");
            id
        };
        let core = add(&mut p, &mut h, DeviceKind::SwitchL3);
        let sw = add(&mut p, &mut h, DeviceKind::SwitchL2);
        let pc = add(&mut p, &mut h, DeviceKind::Pc);

        let staff = Vlan::new(20, "Staff");
        let vid = staff.id;
        h.apply(&mut p, Edit::Vlan { id: vid, before: None, after: Some(staff) }, "");

        let mut s = Subnet::new("Staff", "10.0.20.0/24".parse().unwrap()).with_defaults(true, false);
        s.vlan = Some(vid);
        s.dhcp = Some(DhcpScope { start: ip("10.0.20.100"), end: ip("10.0.20.254"), lease_hours: 8, dns: vec![], server: None });
        s.reserved = vec![AddrRange { start: ip("10.0.20.2"), end: ip("10.0.20.9"), label: "infra".into() }];
        let sid = s.id;
        h.apply(&mut p, Edit::Subnet { id: sid, before: None, after: Some(s) }, "");

        // Access port on the switch for the PC, SVI on the core.
        let mut d = p.device(sw).unwrap().clone();
        let before = d.clone();
        d.ports[0].cfg.mode = SwitchportMode::Access { vlan: Some(vid) };
        h.apply(&mut p, Edit::Device { id: sw, before: Some(before), after: Some(d) }, "");
        let mut c = p.device(core).unwrap().clone();
        let before = c.clone();
        c.logical.push(LogicalInterface::new("Vlan20", LogicalKind::Svi { vlan: Some(vid) }));
        h.apply(&mut p, Edit::Device { id: core, before: Some(before), after: Some(c) }, "");

        let sw_port = p.device(sw).unwrap().ports[0].id;
        let pc_port = p.device(pc).unwrap().ports[0].id;
        let (_, e) = p.add_link_edit(Endpoint { device: sw, port: sw_port }, Endpoint { device: pc, port: pc_port }).unwrap();
        h.apply(&mut p, e, "");
        Fixture { p, h, sw, pc, core, staff: vid, subnet: sid }
    }

    #[test]
    fn vlan_is_inherited_from_switch_port() {
        let f = fixture();
        let pc_port = f.p.device(f.pc).unwrap().ports[0].id;
        assert_eq!(f.p.untagged_vlan_at(f.pc, pc_port), Some(f.staff));
        let u = f.p.vlan_usage(f.staff);
        assert_eq!((u.access_ports, u.logical, u.subnets.len()), (1, 1, 1));
    }

    #[test]
    fn auto_assign_gateway_to_svi_and_next_free_to_host() {
        let mut f = fixture();
        let run = f.p.auto_assign_edit(None);
        assert_eq!(run.assigned.len(), 2, "{:?} / {:?}", run.assigned, run.skipped);
        assert!(run.skipped.is_empty(), "{:?}", run.skipped);
        f.h.apply(&mut f.p, run.edit, "auto");

        let svi = &f.p.device(f.core).unwrap().logical[0];
        assert_eq!(svi.l3.addresses[0].net, "10.0.20.1/24".parse().unwrap());
        let pc = &f.p.device(f.pc).unwrap().ports[0];
        // .1 is the gateway, .2–.9 are reserved.
        assert_eq!(pc.cfg.l3.addresses[0].net, "10.0.20.10/24".parse().unwrap());
        assert_eq!(pc.cfg.l3.gateway, Some(ip("10.0.20.1")));
        // Switch ports never receive host addresses.
        assert!(f.p.device(f.sw).unwrap().ports.iter().all(|p| p.cfg.l3.addresses.is_empty()));

        // Idempotent.
        let again = f.p.auto_assign_edit(None);
        assert!(again.assigned.is_empty());
        assert!(again.edit.is_noop());

        // Undo removes everything in one step.
        f.h.undo(&mut f.p);
        assert!(f.p.assignments().is_empty());
    }

    #[test]
    fn usage_and_next_free() {
        let mut f = fixture();
        let s = f.p.subnet(f.subnet).unwrap().clone();
        assert_eq!(f.p.next_free_ip(&s), Some(ip("10.0.20.10")));
        let u = f.p.subnet_usage(&s);
        assert_eq!(u.usable, 254);
        assert_eq!(u.dhcp_pool, 155);
        assert_eq!(u.reserved, 8);
        assert_eq!(u.assigned, 1, "gateway counts as used");
        assert_eq!(u.free, 254 - 155 - 8 - 1);

        let run = f.p.auto_assign_edit(None);
        f.h.apply(&mut f.p, run.edit, "auto");
        let u = f.p.subnet_usage(&s);
        assert_eq!(u.assigned, 2, "SVI on gateway is not double counted");
        assert_eq!(f.p.next_free_ip(&s), Some(ip("10.0.20.11")));
        assert_eq!(f.p.subnet_containing(ip("10.0.20.77")).map(|x| x.id), Some(f.subnet));
    }

    #[test]
    fn parent_blocks_count_child_subnets() {
        let mut f = fixture();
        let parent = Subnet::new("Site", "10.0.0.0/16".parse().unwrap());
        let pid = parent.id;
        f.h.apply(&mut f.p, Edit::Subnet { id: pid, before: None, after: Some(parent) }, "");
        let parent = f.p.subnet(pid).unwrap();
        assert_eq!(f.p.child_subnets(parent).len(), 1);
        let u = f.p.subnet_usage(parent);
        assert_eq!(u.allocated, 256);
        assert_eq!(u.assigned, 0, "gateway of the child is not counted again");
        assert_eq!(u.free, 65534 - 256);
    }

    #[test]
    fn missing_subnet_is_reported() {
        let mut f = fixture();
        let e = f.p.remove_edit(&crate::select::Selection { subnets: vec![f.subnet], ..Default::default() });
        f.h.apply(&mut f.p, e, "");
        let run = f.p.auto_assign_edit(None);
        assert!(run.assigned.is_empty());
        assert_eq!(run.skipped.len(), 2);
        assert!(run.skipped[0].contains("has no subnet"));
    }

    #[test]
    fn suggests_free_blocks() {
        let f = fixture();
        assert_eq!(f.p.suggest_subnet(Some(30)), "10.0.30.0/24".parse().unwrap());
        // 10.0.20.0/24 is taken by the fixture.
        assert_eq!(f.p.suggest_subnet(Some(20)), "10.0.0.0/24".parse().unwrap());
        assert_eq!(f.p.suggest_subnet(Some(999)), "10.0.0.0/24".parse().unwrap());
    }

    #[test]
    fn scope_limits_devices() {
        let f = fixture();
        let run = f.p.auto_assign_edit(Some(&[f.pc]));
        assert_eq!(run.assigned.len(), 1);
        assert!(run.assigned[0].starts_with("PC-01:"));
    }
}
