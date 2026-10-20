use serde::{Deserialize, Serialize};

use crate::ids::{DeviceId, LinkId, PortId};
use crate::port::PortMedia;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Endpoint {
    pub device: DeviceId,
    pub port: PortId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CableKind {
    Unspecified,
    Cat5e,
    Cat6,
    Cat6A,
    Cat7,
    Cat8,
    Om1,
    Om2,
    Om3,
    Om4,
    Om5,
    Os1,
    Os2,
    Dac,
    Aoc,
    Coax,
    Wireless,
    Console,
    Virtual,
}

/// Broad family, used for colouring and (later) media validation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CableFamily {
    Copper,
    Multimode,
    SingleMode,
    DirectAttach,
    Coax,
    Wireless,
    Console,
    Logical,
}

impl CableKind {
    pub const ALL: [CableKind; 19] = [
        CableKind::Unspecified,
        CableKind::Cat5e,
        CableKind::Cat6,
        CableKind::Cat6A,
        CableKind::Cat7,
        CableKind::Cat8,
        CableKind::Om1,
        CableKind::Om2,
        CableKind::Om3,
        CableKind::Om4,
        CableKind::Om5,
        CableKind::Os1,
        CableKind::Os2,
        CableKind::Dac,
        CableKind::Aoc,
        CableKind::Coax,
        CableKind::Wireless,
        CableKind::Console,
        CableKind::Virtual,
    ];

    pub fn label(self) -> &'static str {
        match self {
            CableKind::Unspecified => "Unspecified",
            CableKind::Cat5e => "Cat5e",
            CableKind::Cat6 => "Cat6",
            CableKind::Cat6A => "Cat6A",
            CableKind::Cat7 => "Cat7",
            CableKind::Cat8 => "Cat8",
            CableKind::Om1 => "OM1 MMF",
            CableKind::Om2 => "OM2 MMF",
            CableKind::Om3 => "OM3 MMF",
            CableKind::Om4 => "OM4 MMF",
            CableKind::Om5 => "OM5 MMF",
            CableKind::Os1 => "OS1 SMF",
            CableKind::Os2 => "OS2 SMF",
            CableKind::Dac => "DAC",
            CableKind::Aoc => "AOC",
            CableKind::Coax => "Coax",
            CableKind::Wireless => "Wireless",
            CableKind::Console => "Console",
            CableKind::Virtual => "Logical / WAN",
        }
    }

    pub fn family(self) -> CableFamily {
        use CableKind::*;
        match self {
            Unspecified | Cat5e | Cat6 | Cat6A | Cat7 | Cat8 => CableFamily::Copper,
            Om1 | Om2 | Om3 | Om4 | Om5 => CableFamily::Multimode,
            Os1 | Os2 => CableFamily::SingleMode,
            Dac | Aoc => CableFamily::DirectAttach,
            Coax => CableFamily::Coax,
            Wireless => CableFamily::Wireless,
            Console => CableFamily::Console,
            Virtual => CableFamily::Logical,
        }
    }

    /// Sensible default cable when connecting two ports of these media types.
    pub fn suggest(a: PortMedia, b: PortMedia) -> CableKind {
        use PortMedia::*;
        match (a, b) {
            (Virtual, _) | (_, Virtual) => CableKind::Virtual,
            (Wireless, _) | (_, Wireless) => CableKind::Wireless,
            (Console, _) | (_, Console) => CableKind::Console,
            (Coax, _) | (_, Coax) => CableKind::Coax,
            (Rj45, Rj45) => CableKind::Cat6,
            (Rj45, _) | (_, Rj45) => CableKind::Cat6A,
            (Sfp, Sfp) => CableKind::Om3,
            _ => CableKind::Om4,
        }
    }
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
