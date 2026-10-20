use serde::{Deserialize, Serialize};

use std::net::IpAddr;

use crate::geom::{Pos, Rect};
use crate::iface::{IfaceKey, L3Config, LogicalInterface, LogicalKind};
use crate::ids::{DeviceId, LogicalId, PortId, VlanId};
use crate::routing::Routing;
use crate::port::{PoeRole, PoeStandard, Port, PortMedia};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DeviceCategory {
    Network,
    EndDevice,
    Infrastructure,
    CloudWan,
}

impl DeviceCategory {
    pub const ALL: [DeviceCategory; 4] = [
        DeviceCategory::Network,
        DeviceCategory::EndDevice,
        DeviceCategory::Infrastructure,
        DeviceCategory::CloudWan,
    ];

    pub fn label(self) -> &'static str {
        match self {
            DeviceCategory::Network => "Network",
            DeviceCategory::EndDevice => "End devices",
            DeviceCategory::Infrastructure => "Infrastructure",
            DeviceCategory::CloudWan => "Cloud & WAN",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DeviceKind {
    // Network
    Router,
    SwitchL2,
    SwitchL3,
    Firewall,
    AccessPoint,
    WirelessController,
    LoadBalancer,
    Modem,
    VpnGateway,
    // End devices
    Pc,
    Laptop,
    Server,
    VmHost,
    Storage,
    Printer,
    IpPhone,
    IpCamera,
    IoT,
    SmartTv,
    Pos,
    // Infrastructure
    PatchPanel,
    Ups,
    Pdu,
    PoeInjector,
    MediaConverter,
    // Cloud & WAN
    Internet,
    Isp,
    Mpls,
    SdWan,
    VpnTunnel,
    CloudVpc,
}

impl DeviceKind {
    pub const ALL: [DeviceKind; 31] = [
        DeviceKind::Router,
        DeviceKind::SwitchL2,
        DeviceKind::SwitchL3,
        DeviceKind::Firewall,
        DeviceKind::AccessPoint,
        DeviceKind::WirelessController,
        DeviceKind::LoadBalancer,
        DeviceKind::Modem,
        DeviceKind::VpnGateway,
        DeviceKind::Pc,
        DeviceKind::Laptop,
        DeviceKind::Server,
        DeviceKind::VmHost,
        DeviceKind::Storage,
        DeviceKind::Printer,
        DeviceKind::IpPhone,
        DeviceKind::IpCamera,
        DeviceKind::IoT,
        DeviceKind::SmartTv,
        DeviceKind::Pos,
        DeviceKind::PatchPanel,
        DeviceKind::Ups,
        DeviceKind::Pdu,
        DeviceKind::PoeInjector,
        DeviceKind::MediaConverter,
        DeviceKind::Internet,
        DeviceKind::Isp,
        DeviceKind::Mpls,
        DeviceKind::SdWan,
        DeviceKind::VpnTunnel,
        DeviceKind::CloudVpc,
    ];

    pub fn category(self) -> DeviceCategory {
        use DeviceKind::*;
        match self {
            Router | SwitchL2 | SwitchL3 | Firewall | AccessPoint | WirelessController
            | LoadBalancer | Modem | VpnGateway => DeviceCategory::Network,
            Pc | Laptop | Server | VmHost | Storage | Printer | IpPhone | IpCamera | IoT
            | SmartTv | Pos => DeviceCategory::EndDevice,
            PatchPanel | Ups | Pdu | PoeInjector | MediaConverter => DeviceCategory::Infrastructure,
            Internet | Isp | Mpls | SdWan | VpnTunnel | CloudVpc => DeviceCategory::CloudWan,
        }
    }

    pub fn label(self) -> &'static str {
        use DeviceKind::*;
        match self {
            Router => "Router",
            SwitchL2 => "L2 Switch",
            SwitchL3 => "L3 Switch",
            Firewall => "Firewall / UTM",
            AccessPoint => "Wi-Fi AP",
            WirelessController => "Wi-Fi Controller",
            LoadBalancer => "Load Balancer",
            Modem => "Modem / ONT",
            VpnGateway => "VPN Gateway",
            Pc => "PC",
            Laptop => "Laptop",
            Server => "Server",
            VmHost => "VM Host",
            Storage => "NAS / SAN",
            Printer => "Printer",
            IpPhone => "IP Phone",
            IpCamera => "IP Camera",
            IoT => "IoT Device",
            SmartTv => "Smart TV",
            Pos => "POS Terminal",
            PatchPanel => "Patch Panel",
            Ups => "UPS",
            Pdu => "PDU",
            PoeInjector => "PoE Injector",
            MediaConverter => "Media Converter",
            Internet => "Internet",
            Isp => "ISP",
            Mpls => "MPLS",
            SdWan => "SD-WAN",
            VpnTunnel => "VPN Tunnel",
            CloudVpc => "Cloud VPC",
        }
    }

    /// Short prefix used for auto-generated hostnames (`SW-01`, `RTR-02`…).
    pub fn name_prefix(self) -> &'static str {
        use DeviceKind::*;
        match self {
            Router => "RTR",
            SwitchL2 => "SW",
            SwitchL3 => "CSW",
            Firewall => "FW",
            AccessPoint => "AP",
            WirelessController => "WLC",
            LoadBalancer => "LB",
            Modem => "MDM",
            VpnGateway => "VPN",
            Pc => "PC",
            Laptop => "LT",
            Server => "SRV",
            VmHost => "HV",
            Storage => "NAS",
            Printer => "PRN",
            IpPhone => "PH",
            IpCamera => "CAM",
            IoT => "IOT",
            SmartTv => "TV",
            Pos => "POS",
            PatchPanel => "PP",
            Ups => "UPS",
            Pdu => "PDU",
            PoeInjector => "INJ",
            MediaConverter => "MC",
            Internet => "INET",
            Isp => "ISP",
            Mpls => "MPLS",
            SdWan => "SDWAN",
            VpnTunnel => "TUN",
            CloudVpc => "VPC",
        }
    }

