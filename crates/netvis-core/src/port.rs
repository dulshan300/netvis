use serde::{Deserialize, Serialize};

pub use netvis_catalog::{PoeStandard, PortMedia, format_speed};

use crate::iface::{InterfaceConfig, RadioConfig};
use crate::ids::PortId;

/// Role of a port in Power-over-Ethernet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum PoeRole {
    #[default]
    None,
    /// Power sourcing equipment (switch / injector port).
    Source(PoeStandard),
    /// Powered device (AP, camera, phone…).
    Powered(PoeStandard),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Port {
    pub id: PortId,
    pub name: String,
    pub media: PortMedia,
    pub speed_mbps: u32,
    #[serde(default)]
    pub poe: PoeRole,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub cfg: InterfaceConfig,
    /// Catalogue id of the pluggable module in an SFP/QSFP cage.
    #[serde(default)]
    pub transceiver: Option<String>,
}

impl Port {
    pub fn new(name: impl Into<String>, media: PortMedia) -> Self {
        Self {
            id: PortId::new(),
            name: name.into(),
            media,
            speed_mbps: media.default_speed_mbps(),
            poe: PoeRole::None,
            description: String::new(),
            cfg: InterfaceConfig {
                radio: (media == PortMedia::Wireless).then(RadioConfig::default),
                ..InterfaceConfig::default()
            },
            transceiver: None,
        }
    }

    pub fn with_speed(mut self, mbps: u32) -> Self {
        self.speed_mbps = mbps;
        self
    }

    pub fn with_poe(mut self, poe: PoeRole) -> Self {
        self.poe = poe;
        self
    }
}
