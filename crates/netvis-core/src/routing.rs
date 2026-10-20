use std::net::IpAddr;

use netvis_ip::IpNet;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StaticRoute {
    pub dest: IpNet,
    #[serde(default)]
    pub next_hop: Option<IpAddr>,
    /// Exit interface name, used when there is no next hop (or alongside it).
    #[serde(default)]
    pub interface: String,
    #[serde(default = "default_distance")]
    pub distance: u8,
    #[serde(default)]
    pub description: String,
}

fn default_distance() -> u8 {
    1
}

impl Default for StaticRoute {
    fn default() -> Self {
        Self {
            dest: "0.0.0.0/0".parse().expect("valid"),
            next_hop: None,
            interface: String::new(),
            distance: 1,
            description: String::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum NatKind {
    /// Many-to-one source NAT (overload / masquerade).
    #[default]
    SourcePat,
    /// One-to-one static NAT.
    Static,
    /// Destination NAT of a service port.
    PortForward,
}

impl NatKind {
    pub const ALL: [NatKind; 3] = [NatKind::SourcePat, NatKind::Static, NatKind::PortForward];

    pub fn label(self) -> &'static str {
        match self {
            NatKind::SourcePat => "Source PAT",
            NatKind::Static => "Static NAT",
            NatKind::PortForward => "Port forward",
        }
    }
}

/// NAT rules are free-form text so they can name objects, ranges or interfaces.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct NatRule {
    pub kind: NatKind,
    pub inside: String,
    pub outside: String,
    /// e.g. `tcp/443`
    pub service: String,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Routing {
    pub static_routes: Vec<StaticRoute>,
    pub nat: Vec<NatRule>,
    /// OSPF areas, BGP ASNs/peers and similar planning notes.
    pub dynamic: String,
}

/// Project-wide network services.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Services {
    pub domain: String,
    pub dns_servers: Vec<IpAddr>,
    /// Hostnames or addresses.
    pub ntp_servers: Vec<String>,
    pub syslog: String,
    pub notes: String,
}
