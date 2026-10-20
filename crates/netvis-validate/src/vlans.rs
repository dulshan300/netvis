//! VLAN database and switching consistency.

use std::collections::{BTreeSet, HashMap};

use netvis_core::{AllowedVlans, DeviceId, DeviceKind, Item, LogicalKind, PortId, Project, SwitchportMode, VlanId};

use crate::{Fix, Sink};

pub fn check(p: &Project, out: &mut Sink) {
    database(p, out);
    dangling(p, out);
    links(p, out);
    carried(p, out);
    svis(p, out);
}

fn database(p: &Project, out: &mut Sink) {
    let mut by_tag: HashMap<u16, Vec<&netvis_core::Vlan>> = HashMap::new();
    for v in &p.vlans {
        by_tag.entry(v.tag).or_default().push(v);
    }
    let mut tags: Vec<_> = by_tag.into_iter().filter(|(_, l)| l.len() > 1).collect();
    tags.sort_by_key(|(t, _)| *t);
    for (tag, list) in tags {
        let names: BTreeSet<_> = list.iter().map(|v| v.name.as_str()).collect();
        let items = list.iter().map(|v| Item::Vlan(v.id)).collect();
        if names.len() > 1 {
            out.warn("vlan.dup-tag", format!("VLAN {tag} is defined {} times with different names ({})", list.len(), names.into_iter().collect::<Vec<_>>().join(", ")), items);
        } else {
            out.info("vlan.dup-tag", format!("VLAN {tag} is defined {} times", list.len()), items);
        }
    }
    for v in &p.vlans {
        let u = p.vlan_usage(v.id);
        if u.access_ports == 0 && u.trunk_ports == 0 && u.logical == 0 && u.subnets.is_empty() && u.ssids == 0 {
            out.info("vlan.unused", format!("{} is not used by any port, interface, subnet or SSID", v.label()), vec![Item::Vlan(v.id)]);
        }
    }
}

fn dangling(p: &Project, out: &mut Sink) {
    let exists = |v: &VlanId| p.vlan(*v).is_some();
    for d in &p.devices {
        let mut refs: Vec<VlanId> = Vec::new();
        for port in &d.ports {
            match &port.cfg.mode {
                SwitchportMode::Access { vlan } => refs.extend(vlan),
                SwitchportMode::Trunk { native, allowed } => {
                    refs.extend(native);
                    if let AllowedVlans::List(l) = allowed {
                        refs.extend(l);
                    }
                }
                _ => {}
            }
        }
        refs.extend(d.logical.iter().filter_map(|l| l.kind.vlan()));
        if refs.iter().any(|v| !exists(v)) {
            out.error("vlan.dangling", format!("{} references a VLAN that no longer exists", d.name), vec![Item::Device(d.id)]);
        }
    }
    for s in p.subnets.iter().filter(|s| s.vlan.is_some_and(|v| !exists(&v))) {
        out.error("vlan.dangling", format!("Subnet {} is bound to a VLAN that no longer exists", s.name), vec![Item::Subnet(s.id)]);
    }
    for s in p.ssids.iter().filter(|s| s.vlan.is_some_and(|v| !exists(&v))) {
        out.error("vlan.dangling", format!("SSID {} is mapped to a VLAN that no longer exists", s.name), vec![Item::Ssid(s.id)]);
    }
}

fn tags(p: &Project, vlans: impl IntoIterator<Item = VlanId>) -> String {
    let mut t: Vec<u16> = vlans.into_iter().filter_map(|v| p.vlan(v).map(|x| x.tag)).collect();
    t.sort();
    t.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(", ")
}

/// VLANs a trunk carries, expanded against the database.
fn trunk_set(p: &Project, native: Option<VlanId>, allowed: &AllowedVlans) -> BTreeSet<VlanId> {
    let mut s: BTreeSet<VlanId> = match allowed {
        AllowedVlans::All => p.vlans.iter().map(|v| v.id).collect(),
        AllowedVlans::List(l) => l.iter().copied().collect(),
    };
    s.extend(native);
    s
}

