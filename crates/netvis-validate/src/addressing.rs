//! IP addressing: duplicates, addresses outside the plan, gateways, VLAN /
//! subnet consistency, DHCP scopes and subnet overlaps.

use std::collections::HashMap;
use std::net::IpAddr;

use netvis_core::ip;
use netvis_core::ipam::Assignment;
use netvis_core::{AddrKind, DeviceId, IfAddr, IfaceKey, Item, Project, Subnet, VlanId};

use crate::{Fix, Sink};

pub fn check(p: &Project, out: &mut Sink) {
    let assigns = p.assignments();
    duplicates(p, &assigns, out);
    placement(p, &assigns, out);
    gateways(p, out);
    subnets(p, out);
    dhcp_clients(p, out);
}

pub(crate) fn iface_label(p: &Project, d: DeviceId, k: IfaceKey) -> String {
    p.device(d).map(|x| format!("{}:{}", x.name, x.iface_name(k).unwrap_or("?"))).unwrap_or_default()
}

/// VLAN an interface lives in: SVIs/sub-interfaces by definition, ports via
/// their own or the cabled switch port's untagged VLAN.
pub(crate) fn iface_vlan(p: &Project, d: DeviceId, k: IfaceKey) -> Option<VlanId> {
    match k {
        IfaceKey::Port(port) => p.untagged_vlan_at(d, port),
        IfaceKey::Logical(l) => p.device(d)?.logical(l)?.kind.vlan(),
    }
}

fn reassign(p: &Project, a: &Assignment, s: &Subnet) -> Option<Fix> {
    let new = p.next_free_ip(s)?;
    Some(Fix::ReplaceAddress {
        device: a.device,
        iface: a.iface,
        old: a.addr.ip(),
        new: IfAddr { net: ip::IpNet::new(new, s.net.prefix_len()).ok()?, kind: a.addr.kind },
    })
}

fn duplicates(p: &Project, assigns: &[Assignment], out: &mut Sink) {
    let mut by_ip: HashMap<IpAddr, Vec<&Assignment>> = HashMap::new();
    for a in assigns {
        by_ip.entry(a.addr.ip()).or_default().push(a);
    }
    let mut dups: Vec<_> = by_ip.into_iter().filter(|(_, v)| v.len() > 1).collect();
    dups.sort_by_key(|(addr, _)| ip::to_u128(*addr));
    for (addr, users) in dups {
        let names: Vec<_> = users.iter().map(|a| iface_label(p, a.device, a.iface)).collect();
        let mut items: Vec<Item> = users.iter().map(|a| Item::Device(a.device)).collect();
        items.dedup();
        let last = users.last().expect("len > 1");
        let fix = p.subnet_containing(addr).and_then(|s| reassign(p, last, s));
        out.error("ip.duplicate", format!("Duplicate IP {addr} on {}", names.join(", ")), items).with_fix(fix);
    }
}

fn placement(p: &Project, assigns: &[Assignment], out: &mut Sink) {
    for a in assigns {
        let addr = a.addr.ip();
        let who = iface_label(p, a.device, a.iface);
        let items = vec![Item::Device(a.device)];
        let Some(s) = p.subnet_containing(addr) else {
            if !p.subnets.is_empty() {
                out.warn("ip.unplanned", format!("{who}: {} is not inside any planned subnet", a.addr.net), items);
            }
            continue;
        };
        let mut items = items;
        items.push(Item::Subnet(s.id));
        if !ip::is_usable_host(&s.net, addr) {
            out.error("ip.reserved", format!("{who}: {addr} is the network or broadcast address of {}", s.net), items)
                .with_fix(reassign(p, a, s));
            continue;
        }
        if a.addr.net.prefix_len() != s.net.prefix_len() {
            let fixed = IfAddr { net: ip::IpNet::new(addr, s.net.prefix_len()).expect("valid"), kind: a.addr.kind };
            out.warn(
                "ip.prefix",
                format!("{who}: {} uses /{} but {} is {}", addr, a.addr.net.prefix_len(), s.name, s.net),
                items.clone(),
            )
            .with_fix(Some(Fix::ReplaceAddress { device: a.device, iface: a.iface, old: addr, new: fixed }));
        }
        if a.addr.kind == AddrKind::Static && s.dhcp.as_ref().is_some_and(|d| d.range().contains(addr)) {
            out.warn("ip.in-pool", format!("{who}: static {addr} lies inside the DHCP pool of {}", s.name), items.clone())
                .with_fix(reassign(p, a, s));
        }
        if let (Some(iv), Some(sv)) = (iface_vlan(p, a.device, a.iface), s.vlan)
            && iv != sv
        {
            let fix = p.subnet_for_vlan(iv).and_then(|right| reassign(p, a, right));
            out.error(
                "ip.wrong-vlan",
                format!("{who} is in {} but {addr} belongs to {} ({})", p.vlan_label(Some(iv)), s.name, p.vlan_label(Some(sv))),
                items,
            )
            .with_fix(fix);
        }
    }
}

