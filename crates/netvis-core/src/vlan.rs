use serde::{Deserialize, Serialize};

use crate::ids::{SsidId, VlanId};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Vlan {
    pub id: VlanId,
    /// 802.1Q tag, 1–4094.
    pub tag: u16,
    pub name: String,
    #[serde(default)]
    pub purpose: String,
    pub color: [u8; 3],
    #[serde(default)]
    pub notes: String,
}

impl Vlan {
    pub const MIN_TAG: u16 = 1;
    pub const MAX_TAG: u16 = 4094;

    pub fn new(tag: u16, name: impl Into<String>) -> Self {
        Self {
            id: VlanId::new(),
            tag,
            name: name.into(),
            purpose: String::new(),
            // Tags are usually multiples of 10, so mix tens and units for distinct colours.
            color: palette_color((tag / 10 + tag % 10) as usize),
            notes: String::new(),
        }
    }

    pub fn label(&self) -> String {
        format!("{} · {}", self.tag, self.name)
    }
}

/// Distinct, readable colours cycled by index.
pub fn palette_color(i: usize) -> [u8; 3] {
    const P: [[u8; 3]; 10] = [
        [59, 130, 246],
        [16, 185, 129],
        [245, 158, 11],
        [239, 68, 68],
        [139, 92, 246],
        [236, 72, 153],
        [20, 184, 166],
        [132, 204, 22],
        [249, 115, 22],
        [100, 116, 139],
    ];
    P[i % P.len()]
}

#[cfg(test)]
mod tests {
    #[test]
    fn common_tags_get_distinct_colours() {
        let colours: std::collections::HashSet<_> = [10, 20, 30, 40, 50, 60].map(|t| super::Vlan::new(t, "").color).into();
        assert_eq!(colours.len(), 6);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum WifiBand {
    G2_4,
    G5,
    G6,
}

impl WifiBand {
    pub const ALL: [WifiBand; 3] = [WifiBand::G2_4, WifiBand::G5, WifiBand::G6];

    pub fn label(self) -> &'static str {
        match self {
            WifiBand::G2_4 => "2.4 GHz",
            WifiBand::G5 => "5 GHz",
            WifiBand::G6 => "6 GHz",
        }
    }

    /// Typical default channel for a new radio on this band.
    pub fn default_channel(self) -> u16 {
        match self {
            WifiBand::G2_4 => 1,
            WifiBand::G5 => 36,
            WifiBand::G6 => 37,
        }
    }

    pub fn widths(self) -> &'static [u16] {
        match self {
            WifiBand::G2_4 => &[20, 40],
            WifiBand::G5 => &[20, 40, 80, 160],
            WifiBand::G6 => &[20, 40, 80, 160, 320],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum WifiSecurity {
    Open,
    /// Opportunistic Wireless Encryption ("Enhanced Open").
    Owe,
    Wpa2Personal,
    Wpa2Enterprise,
    Wpa2Wpa3Transition,
    Wpa3Personal,
    Wpa3Enterprise,
}

impl WifiSecurity {
    pub const ALL: [WifiSecurity; 7] = [
        WifiSecurity::Open,
        WifiSecurity::Owe,
        WifiSecurity::Wpa2Personal,
        WifiSecurity::Wpa2Enterprise,
        WifiSecurity::Wpa2Wpa3Transition,
        WifiSecurity::Wpa3Personal,
        WifiSecurity::Wpa3Enterprise,
    ];

    pub fn label(self) -> &'static str {
        match self {
            WifiSecurity::Open => "Open",
            WifiSecurity::Owe => "Enhanced Open (OWE)",
            WifiSecurity::Wpa2Personal => "WPA2-Personal",
            WifiSecurity::Wpa2Enterprise => "WPA2-Enterprise",
            WifiSecurity::Wpa2Wpa3Transition => "WPA2/WPA3 transition",
            WifiSecurity::Wpa3Personal => "WPA3-Personal (SAE)",
            WifiSecurity::Wpa3Enterprise => "WPA3-Enterprise",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Ssid {
    pub id: SsidId,
    pub name: String,
    pub vlan: Option<VlanId>,
    pub security: WifiSecurity,
    pub bands: Vec<WifiBand>,
    #[serde(default)]
    pub hidden: bool,
    #[serde(default)]
    pub notes: String,
}

impl Ssid {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            id: SsidId::new(),
            name: name.into(),
            vlan: None,
            security: WifiSecurity::Wpa3Personal,
            bands: vec![WifiBand::G2_4, WifiBand::G5],
            hidden: false,
            notes: String::new(),
        }
    }
}
