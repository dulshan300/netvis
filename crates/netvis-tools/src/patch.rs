//! Patch schedule (every cable run end to end) and per-device port maps.

use netvis_core::{
    AllowedVlans, Circuit, DeviceId, Edit, Endpoint, Link, LinkId, PoeRole, Project, SwitchportMode,
};

use crate::bom::csv_field;

/// `C-007` style identifier for the `i`-th run (0-based) in [`Project::circuits`] order.
pub fn circuit_id(i: usize) -> String {
    format!("C-{:03}", i + 1)
}

#[derive(Debug, Clone, PartialEq)]
pub struct Segment {
    pub link: LinkId,
    pub from: String,
    pub to: String,
    pub cable: String,
    pub length_m: Option<f32>,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ScheduleRow {
    pub id: String,
    pub circuit: Circuit,
    pub a: String,
    pub b: String,
    /// Passive ports crossed, e.g. `PP-01:5, WO-01:A`.
    pub via: String,
    pub segments: Vec<Segment>,
    /// Total length when every cable has one.
    pub length_m: Option<f32>,
    /// VLAN or mode of the network-side port (`20 · Staff`, `Trunk`).
    pub service: String,
    /// An active port whose run stops at a panel: nothing patched beyond it.
    pub unterminated: bool,
    /// A permanent link between two panel ports with nothing patched to
    /// either end (a spare outlet, a dark fibre).
    pub spare: bool,
}

fn port_label(p: &Project, e: Endpoint) -> String {
    match p.device(e.device) {
        Some(d) => format!("{}:{}", d.name, d.port(e.port).map(|x| x.name.as_str()).unwrap_or("?")),
        None => "?".into(),
    }
}

/// Access VLAN, or a short trunk/routed description, of a switch port.
pub fn mode_summary(p: &Project, e: Endpoint) -> String {
    let Some(port) = p.device(e.device).and_then(|d| d.port(e.port)) else { return String::new() };
    match &port.cfg.mode {
        SwitchportMode::Unset => String::new(),
        SwitchportMode::Access { vlan } => p.vlan_label(*vlan),
        SwitchportMode::Trunk { native, allowed } => {
            let allowed = match allowed {
                AllowedVlans::All => "all".to_owned(),
                AllowedVlans::List(l) => {
                    let mut tags: Vec<u16> = l.iter().filter_map(|v| p.vlan(*v).map(|x| x.tag)).collect();
                    tags.sort();
                    tags.iter().map(|t| t.to_string()).collect::<Vec<_>>().join(",")
                }
            };
            let native = native.and_then(|v| p.vlan(v)).map(|v| format!(", native {}", v.tag)).unwrap_or_default();
            format!("Trunk ({allowed}{native})")
        }
        SwitchportMode::Routed => "Routed".into(),
    }
}

pub fn schedule(p: &Project) -> Vec<ScheduleRow> {
    p.circuits()
        .into_iter()
        .enumerate()
        .map(|(i, c)| {
            let links: Vec<&Link> = c.links.iter().filter_map(|id| p.link(*id)).collect();
            let ports: Vec<Endpoint> = c.ports().collect();
            let segments = links
                .iter()
                .zip(ports.windows(2))
                .map(|(l, w)| Segment {
                    link: l.id,
                    from: port_label(p, w[0]),
                    to: port_label(p, w[1]),
                    cable: l.cable.label().to_owned(),
                    length_m: l.length_m,
                    label: l.label.clone(),
                })
                .collect();
            ScheduleRow {
                id: circuit_id(i),
                a: port_label(p, c.a),
                b: port_label(p, c.b),
                via: c.via.iter().map(|e| port_label(p, *e)).collect::<Vec<_>>().join(", "),
                segments,
                length_m: c.length_m(p),
                service: mode_summary(p, c.a),
                unterminated: !p.is_passthrough(c.a) && p.is_passthrough(c.b),
                spare: p.is_passthrough(c.a) && p.is_passthrough(c.b),
                circuit: c,
            }
        })
        .collect()
}

/// One line per cable, grouped by run, for installers.
pub fn schedule_csv(rows: &[ScheduleRow]) -> String {
    let mut out = String::from("Run,From,To,Via,Cable label,Segment from,Segment to,Cable,Length (m),Service\r\n");
    for r in rows {
        for s in &r.segments {
            let len = s.length_m.map(|m| format!("{m:.0}")).unwrap_or_default();
            let cols = [&r.id, &r.a, &r.b, &r.via, &s.label, &s.from, &s.to, &s.cable, &len, &r.service];
            out.push_str(&cols.iter().map(|c| csv_field(c)).collect::<Vec<_>>().join(","));
            out.push_str("\r\n");
        }
    }
    out
}

/// Label every cable with its run ID (`C-007`, or `C-007.1`, `C-007.2`… for
/// runs through panels). Cables that already have a label keep it unless
/// `overwrite` is set. Returns the edit and the number of cables labelled.
pub fn auto_label_edit(p: &Project, prefix: &str, overwrite: bool) -> (Edit, usize) {
    let mut edits = Vec::new();
    for (i, c) in p.circuits().into_iter().enumerate() {
        let base = format!("{prefix}{:03}", i + 1);
        for (k, id) in c.links.iter().enumerate() {
            let Some(l) = p.link(*id) else { continue };
            if !overwrite && !l.label.trim().is_empty() {
                continue;
            }
            let label = if c.links.len() == 1 { base.clone() } else { format!("{base}.{}", k + 1) };
            if l.label != label {
                let mut after = l.clone();
                after.label = label;
                edits.push(Edit::Link { id: l.id, before: Some(l.clone()), after: Some(after) });
            }
        }
    }
    let n = edits.len();
    (Edit::Batch(edits), n)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortState {
    /// Cabled through to an active device.
    Up,
    /// Cabled to a panel that goes no further.
    Unterminated,
    /// Panel port with a cable on one side only (e.g. a spare outlet).
    OneSide,
    Free,
    Disabled,
}

impl PortState {
    pub fn label(self) -> &'static str {
        match self {
            PortState::Up => "Connected",
            PortState::Unterminated => "Patched, not terminated",
            PortState::OneSide => "One side cabled",
            PortState::Free => "Free",
            PortState::Disabled => "Shut down",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PortRow {
    pub endpoint: Endpoint,
    pub name: String,
    pub media: String,
    pub speed: String,
    pub state: PortState,
    pub mode: String,
    /// The active port at the far end, or the panel port the run stops at.
    pub far: String,
    pub via: String,
    pub cables: String,
    pub labels: String,
    pub poe: String,
    pub description: String,
    pub run: Option<String>,
}

/// Every port of `device` with what it reaches.
pub fn port_map(p: &Project, device: DeviceId) -> Vec<PortRow> {
    let Some(d) = p.device(device) else { return Vec::new() };
    let runs = p.circuits();
    d.ports
        .iter()
        .map(|port| {
            let ep = Endpoint { device, port: port.id };
            // Patch-panel ports sit in the middle of a run; show both sides.
            let found = runs.iter().enumerate().find(|(_, c)| c.ports().any(|e| e == ep));
            let (state, far, via, cables, labels, run) = match found {
                None => (PortState::Free, String::new(), String::new(), String::new(), String::new(), None),
                Some((i, c)) => {
                    let c = if c.b == ep { c.clone().reversed() } else { c.clone() };
                    let links: Vec<&Link> = c.links.iter().filter_map(|id| p.link(*id)).collect();
                    let cables = links
                        .iter()
                        .map(|l| match l.length_m {
                            Some(m) => format!("{} {m:.0} m", l.cable.label()),
                            None => l.cable.label().to_owned(),
                        })
                        .collect::<Vec<_>>()
                        .join(" + ");
                    let labels = links.iter().map(|l| l.label.as_str()).filter(|s| !s.is_empty()).collect::<Vec<_>>().join(", ");
                    let far = if c.a == ep {
                        port_label(p, c.b)
                    } else {
                        format!("{} ↔ {}", port_label(p, c.a), port_label(p, c.b))
                    };
                    let state = if d.kind.is_passthrough() && p.port_links(port.id).count() < 2 {
                        PortState::OneSide
                    } else if c.is_terminated(p) || d.kind.is_passthrough() {
                        PortState::Up
                    } else {
                        PortState::Unterminated
                    };
                    let via = c.via.iter().filter(|e| **e != ep).map(|e| port_label(p, *e)).collect::<Vec<_>>().join(", ");
                    (state, far, via, cables, labels, Some(circuit_id(i)))
                }
            };
            let state = if !port.cfg.enabled { PortState::Disabled } else { state };
            PortRow {
                endpoint: ep,
                name: port.name.clone(),
                media: port.transceiver.as_deref().and_then(netvis_core::catalog::transceiver::transceiver).map(|t| t.name.to_owned()).unwrap_or_else(|| port.media.label().to_owned()),
                speed: netvis_core::catalog::format_speed(port.speed_mbps),
                state,
                mode: mode_summary(p, ep),
                far,
                via,
                cables,
                labels,
                poe: match port.poe {
                    PoeRole::None => String::new(),
                    PoeRole::Source(s) => format!("PSE {}", s.short()),
                    PoeRole::Powered(s) => format!("PD {}", s.short()),
                },
                description: port.description.clone(),
                run,
            }
        })
        .collect()
}

pub fn port_map_csv(rows: &[PortRow]) -> String {
    let mut out = String::from("Port,Media,Speed,State,Mode / VLAN,Connected to,Via,Cables,Labels,PoE,Run,Description\r\n");
    for r in rows {
        let run = r.run.clone().unwrap_or_default();
        let cols = [&r.name, &r.media, &r.speed, r.state.label(), &r.mode, &r.far, &r.via, &r.cables, &r.labels, &r.poe, &run, &r.description];
        out.push_str(&cols.iter().map(|c| csv_field(c)).collect::<Vec<_>>().join(","));
        out.push_str("\r\n");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use netvis_core::{CableKind, DeviceKind, Pos};

    fn build() -> (Project, [DeviceId; 4]) {
        let mut p = Project::default();
        let mut add = |k| {
            let (id, e) = p.add_device_edit(k, Pos::new(0.0, 0.0));
            e.apply(&mut p);
            id
        };
        let ids = [add(DeviceKind::SwitchL2), add(DeviceKind::PatchPanel), add(DeviceKind::WallOutlet), add(DeviceKind::Pc)];
        let ep = |p: &Project, d: DeviceId, i: usize| Endpoint { device: d, port: p.device(d).unwrap().ports[i].id };
        for (a, b, len) in [((ids[0], 2), (ids[1], 0), 2.0), ((ids[1], 0), (ids[2], 0), 40.0), ((ids[2], 0), (ids[3], 0), 3.0)] {
            let (ea, eb) = (ep(&p, a.0, a.1), ep(&p, b.0, b.1));
            let (_, e) = p.add_link_edit(ea, eb).unwrap();
            e.apply(&mut p);
            let l = p.links.last_mut().unwrap();
            l.cable = CableKind::Cat6A;
            l.length_m = Some(len);
        }
        (p, ids)
    }

    #[test]
    fn schedule_and_labels() {
        let (mut p, _) = build();
        let rows = schedule(&p);
        assert_eq!(rows.len(), 1);
        let r = &rows[0];
        assert_eq!((r.a.as_str(), r.b.as_str(), r.via.as_str()), ("SW-01:ge3", "PC-01:eth1", "PP-01:1, WO-01:A"));
        assert_eq!(r.length_m, Some(45.0));
        assert_eq!(r.segments.len(), 3);
        assert_eq!(schedule_csv(&rows).lines().count(), 4);

        p.links[1].label = "KEEP".into();
        let (e, n) = auto_label_edit(&p, "C-", false);
        assert_eq!(n, 2);
        e.apply(&mut p);
        let labels: Vec<_> = p.links.iter().map(|l| l.label.as_str()).collect();
        assert_eq!(labels, vec!["C-001.1", "KEEP", "C-001.3"]);
        let (_, n) = auto_label_edit(&p, "C-", true);
        assert_eq!(n, 1);
    }

    #[test]
    fn port_maps() {
        let (p, ids) = build();
        let sw = port_map(&p, ids[0]);
        assert_eq!(sw[2].state, PortState::Up);
        assert_eq!(sw[2].far, "PC-01:eth1");
        assert_eq!(sw[2].via, "PP-01:1, WO-01:A");
        assert_eq!(sw[0].state, PortState::Free);
        let pp = port_map(&p, ids[1]);
        assert_eq!(pp[0].state, PortState::Up);
        assert_eq!(pp[0].far, "SW-01:ge3 ↔ PC-01:eth1");
        assert_eq!(pp[0].via, "WO-01:A");
        assert_eq!(port_map_csv(&sw).lines().count(), sw.len() + 1);
    }
}
