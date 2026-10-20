use serde::{Deserialize, Serialize};

use crate::geom::Rect;
use crate::ids::AreaId;

/// A coloured grouping region drawn behind devices.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AreaKind {
    Site,
    Building,
    Floor,
    Rack,
    VlanZone,
    SecurityZone,
}

impl AreaKind {
    pub const ALL: [AreaKind; 6] = [
        AreaKind::Site,
        AreaKind::Building,
        AreaKind::Floor,
        AreaKind::Rack,
        AreaKind::VlanZone,
        AreaKind::SecurityZone,
    ];

    pub fn label(self) -> &'static str {
        match self {
            AreaKind::Site => "Site",
            AreaKind::Building => "Building",
            AreaKind::Floor => "Floor",
            AreaKind::Rack => "Rack",
            AreaKind::VlanZone => "VLAN zone",
            AreaKind::SecurityZone => "Security zone",
        }
    }

    /// Default RGB colour.
    pub fn default_color(self) -> [u8; 3] {
        match self {
            AreaKind::Site => [59, 130, 246],
            AreaKind::Building => [99, 102, 241],
            AreaKind::Floor => [20, 184, 166],
            AreaKind::Rack => [100, 116, 139],
            AreaKind::VlanZone => [34, 197, 94],
            AreaKind::SecurityZone => [239, 68, 68],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Area {
    pub id: AreaId,
    pub kind: AreaKind,
    pub name: String,
    pub rect: Rect,
    pub color: [u8; 3],
    #[serde(default)]
    pub notes: String,
}

impl Area {
    pub const MIN_SIZE: f32 = 80.0;

    pub fn new(kind: AreaKind, name: impl Into<String>, rect: Rect) -> Self {
        Self {
            id: AreaId::new(),
            kind,
            name: name.into(),
            rect,
            color: kind.default_color(),
            notes: String::new(),
        }
    }
}
