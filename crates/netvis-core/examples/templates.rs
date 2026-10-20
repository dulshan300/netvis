//! Generate the starter templates in `templates/`, from a home network up
//! to a small company. File → New from template in the app offers them.
//!
//! cargo run -p netvis-core --example templates

mod common;

use common::{Builder, addr};
use netvis_core::{
    AllowedVlans, AreaId, AreaKind, CableKind, DeviceKind, Edit, NatKind, NatRule, PortMedia, Project, Rect, Ssid,
    StaticRoute, SwitchportMode, VlanId, WifiBand, WifiSecurity,
};

fn main() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../templates");
    std::fs::create_dir_all(&dir).unwrap();
    for (file, p) in [
        ("home.netvis", home()),
        ("small-office.netvis", small_office()),
        ("small-company.netvis", small_company()),
        ("retail-store.netvis", retail_store()),
        ("cctv-system.netvis", cctv_system()),
    ] {
        println!("{file}: {} devices, {} cables", p.devices.len(), p.links.len());
        for i in netvis_validate::validate(&p) {
            println!("  {:<7} {:<18} {}", i.severity.label(), i.code, i.message);
        }
        netvis_core::io::save(&p, &dir.join(file)).unwrap();
    }
}

fn access(vlan: VlanId) -> SwitchportMode {
    SwitchportMode::Access { vlan: Some(vlan) }
}

fn trunk(native: VlanId, allowed: Vec<VlanId>) -> SwitchportMode {
    SwitchportMode::Trunk { native: Some(native), allowed: AllowedVlans::List(allowed) }
}

fn meta(b: &mut Builder, name: &str, description: &str) {
    b.p.meta.name = name.into();
    b.p.meta.description = description.into();
    b.p.meta.revision = "A".into();
}

fn ssid(b: &mut Builder, name: &str, vlan: VlanId, security: WifiSecurity) {
    let mut s = Ssid::new(name);
    s.vlan = Some(vlan);
    s.security = security;
    s.bands = vec![WifiBand::G2_4, WifiBand::G5];
    b.h.apply(&mut b.p, Edit::Ssid { id: s.id, before: None, after: Some(s) }, "ssid");
}

/// The firewall's WAN port on the ISP handoff, with a default route and PAT.
fn wan(b: &mut Builder, fw: netvis_core::DeviceId, isp: netvis_core::DeviceId, ip: &str, gw: &str, inside: &str) {
    let p = b.port(fw, 0);
    b.cable((isp, b.port(isp, 0)), (fw, p), CableKind::Virtual, None);
    b.cfg(fw, p, |c| {
        c.mode = SwitchportMode::Routed;
        c.l3.addresses = vec![addr(ip)];
        c.l3.gateway = Some(gw.parse().unwrap());
    });
    let r = &mut b.p.device_mut(fw).unwrap().routing;
    r.static_routes.push(StaticRoute { next_hop: Some(gw.parse().unwrap()), description: "Default via ISP".into(), ..Default::default() });
    r.nat.push(NatRule { kind: NatKind::SourcePat, inside: inside.into(), outside: "WAN".into(), service: "any".into(), description: "Internet access".into() });
}

/// Mount racks, address everything, label the cables.
fn finish(mut b: Builder, racks: &[AreaId]) -> Project {
    for d in b.p.devices.iter_mut().filter(|d| matches!(d.kind, DeviceKind::SwitchL2 | DeviceKind::SwitchL3)) {
        d.stp = netvis_core::StpMode::Rstp;
    }
    for rack in racks {
        let (e, _, no_room) = b.p.auto_mount_edit(*rack);
        assert!(no_room.is_empty(), "rack full: {no_room:?}");
        b.h.apply(&mut b.p, e, "rack");
    }
    let run = b.p.auto_assign_edit(None);
    for line in &run.skipped {
        println!("  skipped  {line}");
    }
    b.h.apply(&mut b.p, run.edit, "auto-assign");
    let (labels, _) = netvis_tools::patch::auto_label_edit(&b.p, "C-", false);
    b.h.apply(&mut b.p, labels, "labels");
    b.p
}

