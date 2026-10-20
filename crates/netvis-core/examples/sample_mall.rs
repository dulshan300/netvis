//! Generate `samples/shopping-mall.netvis`: the finished project of the
//! shopping-mall tutorial (docs/tutorial-shopping-mall.md).
//!
//! - Nine floors: BF, GF, L1–L4, L4A, L5, L6. Every floor has an ELV room
//!   with a 24U rack: a security switch (IP CCTV and door access), a
//!   services switch (Wi-Fi and background music), a 48-port copper panel,
//!   a 12-port LC fibre panel and a UPS.
//! - Field devices are cabled through the floor panel: 8–18 cameras, 3–15
//!   APs and 2 door controllers per floor; BF to L3 add IP ceiling speakers
//!   for background music.
//! - GF also holds the server room: firewall, a core pair in a LAG, the
//!   video-management and recording servers, access-control and BGM
//!   servers, and the Wi-Fi controller.
//! - Every ELV room reaches the server room over a single-mode backbone
//!   through the fibre panels. L4A is stacked with L4: its switches join the
//!   L4 switches over fibre and share their uplinks.
//!
//! cargo run -p netvis-core --example sample_mall

mod common;

use common::{Builder, addr};
use netvis_core::{
    AllowedVlans, AreaKind, CableKind, DeviceId, DeviceKind, Edit, NatKind, NatRule, Rect, Ssid, StaticRoute,
    SwitchportMode, VlanId, WifiBand, WifiSecurity,
};

struct Vlans {
    mgmt: VlanId,
    cctv: VlanId,
    access: VlanId,
    bgm: VlanId,
    ops: VlanId,
    tenant: VlanId,
    guest: VlanId,
}

/// One floor, from the brief.
struct Floor {
    code: &'static str,
    title: &'static str,
    cameras: usize,
    aps: usize,
    speakers: usize,
}

const FLOORS: [Floor; 9] = [
    Floor { code: "L6", title: "Level 6 · Food court & roof", cameras: 10, aps: 6, speakers: 0 },
    Floor { code: "L5", title: "Level 5 · Cinema", cameras: 10, aps: 8, speakers: 0 },
    Floor { code: "L4A", title: "Level 4A · Mezzanine", cameras: 8, aps: 3, speakers: 0 },
    Floor { code: "L4", title: "Level 4 · Retail", cameras: 12, aps: 10, speakers: 0 },
    Floor { code: "L3", title: "Level 3 · Retail", cameras: 14, aps: 12, speakers: 6 },
    Floor { code: "L2", title: "Level 2 · Retail", cameras: 14, aps: 12, speakers: 6 },
    Floor { code: "L1", title: "Level 1 · Retail", cameras: 16, aps: 14, speakers: 8 },
    Floor { code: "GF", title: "Ground floor · Atrium & server room", cameras: 18, aps: 15, speakers: 8 },
    Floor { code: "BF", title: "Basement · Car park & supermarket", cameras: 18, aps: 6, speakers: 6 },
];

/// Floor index counted from BF (0) up, for riser lengths.
fn storey(code: &str) -> usize {
    ["BF", "GF", "L1", "L2", "L3", "L4", "L4A", "L5", "L6"].iter().position(|c| *c == code).unwrap()
}

/// Channels for a high-density 5 GHz plan at 20 MHz.
const CHANNELS: [u16; 19] = [36, 40, 44, 48, 52, 56, 60, 64, 100, 104, 108, 112, 116, 120, 124, 128, 132, 136, 140];

const PER_ROW: usize = 13;
const PITCH: f32 = 160.0;
const ROW: f32 = 100.0;
const FLOOR_W: f32 = 2840.0;
const SERVER_ROOM_H: f32 = 400.0;

fn rows(f: &Floor) -> usize {
    f.cameras.div_ceil(PER_ROW) + f.aps.div_ceil(PER_ROW) + 1
}

fn floor_h(f: &Floor) -> f32 {
    let h = (50.0 + rows(f) as f32 * ROW).max(340.0);
    if f.code == "GF" { h + SERVER_ROOM_H } else { h }
}

struct Core {
    csw: [DeviceId; 2],
    fpp: [DeviceId; 2],
}

