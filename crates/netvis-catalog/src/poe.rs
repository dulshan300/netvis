use serde::{Deserialize, Serialize};

/// Ordered by power: `Af < At < BtType3 < BtType4`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
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

    /// Power guaranteed at the powered device after cable loss, in watts.
    pub fn pd_watts(self) -> f32 {
        match self {
            PoeStandard::Af => 12.95,
            PoeStandard::At => 25.5,
            PoeStandard::BtType3 => 51.0,
            PoeStandard::BtType4 => 71.3,
        }
    }

    pub fn short(self) -> &'static str {
        match self {
            PoeStandard::Af => "af",
            PoeStandard::At => "at",
            PoeStandard::BtType3 => "bt3",
            PoeStandard::BtType4 => "bt4",
        }
    }
}

/// PoE is specified over a 100 m twisted-pair channel.
pub const POE_MAX_LENGTH_M: f32 = 100.0;
