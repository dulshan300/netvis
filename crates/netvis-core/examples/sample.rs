//! Generate `samples/corescale-hq.netvis`, a small office network used as a
//! demo and for visual checks.
//!
//! cargo run -p netvis-core --example sample

use netvis_core::{
    Area, AreaKind, CableKind, DeviceId, DeviceKind, Edit, Endpoint, History, PortMedia, Pos, Project, Rect,
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

    fn link(&mut self, a: DeviceId, ma: Option<PortMedia>, b: DeviceId, mb: Option<PortMedia>, cable: Option<CableKind>, len: Option<f32>) {
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
    }

    fn area(&mut self, kind: AreaKind, name: &str, rect: Rect) {
        let a = Area::new(kind, name, rect);
        self.h.apply(&mut self.p, Edit::Area { id: a.id, before: None, after: Some(a) }, "area");
    }
}

fn main() {
    let mut b = Builder { p: Project::default(), h: History::default() };
    b.p.meta.name = "Corescale HQ".into();
    b.p.meta.client = "Corescale".into();
    b.p.meta.description = "Head-office LAN: dual-floor access layer, DMZ, and server rack.".into();

    use DeviceKind::*;
    use PortMedia::*;

    b.area(AreaKind::Site, "HQ — Singapore", Rect::new(0.0, 0.0, 1500.0, 900.0));
    b.area(AreaKind::Rack, "MDF Rack A", Rect::new(500.0, 160.0, 400.0, 300.0));
    b.area(AreaKind::SecurityZone, "DMZ", Rect::new(1020.0, 160.0, 400.0, 220.0));
    b.area(AreaKind::Floor, "Level 1", Rect::new(40.0, 520.0, 680.0, 340.0));
    b.area(AreaKind::Floor, "Level 2", Rect::new(780.0, 520.0, 680.0, 340.0));

    let inet = b.device(Internet, "Internet", 620.0, -180.0);
    let fw = b.device(Firewall, "FW-01", 580.0, 40.0);
    let core = b.device(SwitchL3, "CSW-01", 540.0, 220.0);
    let srv = b.device(Server, "SRV-FILE-01", 560.0, 360.0);
    let web = b.device(Server, "SRV-WEB-01", 1080.0, 220.0);
    let sw1 = b.device(SwitchL2, "SW-L1-01", 260.0, 580.0);
    let sw2 = b.device(SwitchL2, "SW-L2-01", 1000.0, 580.0);

    b.link(inet, None, fw, Some(Rj45), Some(CableKind::Virtual), None);
    b.link(fw, Some(SfpPlus), core, Some(SfpPlus), Some(CableKind::Dac), Some(1.0));
    b.link(fw, Some(SfpPlus), web, Some(SfpPlus), Some(CableKind::Om4), Some(15.0));
    b.link(core, Some(SfpPlus), srv, Some(SfpPlus), Some(CableKind::Dac), Some(2.0));
    b.link(core, Some(SfpPlus), sw1, Some(SfpPlus), Some(CableKind::Om4), Some(45.0));
    b.link(core, Some(SfpPlus), sw2, Some(SfpPlus), Some(CableKind::Os2), Some(120.0));

    for (sw, x0) in [(sw1, 80.0), (sw2, 820.0)] {
        let ap = b.device(AccessPoint, if x0 < 500.0 { "AP-L1-01" } else { "AP-L2-01" }, x0, 740.0);
        let pc = b.device(Pc, if x0 < 500.0 { "PC-L1-01" } else { "PC-L2-01" }, x0 + 160.0, 740.0);
        let ph = b.device(IpPhone, if x0 < 500.0 { "PH-L1-01" } else { "PH-L2-01" }, x0 + 320.0, 740.0);
        let cam = b.device(IpCamera, if x0 < 500.0 { "CAM-L1-01" } else { "CAM-L2-01" }, x0 + 480.0, 740.0);
        b.link(sw, Some(Rj45), ap, Some(Rj45), Some(CableKind::Cat6A), Some(32.0));
        b.link(sw, Some(Rj45), pc, Some(Rj45), None, Some(18.0));
        b.link(sw, Some(Rj45), ph, Some(Rj45), None, None);
        b.link(sw, Some(Rj45), cam, Some(Rj45), None, Some(60.0));
    }

    let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../samples/corescale-hq.netvis");
    std::fs::create_dir_all(out.parent().unwrap()).unwrap();
    netvis_core::io::save(&b.p, &out).unwrap();
    println!("wrote {}", out.display());
}
