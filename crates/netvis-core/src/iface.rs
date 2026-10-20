//! Interface configuration: switchport mode, IP addressing, radios, and
//! logical interfaces (SVI, loopback, sub-interface, tunnel).

use std::net::IpAddr;

use netvis_ip::IpNet;
use serde::{Deserialize, Serialize};

use crate::ids::{LogicalId, PortId, VlanId};
use crate::vlan::WifiBand;

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub enum AllowedVlans {
    #[default]
    All,
    List(Vec<VlanId>),
}

impl AllowedVlans {
    pub fn allows(&self, v: VlanId) -> bool {
        match self {
            AllowedVlans::All => true,
            AllowedVlans::List(l) => l.contains(&v),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub enum SwitchportMode {
    /// Host interface or not yet configured.
    #[default]
    Unset,
    Access { vlan: Option<VlanId> },
    Trunk { native: Option<VlanId>, allowed: AllowedVlans },
    /// Layer-3 routed port (no switching).
    Routed,
}

impl SwitchportMode {
    pub fn label(&self) -> &'static str {
        match self {
            SwitchportMode::Unset => "Host / unset",
            SwitchportMode::Access { .. } => "Access",
            SwitchportMode::Trunk { .. } => "Trunk",
            SwitchportMode::Routed => "Routed",
        }
    }

    pub fn is_switchport(&self) -> bool {
        matches!(self, SwitchportMode::Access { .. } | SwitchportMode::Trunk { .. })
    }

    /// Untagged VLAN on this port, if any.
    pub fn untagged_vlan(&self) -> Option<VlanId> {
        match self {
            SwitchportMode::Access { vlan } => *vlan,
            SwitchportMode::Trunk { native, .. } => *native,
            _ => None,
        }
    }

    pub fn carries(&self, v: VlanId) -> bool {
        match self {
            SwitchportMode::Access { vlan } => *vlan == Some(v),
            SwitchportMode::Trunk { native, allowed } => *native == Some(v) || allowed.allows(v),
            _ => false,
        }
    }

    /// Drop references to the given VLANs. Returns true if anything changed.
    pub fn forget_vlans(&mut self, gone: &[VlanId]) -> bool {
        let mut changed = false;
        let mut clear = |v: &mut Option<VlanId>| {
            if v.is_some_and(|x| gone.contains(&x)) {
                *v = None;
                changed = true;
            }
        };
        match self {
            SwitchportMode::Access { vlan } => clear(vlan),
            SwitchportMode::Trunk { native, allowed } => {
                clear(native);
                if let AllowedVlans::List(l) = allowed {
                    let before = l.len();
                    l.retain(|v| !gone.contains(v));
                    changed |= l.len() != before;
                }
            }
            _ => {}
        }
        changed
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum AddrKind {
    #[default]
    Static,
    /// Fixed address handed out by DHCP (MAC reservation).
    Reservation,
}

impl AddrKind {
    pub fn label(self) -> &'static str {
        match self {
            AddrKind::Static => "Static",
            AddrKind::Reservation => "DHCP reservation",
        }
    }
}

/// An address on an interface, written as `addr/prefix` (host bits kept).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct IfAddr {
    pub net: IpNet,
    #[serde(default)]
    pub kind: AddrKind,
}

impl IfAddr {
    pub fn ip(&self) -> IpAddr {
        self.net.addr()
    }
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct L3Config {
    pub dhcp_client: bool,
    pub addresses: Vec<IfAddr>,
    pub gateway: Option<IpAddr>,
}

impl L3Config {
    pub fn is_addressed(&self) -> bool {
        self.dhcp_client || !self.addresses.is_empty()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Duplex {
    #[default]
    Auto,
    Full,
    Half,
}

impl Duplex {
    pub const ALL: [Duplex; 3] = [Duplex::Auto, Duplex::Full, Duplex::Half];

    pub fn label(self) -> &'static str {
        match self {
            Duplex::Auto => "Auto",
            Duplex::Full => "Full",
            Duplex::Half => "Half",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum WifiStandard {
    Wifi4,
    Wifi5,
    Wifi6,
    Wifi6E,
    Wifi7,
}

impl WifiStandard {
    pub const ALL: [WifiStandard; 5] =
        [WifiStandard::Wifi4, WifiStandard::Wifi5, WifiStandard::Wifi6, WifiStandard::Wifi6E, WifiStandard::Wifi7];

    pub fn label(self) -> &'static str {
        match self {
            WifiStandard::Wifi4 => "Wi-Fi 4 (802.11n)",
            WifiStandard::Wifi5 => "Wi-Fi 5 (802.11ac)",
            WifiStandard::Wifi6 => "Wi-Fi 6 (802.11ax)",
            WifiStandard::Wifi6E => "Wi-Fi 6E (802.11ax 6 GHz)",
            WifiStandard::Wifi7 => "Wi-Fi 7 (802.11be)",
        }
    }

    pub fn bands(self) -> &'static [WifiBand] {
        match self {
            WifiStandard::Wifi4 => &[WifiBand::G2_4, WifiBand::G5],
            WifiStandard::Wifi5 => &[WifiBand::G5],
            WifiStandard::Wifi6 => &[WifiBand::G2_4, WifiBand::G5],
            WifiStandard::Wifi6E | WifiStandard::Wifi7 => &[WifiBand::G2_4, WifiBand::G5, WifiBand::G6],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RadioConfig {
    pub standard: WifiStandard,
    pub band: WifiBand,
    pub channel: u16,
    pub width_mhz: u16,
    #[serde(default)]
    pub tx_power_dbm: Option<i8>,
}

impl Default for RadioConfig {
    fn default() -> Self {
        Self { standard: WifiStandard::Wifi6, band: WifiBand::G5, channel: 36, width_mhz: 80, tx_power_dbm: None }
    }
}

/// Per-port configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct InterfaceConfig {
    pub enabled: bool,
    pub mode: SwitchportMode,
    pub l3: L3Config,
    pub mtu: Option<u16>,
    pub duplex: Duplex,
    pub mac: String,
    pub radio: Option<RadioConfig>,
    /// Link-aggregation (port-channel) group number.
    pub lag: Option<u16>,
}

impl Default for InterfaceConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            mode: SwitchportMode::Unset,
            l3: L3Config::default(),
            mtu: None,
            duplex: Duplex::Auto,
            mac: String::new(),
            radio: None,
            lag: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum LogicalKind {
    /// Switched virtual interface (`interface Vlan10`).
    Svi { vlan: Option<VlanId> },
    Loopback,
    /// 802.1Q sub-interface on a physical port (router-on-a-stick).
    SubInterface { parent: Option<PortId>, vlan: Option<VlanId> },
    Tunnel,
}

impl LogicalKind {
    pub fn label(&self) -> &'static str {
        match self {
            LogicalKind::Svi { .. } => "VLAN interface (SVI)",
            LogicalKind::Loopback => "Loopback",
            LogicalKind::SubInterface { .. } => "Sub-interface",
            LogicalKind::Tunnel => "Tunnel",
        }
    }

    pub fn vlan(&self) -> Option<VlanId> {
        match self {
            LogicalKind::Svi { vlan } | LogicalKind::SubInterface { vlan, .. } => *vlan,
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LogicalInterface {
    pub id: LogicalId,
    pub name: String,
    pub kind: LogicalKind,
    #[serde(default = "yes")]
    pub enabled: bool,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub l3: L3Config,
    #[serde(default)]
    pub mtu: Option<u16>,
}

fn yes() -> bool {
    true
}

impl LogicalInterface {
    pub fn new(name: impl Into<String>, kind: LogicalKind) -> Self {
        Self {
            id: LogicalId::new(),
            name: name.into(),
            kind,
            enabled: true,
            description: String::new(),
            l3: L3Config::default(),
            mtu: None,
        }
    }
}

/// Spanning-tree protocol running on a switch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum StpMode {
    /// Not recorded in the plan.
    #[default]
    Unset,
    Off,
    Stp,
    Rstp,
    Mstp,
}

impl StpMode {
    pub const ALL: [StpMode; 5] = [StpMode::Unset, StpMode::Off, StpMode::Stp, StpMode::Rstp, StpMode::Mstp];

    pub fn label(self) -> &'static str {
        match self {
            StpMode::Unset => "Not specified",
            StpMode::Off => "Disabled",
            StpMode::Stp => "STP (802.1D)",
            StpMode::Rstp => "RSTP (802.1w)",
            StpMode::Mstp => "MSTP (802.1s)",
        }
    }

    pub fn is_enabled(self) -> bool {
        matches!(self, StpMode::Stp | StpMode::Rstp | StpMode::Mstp)
    }
}

/// Identifies any addressable interface on a device.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum IfaceKey {
    Port(PortId),
    Logical(LogicalId),
}
