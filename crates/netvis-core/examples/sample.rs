//! Generate `samples/corescale-hq.netvis`, a small office network used as a
//! demo and for visual checks. Host addresses are filled in by the real
//! auto-assign code, so the sample also exercises IPAM end to end.
//!
//! cargo run -p netvis-core --example sample

use netvis_core::{
    AddrKind, AllowedVlans, Area, AreaKind, CableKind, DeviceId, DeviceKind, Edit, Endpoint, History, IfAddr,
    InterfaceConfig, LogicalInterface, LogicalKind, NatKind, NatRule, PortId, PortMedia, Pos, Project, Rect, Ssid,
    StaticRoute, Subnet, SwitchportMode, Vlan, VlanId, WifiBand, WifiSecurity,
};

struct Builder {
    p: Project,
    h: History,
}

impl Builder {
    fn device(&mut self, kind: DeviceKind, name: &str, x: f32, y: f32) -> DeviceId {
        let (id, mut edit) = self.p.add_device_edit(kind, Pos::new(x, y));
        if let Edit::Device { after: Some(d), .. } = &mut edit {
            d.name = name.to_owned();
        }
        self.h.apply(&mut self.p, edit, "add");
        id
    }

    /// Connect first free ports (optionally of a given media); returns (port on a, port on b).
    fn link(
        &mut self,
        a: DeviceId,
        ma: Option<PortMedia>,
        b: DeviceId,
        mb: Option<PortMedia>,
        cable: Option<CableKind>,
        len: Option<f32>,
    ) -> (PortId, PortId) {
        let ea = Endpoint { device: a, port: self.p.first_free_port(a, ma).expect("free port") };
        let eb = Endpoint { device: b, port: self.p.first_free_port(b, mb).expect("free port") };
        let (_, mut edit) = self.p.add_link_edit(ea, eb).expect("valid link");
        if let Edit::Link { after: Some(l), .. } = &mut edit {
            if let Some(c) = cable {
                l.cable = c;
            }
            l.length_m = len;
        }
        self.h.apply(&mut self.p, edit, "link");
        (ea.port, eb.port)
    }

    fn area(&mut self, kind: AreaKind, name: &str, rect: Rect) {
        let a = Area::new(kind, name, rect);
        self.h.apply(&mut self.p, Edit::Area { id: a.id, before: None, after: Some(a) }, "area");
    }

    fn vlan(&mut self, tag: u16, name: &str, purpose: &str) -> VlanId {
        let mut v = Vlan::new(tag, name);
        v.purpose = purpose.into();
        let id = v.id;
        self.h.apply(&mut self.p, Edit::Vlan { id, before: None, after: Some(v) }, "vlan");
        id
    }

    fn subnet(&mut self, name: &str, net: &str, vlan: Option<VlanId>, dhcp: bool) {
        let mut s = Subnet::new(name, net.parse().unwrap()).with_defaults(vlan.is_some(), dhcp);
        s.vlan = vlan;
        self.h.apply(&mut self.p, Edit::Subnet { id: s.id, before: None, after: Some(s) }, "subnet");
    }

    fn cfg(&mut self, d: DeviceId, port: PortId, f: impl FnOnce(&mut InterfaceConfig)) {
        let dev = self.p.device_mut(d).unwrap();
        f(&mut dev.ports.iter_mut().find(|p| p.id == port).unwrap().cfg);
    }

    fn svi(&mut self, d: DeviceId, tag: u16, vlan: VlanId) {
        let l = LogicalInterface::new(format!("Vlan{tag}"), LogicalKind::Svi { vlan: Some(vlan) });
        self.p.device_mut(d).unwrap().logical.push(l);
    }
}

fn addr(s: &str) -> IfAddr {
    IfAddr { net: s.parse().unwrap(), kind: AddrKind::Static }
}