/// Basic: one router with Wi-Fi behind it, a small switch and a few devices.
fn home() -> Project {
    use DeviceKind::*;
    let mut b = Builder::default();
    meta(&mut b, "Home network", "A home or home-office network: internet router, a small PoE switch, one Wi-Fi access point and a few wired devices on one LAN.");
    let lan = b.vlan(1, "LAN", "Everything at home");
    b.subnet("Home LAN", "192.168.1.0/24", Some(lan), true);
    b.subnet("ISP handoff", "100.64.0.0/30", None, false);

    let isp = b.device(Isp, "ISP", 40.0, 40.0);
    let rtr = b.device(Router, "Router", 40.0, 200.0);
    let sw = b.device(SwitchL2, "SW-01", 40.0, 360.0);
    b.model(rtr, "rtr-branch");
    b.model(sw, "sw-compact-8p-poe");
    wan(&mut b, rtr, isp, "100.64.0.2/30", "100.64.0.1", "192.168.1.0/24");
    // Router LAN: an access port in the LAN VLAN with the gateway SVI.
    let (rp, sp) = (b.port(rtr, 1), b.port(sw, 7));
    b.cable((rtr, rp), (sw, sp), CableKind::Cat6, Some(1.0));
    b.cfg(rtr, rp, |c| c.mode = access(lan));
    b.cfg(sw, sp, |c| c.mode = access(lan));
    b.svi(rtr, 1, lan);

    let ap = b.device(AccessPoint, "AP-01", 260.0, 520.0);
    b.model(ap, "ap-wifi6-2x2");
    b.attach(sw, ap, CableKind::Cat6, 12.0, access(lan));
    for (kind, name, x, len, dhcp) in [(Pc, "PC-01", 40.0, 8.0, true), (SmartTv, "TV-01", 440.0, 10.0, true), (Storage, "NAS-01", 620.0, 2.0, false), (Printer, "PRN-01", 800.0, 6.0, false)] {
        let d = b.device(kind, name, x, 520.0);
        let (_, dp) = b.attach(sw, d, CableKind::Cat6, len, access(lan));
        if dhcp {
            b.cfg(d, dp, |c| c.l3.dhcp_client = true);
        }
    }
    ssid(&mut b, "Home", lan, WifiSecurity::Wpa3Personal);
    finish(b, &[])
}