fn links(p: &Project, out: &mut Sink) {
    for l in &p.links {
        let ends = [l.a, l.b].map(|e| p.device(e.device).and_then(|d| Some((d, d.port(e.port)?))));
        let [Some((da, pa)), Some((db, pb))] = ends else { continue };
        let pair = format!("{}:{} ↔ {}:{}", da.name, pa.name, db.name, pb.name);
        let items = vec![Item::Link(l.id), Item::Device(da.id), Item::Device(db.id)];
        use SwitchportMode::*;
        match (&pa.cfg.mode, &pb.cfg.mode) {
            (Access { vlan: va }, Access { vlan: vb }) if va != vb => {
                out.error("vlan.access-mismatch", format!("{pair}: access VLANs differ ({} vs {}) — the two VLANs are bridged together", p.vlan_label(*va), p.vlan_label(*vb)), items)
                    .with_fix(Some(Fix::SetMode { device: db.id, port: pb.id, mode: Access { vlan: *va } }));
            }
            (Access { vlan }, Trunk { native, .. }) | (Trunk { native, .. }, Access { vlan }) if vlan != native => {
                // Make the access side match the trunk's untagged VLAN.
                let (dev, port) = if matches!(pa.cfg.mode, Access { .. }) { (da.id, pa.id) } else { (db.id, pb.id) };
                out.error(
                    "vlan.access-trunk",
                    format!("{pair}: access {} meets a trunk with native {} — untagged traffic changes VLAN", p.vlan_label(*vlan), p.vlan_label(*native)),
                    items,
                )
                .with_fix(Some(Fix::SetMode { device: dev, port, mode: Access { vlan: *native } }));
            }
            (Trunk { native: na, allowed: aa }, Trunk { native: nb, allowed: ab }) => {
                if na != nb {
                    out.error("vlan.native-mismatch", format!("{pair}: native VLAN mismatch ({} vs {})", p.vlan_label(*na), p.vlan_label(*nb)), items.clone())
                        .with_fix(Some(Fix::SetMode { device: db.id, port: pb.id, mode: Trunk { native: *na, allowed: ab.clone() } }));
                }
                let (sa, sb) = (trunk_set(p, *na, aa), trunk_set(p, *nb, ab));
                let only_a: Vec<_> = sa.difference(&sb).copied().collect();
                let only_b: Vec<_> = sb.difference(&sa).copied().collect();
                if !only_a.is_empty() || !only_b.is_empty() {
                    let mut parts = Vec::new();
                    if !only_a.is_empty() {
                        parts.push(format!("{} only on {}", tags(p, only_a.iter().copied()), da.name));
                    }
                    if !only_b.is_empty() {
                        parts.push(format!("{} only on {}", tags(p, only_b.iter().copied()), db.name));
                    }
                    // Offer to extend the narrower side when one side is a strict superset.
                    let fix = match (only_a.first(), only_b.is_empty(), matches!(ab, AllowedVlans::List(_))) {
                        (Some(v), true, true) if only_a.len() == 1 => Some(Fix::AllowVlan { ports: vec![(db.id, pb.id)], vlan: *v }),
                        _ => None,
                    };
                    out.warn("vlan.allowed-mismatch", format!("{pair}: allowed VLANs differ — {}", parts.join("; ")), items).with_fix(fix);
                }
            }
            _ => {}
        }
    }
}

fn is_switch(k: DeviceKind) -> bool {
    matches!(k, DeviceKind::SwitchL2 | DeviceKind::SwitchL3)
}

