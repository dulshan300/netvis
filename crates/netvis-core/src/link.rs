use serde::{Deserialize, Serialize};

use crate::ids::{DeviceId, LinkId, PortId};
pub use netvis_catalog::{CableFamily, CableKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Endpoint {
    pub device: DeviceId,
    pub port: PortId,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Link {
    pub id: LinkId,
    pub a: Endpoint,
    pub b: Endpoint,
    pub cable: CableKind,
    #[serde(default)]
    pub length_m: Option<f32>,
    #[serde(default)]
    pub label: String,
}

impl Link {
    pub fn new(a: Endpoint, b: Endpoint, cable: CableKind) -> Self {
        Self { id: LinkId::new(), a, b, cable, length_m: None, label: String::new() }
    }

    pub fn touches_device(&self, d: DeviceId) -> bool {
        self.a.device == d || self.b.device == d
    }

    pub fn touches_port(&self, p: PortId) -> bool {
        self.a.port == p || self.b.port == p
    }

    /// The far end, seen from `d`.
    pub fn other_end(&self, d: DeviceId) -> Option<Endpoint> {
        if self.a.device == d {
            Some(self.b)
        } else if self.b.device == d {
            Some(self.a)
        } else {
            None
        }
    }
}