/// Basic: firewall, one PoE switch in a small rack, staff, voice and guest
/// VLANs with the firewall routing them (router-on-a-stick).
fn small_office() -> Project {
    use DeviceKind::*;
    let mut b = Builder::default();
    meta(&mut b, "Small office", "Office for 10–20 people: a firewall routing staff, voice, guest and management VLANs, one PoE switch in a wall rack, two Wi-Fi APs, desks with IP phones, a printer and a NAS.");
    let mgmt = b.vlan(99, "Management", "Switch, APs, UPS");
    let staff = b.vlan(10, "Staff", "Desktops, printer, NAS");
    let voice = b.vlan(20, "Voice", "IP phones");
    let guest = b.vlan(30, "Guest", "Visitor Wi-Fi, internet only");
    b.subnet("Management", "10.0.99.0/24", Some(mgmt), false);
    b.subnet("Staff", "10.0.10.0/24", Some(staff), true);
    b.subnet("Voice", "10.0.20.0/24", Some(voice), true);
    b.subnet("Guest", "10.0.30.0/24", Some(guest), true);
    b.subnet("ISP handoff", "203.0.113.0/29", None, false);

    b.area(AreaKind::Site, "Office", Rect::new(0.0, 0.0, 1500.0, 760.0));
    let rack = b.area_id(AreaKind::Rack, "Comms cabinet", Rect::new(20.0, 120.0, 520.0, 260.0));
    if let Some(a) = b.p.area_mut(rack) {
        a.rack_units = 12;
    }
    let isp = b.device(Isp, "ISP", 60.0, -160.0);
    let fw = b.device(Firewall, "FW-01", 60.0, 170.0);
    let sw = b.device(SwitchL2, "SW-01", 220.0, 170.0);
    let pp = b.device(PatchPanel, "PP-01", 380.0, 170.0);
    let ups = b.device(Ups, "UPS-01", 60.0, 290.0);
    for (d, m) in [(fw, "fw-branch"), (sw, "sw-access-24p-poe"), (pp, "pp-24-cat6a"), (ups, "ups-1500")] {
        b.model(d, m);
    }
    wan(&mut b, fw, isp, "203.0.113.2/29", "203.0.113.1", "10.0.0.0/16");
    let (fp, sp) = (b.port(fw, 1), b.port(sw, 23));
    b.cable((fw, fp), (sw, sp), CableKind::Cat6A, Some(1.0));
    b.cfg(sw, sp, |c| c.mode = trunk(mgmt, vec![mgmt, staff, voice, guest]));
    for (tag, v) in [(99, mgmt), (10, staff), (20, voice), (30, guest)] {
        b.sub_if(fw, fp, tag, v);
    }
    b.svi(sw, 99, mgmt);
    b.attach(sw, ups, CableKind::Cat6, 1.0, access(mgmt));

    // Everything else runs through the patch panel.
    let mut n = 0;
    let mut patched = |b: &mut Builder, dev: netvis_core::DeviceId, port: usize, len: f32, mode: SwitchportMode| {
        let swp = b.p.first_free_port(sw, Some(PortMedia::Rj45)).unwrap();
        let (pr, dp) = (b.port(pp, n), b.port(dev, port));
        b.cable((sw, swp), (pp, pr), CableKind::Cat6, Some(0.5));
        b.cable((pp, pr), (dev, dp), CableKind::Cat6A, Some(len));
        b.cfg(sw, swp, |c| c.mode = mode);
        n += 1;
        dp
    };
    for (i, ch) in [36u16, 52].into_iter().enumerate() {
        let ap = b.device(AccessPoint, &format!("AP-{:02}", i + 1), 620.0 + i as f32 * 180.0, 170.0);
        b.model(ap, "ap-wifi6-2x2");
        let radio = b.port(ap, 1);
        b.cfg(ap, radio, |c| c.radio.as_mut().unwrap().channel = ch);
        patched(&mut b, ap, 0, 20.0 + i as f32 * 8.0, trunk(mgmt, vec![mgmt, staff, guest]));
    }
    for i in 0..6 {
        let x = 60.0 + i as f32 * 180.0;
        let pc = b.device(Pc, &format!("PC-{:02}", i + 1), x, 460.0);
        let dp = patched(&mut b, pc, 0, 15.0 + i as f32 * 4.0, access(staff));
        b.cfg(pc, dp, |c| c.l3.dhcp_client = true);
        let ph = b.device(IpPhone, &format!("PH-{:02}", i + 1), x, 580.0);
        b.model(ph, "phone-desk");
        let dp = patched(&mut b, ph, 0, 15.0 + i as f32 * 4.0, access(voice));
        b.cfg(ph, dp, |c| c.l3.dhcp_client = true);
    }
    let prn = b.device(Printer, "PRN-01", 1160.0, 460.0);
    patched(&mut b, prn, 0, 12.0, access(staff));
    let nas = b.device(Storage, "NAS-01", 1160.0, 580.0);
    patched(&mut b, nas, 0, 3.0, access(staff));

    ssid(&mut b, "Office", staff, WifiSecurity::Wpa2Wpa3Transition);
    ssid(&mut b, "Office-Guest", guest, WifiSecurity::Owe);
    finish(b, &[rack])
}

