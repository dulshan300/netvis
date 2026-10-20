//! Generate `samples/corescale-campus.netvis`: a larger campus used to test
//! the app, the design rules and the PDF report at scale.
//!
//! - Data centre: dual-ISP firewall, two core switches in a LAG, a
//!   management switch, VM hosts and storage dual-homed to both cores,
//!   a wireless controller, a DMZ with web and mail servers.
//! - Building A (3 floors) and Building B (2 floors). Each floor has an IDF
//!   rack with two access switches, a 48-port copper panel and a 12-port LC
//!   fibre panel, desks on wall outlets (PC on A, phone on B), three APs,
//!   two cameras and a printer, all patched through the panel.
//! - Every access switch uplinks over a single-mode riser through the floor
//!   and data-centre fibre panels to one of the cores.
//!
//! cargo run -p netvis-core --example sample_campus

mod common;

use common::{Builder, addr};
use netvis_core::{
    AllowedVlans, AreaKind, CableKind, DeviceId, DeviceKind, Edit, NatKind, NatRule, PortMedia, Rect, Ssid,
    StaticRoute, SwitchportMode, VlanId, WifiBand, WifiSecurity,
};

struct Vlans {
    mgmt: VlanId,
    staff: VlanId,
    voice: VlanId,
    cctv: VlanId,
    guest: VlanId,
    printers: VlanId,
    servers: VlanId,
    storage: VlanId,
    dmz: VlanId,
}

/// Core ports used for floor uplinks start at xe13 (index 12).
const FLOOR_UPLINK_BASE: usize = 12;