fn gateways(p: &Project, out: &mut Sink) {
    for d in &p.devices {
        for i in d.interfaces() {
            let (Some(g), Some(first)) = (i.l3.gateway, i.l3.addresses.first()) else { continue };
            // If the gateway fits the subnet of the interface's VLAN, the address is
            // what's wrong (reported as ip.wrong-vlan), not the gateway.
            let vlan_subnet = iface_vlan(p, d.id, i.key).and_then(|v| p.subnet_for_vlan(v));
            if vlan_subnet.is_some_and(|s| s.net.contains(&g) && !s.net.contains(&first.ip())) {
                continue;
            }
            let who = format!("{}:{}", d.name, i.name);
            let suggestion = p.subnet_containing(first.ip()).and_then(|s| s.gateway).filter(|sg| *sg != first.ip() && *sg != g);
            let fix = suggestion.map(|gateway| Fix::SetGateway { device: d.id, iface: i.key, gateway });
            if i.l3.addresses.iter().any(|a| a.ip() == g) {
                out.error("ip.gateway-self", format!("{who}: default gateway {g} is the interface's own address"), vec![Item::Device(d.id)])
                    .with_fix(fix);
            } else if !i.l3.addresses.iter().any(|a| a.net.contains(&g)) {
                out.error("ip.gateway", format!("{who}: gateway {g} is outside {}", first.net.trunc()), vec![Item::Device(d.id)])
                    .with_fix(fix);
            }
        }
    }
}

fn subnets(p: &Project, out: &mut Sink) {
    for s in &p.subnets {
        let items = vec![Item::Subnet(s.id)];
        if let Some(g) = s.gateway {
            if !ip::is_usable_host(&s.net, g) {
                let fix = ip::nth_usable(&s.net, 0).map(|gateway| Fix::SetSubnetGateway { subnet: s.id, gateway });
                out.error("subnet.gateway", format!("{}: gateway {g} is not a usable host in {}", s.name, s.net), items.clone()).with_fix(fix);
            } else if s.dhcp.as_ref().is_some_and(|d| d.range().contains(g)) {
                out.error("subnet.gateway-pool", format!("{}: gateway {g} lies inside the DHCP pool", s.name), items.clone());
            }
        }
        if let Some(d) = &s.dhcp {
            let inside = s.net.contains(&d.start) && s.net.contains(&d.end);
            if !inside || ip::range_len(d.start, d.end) == 0 {
                out.error("dhcp.range", format!("{}: DHCP range {} – {} is not a valid range inside {}", s.name, d.start, d.end, s.net), items.clone());
            } else if let Some(r) = s.reserved.iter().find(|r| d.range().contains(r.start) || d.range().contains(r.end) || r.contains(d.start)) {
                out.warn("dhcp.reserved", format!("{}: DHCP pool overlaps the reserved range “{}”", s.name, r.label), items.clone());
            }
        }
    }

    for (i, a) in p.subnets.iter().enumerate() {
        for b in &p.subnets[i + 1..] {
            if a.net.addr().is_ipv4() != b.net.addr().is_ipv4() || !ip::overlaps(&a.net, &b.net) {
                continue;
            }
            let items = vec![Item::Subnet(a.id), Item::Subnet(b.id)];
            if a.net == b.net {
                out.error("subnet.duplicate", format!("{} and {} are the same prefix {}", a.name, b.name, a.net), items);
            } else if a.vlan.is_some() && b.vlan.is_some() {
                out.error(
                    "subnet.overlap",
                    format!("{} ({}) overlaps {} ({}); both are bound to VLANs", a.name, a.net, b.name, b.net),
                    items,
                );
            }
        }
    }

    let mut by_vlan: HashMap<(VlanId, bool), Vec<&Subnet>> = HashMap::new();
    for s in &p.subnets {
        if let Some(v) = s.vlan {
            by_vlan.entry((v, s.net.addr().is_ipv4())).or_default().push(s);
        }
    }
    for ((v, _), list) in by_vlan.into_iter().filter(|(_, l)| l.len() > 1) {
        let names: Vec<_> = list.iter().map(|s| format!("{} {}", s.name, s.net)).collect();
        out.info(
            "subnet.multi",
            format!("{} has several subnets ({}); auto-assign uses the first", p.vlan_label(Some(v)), names.join(", ")),
            list.iter().map(|s| Item::Subnet(s.id)).collect(),
        );
    }
}

