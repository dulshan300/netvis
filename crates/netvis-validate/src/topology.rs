//! Graph rules: port reuse and layer-2 loops.

use std::collections::{HashMap, HashSet};

use netvis_core::{DeviceId, DeviceKind, Item, Project, SwitchportMode};

use crate::{Fix, Sink};

pub fn check(p: &Project, out: &mut Sink) {
    port_reuse(p, out);
    loops(p, out);
}

fn port_reuse(p: &Project, out: &mut Sink) {
    let mut uses: HashMap<netvis_core::PortId, Vec<netvis_core::LinkId>> = HashMap::new();
    for l in &p.links {
        if l.a.device == l.b.device {
            let name = p.device(l.a.device).map(|d| d.name.as_str()).unwrap_or("?");
            out.warn("topo.self-link", format!("{name} is cabled to itself"), vec![Item::Link(l.id)]);
        }
        for e in [l.a, l.b] {
            uses.entry(e.port).or_default().push(l.id);
        }
    }
    for (port, links) in uses.into_iter().filter(|(_, v)| v.len() > 1) {
        let who = p
            .devices
            .iter()
            .find_map(|d| d.port(port).map(|x| format!("{}:{}", d.name, x.name)))
            .unwrap_or_else(|| "a port".into());
        out.error("topo.port-reuse", format!("{who} is used by {} links", links.len()), links.into_iter().map(Item::Link).collect());
    }
}

fn bridges(k: DeviceKind) -> bool {
    matches!(k, DeviceKind::SwitchL2 | DeviceKind::SwitchL3)
}

/// Union-find over switches. Parallel links in the same LAG count once; any
/// other edge that joins already-connected switches closes a loop.
fn loops(p: &Project, out: &mut Sink) {
    let mut parent: HashMap<DeviceId, DeviceId> = HashMap::new();
    fn find(parent: &mut HashMap<DeviceId, DeviceId>, x: DeviceId) -> DeviceId {
        let up = *parent.entry(x).or_insert(x);
        if up == x {
            return x;
        }
        let root = find(parent, up);
        parent.insert(x, root);
        root
    }

    let mut seen_lag: HashSet<(DeviceId, DeviceId, u16)> = HashSet::new();
    let mut looped_roots: Vec<DeviceId> = Vec::new();
    for l in &p.links {
        let ends = [l.a, l.b].map(|e| p.device(e.device).and_then(|d| Some((d, d.port(e.port)?))));
        let [Some((da, pa)), Some((db, pb))] = ends else { continue };
        if da.id == db.id || !bridges(da.kind) || !bridges(db.kind) {
            continue;
        }
        let switched = |m: &SwitchportMode| !matches!(m, SwitchportMode::Routed);
        if !switched(&pa.cfg.mode) || !switched(&pb.cfg.mode) || !pa.cfg.enabled || !pb.cfg.enabled {
            continue;
        }
        if let (Some(la), Some(_)) = (pa.cfg.lag, pb.cfg.lag) {
            let key = if da.id < db.id { (da.id, db.id, la) } else { (db.id, da.id, la) };
            if !seen_lag.insert(key) {
                continue; // another member of a LAG already counted
            }
        }
        let (ra, rb) = (find(&mut parent, da.id), find(&mut parent, db.id));
        if ra == rb {
            looped_roots.push(ra);
        } else {
            parent.insert(ra, rb);
        }
    }
    if looped_roots.is_empty() {
        return;
    }
    let mut reported: HashSet<DeviceId> = HashSet::new();
    let switches: Vec<DeviceId> = p.devices.iter().filter(|d| bridges(d.kind)).map(|d| d.id).collect();
    for r in looped_roots {
        let root = find(&mut parent, r);
        if !reported.insert(root) {
            continue;
        }
        let members: Vec<&netvis_core::Device> = switches
            .iter()
            .filter(|id| parent.contains_key(id) && find(&mut parent, **id) == root)
            .filter_map(|id| p.device(*id))
            .collect();
        let names: Vec<_> = members.iter().map(|d| d.name.as_str()).collect();
        let items = members.iter().map(|d| Item::Device(d.id)).collect();
        let without: Vec<DeviceId> = members.iter().filter(|d| !d.stp.is_enabled()).map(|d| d.id).collect();
        if without.is_empty() {
            out.info("topo.loop-stp", format!("Redundant layer-2 paths between {} — blocked by spanning tree", names.join(", ")), items);
        } else {
            out.warn(
                "topo.loop",
                format!("Switching loop between {} with no spanning tree noted on {} switch(es) — enable STP or bundle the links in a LAG", names.join(", "), without.len()),
                items,
            )
            .with_fix(Some(Fix::EnableStp { devices: without }));
        }
    }
}

#[cfg(test)]
mod tests {
    use netvis_core::{CableKind, DeviceKind, PortMedia};

    use crate::testutil::T;

    #[test]
    fn triangle_is_a_loop_until_stp() {
        let mut t = T::new();
        let s: Vec<_> = (0..3).map(|_| t.dev(DeviceKind::SwitchL2)).collect();
        for (a, b) in [(0, 1), (1, 2), (2, 0)] {
            let (pa, pb) = (t.port(s[a], PortMedia::SfpPlus), t.port(s[b], PortMedia::SfpPlus));
            t.link((s[a], pa), (s[b], pb), CableKind::Dac, Some(1.0));
        }
        assert_eq!(t.codes(), vec!["topo.loop"]);
        t.fix("topo.loop");
        assert_eq!(t.codes(), vec!["topo.loop-stp"]);
    }

    #[test]
    fn lag_members_are_not_a_loop() {
        let mut t = T::new();
        let (a, b) = (t.dev(DeviceKind::SwitchL3), t.dev(DeviceKind::SwitchL2));
        for _ in 0..2 {
            let (pa, pb) = (t.port(a, PortMedia::SfpPlus), t.port(b, PortMedia::SfpPlus));
            t.link((a, pa), (b, pb), CableKind::Dac, Some(1.0));
        }
        assert_eq!(t.codes(), vec!["topo.loop"], "two plain parallel links loop");
        for d in [a, b] {
            for x in t.p.device_mut(d).unwrap().ports.iter_mut().filter(|x| x.media == PortMedia::SfpPlus) {
                x.cfg.lag = Some(1);
            }
        }
        assert!(t.codes().is_empty());
    }
}