fn main() {
    use DeviceKind::*;
    let mut b = Builder::default();
    b.p.meta.name = "Corescale Campus".into();
    b.p.meta.client = "Corescale".into();
    b.p.meta.author = "Network Engineering".into();
    b.p.meta.revision = "B".into();
    b.p.meta.description =
        "Two-building campus with a data centre: redundant core, dual ISPs, five floors of access with patched desks and Wi-Fi, single-mode risers.".into();
    b.p.services.domain = "campus.corescale.local".into();
    b.p.services.dns_servers = vec!["10.20.100.10".parse().unwrap(), "10.20.100.11".parse().unwrap()];
    b.p.services.ntp_servers = vec!["pool.ntp.org".into()];
    b.p.services.syslog = "10.20.100.20".into();

    let v = Vlans {
        mgmt: b.vlan(10, "Management", "Switches, APs, UPS and out-of-band"),
        staff: b.vlan(20, "Staff", "Desktops and corporate Wi-Fi"),
        voice: b.vlan(30, "Voice", "Desk phones"),
        cctv: b.vlan(40, "CCTV", "Cameras"),
        guest: b.vlan(50, "Guest", "Guest Wi-Fi, internet only"),
        printers: b.vlan(60, "Printers", "Floor printers"),
        servers: b.vlan(100, "Servers", "VM hosts and services"),
        storage: b.vlan(110, "Storage", "iSCSI / NFS"),
        dmz: b.vlan(200, "DMZ", "Public-facing servers"),
    };
    b.subnet("Management", "10.20.10.0/24", Some(v.mgmt), false);
    b.subnet("Staff", "10.20.20.0/22", Some(v.staff), true);
    b.subnet("Voice", "10.20.30.0/24", Some(v.voice), true);
    b.subnet("CCTV", "10.20.40.0/24", Some(v.cctv), false);
    b.subnet("Guest", "10.20.64.0/20", Some(v.guest), true);
    b.subnet("Printers", "10.20.60.0/24", Some(v.printers), false);
    b.subnet("Servers", "10.20.100.0/24", Some(v.servers), false);
    b.subnet("Storage", "10.20.110.0/24", Some(v.storage), false);
    b.subnet("DMZ", "172.16.99.0/28", Some(v.dmz), false);
    b.subnet("Transit FW-core", "10.20.255.0/30", None, false);
    b.subnet("ISP-A handoff", "203.0.113.8/29", None, false);
    b.subnet("ISP-B handoff", "198.51.100.16/29", None, false);
    // The DMZ gateway is the firewall's address, not the first host.
    if let Some(s) = b.p.subnets.iter_mut().find(|s| s.name == "DMZ") {
        s.gateway = Some("172.16.99.1".parse().unwrap());
    }

    // ---- areas ----
    b.area(AreaKind::Site, "Corescale Campus", Rect::new(0.0, 0.0, 3360.0, 2700.0));
    b.area(AreaKind::Building, "Data Centre", Rect::new(40.0, 40.0, 3280.0, 600.0));
    let rack_a = b.area_id(AreaKind::Rack, "DC-A Network", Rect::new(80.0, 90.0, 820.0, 520.0));
    let rack_b = b.area_id(AreaKind::Rack, "DC-B Compute", Rect::new(960.0, 90.0, 700.0, 520.0));
    b.area(AreaKind::SecurityZone, "DMZ", Rect::new(1720.0, 90.0, 720.0, 300.0));

    // ---- WAN and data centre ----
    let isp_a = b.device(Isp, "ISP-A", 900.0, -260.0);
    let isp_b = b.device(Isp, "ISP-B", 1500.0, -260.0);
    let fw = b.device(Firewall, "FW-01", 120.0, 130.0);
    let sw_dc = b.device(SwitchL2, "SW-DC-01", 300.0, 130.0);
    let wlc = b.device(WirelessController, "WLC-01", 480.0, 130.0);
    let csw1 = b.device(SwitchL3, "CSW-01", 120.0, 300.0);
    let csw2 = b.device(SwitchL3, "CSW-02", 480.0, 300.0);
    let fpp_dc1 = b.device(PatchPanel, "FPP-DC-01", 120.0, 450.0);
    let fpp_dc2 = b.device(PatchPanel, "FPP-DC-02", 480.0, 450.0);
    let ups_a = b.device(Ups, "UPS-DC-A", 700.0, 130.0);
    for (d, m) in [(fw, "fw-campus"), (sw_dc, "sw-access-48p"), (csw1, "sw-dist-24x10g"), (csw2, "sw-dist-24x10g"), (fpp_dc1, "pp-12-lc"), (fpp_dc2, "pp-12-lc"), (ups_a, "ups-3000")] {
        b.model(d, m);
    }

    let mut hosts = Vec::new();
    for i in 0..4 {
        let h = b.device(VmHost, &format!("HV-{:02}", i + 1), 1000.0 + (i % 2) as f32 * 180.0, 130.0 + (i / 2) as f32 * 120.0);
        b.model(h, "hv-2u");
        hosts.push(h);
    }
    let mut nas = Vec::new();
    for i in 0..2 {
        let n = b.device(Storage, &format!("NAS-{:02}", i + 1), 1000.0 + i as f32 * 180.0, 380.0);
        b.model(n, "nas-2u-12bay");
        nas.push(n);
    }
    let ups_b1 = b.device(Ups, "UPS-DC-B1", 1400.0, 130.0);
    let ups_b2 = b.device(Ups, "UPS-DC-B2", 1400.0, 250.0);
    b.model(ups_b1, "ups-3000");
    b.model(ups_b2, "ups-3000");
    // 25G host NICs run at 10G towards the 10G cores.
    for d in &hosts {
        for port in &mut b.p.device_mut(*d).unwrap().ports {
            if port.media == PortMedia::Sfp28 {
                port.speed_mbps = 10_000;
            }
        }
    }

    let sw_dmz = b.device(SwitchL2, "SW-DMZ-01", 1760.0, 140.0);
    let web = b.device(Server, "SRV-WEB-01", 1960.0, 140.0);
    let mail = b.device(Server, "SRV-MAIL-01", 2140.0, 140.0);
    b.model(sw_dmz, "sw-access-24p-poe");
    b.model(web, "srv-1u");
    b.model(mail, "srv-1u");

    let trunk_all = SwitchportMode::Trunk { native: Some(v.mgmt), allowed: AllowedVlans::All };
    let access = |vlan| SwitchportMode::Access { vlan: Some(vlan) };

    // WAN.
    let (fw_wan_a, fw_wan_b) = (b.port(fw, 0), b.port(fw, 1));
    b.cable((isp_a, b.port(isp_a, 0)), (fw, fw_wan_a), CableKind::Virtual, None);
    b.cable((isp_b, b.port(isp_b, 0)), (fw, fw_wan_b), CableKind::Virtual, None);
    b.cfg(fw, fw_wan_a, |c| {
        c.mode = SwitchportMode::Routed;
        c.l3.addresses = vec![addr("203.0.113.10/29")];
        c.l3.gateway = Some("203.0.113.9".parse().unwrap());
    });
    b.cfg(fw, fw_wan_b, |c| {
        c.mode = SwitchportMode::Routed;
        c.l3.addresses = vec![addr("198.51.100.18/29")];
        c.l3.gateway = Some("198.51.100.17".parse().unwrap());
    });
    // Firewall ↔ core transit, DMZ.
    let (fw_in, core_up) = (b.port(fw, 16), b.port(csw1, 0));
    b.cable((fw, fw_in), (csw1, core_up), CableKind::Dac, Some(1.0));
    b.cfg(fw, fw_in, |c| {
        c.mode = SwitchportMode::Routed;
        c.l3.addresses = vec![addr("10.20.255.1/30")];
    });
    b.cfg(csw1, core_up, |c| {
        c.mode = SwitchportMode::Routed;
        c.l3.addresses = vec![addr("10.20.255.2/30")];
    });
    let (fw_dmz, dmz_up) = (b.port(fw, 17), b.port(sw_dmz, 24));
    b.cable((fw, fw_dmz), (sw_dmz, dmz_up), CableKind::Dac, Some(3.0));
    b.cfg(fw, fw_dmz, |c| {
        c.mode = SwitchportMode::Routed;
        c.l3.addresses = vec![addr("172.16.99.1/28")];
    });
    b.cfg(sw_dmz, dmz_up, |c| c.mode = access(v.dmz));
    for (i, (srv, ip)) in [(web, "172.16.99.10/28"), (mail, "172.16.99.11/28")].into_iter().enumerate() {
        let (sp, nic) = (b.port(sw_dmz, i), b.port(srv, 0));
        b.cable((sw_dmz, sp), (srv, nic), CableKind::Cat6A, Some(3.0));
        b.cfg(sw_dmz, sp, |c| c.mode = access(v.dmz));
        b.cfg(srv, nic, |c| {
            c.l3.addresses = vec![addr(ip)];
            c.l3.gateway = Some("172.16.99.1".parse().unwrap());
        });
    }
    {
        let r = &mut b.p.device_mut(fw).unwrap().routing;
        r.static_routes.push(StaticRoute { dest: "10.20.0.0/16".parse().unwrap(), next_hop: Some("10.20.255.2".parse().unwrap()), description: "Campus via core".into(), ..Default::default() });
        r.static_routes.push(StaticRoute { next_hop: Some("203.0.113.9".parse().unwrap()), description: "Default via ISP-A".into(), ..Default::default() });
        r.static_routes.push(StaticRoute { next_hop: Some("198.51.100.17".parse().unwrap()), distance: 10, description: "Backup default via ISP-B".into(), ..Default::default() });
        r.nat.push(NatRule { kind: NatKind::SourcePat, inside: "10.20.0.0/16".into(), outside: "port1 / port2".into(), service: "any".into(), description: "Internet access".into() });
        r.nat.push(NatRule { kind: NatKind::PortForward, inside: "172.16.99.10".into(), outside: "203.0.113.11".into(), service: "tcp/443".into(), description: "Public website".into() });
        r.nat.push(NatRule { kind: NatKind::PortForward, inside: "172.16.99.11".into(), outside: "203.0.113.12".into(), service: "tcp/25,587".into(), description: "Mail".into() });
    }
    b.p.device_mut(csw1).unwrap().routing.static_routes.push(StaticRoute {
        next_hop: Some("10.20.255.1".parse().unwrap()),
        description: "Default via firewall".into(),
        ..Default::default()
    });

    // Core pair: two 40G DACs in LAG 1.
    for i in 0..2 {
        let (a, c) = (b.port(csw1, 24 + i), b.port(csw2, 24 + i));
        b.cable((csw1, a), (csw2, c), CableKind::Dac, Some(1.0));
        for (d, p) in [(csw1, a), (csw2, c)] {
            let t = trunk_all.clone();
            b.cfg(d, p, |cfg| {
                cfg.mode = t;
                cfg.lag = Some(1);
            });
        }
    }
    for (tag, vl) in [(10, v.mgmt), (20, v.staff), (30, v.voice), (40, v.cctv), (50, v.guest), (60, v.printers), (100, v.servers), (110, v.storage)] {
        b.svi(csw1, tag, vl);
    }
    b.svi(csw2, 10, v.mgmt);

    // Management switch and wireless controller.
    let (c, s) = (b.port(csw1, 1), b.port(sw_dc, 48));
    b.cable((csw1, c), (sw_dc, s), CableKind::Dac, Some(2.0));
    let t = trunk_all.clone();
    b.cfg(csw1, c, |cfg| cfg.mode = t.clone());
    b.cfg(sw_dc, s, |cfg| cfg.mode = t);
    b.svi(sw_dc, 10, v.mgmt);
    let (c, w) = (b.port(csw1, 2), b.port(wlc, 2));
    b.cable((csw1, c), (wlc, w), CableKind::Dac, Some(1.0));
    b.cfg(csw1, c, |cfg| cfg.mode = access(v.mgmt));

    // Hosts and storage: one leg to each core; management on SW-DC-01.
    let mut dc_mgmt_port = 0;
    let mut mgmt_link = |b: &mut Builder, dev: DeviceId, port: usize| {
        let (sp, dp) = (b.port(sw_dc, dc_mgmt_port), b.port(dev, port));
        b.cable((sw_dc, sp), (dev, dp), CableKind::Cat6A, Some(2.0));
        b.cfg(sw_dc, sp, |c| c.mode = access(v.mgmt));
        dc_mgmt_port += 1;
    };
    for (i, h) in hosts.iter().enumerate() {
        for (core, leg) in [(csw1, 1), (csw2, 2)] {
            let (cp, hp) = (b.port(core, 3 + i), b.port(*h, leg));
            b.cable((core, cp), (*h, hp), CableKind::Dac, Some(2.0));
            b.cfg(core, cp, |c| c.mode = access(v.servers));
        }
        mgmt_link(&mut b, *h, 0);
    }
    for (i, n) in nas.iter().enumerate() {
        for (core, leg) in [(csw1, 4), (csw2, 5)] {
            let (cp, np) = (b.port(core, 7 + i), b.port(*n, leg));
            b.cable((core, cp), (*n, np), CableKind::Dac, Some(2.0));
            b.cfg(core, cp, |c| c.mode = access(v.storage));
        }
        mgmt_link(&mut b, *n, 0);
    }
    for u in [ups_a, ups_b1, ups_b2] {
        mgmt_link(&mut b, u, 0);
    }

    // ---- buildings ----
    let floors: Vec<(&str, usize, f32, f32)> = vec![("A", 3, 40.0, 120.0), ("B", 2, 1720.0, 260.0)];
    let mut floor_no = 0;
    let mut idfs = Vec::new();
    for (bld, n, bx, riser0) in floors {
        let by = 700.0;
        b.area(AreaKind::Building, &format!("Building {bld}"), Rect::new(bx, by, 1600.0, n as f32 * 640.0 + 40.0));
        for f in 0..n {
            let name = format!("{bld}{}", f + 1);
            let (fx, fy) = (bx + 20.0, by + 30.0 + f as f32 * 640.0);
            b.area(AreaKind::Floor, &format!("Building {bld} · Level {}", f + 1), Rect::new(fx, fy, 1560.0, 610.0));
            let idf = b.area_id(AreaKind::Rack, &format!("IDF {name}"), Rect::new(fx + 20.0, fy + 30.0, 660.0, 270.0));
            if let Some(a) = b.p.area_mut(idf) {
                a.rack_units = 24;
            }
            idfs.push(idf);
            let riser = riser0 + f as f32 * 15.0;
            floor(&mut b, &v, &name, fx, fy, riser, floor_no, [csw1, csw2], [fpp_dc1, fpp_dc2]);
            floor_no += 1;
        }
    }

    // Wireless networks.
    let mut corp = Ssid::new("Corescale");
    corp.vlan = Some(v.staff);
    corp.security = WifiSecurity::Wpa3Enterprise;
    corp.bands = vec![WifiBand::G5, WifiBand::G6];
    let mut guest = Ssid::new("Corescale-Guest");
    guest.vlan = Some(v.guest);
    guest.security = WifiSecurity::Owe;
    guest.bands = vec![WifiBand::G2_4, WifiBand::G5];
    for s in [corp, guest] {
        b.h.apply(&mut b.p, Edit::Ssid { id: s.id, before: None, after: Some(s) }, "ssid");
    }

    for d in b.p.devices.iter_mut().filter(|d| matches!(d.kind, SwitchL2 | SwitchL3)) {
        d.stp = netvis_core::StpMode::Rstp;
    }

    for rack in [rack_a, rack_b].into_iter().chain(idfs) {
        let (e, _, no_room) = b.p.auto_mount_edit(rack);
        assert!(no_room.is_empty(), "rack full: {no_room:?}");
        b.h.apply(&mut b.p, e, "rack");
    }

    let run = b.p.auto_assign_edit(None);
    println!("  auto-assigned {} address(es), skipped {}", run.assigned.len(), run.skipped.len());
    for line in &run.skipped {
        println!("  skipped  {line}");
    }
    b.h.apply(&mut b.p, run.edit, "auto-assign");
    let (labels, n) = netvis_tools::patch::auto_label_edit(&b.p, "C-", false);
    b.h.apply(&mut b.p, labels, "labels");

    println!(
        "  {} devices, {} cables, {} runs, {} labelled",
        b.p.devices.len(),
        b.p.links.len(),
        b.p.circuits().len(),
        n
    );
    for i in netvis_validate::validate(&b.p) {
        println!("  {:<7} {:<18} {}", i.severity.label(), i.code, i.message);
    }

    let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../samples/corescale-campus.netvis");
    netvis_core::io::save(&b.p, &out).unwrap();
    println!("wrote {}", out.display());
}