/// Intermediate: L3 core, server rack, two floors with fibre uplinks,
/// patch panels and wall outlets.
fn small_company() -> Project {
    use DeviceKind::*;
    let mut b = Builder::default();
    meta(&mut b, "Small company", "Company of 50–80 people on two floors: firewall, L3 core switch routing six VLANs, a server rack, an access switch per floor on fibre uplinks, desks on wall outlets, Wi-Fi and printers.");
    let mgmt = b.vlan(99, "Management", "Switches, APs, UPS, iLO");
    let staff = b.vlan(10, "Staff", "Desktops and corporate Wi-Fi");
    let voice = b.vlan(20, "Voice", "IP phones");
    let guest = b.vlan(30, "Guest", "Visitor Wi-Fi, internet only");
    let printers = b.vlan(40, "Printers", "Floor printers");
    let servers = b.vlan(100, "Servers", "File, directory and backup servers");
    b.subnet("Management", "10.10.99.0/24", Some(mgmt), false);
    b.subnet("Staff", "10.10.16.0/22", Some(staff), true);
    b.subnet("Voice", "10.10.20.0/24", Some(voice), true);
    b.subnet("Guest", "10.10.32.0/22", Some(guest), true);
    b.subnet("Printers", "10.10.40.0/24", Some(printers), false);
    b.subnet("Servers", "10.10.100.0/24", Some(servers), false);
    b.subnet("Transit FW-core", "10.10.255.0/30", None, false);
    b.subnet("ISP handoff", "203.0.113.8/29", None, false);

    b.area(AreaKind::Building, "Head office", Rect::new(0.0, 0.0, 2000.0, 1500.0));
    b.area(AreaKind::Floor, "Level 1 · Server room", Rect::new(20.0, 30.0, 1960.0, 560.0));
    let srv_rack = b.area_id(AreaKind::Rack, "Server rack", Rect::new(40.0, 60.0, 880.0, 300.0));
    let isp = b.device(Isp, "ISP", 60.0, -180.0);
    let fw = b.device(Firewall, "FW-01", 60.0, 100.0);
    let core = b.device(SwitchL3, "CSW-01", 220.0, 100.0);
    let fpp = b.device(PatchPanel, "FPP-SR-01", 380.0, 100.0);
    let ups = b.device(Ups, "UPS-SR-01", 540.0, 100.0);
    let dc = b.device(Server, "SRV-DC-01", 60.0, 240.0);
    let file = b.device(Server, "SRV-FILE-01", 220.0, 240.0);
    let nas = b.device(Storage, "NAS-BACKUP-01", 380.0, 240.0);
    for (d, m) in [(fw, "fw-branch"), (core, "sw-l3-48p-poe"), (fpp, "pp-12-lc"), (ups, "ups-3000"), (dc, "srv-1u"), (file, "srv-2u"), (nas, "nas-2u-12bay")] {
        b.model(d, m);
    }
    wan(&mut b, fw, isp, "203.0.113.10/29", "203.0.113.9", "10.10.0.0/16");
    let (fp, cp) = (b.port(fw, 1), b.port(core, 0));
    b.cable((fw, fp), (core, cp), CableKind::Cat6A, Some(1.0));
    b.cfg(fw, fp, |c| {
        c.mode = SwitchportMode::Routed;
        c.l3.addresses = vec![addr("10.10.255.1/30")];
    });
    b.cfg(core, cp, |c| {
        c.mode = SwitchportMode::Routed;
        c.l3.addresses = vec![addr("10.10.255.2/30")];
    });
    b.p.device_mut(fw).unwrap().routing.static_routes.push(StaticRoute { dest: "10.10.0.0/16".parse().unwrap(), next_hop: Some("10.10.255.2".parse().unwrap()), description: "Office via core".into(), ..Default::default() });
    b.p.device_mut(core).unwrap().routing.static_routes.push(StaticRoute { next_hop: Some("10.10.255.1".parse().unwrap()), description: "Default via firewall".into(), ..Default::default() });
    for (tag, v) in [(99, mgmt), (10, staff), (20, voice), (30, guest), (40, printers), (100, servers)] {
        b.svi(core, tag, v);
    }
    for d in [dc, file, nas] {
        b.attach(core, d, CableKind::Cat6A, 2.0, access(servers));
    }
    b.attach(core, ups, CableKind::Cat6, 1.0, access(mgmt));

    let mut racks = vec![srv_rack];
    for f in 0..2 {
        let fy = 620.0 + f as f32 * 440.0;
        let lvl = f + 2;
        b.area(AreaKind::Floor, &format!("Level {lvl} · Office"), Rect::new(20.0, fy, 1960.0, 410.0));
        let rack = b.area_id(AreaKind::Rack, &format!("Comms L{lvl}"), Rect::new(40.0, fy + 30.0, 520.0, 160.0));
        if let Some(a) = b.p.area_mut(rack) {
            a.rack_units = 18;
        }
        racks.push(rack);
        let sw = b.device(SwitchL2, &format!("SW-L{lvl}-01"), 60.0, fy + 60.0);
        let pp = b.device(PatchPanel, &format!("PP-L{lvl}-01"), 220.0, fy + 60.0);
        let ffp = b.device(PatchPanel, &format!("FPP-L{lvl}-01"), 380.0, fy + 60.0);
        for (d, m) in [(sw, "sw-access-48p-poe"), (pp, "pp-48-cat6a"), (ffp, "pp-12-lc")] {
            b.model(d, m);
        }
        // Uplink: switch → floor fibre panel → riser → server-room panel → core.
        let (sp, fpp_floor, fpp_sr, cup) = (b.port(sw, 48), b.port(ffp, 0), b.port(fpp, f), b.port(core, 48 + f));
        b.cable((sw, sp), (ffp, fpp_floor), CableKind::Om4, Some(2.0));
        b.cable((ffp, fpp_floor), (fpp, fpp_sr), CableKind::Om4, Some(25.0 + f as f32 * 5.0));
        b.cable((fpp, fpp_sr), (core, cup), CableKind::Om4, Some(2.0));
        b.tx(sw, sp, "sfpp-10g-sr");
        b.tx(core, cup, "sfpp-10g-sr");
        let t = SwitchportMode::Trunk { native: Some(mgmt), allowed: AllowedVlans::All };
        b.cfg(sw, sp, |c| c.mode = t.clone());
        b.cfg(core, cup, |c| c.mode = t);
        b.svi(sw, 99, mgmt);

        let mut pp_port = 0;
        for d in 0..8 {
            let x = 620.0 + (d % 8) as f32 * 160.0;
            let wo = b.device(WallOutlet, &format!("WO-L{lvl}-{:02}", d + 1), x, fy + 60.0);
            let pc = b.device(Pc, &format!("PC-L{lvl}-{:02}", d + 1), x, fy + 170.0);
            let ph = b.device(IpPhone, &format!("PH-L{lvl}-{:02}", d + 1), x, fy + 280.0);
            b.model(ph, "phone-desk");
            let run = 20.0 + (d * 6) as f32;
            for (side, dev, vlan) in [(0, pc, staff), (1, ph, voice)] {
                let (pr, wp) = (b.port(pp, pp_port), b.port(wo, side));
                b.cable((pp, pr), (wo, wp), CableKind::Cat6A, Some(run));
                let swp = b.p.first_free_port(sw, Some(PortMedia::Rj45)).unwrap();
                b.cable((sw, swp), (pp, pr), CableKind::Cat6, Some(0.5));
                b.cfg(sw, swp, |c| c.mode = access(vlan));
                let dp = b.port(dev, 0);
                b.cable((wo, wp), (dev, dp), CableKind::Cat6, Some(3.0));
                b.cfg(dev, dp, |c| c.l3.dhcp_client = true);
                pp_port += 1;
            }
        }
        for (i, ch) in [36u16, 52, 100].into_iter().enumerate() {
            let ap = b.device(AccessPoint, &format!("AP-L{lvl}-{:02}", i + 1), 60.0 + i as f32 * 160.0, fy + 240.0);
            b.model(ap, "ap-wifi6-2x2");
            let radio = b.port(ap, 1);
            b.cfg(ap, radio, |c| c.radio.as_mut().unwrap().channel = ch);
            let (pr, dp) = (b.port(pp, pp_port), b.port(ap, 0));
            b.cable((pp, pr), (ap, dp), CableKind::Cat6A, Some(30.0 + i as f32 * 10.0));
            let swp = b.p.first_free_port(sw, Some(PortMedia::Rj45)).unwrap();
            b.cable((sw, swp), (pp, pr), CableKind::Cat6, Some(0.5));
            b.cfg(sw, swp, |c| c.mode = trunk(mgmt, vec![mgmt, staff, guest]));
            pp_port += 1;
        }
        let prn = b.device(Printer, &format!("PRN-L{lvl}-01"), 540.0, fy + 280.0);
        let (pr, dp) = (b.port(pp, pp_port), b.port(prn, 0));
        b.cable((pp, pr), (prn, dp), CableKind::Cat6A, Some(18.0));
        let swp = b.p.first_free_port(sw, Some(PortMedia::Rj45)).unwrap();
        b.cable((sw, swp), (pp, pr), CableKind::Cat6, Some(0.5));
        b.cfg(sw, swp, |c| c.mode = access(printers));
    }
    let mut corp = Ssid::new("Company");
    corp.vlan = Some(staff);
    corp.security = WifiSecurity::Wpa3Enterprise;
    corp.bands = vec![WifiBand::G5];
    b.h.apply(&mut b.p, Edit::Ssid { id: corp.id, before: None, after: Some(corp) }, "ssid");
    ssid(&mut b, "Company-Guest", guest, WifiSecurity::Owe);
    finish(b, &racks)
}

