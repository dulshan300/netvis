//! Project-building helpers shared by the sample generators. Every change
//! goes through `History`, like edits in the app.

#![allow(dead_code)]

use netvis_core::{
    AddrKind, Area, AreaKind, CableKind, DeviceId, DeviceKind, Edit, Endpoint, History, IfAddr, InterfaceConfig,
    LogicalInterface, LogicalKind, PortId, PortMedia, Pos, Project, Rect, Subnet, Vlan, VlanId,
};

#[derive(Default)]
pub struct Builder {
    pub p: Project,
    pub h: History,
}

impl Builder {
    pub fn device(&mut self, kind: DeviceKind, name: &str, x: f32, y: f32) -> DeviceId {
        let (id, mut edit) = self.p.add_device_edit(kind, Pos::new(x, y));
        if let Edit::Device { after: Some(d), .. } = &mut edit {
            d.name = name.to_owned();
        }
        self.h.apply(&mut self.p, edit, "add");
        id
    }

    /// Connect first free ports (optionally of a given media); returns (port on a, port on b).
    pub fn link(
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

    /// Cable from a specific port (e.g. the rear of a panel port) to the
    /// first free port on `b`; returns (port on a, port on b).
    pub fn link_ports(&mut self, a: (DeviceId, PortId), b: DeviceId, mb: Option<PortMedia>, cable: CableKind, len: Option<f32>) -> (PortId, PortId) {
        let ea = Endpoint { device: a.0, port: a.1 };
        let eb = Endpoint { device: b, port: self.p.first_free_port(b, mb).expect("free port") };
        let (_, mut edit) = self.p.add_link_edit(ea, eb).expect("valid link");
        if let Edit::Link { after: Some(l), .. } = &mut edit {
            l.cable = cable;
            l.length_m = len;
        }
        self.h.apply(&mut self.p, edit, "link");
        (ea.port, eb.port)
    }

    /// The `i`-th port of a device (0-based).
    pub fn port(&self, d: DeviceId, i: usize) -> PortId {
        self.p.device(d).expect("device").ports[i].id
    }

    /// Cable two specific ports.
    pub fn cable(&mut self, a: (DeviceId, PortId), b: (DeviceId, PortId), cable: CableKind, len: Option<f32>) {
        let (ea, eb) = (Endpoint { device: a.0, port: a.1 }, Endpoint { device: b.0, port: b.1 });
        let (_, mut edit) = self.p.add_link_edit(ea, eb).expect("valid link");
        if let Edit::Link { after: Some(l), .. } = &mut edit {
            l.cable = cable;
            l.length_m = len;
        }
        self.h.apply(&mut self.p, edit, "link");
    }

    /// An area, returning its id.
    pub fn area_id(&mut self, kind: AreaKind, name: &str, rect: Rect) -> netvis_core::AreaId {
        let a = Area::new(kind, name, rect);
        let id = a.id;
        self.h.apply(&mut self.p, Edit::Area { id, before: None, after: Some(a) }, "area");
        id
    }

    pub fn area(&mut self, kind: AreaKind, name: &str, rect: Rect) {
        let a = Area::new(kind, name, rect);
        self.h.apply(&mut self.p, Edit::Area { id: a.id, before: None, after: Some(a) }, "area");
    }

    pub fn vlan(&mut self, tag: u16, name: &str, purpose: &str) -> VlanId {
        let mut v = Vlan::new(tag, name);
        v.purpose = purpose.into();
        let id = v.id;
        self.h.apply(&mut self.p, Edit::Vlan { id, before: None, after: Some(v) }, "vlan");
        id
    }

    pub fn subnet(&mut self, name: &str, net: &str, vlan: Option<VlanId>, dhcp: bool) {
        let mut s = Subnet::new(name, net.parse().unwrap()).with_defaults(vlan.is_some(), dhcp);
        s.vlan = vlan;
        self.h.apply(&mut self.p, Edit::Subnet { id: s.id, before: None, after: Some(s) }, "subnet");
    }

    pub fn cfg(&mut self, d: DeviceId, port: PortId, f: impl FnOnce(&mut InterfaceConfig)) {
        let dev = self.p.device_mut(d).unwrap();
        f(&mut dev.ports.iter_mut().find(|p| p.id == port).unwrap().cfg);
    }

    pub fn model(&mut self, d: DeviceId, model: &str) {
        let (e, _) = self.p.apply_model_edit(d, model).expect("known model");
        self.h.apply(&mut self.p, e, "model");
    }

    pub fn tx(&mut self, d: DeviceId, port: PortId, id: &str) {
        let dev = self.p.device_mut(d).unwrap();
        dev.ports.iter_mut().find(|p| p.id == port).unwrap().transceiver = Some(id.to_owned());
    }

    /// Cable the next free copper port of switch `sw` to `dev`'s first free
    /// port and set the switch port's mode. Returns (switch port, device port).
    pub fn attach(&mut self, sw: DeviceId, dev: DeviceId, cable: CableKind, len: f32, mode: netvis_core::SwitchportMode) -> (PortId, PortId) {
        let (sp, dp) = self.link(sw, Some(PortMedia::Rj45), dev, None, Some(cable), Some(len));
        self.cfg(sw, sp, |c| c.mode = mode);
        (sp, dp)
    }

    /// Router-on-a-stick: an 802.1Q sub-interface for `vlan` on `parent`.
    pub fn sub_if(&mut self, d: DeviceId, parent: PortId, tag: u16, vlan: VlanId) {
        let pname = self.p.device(d).unwrap().ports.iter().find(|p| p.id == parent).unwrap().name.clone();
        let l = LogicalInterface::new(format!("{pname}.{tag}"), LogicalKind::SubInterface { parent: Some(parent), vlan: Some(vlan) });
        self.p.device_mut(d).unwrap().logical.push(l);
    }

    pub fn svi(&mut self, d: DeviceId, tag: u16, vlan: VlanId) {
        let l = LogicalInterface::new(format!("Vlan{tag}"), LogicalKind::Svi { vlan: Some(vlan) });
        self.p.device_mut(d).unwrap().logical.push(l);
    }
}

pub fn addr(s: &str) -> IfAddr {
    IfAddr { net: s.parse().unwrap(), kind: AddrKind::Static }
}