fn main() {
    use DeviceKind::*;
    let mut b = Builder::default();
    b.p.meta.name = "Shopping Mall".into();
    b.p.meta.client = "Mall owner".into();
    b.p.meta.author = "Network Engineering".into();
    b.p.meta.revision = "A".into();
    b.p.meta.description = "Nine-storey shopping mall: IP CCTV, door access, Wi-Fi and background music on every floor, \
        an ELV room per floor and a single-mode backbone to the ground-floor server room."
        .into();
    b.p.services.domain = "mall.local".into();
    b.p.services.dns_servers = vec!["1.1.1.1".parse().unwrap(), "8.8.8.8".parse().unwrap()];
    b.p.services.ntp_servers = vec!["pool.ntp.org".into()];

    let v = Vlans {
        mgmt: b.vlan(10, "Management", "Switches, APs, UPS"),
        cctv: b.vlan(20, "CCTV", "IP cameras, video management and recording"),
        access: b.vlan(30, "Access control", "Door controllers and the access-control server"),
        bgm: b.vlan(40, "BGM", "Background-music speakers and server"),
        ops: b.vlan(50, "Wi-Fi Ops", "Mall operations staff"),
        tenant: b.vlan(60, "Wi-Fi Tenant", "Shop tenants"),
        guest: b.vlan(70, "Wi-Fi Guest", "Shoppers, internet only"),
    };
    b.subnet("Management", "10.50.10.0/24", Some(v.mgmt), false);
    b.subnet("CCTV", "10.50.20.0/23", Some(v.cctv), false);
    b.subnet("Access control", "10.50.30.0/24", Some(v.access), false);
    b.subnet("BGM", "10.50.40.0/24", Some(v.bgm), false);
    b.subnet("Wi-Fi Ops", "10.50.64.0/22", Some(v.ops), true);
    b.subnet("Wi-Fi Tenant", "10.50.80.0/21", Some(v.tenant), true);
    b.subnet("Wi-Fi Guest", "10.60.0.0/20", Some(v.guest), true);
    b.subnet("Transit FW-core", "10.50.255.0/30", None, false);
    b.subnet("ISP handoff", "203.0.113.16/29", None, false);

    // ---- areas, top floor first ----
    let total_h: f32 = FLOORS.iter().map(floor_h).sum::<f32>() + 30.0 * FLOORS.len() as f32 + 40.0;
    b.area(AreaKind::Building, "Shopping Mall", Rect::new(0.0, 0.0, FLOOR_W + 40.0, total_h));
    let mut y = 40.0;
    let mut tops = Vec::new();
    for f in &FLOORS {
        b.area(AreaKind::Floor, &format!("{} · {}", f.code, f.title), Rect::new(20.0, y, FLOOR_W, floor_h(f)));
        tops.push(y);
        y += floor_h(f) + 30.0;
    }

    // ---- server room (GF) ----
    let gf = FLOORS.iter().position(|f| f.code == "GF").unwrap();
    let (sx, sy) = (40.0, tops[gf] + floor_h(&FLOORS[gf]) - SERVER_ROOM_H);
    b.area(AreaKind::SecurityZone, "GF Server Room", Rect::new(sx, sy, 1640.0, SERVER_ROOM_H - 20.0));
    let rack1 = b.area_id(AreaKind::Rack, "SR-01 Network", Rect::new(sx + 20.0, sy + 30.0, 680.0, 330.0));
    let rack2 = b.area_id(AreaKind::Rack, "SR-02 Servers", Rect::new(sx + 740.0, sy + 30.0, 860.0, 330.0));
    let (r1, r2) = ((sx + 40.0, sy + 70.0), (sx + 760.0, sy + 70.0));

    let isp = b.device(Isp, "ISP-01", -300.0, sy + 70.0);
    let fw = b.device(Firewall, "FW-01", r1.0, r1.1);
    let csw1 = b.device(SwitchL3, "CSW-01", r1.0 + 160.0, r1.1);
    let csw2 = b.device(SwitchL3, "CSW-02", r1.0 + 320.0, r1.1);
    let sw_sr = b.device(SwitchL2, "SW-SR-01", r1.0 + 480.0, r1.1);
    let fpp_sr1 = b.device(PatchPanel, "FPP-SR-01", r1.0, r1.1 + 150.0);
    let fpp_sr2 = b.device(PatchPanel, "FPP-SR-02", r1.0 + 160.0, r1.1 + 150.0);
    let ups_sr1 = b.device(Ups, "UPS-SR-01", r1.0 + 320.0, r1.1 + 150.0);
    for (d, m) in [(fw, "fw-campus"), (csw1, "sw-dist-24x10g"), (csw2, "sw-dist-24x10g"), (sw_sr, "sw-access-48p"), (fpp_sr1, "pp-24-lc"), (fpp_sr2, "pp-24-lc"), (ups_sr1, "ups-3000")] {
        b.model(d, m);
    }
    let vms: Vec<DeviceId> = (0..2).map(|i| b.device(Server, &format!("VMS-{:02}", i + 1), r2.0 + i as f32 * 160.0, r2.1)).collect();
    let nvr: Vec<DeviceId> = (0..2).map(|i| b.device(Storage, &format!("NAS-CCTV-{:02}", i + 1), r2.0 + 320.0 + i as f32 * 160.0, r2.1)).collect();
    let acs_srv = b.device(Server, "ACS-SRV-01", r2.0, r2.1 + 150.0);
    let bgm_srv = b.device(Server, "BGM-SRV-01", r2.0 + 160.0, r2.1 + 150.0);
    let wlc = b.device(WirelessController, "WLC-01", r2.0 + 320.0, r2.1 + 150.0);
    let ups_sr2 = b.device(Ups, "UPS-SR-02", r2.0 + 480.0, r2.1 + 150.0);
    let ups_sr3 = b.device(Ups, "UPS-SR-03", r2.0 + 640.0, r2.1 + 150.0);
    for d in &vms {
        b.model(*d, "srv-2u");
        // 25G NICs run at 10G towards the 10G core.
        for port in &mut b.p.device_mut(*d).unwrap().ports {
            if port.media == netvis_core::PortMedia::Sfp28 {
                port.speed_mbps = 10_000;
            }
        }
    }
    for d in &nvr {
        b.model(*d, "nas-2u-12bay");
    }
    for (d, m) in [(acs_srv, "srv-1u"), (bgm_srv, "srv-1u"), (ups_sr2, "ups-3000"), (ups_sr3, "ups-3000")] {
        b.model(d, m);
    }
    b.p.device_mut(vms[0]).unwrap().notes = "Video management system, primary recorder".into();
    b.p.device_mut(vms[1]).unwrap().notes = "Video management system, failover recorder".into();

    let trunk_all = SwitchportMode::Trunk { native: Some(v.mgmt), allowed: AllowedVlans::All };
    let access = |vlan| SwitchportMode::Access { vlan: Some(vlan) };

    // WAN and transit.
    let fw_wan = b.port(fw, 0);
    b.cable((isp, b.port(isp, 0)), (fw, fw_wan), CableKind::Virtual, None);
    b.cfg(fw, fw_wan, |c| {
        c.mode = SwitchportMode::Routed;
        c.l3.addresses = vec![addr("203.0.113.18/29")];
        c.l3.gateway = Some("203.0.113.17".parse().unwrap());
    });
    let (fw_in, core_up) = (b.port(fw, 16), b.port(csw1, 0));
    b.cable((fw, fw_in), (csw1, core_up), CableKind::Dac, Some(1.0));
    b.cfg(fw, fw_in, |c| {
        c.mode = SwitchportMode::Routed;
        c.l3.addresses = vec![addr("10.50.255.1/30")];
    });
    b.cfg(csw1, core_up, |c| {
        c.mode = SwitchportMode::Routed;
        c.l3.addresses = vec![addr("10.50.255.2/30")];
    });
    {
        let r = &mut b.p.device_mut(fw).unwrap().routing;
        r.static_routes.push(StaticRoute { dest: "10.50.0.0/16".parse().unwrap(), next_hop: Some("10.50.255.2".parse().unwrap()), description: "Mall networks via core".into(), ..Default::default() });
        r.static_routes.push(StaticRoute { dest: "10.60.0.0/20".parse().unwrap(), next_hop: Some("10.50.255.2".parse().unwrap()), description: "Guest Wi-Fi via core".into(), ..Default::default() });
        r.static_routes.push(StaticRoute { next_hop: Some("203.0.113.17".parse().unwrap()), description: "Default via ISP".into(), ..Default::default() });
        r.nat.push(NatRule { kind: NatKind::SourcePat, inside: "10.50.0.0/16, 10.60.0.0/20".into(), outside: "port1".into(), service: "any".into(), description: "Internet access".into() });
    }
    b.p.device_mut(csw1).unwrap().routing.static_routes.push(StaticRoute {
        next_hop: Some("10.50.255.1".parse().unwrap()),
        description: "Default via firewall".into(),
        ..Default::default()
    });

    // Core pair: two 40G DACs in LAG 1. CSW-01 routes every VLAN.
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
    for (tag, vl) in [(10, v.mgmt), (20, v.cctv), (30, v.access), (40, v.bgm), (50, v.ops), (60, v.tenant), (70, v.guest)] {
        b.svi(csw1, tag, vl);
    }
    b.svi(csw2, 10, v.mgmt);

    // Recording: VMS servers and storage on 10G DACs to CSW-01, CCTV VLAN.
    for (i, d) in vms.iter().enumerate() {
        let (cp, sp) = (b.port(csw1, 9 + i), b.port(*d, 5));
        b.cable((csw1, cp), (*d, sp), CableKind::Dac, Some(2.0));
        b.cfg(csw1, cp, |c| c.mode = access(v.cctv));
    }
    for (i, d) in nvr.iter().enumerate() {
        let (cp, sp) = (b.port(csw1, 11 + i), b.port(*d, 4));
        b.cable((csw1, cp), (*d, sp), CableKind::Dac, Some(2.0));
        b.cfg(csw1, cp, |c| c.mode = access(v.cctv));
    }
    // Server switch: the smaller servers, the controller and UPS management.
    let (cp, sp) = (b.port(csw1, 13), b.port(sw_sr, 48));
    b.cable((csw1, cp), (sw_sr, sp), CableKind::Dac, Some(1.0));
    let t = trunk_all.clone();
    b.cfg(csw1, cp, |c| c.mode = t.clone());
    b.cfg(sw_sr, sp, |c| c.mode = t);
    b.svi(sw_sr, 10, v.mgmt);
    for (i, (d, vlan)) in [(acs_srv, v.access), (bgm_srv, v.bgm), (wlc, v.mgmt), (ups_sr1, v.mgmt), (ups_sr2, v.mgmt), (ups_sr3, v.mgmt)].into_iter().enumerate() {
        let (sp, dp) = (b.port(sw_sr, i), b.port(d, 0));
        b.cable((sw_sr, sp), (d, dp), CableKind::Cat6A, Some(2.0));
        b.cfg(sw_sr, sp, |c| c.mode = access(vlan));
    }

    // ---- floors ----
    let core = Core { csw: [csw1, csw2], fpp: [fpp_sr1, fpp_sr2] };
    let mut racks = vec![rack1, rack2];
    let mut l4: Option<[DeviceId; 3]> = None;
    // Bottom up, so L4 exists before L4A joins its stack.
    let mut uplinked = 0;
    for (i, f) in FLOORS.iter().enumerate().rev() {
        let rack = b.area_id(AreaKind::Rack, &format!("ELV {}", f.code), Rect::new(40.0, tops[i] + 30.0, 660.0, 290.0));
        if let Some(a) = b.p.area_mut(rack) {
            a.rack_units = 24;
        }
        racks.push(rack);
        let stack_with = if f.code == "L4A" { l4 } else { None };
        let sw = floor(&mut b, &v, f, tops[i], &core, uplinked, stack_with);
        if stack_with.is_none() {
            uplinked += 1;
        }
        if f.code == "L4" {
            l4 = Some(sw);
        }
    }

    // Wireless networks.
    let mut ops = Ssid::new("Mall-Ops");
    ops.vlan = Some(v.ops);
    ops.security = WifiSecurity::Wpa3Enterprise;
    ops.bands = vec![WifiBand::G5];
    let mut tenant = Ssid::new("Mall-Tenant");
    tenant.vlan = Some(v.tenant);
    tenant.security = WifiSecurity::Wpa2Wpa3Transition;
    tenant.bands = vec![WifiBand::G5];
    let mut guest = Ssid::new("Mall-Free-WiFi");
    guest.vlan = Some(v.guest);
    guest.security = WifiSecurity::Owe;
    guest.bands = vec![WifiBand::G5];
    for s in [ops, tenant, guest] {
        b.h.apply(&mut b.p, Edit::Ssid { id: s.id, before: None, after: Some(s) }, "ssid");
    }

    for d in b.p.devices.iter_mut().filter(|d| matches!(d.kind, SwitchL2 | SwitchL3)) {
        d.stp = netvis_core::StpMode::Rstp;
    }
    for rack in racks {
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

    println!("  {} devices, {} cables, {} runs, {} labelled", b.p.devices.len(), b.p.links.len(), b.p.circuits().len(), n);
    for i in netvis_validate::validate(&b.p) {
        println!("  {:<7} {:<18} {}", i.severity.label(), i.code, i.message);
    }
    let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../samples/shopping-mall.netvis");
    netvis_core::io::save(&b.p, &out).unwrap();
    println!("wrote {}", out.display());
}

/// One floor's ELV room and field devices. `n` is the floor's backbone
/// slot on the server-room fibre panels and core. With `stack_with`, the
/// floor's switches join that floor's stack instead of uplinking.
/// Returns the security switch, services switch and fibre panel.
fn floor(b: &mut Builder, v: &Vlans, f: &Floor, fy: f32, core: &Core, n: usize, stack_with: Option<[DeviceId; 3]>) -> [DeviceId; 3] {
    use DeviceKind::*;
    let c = f.code;
    let (rx, ry) = (60.0, fy + 70.0);
    let fpp = b.device(PatchPanel, &format!("FPP-{c}-01"), rx, ry);
    let sec = b.device(SwitchL2, &format!("SW-{c}-SEC"), rx + 160.0, ry);
    let svc = b.device(SwitchL2, &format!("SW-{c}-SVC"), rx + 320.0, ry);
    let pp = b.device(PatchPanel, &format!("PP-{c}-01"), rx, ry + 140.0);
    let ups = b.device(Ups, &format!("UPS-{c}-01"), rx + 320.0, ry + 140.0);
    // PoE: an AP is allocated 30 W, cameras, doors and speakers 15.4 W.
    let svc_poe = f.aps as f32 * 30.0 + f.speakers as f32 * 15.4;
    let svc_model = if svc_poe > 330.0 { "sw-access-48p-poe" } else { "sw-access-24p-poe" };
    for (d, m) in [(fpp, "pp-12-lc"), (sec, "sw-access-24p-poe"), (svc, svc_model), (pp, "pp-48-cat6a"), (ups, "ups-1500")] {
        b.model(d, m);
    }
    let sec_up = b.p.device(sec).unwrap().ports.iter().position(|p| p.media == netvis_core::PortMedia::SfpPlus).unwrap();
    let svc_up = b.p.device(svc).unwrap().ports.iter().position(|p| p.media == netvis_core::PortMedia::SfpPlus).unwrap();
    let trunk_all = SwitchportMode::Trunk { native: Some(v.mgmt), allowed: AllowedVlans::All };
    let access = |vlan| SwitchportMode::Access { vlan: Some(vlan) };
    let riser = 30.0 + storey(c).abs_diff(storey("GF")) as f32 * 6.0;

    for (k, (sw, up)) in [(sec, sec_up), (svc, svc_up)].into_iter().enumerate() {
        let (sp, fp) = (b.port(sw, up), b.port(fpp, k));
        b.cable((sw, sp), (fpp, fp), CableKind::Os2, Some(2.0));
        b.tx(sw, sp, "sfpp-10g-lr");
        let t = trunk_all.clone();
        b.cfg(sw, sp, |cfg| cfg.mode = t);
        b.svi(sw, 10, v.mgmt);
        match stack_with {
            // Stack member: fibre to the L4 switch of the same system.
            Some([l4_sec, l4_svc, l4_fpp]) => {
                let peer = if k == 0 { l4_sec } else { l4_svc };
                let peer_up = b.p.device(peer).unwrap().ports.iter().position(|p| p.media == netvis_core::PortMedia::SfpPlus).unwrap() + 1;
                let (pp_port, peer_port) = (b.port(l4_fpp, 2 + k), b.port(peer, peer_up));
                b.cable((fpp, fp), (l4_fpp, pp_port), CableKind::Os2, Some(8.0));
                b.cable((l4_fpp, pp_port), (peer, peer_port), CableKind::Os2, Some(2.0));
                b.tx(peer, peer_port, "sfpp-10g-lr");
                let t = trunk_all.clone();
                b.cfg(peer, peer_port, |cfg| cfg.mode = t);
                let name = b.p.device(peer).unwrap().name.clone();
                b.p.device_mut(sw).unwrap().notes = format!("Stack member 2 with {name}; uplinks through the L4 stack");
                let me = b.p.device(sw).unwrap().name.clone();
                b.p.device_mut(peer).unwrap().notes = format!("Stack member 1; {me} on L4A is member 2");
            }
            // Backbone: floor panel → riser → server-room panel → core.
            None => {
                let (dp, cp) = (b.port(core.fpp[k], n), b.port(core.csw[k], 1 + n));
                b.cable((fpp, fp), (core.fpp[k], dp), CableKind::Os2, Some(riser));
                b.cable((core.fpp[k], dp), (core.csw[k], cp), CableKind::Os2, Some(2.0));
                b.tx(core.csw[k], cp, "sfpp-10g-lr");
                let t = trunk_all.clone();
                b.cfg(core.csw[k], cp, |cfg| cfg.mode = t);
            }
        }
    }
    // UPS management on the services switch's last copper port.
    let last = b.p.device(svc).unwrap().ports.iter().filter(|p| p.media == netvis_core::PortMedia::Rj45).count() - 1;
    let (sp, up) = (b.port(svc, last), b.port(ups, 0));
    b.cable((svc, sp), (ups, up), CableKind::Cat6, Some(1.0));
    b.cfg(svc, sp, |cfg| cfg.mode = access(v.mgmt));

    // Field devices, each through the copper panel.
    let mut pp_port = 0;
    let mut sw_port = [0usize; 2];
    let mut place = |b: &mut Builder, kind: DeviceKind, name: String, model: &str, row: usize, col: usize, sys: usize, mode: SwitchportMode| {
        let x = 760.0 + col as f32 * PITCH;
        let d = b.device(kind, &name, x, fy + 50.0 + row as f32 * ROW);
        b.model(d, model);
        let sw = if sys == 0 { sec } else { svc };
        let len = 15.0 + ((pp_port * 13 + storey(c) * 7) % 70) as f32;
        let (s, pr, dp) = (b.port(sw, sw_port[sys]), b.port(pp, pp_port), b.port(d, 0));
        b.cable((sw, s), (pp, pr), CableKind::Cat6, Some(1.0));
        b.cable((pp, pr), (d, dp), CableKind::Cat6A, Some(len));
        b.cfg(sw, s, |cfg| cfg.mode = mode);
        pp_port += 1;
        sw_port[sys] += 1;
        d
    };
    let mut row = 0;
    for i in 0..f.cameras {
        let model = if i % 3 == 2 { "cam-bullet" } else { "cam-dome" };
        place(b, IpCamera, format!("CAM-{c}-{:02}", i + 1), model, row + i / PER_ROW, i % PER_ROW, 0, access(v.cctv));
    }
    row += f.cameras.div_ceil(PER_ROW);
    let ap_mode = SwitchportMode::Trunk { native: Some(v.mgmt), allowed: AllowedVlans::List(vec![v.mgmt, v.ops, v.tenant, v.guest]) };
    for i in 0..f.aps {
        let ap = place(b, AccessPoint, format!("AP-{c}-{:02}", i + 1), "ap-wifi6-2x2", row + i / PER_ROW, i % PER_ROW, 1, ap_mode.clone());
        let radio = b.p.device(ap).unwrap().ports[1].id;
        b.cfg(ap, radio, |cfg| {
            if let Some(r) = &mut cfg.radio {
                r.channel = CHANNELS[i % CHANNELS.len()];
                r.width_mhz = 20;
            }
        });
    }
    row += f.aps.div_ceil(PER_ROW);
    for i in 0..2 {
        let d = place(b, IoT, format!("ACS-{c}-{:02}", i + 1), "acs-door-controller", row, i, 0, access(v.access));
        b.p.device_mut(d).unwrap().notes = if i == 0 { "Service corridor doors".into() } else { "ELV and plant room doors".into() };
    }
    for i in 0..f.speakers {
        place(b, IoT, format!("SPK-{c}-{:02}", i + 1), "ip-speaker-ceiling", row, 2 + i, 1, access(v.bgm));
    }
    [sec, svc, fpp]
}