/// Basic: a shop with tills kept apart from CCTV, back office and guest Wi-Fi.
fn retail_store() -> Project {
    use DeviceKind::*;
    let mut b = Builder::default();
    meta(&mut b, "Retail store", "A shop: tills on their own VLAN (keep card payments separate), IP cameras with a recorder, a back-office PC and printer, and customer Wi-Fi that only reaches the internet.");
    let mgmt = b.vlan(99, "Management", "Switch and APs");
    let pos = b.vlan(10, "POS", "Tills and payment terminals (PCI scope)");
    let office = b.vlan(20, "Office", "Back office");
    let cctv = b.vlan(30, "CCTV", "Cameras and recorder");
    let guest = b.vlan(40, "Guest", "Customer Wi-Fi, internet only");
    b.subnet("Management", "172.16.99.0/24", Some(mgmt), false);
    b.subnet("POS", "172.16.10.0/24", Some(pos), false);
    b.subnet("Office", "172.16.20.0/24", Some(office), true);
    b.subnet("CCTV", "172.16.30.0/24", Some(cctv), false);
    b.subnet("Guest", "172.16.64.0/22", Some(guest), true);
    b.subnet("ISP handoff", "198.51.100.0/29", None, false);

    b.area(AreaKind::Site, "Store", Rect::new(0.0, 0.0, 1300.0, 700.0));
    b.area(AreaKind::SecurityZone, "PCI zone", Rect::new(560.0, 330.0, 560.0, 140.0));
    let isp = b.device(Isp, "ISP", 60.0, -160.0);
    let fw = b.device(Firewall, "FW-01", 60.0, 60.0);
    let sw = b.device(SwitchL2, "SW-01", 60.0, 200.0);
    b.model(fw, "fw-branch");
    b.model(sw, "sw-access-24p-poe");
    wan(&mut b, fw, isp, "198.51.100.2/29", "198.51.100.1", "172.16.0.0/16");
    let (fp, sp) = (b.port(fw, 1), b.port(sw, 0));
    b.cable((fw, fp), (sw, sp), CableKind::Cat6, Some(1.0));
    b.cfg(sw, sp, |c| c.mode = trunk(mgmt, vec![mgmt, pos, office, cctv, guest]));
    for (tag, v) in [(99, mgmt), (10, pos), (20, office), (30, cctv), (40, guest)] {
        b.sub_if(fw, fp, tag, v);
    }
    b.svi(sw, 99, mgmt);

    for i in 0..3 {
        let t = b.device(Pos, &format!("POS-{:02}", i + 1), 600.0 + i as f32 * 170.0, 370.0);
        b.model(t, "pos-terminal");
        b.attach(sw, t, CableKind::Cat6, 15.0 + i as f32 * 3.0, access(pos));
    }
    let nvr = b.device(Storage, "NVR-01", 260.0, 200.0);
    b.attach(sw, nvr, CableKind::Cat6, 2.0, access(cctv));
    for i in 0..4 {
        let c = b.device(IpCamera, &format!("CAM-{:02}", i + 1), 60.0 + i as f32 * 170.0, 540.0);
        b.model(c, if i == 0 { "cam-bullet" } else { "cam-dome" });
        b.attach(sw, c, CableKind::Cat6, 20.0 + i as f32 * 6.0, access(cctv));
    }
    let pc = b.device(Pc, "PC-OFFICE-01", 600.0, 60.0);
    let (_, dp) = b.attach(sw, pc, CableKind::Cat6, 10.0, access(office));
    b.cfg(pc, dp, |c| c.l3.dhcp_client = true);
    let prn = b.device(Printer, "PRN-01", 780.0, 60.0);
    b.attach(sw, prn, CableKind::Cat6, 10.0, access(office));
    for (i, ch) in [36u16, 149].into_iter().enumerate() {
        let ap = b.device(AccessPoint, &format!("AP-{:02}", i + 1), 600.0 + i as f32 * 180.0, 200.0);
        b.model(ap, "ap-wifi6-2x2");
        let radio = b.port(ap, 1);
        b.cfg(ap, radio, |c| c.radio.as_mut().unwrap().channel = ch);
        b.attach(sw, ap, CableKind::Cat6, 18.0, trunk(mgmt, vec![mgmt, office, guest]));
    }
    ssid(&mut b, "Store-Staff", office, WifiSecurity::Wpa3Personal);
    ssid(&mut b, "Store-Free-WiFi", guest, WifiSecurity::Owe);
    finish(b, &[])
}