/// One floor: IDF rack (two access switches, copper and fibre panels, UPS),
/// eight desks on wall outlets, three APs, two cameras and a printer.
#[allow(clippy::too_many_arguments)]
fn floor(b: &mut Builder, v: &Vlans, name: &str, fx: f32, fy: f32, riser_m: f32, floor_no: usize, cores: [DeviceId; 2], dc_panels: [DeviceId; 2]) {
    use DeviceKind::*;
    let sw1 = b.device(SwitchL2, &format!("SW-{name}-01"), fx + 40.0, fy + 60.0);
    let sw2 = b.device(SwitchL2, &format!("SW-{name}-02"), fx + 200.0, fy + 60.0);
    let pp = b.device(PatchPanel, &format!("PP-{name}-01"), fx + 360.0, fy + 60.0);
    let fpp = b.device(PatchPanel, &format!("FPP-{name}-01"), fx + 520.0, fy + 60.0);
    let ups = b.device(Ups, &format!("UPS-{name}-01"), fx + 40.0, fy + 200.0);
    for (d, m) in [(sw1, "sw-access-48p-poe"), (sw2, "sw-access-24p-poe"), (pp, "pp-48-cat6a"), (fpp, "pp-12-lc"), (ups, "ups-1500")] {
        b.model(d, m);
    }
    let trunk_all = SwitchportMode::Trunk { native: Some(v.mgmt), allowed: AllowedVlans::All };
    let access = |vlan| SwitchportMode::Access { vlan: Some(vlan) };

    // Uplinks: switch → floor fibre panel → riser → DC panel → core.
    for (k, (sw, uplink)) in [(sw1, 48), (sw2, 24)].into_iter().enumerate() {
        let (core, dc) = (cores[k], dc_panels[k]);
        let (sp, fp) = (b.port(sw, uplink), b.port(fpp, k));
        let (dp, cp) = (b.port(dc, floor_no), b.port(core, super_index(floor_no)));
        b.cable((sw, sp), (fpp, fp), CableKind::Os2, Some(2.0));
        b.cable((fpp, fp), (dc, dp), CableKind::Os2, Some(riser_m));
        b.cable((dc, dp), (core, cp), CableKind::Os2, Some(2.0));
        b.tx(sw, sp, "sfpp-10g-lr");
        b.tx(core, cp, "sfpp-10g-lr");
        let t = trunk_all.clone();
        b.cfg(sw, sp, |c| c.mode = t.clone());
        b.cfg(core, cp, |c| c.mode = t);
        b.svi(sw, 10, v.mgmt);
    }
    // UPS management on the desk switch.
    let (sp, up) = (b.port(sw1, 46), b.port(ups, 0));
    b.cable((sw1, sp), (ups, up), CableKind::Cat6, Some(1.0));
    b.cfg(sw1, sp, |c| c.mode = access(v.mgmt));

    // Desks: wall outlet A → PC, B → phone (first four desks; the rest are spare).
    for d in 0..8 {
        let x = fx + 40.0 + d as f32 * 190.0;
        let wo = b.device(WallOutlet, &format!("WO-{name}-{:02}", d + 1), x, fy + 330.0);
        let pc = b.device(Pc, &format!("PC-{name}-{:02}", d + 1), x, fy + 420.0);
        let run = 18.0 + ((d * 7 + floor_no * 5) % 50) as f32;
        for side in 0..2 {
            let (pp_port, wo_port) = (b.port(pp, 2 * d + side), b.port(wo, side));
            b.cable((pp, pp_port), (wo, wo_port), CableKind::Cat6A, Some(run));
        }
        let (sp, pc_nic) = (b.port(sw1, 2 * d), b.port(pc, 0));
        b.cable((sw1, sp), (pp, b.port(pp, 2 * d)), CableKind::Cat6, Some(1.0));
        b.cable((wo, b.port(wo, 0)), (pc, pc_nic), CableKind::Cat6, Some(3.0));
        b.cfg(sw1, sp, |c| c.mode = access(v.staff));
        b.cfg(pc, pc_nic, |c| c.l3.dhcp_client = true);
        if d < 4 {
            let ph = b.device(IpPhone, &format!("PH-{name}-{:02}", d + 1), x, fy + 520.0);
            b.model(ph, "phone-desk");
            let (sp, lan) = (b.port(sw1, 2 * d + 1), b.port(ph, 0));
            b.cable((sw1, sp), (pp, b.port(pp, 2 * d + 1)), CableKind::Cat6, Some(1.0));
            b.cable((wo, b.port(wo, 1)), (ph, lan), CableKind::Cat6, Some(2.0));
            b.cfg(sw1, sp, |c| c.mode = access(v.voice));
            b.cfg(ph, lan, |c| c.l3.dhcp_client = true);
        }
    }

    // APs, cameras and a printer: ceiling and wall runs through the panel.
    let patch_through = |b: &mut Builder, sw_port: usize, pp_port: usize, dev: DeviceId, len: f32, mode: SwitchportMode| {
        let (sp, pr, dp) = (b.port(sw2, sw_port), b.port(pp, pp_port), b.port(dev, 0));
        b.cable((pp, pr), (dev, dp), CableKind::Cat6A, Some(len));
        b.cable((sw2, sp), (pp, pr), CableKind::Cat6, Some(1.0));
        b.cfg(sw2, sp, |c| c.mode = mode);
    };
    for (i, channel) in [36u16, 52, 100].into_iter().enumerate() {
        let ap = b.device(AccessPoint, &format!("AP-{name}-{:02}", i + 1), fx + 720.0 + i as f32 * 160.0, fy + 60.0);
        b.model(ap, "ap-wifi6-2x2");
        let radio = b.p.device(ap).unwrap().ports[1].id;
        b.cfg(ap, radio, |c| {
            if let Some(r) = &mut c.radio {
                r.channel = channel;
            }
        });
        let mode = SwitchportMode::Trunk { native: Some(v.mgmt), allowed: AllowedVlans::List(vec![v.mgmt, v.staff, v.guest]) };
        patch_through(b, i, 16 + i, ap, 25.0 + i as f32 * 9.0, mode);
    }
    for i in 0..2 {
        let cam = b.device(IpCamera, &format!("CAM-{name}-{:02}", i + 1), fx + 1200.0 + i as f32 * 160.0, fy + 60.0);
        b.model(cam, "cam-dome");
        patch_through(b, 4 + i, 19 + i, cam, 40.0 + i as f32 * 12.0, access(v.cctv));
    }
    let prn = b.device(Printer, &format!("PRN-{name}-01"), fx + 720.0, fy + 190.0);
    patch_through(b, 7, 21, prn, 22.0, access(v.printers));
}

/// Core port for floor `n`'s uplink.
fn super_index(n: usize) -> usize {
    FLOOR_UPLINK_BASE + n
}