/// Trunk ports on `d` whose far end also carries `v`.
fn carrying_trunks(p: &Project, d: &netvis_core::Device, v: VlanId) -> (Vec<PortId>, Vec<(DeviceId, PortId)>) {
    let mut good = Vec::new();
    let mut fixable = Vec::new();
    for port in d.ports.iter().filter(|x| matches!(x.cfg.mode, SwitchportMode::Trunk { .. })) {
        let Some(link) = p.port_link(port.id) else { continue };
        let far = if link.a.port == port.id { link.b } else { link.a };
        let Some(fd) = p.device(far.device) else { continue };
        if !is_switch(fd.kind) {
            continue;
        }
        let Some(fp) = fd.port(far.port) else { continue };
        if port.cfg.mode.carries(v) && fp.cfg.mode.carries(v) {
            good.push(port.id);
        } else {
            for (dev, x) in [(d.id, port), (fd.id, fp)] {
                if !x.cfg.mode.carries(v) && matches!(x.cfg.mode, SwitchportMode::Trunk { allowed: AllowedVlans::List(_), .. }) {
                    fixable.push((dev, x.id));
                }
            }
        }
    }
    (good, fixable)
}

/// A VLAN used on a switch without a local SVI needs a trunk that carries it
/// towards another switch.
fn carried(p: &Project, out: &mut Sink) {
    let switch_count = p.devices.iter().filter(|d| is_switch(d.kind)).count();
    if switch_count < 2 {
        return;
    }
    for d in p.devices.iter().filter(|d| is_switch(d.kind)) {
        let has_uplink = d.ports.iter().any(|x| {
            p.port_link(x.id).is_some_and(|l| {
                let far = if l.a.port == x.id { l.b } else { l.a };
                p.device(far.device).is_some_and(|fd| is_switch(fd.kind))
            })
        });
        if !has_uplink {
            continue;
        }
        let used: BTreeSet<VlanId> = d
            .ports
            .iter()
            .filter(|x| p.port_link(x.id).is_some())
            .filter_map(|x| match x.cfg.mode {
                SwitchportMode::Access { vlan } => vlan,
                _ => None,
            })
            .collect();
        for v in used {
            let local_svi = d.logical.iter().any(|l| matches!(l.kind, LogicalKind::Svi { vlan: Some(x) } if x == v));
            if local_svi {
                continue;
            }
            let (good, fixable) = carrying_trunks(p, d, v);
            if good.is_empty() {
                let fix = (!fixable.is_empty()).then_some(Fix::AllowVlan { ports: fixable, vlan: v });
                out.warn("vlan.not-carried", format!("{} is used on {} but no trunk to another switch carries it", p.vlan_label(Some(v)), d.name), vec![Item::Device(d.id), Item::Vlan(v)])
                    .with_fix(fix);
            }
        }
    }
}