/// Basic/intermediate: a stand-alone IP CCTV network with recording.
fn cctv_system() -> Project {
    use DeviceKind::*;
    let mut b = Builder::default();
    meta(&mut b, "IP CCTV system", "A stand-alone surveillance network: two PoE camera switches on fibre uplinks to a core switch, 20 cameras, a recording server with storage and a monitoring workstation.");
    let cctv = b.vlan(30, "CCTV", "Cameras and recording");
    let mgmt = b.vlan(99, "Management", "Switches and UPS");
    b.subnet("CCTV", "10.30.0.0/23", Some(cctv), false);
    b.subnet("Management", "10.30.99.0/24", Some(mgmt), false);

    b.area(AreaKind::Site, "Site", Rect::new(0.0, 0.0, 2300.0, 640.0));
    let rack = b.area_id(AreaKind::Rack, "Security rack", Rect::new(20.0, 30.0, 700.0, 300.0));
    let core = b.device(SwitchL3, "CSW-01", 60.0, 80.0);
    let vms = b.device(Server, "VMS-01", 220.0, 80.0);
    let nas = b.device(Storage, "NAS-REC-01", 380.0, 80.0);
    let ups = b.device(Ups, "UPS-01", 540.0, 80.0);
    for (d, m) in [(core, "sw-dist-24x10g"), (vms, "srv-2u"), (nas, "nas-2u-12bay"), (ups, "ups-3000")] {
        b.model(d, m);
    }
    b.svi(core, 30, cctv);
    b.svi(core, 99, mgmt);
    for port in &mut b.p.device_mut(vms).unwrap().ports {
        if port.media == PortMedia::Sfp28 {
            port.speed_mbps = 10_000;
        }
    }
    for (i, (d, p)) in [(vms, 5), (nas, 4)].into_iter().enumerate() {
        let (cp, dp) = (b.port(core, i), b.port(d, p));
        b.cable((core, cp), (d, dp), CableKind::Dac, Some(2.0));
        b.cfg(core, cp, |c| c.mode = access(cctv));
    }
    let ws = b.device(Pc, "PC-MONITOR-01", 220.0, 220.0);
    let mon_sw = b.device(SwitchL2, "SW-SEC-00", 60.0, 220.0);
    b.model(mon_sw, "sw-access-24p-poe");
    let (cp, sp) = (b.port(core, 2), b.port(mon_sw, 24));
    b.cable((core, cp), (mon_sw, sp), CableKind::Dac, Some(1.0));
    let t = SwitchportMode::Trunk { native: Some(mgmt), allowed: AllowedVlans::All };
    b.cfg(core, cp, |c| c.mode = t.clone());
    b.cfg(mon_sw, sp, |c| c.mode = t.clone());
    b.svi(mon_sw, 99, mgmt);
    b.attach(mon_sw, ws, CableKind::Cat6, 3.0, access(cctv));
    b.attach(mon_sw, ups, CableKind::Cat6, 1.0, access(mgmt));

    let mut racks = vec![rack];
    for z in 0..2 {
        let zx = 760.0 + z as f32 * 760.0;
        let cab = b.area_id(AreaKind::Rack, &format!("Field cabinet {}", z + 1), Rect::new(zx, 30.0, 340.0, 150.0));
        if let Some(a) = b.p.area_mut(cab) {
            a.rack_units = 9;
        }
        racks.push(cab);
        let sw = b.device(SwitchL2, &format!("SW-SEC-{:02}", z + 1), zx + 20.0, 80.0);
        b.model(sw, "sw-access-24p-poe");
        let (sp, cp) = (b.port(sw, 24), b.port(core, 3 + z));
        b.cable((sw, sp), (core, cp), CableKind::Os2, Some(120.0 + z as f32 * 80.0));
        b.tx(sw, sp, "sfpp-10g-lr");
        b.tx(core, cp, "sfpp-10g-lr");
        b.cfg(sw, sp, |c| c.mode = t.clone());
        b.cfg(core, cp, |c| c.mode = t.clone());
        b.svi(sw, 99, mgmt);
        for i in 0..10 {
            let c = b.device(IpCamera, &format!("CAM-{}{:02}", (b'A' + z as u8) as char, i + 1), zx + (i % 4) as f32 * 170.0, 260.0 + (i / 4) as f32 * 110.0);
            b.model(c, if i % 3 == 0 { "cam-bullet" } else { "cam-dome" });
            b.attach(sw, c, CableKind::Cat6, 25.0 + (i * 6) as f32, access(cctv));
        }
    }
    finish(b, &racks)
}
