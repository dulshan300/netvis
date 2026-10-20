//! Bill of materials from a project: hardware by model, optics, cabling by
//! type and length, and racks.

use std::collections::BTreeMap;

use netvis_catalog::{CableFamily, models, transceiver};
use netvis_core::{AreaKind, DeviceCategory, Project};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum BomSection {
    Hardware,
    Optics,
    Cabling,
    Racks,
}

impl BomSection {
    pub fn label(self) -> &'static str {
        match self {
            BomSection::Hardware => "Hardware",
            BomSection::Optics => "Optics",
            BomSection::Cabling => "Cabling",
            BomSection::Racks => "Racks",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct BomLine {
    pub section: BomSection,
    pub item: String,
    pub qty: u32,
    /// Cable length to order, including slack, when lengths are known.
    pub length_m: Option<f32>,
    pub notes: String,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BomOptions {
    /// Extra cable to order, as a fraction (0.1 = 10 %).
    pub slack: f32,
    /// Include devices with no model chosen.
    pub include_generic: bool,
}

impl Default for BomOptions {
    fn default() -> Self {
        Self { slack: 0.1, include_generic: true }
    }
}

fn names(list: &[&str]) -> String {
    const SHOW: usize = 6;
    let mut s = list.iter().take(SHOW).copied().collect::<Vec<_>>().join(", ");
    if list.len() > SHOW {
        s.push_str(&format!(" +{} more", list.len() - SHOW));
    }
    s
}

pub fn build(p: &Project, opts: BomOptions) -> Vec<BomLine> {
    let mut out = Vec::new();

    // Hardware, keyed by catalogue model or by device type when generic.
    // Cloud/WAN shapes are services, not things to buy.
    let mut hw: BTreeMap<(u8, String), Vec<&str>> = BTreeMap::new();
    for d in p.devices.iter().filter(|d| d.kind.category() != DeviceCategory::CloudWan) {
        let key = match d.hw.model.as_deref().and_then(models::model) {
            Some(m) => (0, m.name.clone()),
            None if opts.include_generic => (1, format!("{} (model not chosen)", d.kind.label())),
            None => continue,
        };
        hw.entry(key).or_default().push(&d.name);
    }
    for ((_, item), devs) in hw {
        out.push(BomLine { section: BomSection::Hardware, item, qty: devs.len() as u32, length_m: None, notes: names(&devs) });
    }

    let mut optics: BTreeMap<&str, u32> = BTreeMap::new();
    for port in p.devices.iter().flat_map(|d| &d.ports) {
        if let Some(t) = port.transceiver.as_deref().and_then(transceiver::transceiver) {
            *optics.entry(t.id).or_default() += 1;
        }
    }
    for (id, qty) in optics {
        let t = transceiver::transceiver(id).expect("from catalogue");
        out.push(BomLine {
            section: BomSection::Optics,
            item: format!("{} {}", t.form.label(), t.name),
            qty,
            length_m: None,
            notes: t.summary(),
        });
    }

    let mut cables: BTreeMap<String, (u32, f32, u32)> = BTreeMap::new();
    for l in &p.links {
        if matches!(l.cable.family(), CableFamily::Wireless | CableFamily::Logical) {
            continue;
        }
        let e = cables.entry(l.cable.label().to_owned()).or_default();
        e.0 += 1;
        match l.length_m {
            Some(m) => e.1 += m,
            None => e.2 += 1,
        }
    }
    for (item, (qty, metres, unknown)) in cables {
        let known = qty - unknown;
        let mut notes = String::new();
        if known > 0 {
            notes = format!("{metres:.0} m measured + {:.0}% slack", opts.slack * 100.0);
        }
        if unknown > 0 {
            if !notes.is_empty() {
                notes.push_str("; ");
            }
            notes.push_str(&format!("{unknown} without a length"));
        }
        out.push(BomLine {
            section: BomSection::Cabling,
            item,
            qty,
            length_m: (known > 0).then(|| (metres * (1.0 + opts.slack)).ceil()),
            notes,
        });
    }

    let mut racks: BTreeMap<u8, Vec<&str>> = BTreeMap::new();
    for a in p.areas.iter().filter(|a| a.kind == AreaKind::Rack) {
        racks.entry(a.rack_units).or_default().push(&a.name);
    }
    for (units, list) in racks {
        out.push(BomLine {
            section: BomSection::Racks,
            item: format!("{units}U rack"),
            qty: list.len() as u32,
            length_m: None,
            notes: names(&list),
        });
    }
    out
}

/// RFC 4180 CSV with a header row.
/// One RFC 4180 field, quoted when needed.
pub fn csv_field(s: &str) -> String {
    if s.contains([',', '"', '\n', '\r']) { format!("\"{}\"", s.replace('"', "\"\"")) } else { s.to_owned() }
}

pub fn to_csv(lines: &[BomLine]) -> String {
    let field = csv_field;
    let mut out = String::from("Section,Item,Qty,Length (m),Notes\r\n");
    for l in lines {
        let len = l.length_m.map(|m| format!("{m:.0}")).unwrap_or_default();
        out.push_str(&format!(
            "{},{},{},{},{}\r\n",
            field(l.section.label()),
            field(&l.item),
            l.qty,
            len,
            field(&l.notes)
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use netvis_core::{CableKind, DeviceKind, Endpoint, Pos};

    #[test]
    fn groups_and_csv() {
        let mut p = Project::default();
        let mut ids = Vec::new();
        for kind in [DeviceKind::SwitchL2, DeviceKind::SwitchL2, DeviceKind::Pc, DeviceKind::Internet] {
            let (id, e) = p.add_device_edit(kind, Pos::new(0.0, 0.0));
            e.apply(&mut p);
            ids.push(id);
        }
        let m = netvis_core::DeviceKind::SwitchL2.models().next().unwrap().id.clone();
        for &d in &ids[..2] {
            let (e, _) = p.apply_model_edit(d, &m).unwrap();
            e.apply(&mut p);
        }
        for (a, b, len) in [(ids[0], ids[2], Some(20.0)), (ids[0], ids[1], None)] {
            let (pa, pb) = (p.first_free_port(a, None).unwrap(), p.first_free_port(b, None).unwrap());
            let (_, e) = p.add_link_edit(Endpoint { device: a, port: pa }, Endpoint { device: b, port: pb }).unwrap();
            e.apply(&mut p);
            let l = p.links.last_mut().unwrap();
            l.cable = CableKind::Cat6A;
            l.length_m = len;
        }
        let bom = build(&p, BomOptions::default());
        let hw: Vec<_> = bom.iter().filter(|l| l.section == BomSection::Hardware).collect();
        assert_eq!(hw.len(), 2, "{hw:?}");
        assert_eq!(hw[0].qty, 2);
        assert!(hw[1].item.starts_with("PC"), "generic last, Internet excluded");
        let cab = bom.iter().find(|l| l.section == BomSection::Cabling).unwrap();
        assert_eq!((cab.qty, cab.length_m), (2, Some(22.0)));
        assert!(cab.notes.contains("1 without a length"));

        let csv = to_csv(&bom);
        assert!(csv.starts_with("Section,Item,Qty"));
        assert_eq!(csv.lines().count(), bom.len() + 1);
        assert!(csv.contains(",22,20 m measured + 10% slack; 1 without a length\r\n"));
        assert_eq!(to_csv(&[BomLine { notes: "a, \"b\"".into(), ..cab.clone() }]).lines().nth(1).unwrap().split_once("22,").unwrap().1, "\"a, \"\"b\"\"\"");
    }
}
