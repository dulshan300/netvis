//! The bundled sample projects must open and pass every design rule.

use std::path::Path;

fn load(name: &str) -> netvis_core::Project {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../samples").join(name);
    netvis_core::io::load(&path).unwrap_or_else(|e| panic!("{name}: {e}"))
}

#[test]
fn samples_validate_clean() {
    for name in ["corescale-hq.netvis", "corescale-campus.netvis"] {
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