    /// Default port layout for a freshly placed device.
    pub fn default_ports(self) -> Vec<Port> {
        use DeviceKind::*;
        let rj45 = |prefix: &str, n: usize| -> Vec<Port> {
            (1..=n).map(|i| Port::new(format!("{prefix}{i}"), PortMedia::Rj45)).collect()
        };
        let cage = |prefix: &str, n: usize, media: PortMedia| -> Vec<Port> {
            (1..=n).map(|i| Port::new(format!("{prefix}{i}"), media)).collect()
        };
        let pse = |ports: Vec<Port>, std: PoeStandard| -> Vec<Port> {
            ports.into_iter().map(|p| p.with_poe(PoeRole::Source(std))).collect()
        };
        let pd = |name: &str, mbps: u32, std: PoeStandard| {
            Port::new(name, PortMedia::Rj45).with_speed(mbps).with_poe(PoeRole::Powered(std))
        };

        match self {
            Router => [rj45("ge", 4), cage("sfp", 2, PortMedia::Sfp)].concat(),
            SwitchL2 => {
                [pse(rj45("ge", 24), PoeStandard::At), cage("xe", 4, PortMedia::SfpPlus)].concat()
            }
            SwitchL3 => [rj45("ge", 48), cage("xe", 4, PortMedia::SfpPlus)].concat(),
            Firewall => [rj45("port", 8), cage("x", 2, PortMedia::SfpPlus)].concat(),
            AccessPoint => vec![
                pd("eth0", 2_500, PoeStandard::At),
                Port::new("radio0", PortMedia::Wireless),
            ],
            WirelessController => [rj45("ge", 2), cage("xe", 2, PortMedia::SfpPlus)].concat(),
            LoadBalancer => [rj45("ge", 4), cage("xe", 2, PortMedia::SfpPlus)].concat(),
            Modem => vec![Port::new("wan", PortMedia::Coax), Port::new("lan1", PortMedia::Rj45)],
            VpnGateway => rj45("ge", 4),
            Pc | Printer | SmartTv | Pos => rj45("eth", 1),
            Laptop => vec![Port::new("eth0", PortMedia::Rj45), Port::new("wlan0", PortMedia::Wireless)],
            Server => [rj45("eno", 2), cage("ens", 2, PortMedia::SfpPlus)].concat(),
            VmHost => [rj45("eno", 2), cage("ens", 2, PortMedia::Sfp28)].concat(),
            Storage => [rj45("eth", 2), cage("ens", 2, PortMedia::SfpPlus)].concat(),
            IpPhone => vec![pd("lan", 1_000, PoeStandard::Af), Port::new("pc", PortMedia::Rj45)],
            IpCamera => vec![pd("eth0", 100, PoeStandard::Af)],
            IoT => vec![Port::new("wlan0", PortMedia::Wireless)],
            PatchPanel => rj45("", 24),
            Ups | Pdu => vec![Port::new("mgmt", PortMedia::Rj45).with_speed(100)],
            PoeInjector => vec![
                Port::new("data-in", PortMedia::Rj45),
                Port::new("poe-out", PortMedia::Rj45).with_poe(PoeRole::Source(PoeStandard::BtType3)),
            ],
            MediaConverter => vec![Port::new("copper", PortMedia::Rj45), Port::new("fiber", PortMedia::Sfp)],
            Internet | Isp | Mpls | SdWan | VpnTunnel | CloudVpc => (1..=4)
                .map(|i| Port::new(format!("link{i}"), PortMedia::Virtual))
                .collect(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Device {
    pub id: DeviceId,
    pub kind: DeviceKind,
    pub name: String,
    /// Top-left corner of the device card on the canvas.
    pub pos: Pos,
    pub ports: Vec<Port>,
    #[serde(default)]
    pub vendor: String,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub notes: String,
    /// SVIs, loopbacks, sub-interfaces and tunnels.
    #[serde(default)]
    pub logical: Vec<LogicalInterface>,
    /// Resolver override; empty = project defaults.
    #[serde(default)]
    pub dns: Vec<IpAddr>,
    #[serde(default)]
    pub routing: Routing,
}

/// Borrowed view of any addressable interface.
#[derive(Debug, Clone, Copy)]
pub struct IfaceRef<'a> {
    pub key: IfaceKey,
    pub name: &'a str,
    pub l3: &'a L3Config,
    pub enabled: bool,
}

/// Device-card geometry shared by the canvas and the SVG renderer.
pub mod layout {
    pub const MIN_WIDTH: f32 = 132.0;
    pub const HEADER_HEIGHT: f32 = 50.0;
    pub const PORTS_PER_ROW: usize = 12;
    pub const PORT_PITCH: f32 = 10.0;
    pub const PORT_SIZE: f32 = 7.0;
    pub const PADDING: f32 = 8.0;
}

impl Device {
    pub fn new(kind: DeviceKind, name: impl Into<String>, pos: Pos) -> Self {
        Self {
            id: DeviceId::new(),
            kind,
            name: name.into(),
            pos,
            ports: kind.default_ports(),
            vendor: String::new(),
            model: String::new(),
            notes: String::new(),
            logical: Vec::new(),
            dns: Vec::new(),
            routing: Routing::default(),
        }
    }

    pub fn logical(&self, id: LogicalId) -> Option<&LogicalInterface> {
        self.logical.iter().find(|l| l.id == id)
    }

    /// Ports first, then logical interfaces.
    pub fn interfaces(&self) -> impl Iterator<Item = IfaceRef<'_>> {
        let ports = self.ports.iter().map(|p| IfaceRef {
            key: IfaceKey::Port(p.id),
            name: &p.name,
            l3: &p.cfg.l3,
            enabled: p.cfg.enabled,
        });
        let logical = self.logical.iter().map(|l| IfaceRef {
            key: IfaceKey::Logical(l.id),
            name: &l.name,
            l3: &l.l3,
            enabled: l.enabled,
        });
        ports.chain(logical)
    }

    pub fn iface_name(&self, key: IfaceKey) -> Option<&str> {
        match key {
            IfaceKey::Port(id) => self.port(id).map(|p| p.name.as_str()),
            IfaceKey::Logical(id) => self.logical(id).map(|l| l.name.as_str()),
        }
    }

    pub fn l3_mut(&mut self, key: IfaceKey) -> Option<&mut L3Config> {
        match key {
            IfaceKey::Port(id) => self.ports.iter_mut().find(|p| p.id == id).map(|p| &mut p.cfg.l3),
            IfaceKey::Logical(id) => self.logical.iter_mut().find(|l| l.id == id).map(|l| &mut l.l3),
        }
    }

    /// First configured address (logical interfaces first, as they usually carry management).
    pub fn primary_address(&self) -> Option<crate::iface::IfAddr> {
        self.logical
            .iter()
            .flat_map(|l| l.l3.addresses.first())
            .chain(self.ports.iter().flat_map(|p| p.cfg.l3.addresses.first()))
            .next()
            .copied()
    }

    /// Drop references to deleted VLANs. Returns true if anything changed.
    pub fn forget_vlans(&mut self, gone: &[VlanId]) -> bool {
        let mut changed = false;
        for p in &mut self.ports {
            changed |= p.cfg.mode.forget_vlans(gone);
        }
        for l in &mut self.logical {
            if let LogicalKind::Svi { vlan } | LogicalKind::SubInterface { vlan, .. } = &mut l.kind
                && vlan.is_some_and(|v| gone.contains(&v))
            {
                *vlan = None;
                changed = true;
            }
        }
        changed
    }

    pub fn port(&self, id: PortId) -> Option<&Port> {
        self.ports.iter().find(|p| p.id == id)
    }

    fn port_rows(&self) -> usize {
        self.ports.len().div_ceil(layout::PORTS_PER_ROW)
    }

    pub fn size(&self) -> Pos {
        use layout::*;
        let cols = self.ports.len().min(PORTS_PER_ROW) as f32;
        let w = (cols * PORT_PITCH + 2.0 * PADDING).max(MIN_WIDTH);
        let h = HEADER_HEIGHT + self.port_rows() as f32 * PORT_PITCH + PADDING;
        Pos::new(w, h)
    }

    pub fn rect(&self) -> Rect {
        let s = self.size();
        Rect { min: self.pos, size: s }
    }

    /// Centre of the port marker with the given index, in world coordinates.
    pub fn port_center(&self, index: usize) -> Pos {
        use layout::*;
        let row = index / PORTS_PER_ROW;
        let col = index % PORTS_PER_ROW;
        let cols_in_row = (self.ports.len() - row * PORTS_PER_ROW).min(PORTS_PER_ROW) as f32;
        let row_width = cols_in_row * PORT_PITCH;
        let x0 = self.pos.x + (self.size().x - row_width) / 2.0;
        Pos::new(
            x0 + col as f32 * PORT_PITCH + PORT_PITCH / 2.0,
            self.pos.y + HEADER_HEIGHT + row as f32 * PORT_PITCH + PORT_PITCH / 2.0,
        )
    }

    pub fn port_center_by_id(&self, id: PortId) -> Option<Pos> {
        self.ports.iter().position(|p| p.id == id).map(|i| self.port_center(i))
    }

    /// Deep copy with fresh ids. Static addresses and MACs are cleared so the
    /// copy does not instantly conflict with the original.
    pub fn duplicate(&self, name: String, pos: Pos) -> Device {
        let mut d = self.clone();
        d.id = DeviceId::new();
        d.name = name;
        d.pos = pos;
        let mut remap = std::collections::HashMap::new();
        for p in &mut d.ports {
            let new = PortId::new();
            remap.insert(p.id, new);
            p.id = new;
            p.cfg.l3.addresses.clear();
            p.cfg.mac.clear();
        }
        for l in &mut d.logical {
            l.id = LogicalId::new();
            l.l3.addresses.clear();
            if let LogicalKind::SubInterface { parent: Some(parent), .. } = &mut l.kind {
                *parent = remap.get(parent).copied().unwrap_or(*parent);
            }
        }
        d
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_kind_has_ports_and_category() {
        for kind in DeviceKind::ALL {
            assert!(!kind.default_ports().is_empty(), "{kind:?} has no ports");
            assert!(DeviceCategory::ALL.contains(&kind.category()));
        }
    }

    #[test]
    fn access_switch_template() {
        let ports = DeviceKind::SwitchL2.default_ports();
        assert_eq!(ports.len(), 28);
        assert_eq!(ports.iter().filter(|p| p.media == PortMedia::SfpPlus).count(), 4);
        assert!(ports[..24].iter().all(|p| p.poe == PoeRole::Source(PoeStandard::At)));
    }

    #[test]
    fn port_centres_lie_inside_card() {
        let d = Device::new(DeviceKind::SwitchL3, "CSW-01", Pos::new(100.0, 50.0));
        let r = d.rect();
        for i in 0..d.ports.len() {
            assert!(r.contains(d.port_center(i)), "port {i} outside card");
        }
    }

    #[test]
    fn duplicate_gets_fresh_ids() {
        let d = Device::new(DeviceKind::Router, "RTR-01", Pos::default());
        let c = d.duplicate("RTR-02".into(), Pos::new(10.0, 10.0));
        assert_ne!(c.id, d.id);
        assert!(c.ports.iter().zip(&d.ports).all(|(a, b)| a.id != b.id && a.name == b.name));
    }
}