fn main() {
    let mut b = Builder { p: Project::default(), h: History::default() };
    b.p.meta.name = "Corescale HQ".into();
    b.p.meta.client = "Corescale".into();
    b.p.meta.description = "Head-office LAN: dual-floor access layer, DMZ, and server rack.".into();
    b.p.services.domain = "hq.corescale.local".into();
    b.p.services.dns_servers = vec!["1.1.1.1".parse().unwrap(), "9.9.9.9".parse().unwrap()];
    b.p.services.ntp_servers = vec!["pool.ntp.org".into()];

    use DeviceKind::*;
    use PortMedia::*;

    b.area(AreaKind::Site, "HQ — Singapore", Rect::new(0.0, 0.0, 1500.0, 900.0));
    b.area(AreaKind::Rack, "MDF Rack A", Rect::new(500.0, 160.0, 400.0, 300.0));
    b.area(AreaKind::SecurityZone, "DMZ", Rect::new(1020.0, 160.0, 400.0, 220.0));
    b.area(AreaKind::Floor, "Level 1", Rect::new(40.0, 520.0, 680.0, 340.0));
    b.area(AreaKind::Floor, "Level 2", Rect::new(780.0, 520.0, 680.0, 340.0));

    // ---- VLANs and subnets ----
    let mgmt = b.vlan(10, "Management", "Network device and server management");
    let staff = b.vlan(20, "Staff", "Wired and wireless staff clients");
    let voice = b.vlan(30, "Voice", "IP phones");
    let cctv = b.vlan(40, "CCTV", "IP cameras and NVR");
    let guest = b.vlan(50, "Guest", "Internet-only guest Wi-Fi");
    b.subnet("HQ block", "10.10.0.0/16", None, false);
    b.subnet("Core transit", "10.10.0.0/30", None, false);
    b.subnet("Management", "10.10.10.0/24", Some(mgmt), false);
    b.subnet("Staff", "10.10.20.0/24", Some(staff), true);
    b.subnet("Voice", "10.10.30.0/24", Some(voice), true);
    b.subnet("CCTV", "10.10.40.0/24", Some(cctv), false);
    b.subnet("Guest", "10.10.50.0/23", Some(guest), true);
    b.subnet("DMZ", "172.16.99.0/28", None, false);

    // ---- devices ----
    let inet = b.device(Internet, "Internet", 620.0, -180.0);
    let fw = b.device(Firewall, "FW-01", 580.0, 40.0);
    let core = b.device(SwitchL3, "CSW-01", 540.0, 220.0);
    let srv = b.device(Server, "SRV-FILE-01", 560.0, 360.0);
    let web = b.device(Server, "SRV-WEB-01", 1080.0, 220.0);
    let sw1 = b.device(SwitchL2, "SW-L1-01", 260.0, 580.0);
    let sw2 = b.device(SwitchL2, "SW-L2-01", 1000.0, 580.0);

    let (_, fw_wan) = b.link(inet, None, fw, Some(Rj45), Some(CableKind::Virtual), None);
    let (fw_in, core_up) = b.link(fw, Some(SfpPlus), core, Some(SfpPlus), Some(CableKind::Dac), Some(1.0));
    let (fw_dmz, web_nic) = b.link(fw, Some(SfpPlus), web, Some(SfpPlus), Some(CableKind::Om4), Some(15.0));
    let (core_srv, _) = b.link(core, Some(SfpPlus), srv, Some(SfpPlus), Some(CableKind::Dac), Some(2.0));
    let (core_l1, sw1_up) = b.link(core, Some(SfpPlus), sw1, Some(SfpPlus), Some(CableKind::Om4), Some(45.0));
    let (core_l2, sw2_up) = b.link(core, Some(SfpPlus), sw2, Some(SfpPlus), Some(CableKind::Os2), Some(120.0));

    // Firewall: WAN, routed transit to the core, DMZ gateway, default route and PAT.
    b.cfg(fw, fw_wan, |c| {
        c.mode = SwitchportMode::Routed;
        c.l3.addresses = vec![addr("203.0.113.10/29")];
        c.l3.gateway = Some("203.0.113.9".parse().unwrap());
    });
    b.cfg(fw, fw_in, |c| {
        c.mode = SwitchportMode::Routed;
        c.l3.addresses = vec![addr("10.10.0.1/30")];
    });
    b.cfg(fw, fw_dmz, |c| {
        c.mode = SwitchportMode::Routed;
        c.l3.addresses = vec![addr("172.16.99.1/28")];
    });
    {
        let r = &mut b.p.device_mut(fw).unwrap().routing;
        r.static_routes.push(StaticRoute { dest: "10.10.0.0/16".parse().unwrap(), next_hop: Some("10.10.0.2".parse().unwrap()), description: "Internal networks via core".into(), ..Default::default() });
        r.nat.push(NatRule { kind: NatKind::SourcePat, inside: "10.10.0.0/16".into(), outside: "port1 (203.0.113.10)".into(), service: "any".into(), description: "Internet access".into() });
        r.nat.push(NatRule { kind: NatKind::PortForward, inside: "172.16.99.10".into(), outside: "203.0.113.11".into(), service: "tcp/443".into(), description: "Public website".into() });
    }
    b.cfg(web, web_nic, |c| {
        c.l3.addresses = vec![addr("172.16.99.10/28")];
        c.l3.gateway = Some("172.16.99.1".parse().unwrap());
    });

    // Core: routed uplink, SVI gateways, trunks to access switches, server on management.
    b.cfg(core, core_up, |c| {
        c.mode = SwitchportMode::Routed;
        c.l3.addresses = vec![addr("10.10.0.2/30")];
    });
    for (tag, v) in [(10, mgmt), (20, staff), (30, voice), (40, cctv), (50, guest)] {
        b.svi(core, tag, v);
    }
    b.p.device_mut(core).unwrap().routing.static_routes.push(StaticRoute {
        next_hop: Some("10.10.0.1".parse().unwrap()),
        description: "Default via firewall".into(),
        ..Default::default()
    });
    b.cfg(core, core_srv, |c| c.mode = SwitchportMode::Access { vlan: Some(mgmt) });
    for (up, sw, sw_up) in [(core_l1, sw1, sw1_up), (core_l2, sw2, sw2_up)] {
        let trunk = SwitchportMode::Trunk { native: Some(mgmt), allowed: AllowedVlans::All };
        b.cfg(core, up, |c| c.mode = trunk.clone());
        b.cfg(sw, sw_up, |c| c.mode = trunk);
        b.svi(sw, 10, mgmt);
    }

    // Access layer: AP on a trunk (management native, SSIDs tagged), PC, phone (DHCP), camera.
    for (sw, floor, x0, channel) in [(sw1, "L1", 80.0, 36), (sw2, "L2", 820.0, 44)] {
        let ap = b.device(AccessPoint, &format!("AP-{floor}-01"), x0, 740.0);
        let pc = b.device(Pc, &format!("PC-{floor}-01"), x0 + 160.0, 740.0);
        let ph = b.device(IpPhone, &format!("PH-{floor}-01"), x0 + 320.0, 740.0);
        let cam = b.device(IpCamera, &format!("CAM-{floor}-01"), x0 + 480.0, 740.0);
        let (sp, _) = b.link(sw, Some(Rj45), ap, Some(Rj45), Some(CableKind::Cat6A), Some(32.0));
        b.cfg(sw, sp, |c| c.mode = SwitchportMode::Trunk { native: Some(mgmt), allowed: AllowedVlans::List(vec![mgmt, staff, guest]) });
        let radio = b.p.device(ap).unwrap().ports[1].id;
        b.cfg(ap, radio, |c| {
            if let Some(r) = &mut c.radio {
                r.channel = channel;
            }
        });
        let (sp, _) = b.link(sw, Some(Rj45), pc, Some(Rj45), None, Some(18.0));
        b.cfg(sw, sp, |c| c.mode = SwitchportMode::Access { vlan: Some(staff) });
        let (sp, php) = b.link(sw, Some(Rj45), ph, Some(Rj45), None, None);
        b.cfg(sw, sp, |c| c.mode = SwitchportMode::Access { vlan: Some(voice) });
        b.cfg(ph, php, |c| c.l3.dhcp_client = true);
        let (sp, _) = b.link(sw, Some(Rj45), cam, Some(Rj45), None, Some(60.0));
        b.cfg(sw, sp, |c| c.mode = SwitchportMode::Access { vlan: Some(cctv) });
    }

    // Wireless networks.
    let mut corp = Ssid::new("Corescale");
    corp.vlan = Some(staff);
    corp.security = WifiSecurity::Wpa3Enterprise;
    corp.bands = vec![WifiBand::G5, WifiBand::G6];
    let mut guest_ssid = Ssid::new("Corescale-Guest");
    guest_ssid.vlan = Some(guest);
    guest_ssid.security = WifiSecurity::Owe;
    for s in [corp, guest_ssid] {
        b.h.apply(&mut b.p, Edit::Ssid { id: s.id, before: None, after: Some(s) }, "ssid");
    }

    let run = b.p.auto_assign_edit(None);
    for line in &run.assigned {
        println!("  assigned {line}");
    }
    for line in &run.skipped {
        println!("  skipped  {line}");
    }
    b.h.apply(&mut b.p, run.edit, "auto-assign");

    let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../samples/corescale-hq.netvis");
    std::fs::create_dir_all(out.parent().unwrap()).unwrap();
    netvis_core::io::save(&b.p, &out).unwrap();
    println!("wrote {}", out.display());
}