/// An SVI is down when no port on the device carries its VLAN.
fn svis(p: &Project, out: &mut Sink) {
    for d in &p.devices {
        for l in &d.logical {
            let LogicalKind::Svi { vlan: Some(v) } = l.kind else { continue };
            let up = d.ports.iter().any(|x| x.cfg.mode.carries(v) && p.port_link(x.id).is_some());
            if !up && l.enabled {
                out.warn("vlan.svi-down", format!("{}:{} — no connected port on {} carries {}", d.name, l.name, d.name, p.vlan_label(Some(v))), vec![Item::Device(d.id), Item::Vlan(v)]);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use netvis_core::{AllowedVlans, CableKind, DeviceKind, Edit, LogicalInterface, LogicalKind, PortMedia, SwitchportMode, Vlan};

    use crate::testutil::T;

    #[test]
    fn link_vlan_mismatches() {
        let mut t = T::new();
        let (v10, v20) = (Vlan::new(10, "Mgmt"), Vlan::new(20, "Staff"));
        let (i10, i20) = (v10.id, v20.id);
        for v in [v10, v20] {
            t.h.apply(&mut t.p, Edit::Vlan { id: v.id, before: None, after: Some(v) }, "");
        }
        let (a, b) = (t.dev(DeviceKind::SwitchL2), t.dev(DeviceKind::SwitchL2));
        let (pa, pb) = (t.port(a, PortMedia::Rj45), t.port(b, PortMedia::Rj45));
        t.link((a, pa), (b, pb), CableKind::Cat6, None);
        let set = |t: &mut T, d, m: SwitchportMode| t.p.device_mut(d).unwrap().ports[0].cfg.mode = m;

        set(&mut t, a, SwitchportMode::Access { vlan: Some(i10) });
        set(&mut t, b, SwitchportMode::Access { vlan: Some(i20) });
        assert!(t.codes().contains(&"vlan.access-mismatch"));
        t.fix("vlan.access-mismatch");
        assert!(!t.codes().contains(&"vlan.access-mismatch"));

        set(&mut t, b, SwitchportMode::Trunk { native: Some(i20), allowed: AllowedVlans::All });
        assert!(t.codes().contains(&"vlan.access-trunk"));

        set(&mut t, a, SwitchportMode::Trunk { native: Some(i10), allowed: AllowedVlans::List(vec![i10]) });
        let codes = t.codes();
        assert!(codes.contains(&"vlan.native-mismatch") && codes.contains(&"vlan.allowed-mismatch"), "{codes:?}");
        t.fix("vlan.native-mismatch");
        assert!(!t.codes().contains(&"vlan.native-mismatch"));
    }

    #[test]
    fn vlan_not_carried_and_svi_down() {
        let mut t = T::new();
        let (v10, v30) = (Vlan::new(10, "Mgmt"), Vlan::new(30, "Voice"));
        let (i10, i30) = (v10.id, v30.id);
        for v in [v10, v30] {
            t.h.apply(&mut t.p, Edit::Vlan { id: v.id, before: None, after: Some(v) }, "");
        }
        let (core, acc, ph) = (t.dev(DeviceKind::SwitchL3), t.dev(DeviceKind::SwitchL2), t.dev(DeviceKind::IpPhone));
        let (pc, pa) = (t.port(core, PortMedia::SfpPlus), t.port(acc, PortMedia::SfpPlus));
        t.link((core, pc), (acc, pa), CableKind::Dac, Some(1.0));
        let trunk = SwitchportMode::Trunk { native: Some(i10), allowed: AllowedVlans::List(vec![i10]) };
        for (d, port) in [(core, pc), (acc, pa)] {
            t.p.device_mut(d).unwrap().ports.iter_mut().find(|x| x.id == port).unwrap().cfg.mode = trunk.clone();
        }
        let (sp, pp) = (t.port(acc, PortMedia::Rj45), t.port(ph, PortMedia::Rj45));
        t.link((acc, sp), (ph, pp), CableKind::Cat6, None);
        t.p.device_mut(acc).unwrap().ports[0].cfg.mode = SwitchportMode::Access { vlan: Some(i30) };
        t.p.device_mut(core).unwrap().logical.push(LogicalInterface::new("Vlan30", LogicalKind::Svi { vlan: Some(i30) }));

        let codes = t.codes();
        assert!(codes.contains(&"vlan.not-carried") && codes.contains(&"vlan.svi-down"), "{codes:?}");
        t.fix("vlan.not-carried");
        let codes = t.codes();
        assert!(!codes.contains(&"vlan.not-carried") && !codes.contains(&"vlan.svi-down"), "{codes:?}");
    }

    #[test]
    fn duplicate_and_unused_vlans() {
        let mut t = T::new();
        for v in [Vlan::new(20, "Staff"), Vlan::new(20, "Users")] {
            t.h.apply(&mut t.p, Edit::Vlan { id: v.id, before: None, after: Some(v) }, "");
        }
        let codes = t.codes();
        assert_eq!(codes.iter().filter(|c| **c == "vlan.dup-tag").count(), 1);
        assert_eq!(codes.iter().filter(|c| **c == "vlan.unused").count(), 2);
    }
}
