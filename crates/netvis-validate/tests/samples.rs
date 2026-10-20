//! The bundled sample projects must open and pass every design rule.

use std::path::Path;

fn load(name: &str) -> netvis_core::Project {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../samples").join(name);
    netvis_core::io::load(&path).unwrap_or_else(|e| panic!("{name}: {e}"))
}

#[test]
fn samples_validate_clean() {
    for name in ["corescale-hq.netvis", "corescale-campus.netvis", "shopping-mall.netvis"] {
        let issues = netvis_validate::validate(&load(name));
        assert!(issues.is_empty(), "{name}: {:#?}", issues.iter().map(|i| &i.message).collect::<Vec<_>>());
    }
}

#[test]
fn campus_is_large_and_fully_patched() {
    let p = load("corescale-campus.netvis");
    assert!(p.devices.len() > 150 && p.links.len() > 300);
    // Every PC reaches a switch through a panel and an outlet.
    for pc in p.devices.iter().filter(|d| d.kind == netvis_core::DeviceKind::Pc) {
        let ep = netvis_core::Endpoint { device: pc.id, port: pc.ports[0].id };
        let c = p.circuit_from(ep).expect("cabled");
        assert_eq!(c.via.len(), 2, "{}", pc.name);
        let far = p.device(c.b.device).unwrap();
        assert_eq!(far.kind, netvis_core::DeviceKind::SwitchL2, "{}", pc.name);
    }
}

#[test]
fn mall_matches_the_brief() {
    use netvis_core::DeviceKind;
    let p = load("shopping-mall.netvis");
    let floors = ["BF", "GF", "L1", "L2", "L3", "L4", "L4A", "L5", "L6"];
    for f in floors {
        let count = |prefix: &str| p.devices.iter().filter(|d| d.name.starts_with(&format!("{prefix}-{f}-"))).count();
        let (cams, aps, acs, spk) = (count("CAM"), count("AP"), count("ACS"), count("SPK"));
        assert!((8..=18).contains(&cams), "{f}: {cams} cameras");
        assert!((3..=15).contains(&aps), "{f}: {aps} APs");
        assert_eq!(acs, 2, "{f}");
        assert_eq!(spk > 0, ["BF", "GF", "L1", "L2", "L3"].contains(&f), "{f}: BGM only on BF to L3");
        // Two switches per ELV room.
        for sys in ["SEC", "SVC"] {
            assert!(p.devices.iter().any(|d| d.name == format!("SW-{f}-{sys}")), "{f} {sys}");
        }
    }
    // Every camera is patched through its floor panel to its floor's security switch.
    for cam in p.devices.iter().filter(|d| d.kind == DeviceKind::IpCamera) {
        let floor = cam.name.split('-').nth(1).unwrap();
        let c = p.circuit_from(netvis_core::Endpoint { device: cam.id, port: cam.ports[0].id }).expect("cabled");
        assert_eq!(c.via.len(), 1, "{}", cam.name);
        assert_eq!(p.device(c.b.device).unwrap().name, format!("SW-{floor}-SEC"));
    }
    // L4A reaches the server room only through the L4 stack.
    let l4a = p.devices.iter().find(|d| d.name == "SW-L4A-SEC").unwrap();
    let up = l4a.ports.iter().find(|x| x.media == netvis_core::PortMedia::SfpPlus).unwrap();
    let c = p.circuit_from(netvis_core::Endpoint { device: l4a.id, port: up.id }).unwrap();
    assert_eq!(p.device(c.b.device).unwrap().name, "SW-L4-SEC");
}
