use std::net::IpAddr;

use netvis_ip::IpNet;
use serde::{Deserialize, Serialize};

use crate::ids::{DeviceId, SubnetId, VlanId};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AddrRange {
    pub start: IpAddr,
    pub end: IpAddr,
    #[serde(default)]
    pub label: String,
}

impl AddrRange {
    pub fn contains(&self, ip: IpAddr) -> bool {
        netvis_ip::in_range(ip, self.start, self.end)
    }

    pub fn len(&self) -> u128 {
        netvis_ip::range_len(self.start, self.end)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DhcpScope {
    pub start: IpAddr,
    pub end: IpAddr,
    #[serde(default = "default_lease")]
    pub lease_hours: u32,
    /// DNS servers handed to clients; empty = use the project defaults.
    #[serde(default)]
    pub dns: Vec<IpAddr>,
    /// Device that serves this scope (router, firewall, server…).
    #[serde(default)]
    pub server: Option<DeviceId>,
}

fn default_lease() -> u32 {
    24
}

impl DhcpScope {
    pub fn range(&self) -> AddrRange {
        AddrRange { start: self.start, end: self.end, label: "DHCP pool".into() }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Subnet {
    pub id: SubnetId,
    pub name: String,
    /// Network prefix (host bits are always zero).
    pub net: IpNet,
    #[serde(default)]
    pub vlan: Option<VlanId>,
    #[serde(default)]
    pub gateway: Option<IpAddr>,
    #[serde(default)]
    pub dhcp: Option<DhcpScope>,
    /// Blocks kept free from auto-assignment (e.g. "infrastructure .2–.19").
    #[serde(default)]
    pub reserved: Vec<AddrRange>,
    #[serde(default)]
    pub notes: String,
}

impl Subnet {
    pub fn new(name: impl Into<String>, net: IpNet) -> Self {
        Self {
            id: SubnetId::new(),
            name: name.into(),
            net: net.trunc(),
            vlan: None,
            gateway: None,
            dhcp: None,
            reserved: Vec::new(),
            notes: String::new(),
        }
    }

    /// Conventional defaults: gateway on the first host, DHCP over the upper half.
    pub fn with_defaults(mut self, gateway: bool, dhcp: bool) -> Self {
        if gateway {
            self.gateway = netvis_ip::nth_usable(&self.net, 0);
        }
        let usable = netvis_ip::usable_count(&self.net);
        if dhcp && usable >= 8 {
            let (_, last) = netvis_ip::usable_range(&self.net);
            if let Some(start) = netvis_ip::nth_usable(&self.net, usable / 2) {
                self.dhcp = Some(DhcpScope { start, end: last, lease_hours: 24, dns: Vec::new(), server: None });
            }
        }
        self
    }

    /// Addresses that auto-assignment must not hand out (besides assigned ones).
    pub fn is_blocked(&self, ip: IpAddr) -> bool {
        self.gateway == Some(ip)
            || self.dhcp.as_ref().is_some_and(|d| d.range().contains(ip))
            || self.reserved.iter().any(|r| r.contains(ip))
    }
}
