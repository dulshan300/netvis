//! Hostnames and MAC addresses.

use std::collections::HashMap;

use netvis_core::{Item, Project};

use crate::{Fix, Sink};

pub fn check(p: &Project, out: &mut Sink) {
    hostnames(p, out);
    macs(p, out);
    cable_labels(p, out);
}

/// Cable labels must be unique so a label identifies one cable.
fn cable_labels(p: &Project, out: &mut Sink) {
    let mut by_label: HashMap<String, Vec<netvis_core::LinkId>> = HashMap::new();
    for l in p.links.iter().filter(|l| !l.label.trim().is_empty()) {
        by_label.entry(l.label.trim().to_ascii_uppercase()).or_default().push(l.id);
    }
    let mut dups: Vec<_> = by_label.into_iter().filter(|(_, v)| v.len() > 1).collect();
    dups.sort();
    for (label, links) in dups {
        out.warn("cable.label-dup", format!("Cable label {label} is used on {} cables", links.len()), links.into_iter().map(Item::Link).collect());
    }
}

/// RFC 1123 label: letters, digits and hyphens, 1–63 chars, no leading/trailing hyphen.
fn valid_hostname(n: &str) -> bool {
    !n.is_empty()
        && n.len() <= 63
        && !n.starts_with('-')
        && !n.ends_with('-')
        && n.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

fn hostnames(p: &Project, out: &mut Sink) {
    let mut by_name: HashMap<String, Vec<&netvis_core::Device>> = HashMap::new();
    for d in &p.devices {
        if d.name.trim().is_empty() {
            out.warn("name.empty", format!("A {} has no hostname", d.kind.label()), vec![Item::Device(d.id)]);
            continue;
        }
        by_name.entry(d.name.to_ascii_lowercase()).or_default().push(d);
        if !valid_hostname(&d.name) {
            out.info("name.invalid", format!("“{}” is not a valid DNS hostname (letters, digits and hyphens only)", d.name), vec![Item::Device(d.id)]);
        }
    }
    let mut dups: Vec<_> = by_name.into_values().filter(|v| v.len() > 1).collect();
    dups.sort_by(|a, b| a[0].name.cmp(&b[0].name));
    for list in dups {
        let last = list.last().expect("len > 1");
        out.error("name.duplicate", format!("Hostname {} is used by {} devices", list[0].name, list.len()), list.iter().map(|d| Item::Device(d.id)).collect())
            .with_fix(Some(Fix::RenameDevice { device: last.id, name: p.next_device_name(last.kind) }));
    }
}

/// Normalise `aa:bb:cc:dd:ee:ff`, `aa-bb-…`, `aabb.ccdd.eeff` or bare hex.
pub(crate) fn parse_mac(s: &str) -> Option<[u8; 6]> {
    netvis_core::ip::Mac::parse(s).map(|m| m.0)
}

fn macs(p: &Project, out: &mut Sink) {
    let mut seen: HashMap<[u8; 6], Vec<String>> = HashMap::new();
    let mut owners: HashMap<[u8; 6], Vec<Item>> = HashMap::new();
    for d in &p.devices {
        for port in d.ports.iter().filter(|x| !x.cfg.mac.trim().is_empty()) {
            let who = format!("{}:{}", d.name, port.name);
            match parse_mac(&port.cfg.mac) {
                Some(m) => {
                    seen.entry(m).or_default().push(who);
                    owners.entry(m).or_default().push(Item::Device(d.id));
                }
                None => {
                    out.warn("mac.format", format!("{who}: “{}” is not a valid MAC address", port.cfg.mac), vec![Item::Device(d.id)]);
                }
            }
        }
    }
    for (m, who) in seen.into_iter().filter(|(_, v)| v.len() > 1) {
        let text = m.iter().map(|b| format!("{b:02x}")).collect::<Vec<_>>().join(":");
        out.error("mac.duplicate", format!("MAC {text} is used on {}", who.join(", ")), owners.remove(&m).unwrap_or_default());
    }
}

#[cfg(test)]
mod tests {
    use netvis_core::DeviceKind;

    use crate::testutil::T;

    #[test]
    fn mac_parsing() {
        assert_eq!(super::parse_mac("AA:bb:cc:00:11:22"), Some([0xaa, 0xbb, 0xcc, 0, 0x11, 0x22]));
        assert_eq!(super::parse_mac("aabb.cc00.1122"), super::parse_mac("aa-bb-cc-00-11-22"));
        assert_eq!(super::parse_mac("aabbcc00112"), None);
        assert_eq!(super::parse_mac("zz:bb:cc:00:11:22"), None);
    }

    #[test]
    fn duplicate_names_and_macs() {
        let mut t = T::new();
        let (a, b) = (t.dev(DeviceKind::Pc), t.dev(DeviceKind::Pc));
        t.p.device_mut(b).unwrap().name = "pc-01".into();
        assert_eq!(t.codes(), vec!["name.duplicate"]);
        t.fix("name.duplicate");
        assert!(t.codes().is_empty());

        t.p.device_mut(a).unwrap().ports[0].cfg.mac = "00:11:22:33:44:55".into();
        t.p.device_mut(b).unwrap().ports[0].cfg.mac = "0011.2233.4455".into();
        assert_eq!(t.codes(), vec!["mac.duplicate"]);
        t.p.device_mut(b).unwrap().ports[0].cfg.mac = "nope".into();
        assert_eq!(t.codes(), vec!["mac.format"]);
        t.p.device_mut(b).unwrap().name = "bad name!".into();
        assert!(t.codes().contains(&"name.invalid"));
    }
}