fn dhcp_clients(p: &Project, out: &mut Sink) {
    let mut clients: HashMap<netvis_core::SubnetId, Vec<(DeviceId, String)>> = HashMap::new();
    for d in &p.devices {
        for i in d.interfaces().filter(|i| i.l3.dhcp_client && i.enabled) {
            let Some(s) = iface_vlan(p, d.id, i.key).and_then(|v| p.subnet_for_vlan(v)) else { continue };
            clients.entry(s.id).or_default().push((d.id, format!("{}:{}", d.name, i.name)));
        }
    }
    for (sid, list) in clients {
        let Some(s) = p.subnet(sid) else { continue };
        let mut items = vec![Item::Subnet(sid)];
        items.extend(list.iter().map(|(d, _)| Item::Device(*d)));
        match &s.dhcp {
            None => {
                out.warn(
                    "dhcp.none",
                    format!("{} use DHCP but {} ({}) has no DHCP scope", list.iter().map(|x| x.1.as_str()).collect::<Vec<_>>().join(", "), s.name, s.net),
                    items,
                )
                .with_fix(Some(Fix::EnableDhcp { subnet: sid }));
            }
            Some(d) => {
                let pool = d.range().len();
                let n = list.len() as u128;
                if n > pool {
                    out.error("dhcp.exhausted", format!("{}: {n} DHCP clients but only {pool} addresses in the pool", s.name), items);
                } else if pool > 0 && n * 10 > pool * 8 {
                    out.warn("dhcp.exhausted", format!("{}: DHCP pool {:.0}% used by planned clients ({n} of {pool})", s.name, n as f64 / pool as f64 * 100.0), items);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use netvis_core::{AddrKind, CableKind, DeviceKind, Edit, IfAddr, PortMedia, Subnet, SwitchportMode, Vlan};

    use crate::testutil::T;

    fn addr(s: &str) -> IfAddr {
        IfAddr { net: s.parse().unwrap(), kind: AddrKind::Static }
    }

    fn setup() -> (T, netvis_core::DeviceId, netvis_core::DeviceId, netvis_core::VlanId, netvis_core::VlanId) {
        let mut t = T::new();
        let (v20, v30) = (Vlan::new(20, "Staff"), Vlan::new(30, "Voice"));
        let (i20, i30) = (v20.id, v30.id);
        for v in [v20, v30] {
            t.h.apply(&mut t.p, Edit::Vlan { id: v.id, before: None, after: Some(v) }, "");
        }
        for (name, net, v) in [("Staff", "10.0.20.0/24", i20), ("Voice", "10.0.30.0/24", i30)] {
            let mut s = Subnet::new(name, net.parse().unwrap()).with_defaults(true, true);
            s.vlan = Some(v);
            t.h.apply(&mut t.p, Edit::Subnet { id: s.id, before: None, after: Some(s) }, "");
        }
        let sw = t.dev(DeviceKind::SwitchL2);
        let pc = t.dev(DeviceKind::Pc);
        let (a, b) = (t.port(sw, PortMedia::Rj45), t.port(pc, PortMedia::Rj45));
        t.link((sw, a), (pc, b), CableKind::Cat6, None);
        t.p.device_mut(sw).unwrap().ports[0].cfg.mode = SwitchportMode::Access { vlan: Some(i20) };
        (t, sw, pc, i20, i30)
    }

    #[test]
    fn duplicate_ip_and_fix() {
        let (mut t, sw, pc, _, _) = setup();
        t.p.device_mut(pc).unwrap().ports[0].cfg.l3.addresses = vec![addr("10.0.20.5/24")];
        let svi = netvis_core::LogicalInterface::new("Vlan20", netvis_core::LogicalKind::Svi { vlan: None });
        t.p.device_mut(sw).unwrap().logical.push(svi);
        t.p.device_mut(sw).unwrap().logical[0].l3.addresses = vec![addr("10.0.20.5/24")];
        assert!(t.codes().contains(&"ip.duplicate"));
        t.fix("ip.duplicate");
        assert!(!t.codes().contains(&"ip.duplicate"), "{:?}", t.codes());
    }

    #[test]
    fn wrong_vlan_prefix_pool_and_gateway() {
        let (mut t, _, pc, _, _) = setup();
        let set = |t: &mut T, a: &str, gw: Option<&str>| {
            let l3 = &mut t.p.device_mut(pc).unwrap().ports[0].cfg.l3;
            l3.addresses = vec![addr(a)];
            l3.gateway = gw.map(|g| g.parse().unwrap());
        };
        set(&mut t, "10.0.30.7/24", Some("10.0.20.1"));
        assert_eq!(t.codes(), vec!["ip.wrong-vlan"], "the gateway is right for the VLAN; only the address is wrong");
        t.fix("ip.wrong-vlan");
        let now = t.p.device(pc).unwrap().ports[0].cfg.l3.addresses[0].net;
        assert_eq!(now, "10.0.20.2/24".parse().unwrap());
        assert!(t.codes().is_empty(), "{:?}", t.codes());

        set(&mut t, "10.0.20.7/16", None);
        assert_eq!(t.codes(), vec!["ip.prefix"]);
        set(&mut t, "10.0.20.200/24", None);
        assert_eq!(t.codes(), vec!["ip.in-pool"]);
        set(&mut t, "10.0.20.255/24", None);
        assert_eq!(t.codes(), vec!["ip.reserved"]);
        set(&mut t, "10.0.20.7/24", Some("10.0.21.1"));
        assert_eq!(t.codes(), vec!["ip.gateway"]);
        t.fix("ip.gateway");
        assert!(t.codes().is_empty());
        set(&mut t, "192.168.1.5/24", None);
        assert_eq!(t.codes(), vec!["ip.unplanned"]);
    }

    #[test]
    fn subnet_rules() {
        let (mut t, ..) = setup();
        t.p.subnets[0].gateway = Some("10.0.20.0".parse().unwrap());
        t.p.subnets[1].net = "10.0.20.0/24".parse().unwrap();
        let codes = t.codes();
        assert!(codes.contains(&"subnet.gateway") && codes.contains(&"subnet.duplicate"), "{codes:?}");
        t.p.subnets[1].net = "10.0.0.0/16".parse().unwrap();
        t.p.subnets[1].dhcp = None;
        assert!(t.codes().contains(&"subnet.overlap"));
    }

    #[test]
    fn dhcp_clients_need_a_scope() {
        let (mut t, _, pc, _, _) = setup();
        t.p.device_mut(pc).unwrap().ports[0].cfg.l3.dhcp_client = true;
        assert!(t.codes().is_empty());
        t.p.subnets[0].dhcp = None;
        assert_eq!(t.codes(), vec!["dhcp.none"]);
        t.fix("dhcp.none");
        assert!(t.codes().is_empty());
    }
}
