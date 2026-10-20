use serde::{Deserialize, Serialize};

use crate::iface::{InterfaceConfig, RadioConfig};
use crate::ids::PortId;

/// Physical connector / cage type of a port.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PortMedia {
    Rj45,
    Sfp,
    SfpPlus,
    Sfp28,
    QsfpPlus,
    Qsfp28,
    Wireless,
    Coax,
    Console,
    /// Logical attachment point for cloud/WAN objects.
    Virtual,
}

impl PortMedia {
    pub const ALL: [PortMedia; 10] = [
        PortMedia::Rj45,
        PortMedia::Sfp,
        PortMedia::SfpPlus,
        PortMedia::Sfp28,
        PortMedia::QsfpPlus,
        PortMedia::Qsfp28,
        PortMedia::Wireless,
        PortMedia::Coax,
        PortMedia::Console,
        PortMedia::Virtual,
    ];

    pub fn label(self) -> &'static str {
        match self {
            PortMedia::Rj45 => "RJ45",
            PortMedia::Sfp => "SFP",
            PortMedia::SfpPlus => "SFP+",
            PortMedia::Sfp28 => "SFP28",
            PortMedia::QsfpPlus => "QSFP+",
            PortMedia::Qsfp28 => "QSFP28",
            PortMedia::Wireless => "Wireless",
            PortMedia::Coax => "Coax",
            PortMedia::Console => "Console",
            PortMedia::Virtual => "Virtual",
        }
    }

    /// Nominal maximum speed of the cage, in Mbit/s.
    pub fn default_speed_mbps(self) -> u32 {
        match self {
            PortMedia::Rj45 | PortMedia::Sfp => 1_000,
            PortMedia::SfpPlus => 10_000,
            PortMedia::Sfp28 => 25_000,
            PortMedia::QsfpPlus => 40_000,
            PortMedia::Qsfp28 => 100_000,
            PortMedia::Wireless => 1_200,
            PortMedia::Coax => 1_000,
            PortMedia::Console => 0,
            PortMedia::Virtual => 1_000,
        }
    }

    pub fn is_optical_cage(self) -> bool {
        matches!(
            self,
            PortMedia::Sfp | PortMedia::SfpPlus | PortMedia::Sfp28 | PortMedia::QsfpPlus | PortMedia::Qsfp28
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PoeStandard {
    /// 802.3af, 15.4 W at the PSE.
    Af,
    /// 802.3at (PoE+), 30 W.
    At,
    /// 802.3bt Type 3, 60 W.
    BtType3,
    /// 802.3bt Type 4, 90 W.
    BtType4,
}

impl PoeStandard {
    pub const ALL: [PoeStandard; 4] =
        [PoeStandard::Af, PoeStandard::At, PoeStandard::BtType3, PoeStandard::BtType4];

    pub fn label(self) -> &'static str {
        match self {
            PoeStandard::Af => "802.3af (PoE)",
            PoeStandard::At => "802.3at (PoE+)",
            PoeStandard::BtType3 => "802.3bt Type 3",
            PoeStandard::BtType4 => "802.3bt Type 4",
        }
    }

    /// Maximum power sourced by the PSE, in watts.
    pub fn pse_watts(self) -> f32 {
        match self {
            PoeStandard::Af => 15.4,
            PoeStandard::At => 30.0,
            PoeStandard::BtType3 => 60.0,
            PoeStandard::BtType4 => 90.0,
        }
    }
}

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

/// Human readable speed, e.g. `10G`, `2.5G`, `100M`.
pub fn format_speed(mbps: u32) -> String {
    match mbps {
        0 => "—".to_owned(),
        m if m >= 1_000 && m % 1_000 == 0 => format!("{}G", m / 1_000),
        m if m >= 1_000 => format!("{}G", m as f32 / 1_000.0),
        m => format!("{m}M"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn speed_formatting() {
        assert_eq!(format_speed(1_000), "1G");
        assert_eq!(format_speed(2_500), "2.5G");
        assert_eq!(format_speed(100), "100M");
        assert_eq!(format_speed(100_000), "100G");
    }
}
