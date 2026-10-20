use serde::{Deserialize, Serialize};

use crate::media::PortMedia;

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
            (Lc | Mpo, Lc | Mpo) => CableKind::Os2,
            (Rj45, Rj45) => CableKind::Cat6,
            (Rj45, _) | (_, Rj45) => CableKind::Cat6A,
            (Sfp, Sfp) => CableKind::Om3,
            _ => CableKind::Om4,
        }
    }
}

/// How far a cable can carry a given speed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Reach {
    /// Maximum channel length in metres.
    Max(f32),
    /// The cable cannot carry this speed at any length.
    Unsupported,
    /// Not length-limited in this model (wireless, logical links).
    Unlimited,
    /// Cable type not specified, or reach depends entirely on the optics.
    Unknown,
}

impl CableKind {
    /// `(speed Mbit/s, max metres)` steps, ascending by speed. A speed is
    /// supported up to the length of the first step at or above it.
    ///
    /// Copper: IEEE 802.3 BASE-T channel lengths. Multimode: 850 nm SR optics.
    /// Single-mode: 10 km LR-class optics (transceivers override this).
    pub fn reach_table(self) -> &'static [(u32, f32)] {
        use CableKind::*;
        match self {
            Cat5e => &[(1_000, 100.0), (2_500, 100.0), (5_000, 100.0)],
            Cat6 => &[(5_000, 100.0), (10_000, 55.0)],
            Cat6A | Cat7 => &[(10_000, 100.0)],
            Cat8 => &[(10_000, 100.0), (40_000, 30.0)],
            Om1 => &[(1_000, 275.0), (10_000, 33.0)],
            Om2 => &[(1_000, 550.0), (10_000, 82.0)],
            Om3 => &[(1_000, 550.0), (10_000, 300.0), (25_000, 70.0), (40_000, 100.0), (100_000, 70.0)],
            Om4 => &[(1_000, 550.0), (10_000, 400.0), (25_000, 100.0), (40_000, 150.0), (100_000, 100.0)],
            Om5 => &[(1_000, 550.0), (10_000, 400.0), (25_000, 100.0), (40_000, 150.0), (100_000, 150.0)],
            Os1 | Os2 => &[(100_000, 10_000.0)],
            Dac => &[(10_000, 7.0), (25_000, 5.0), (40_000, 7.0), (100_000, 5.0)],
            Aoc => &[(100_000, 100.0)],
            Console => &[(u32::MAX, 15.0)],
            Unspecified | Coax | Wireless | Virtual => &[],
        }
    }

    pub fn reach(self, speed_mbps: u32) -> Reach {
        match self {
            CableKind::Wireless | CableKind::Virtual => return Reach::Unlimited,
            CableKind::Unspecified | CableKind::Coax => return Reach::Unknown,
            _ => {}
        }
        // 10/100 Mbit/s runs on anything rated for 1G.
        let speed = speed_mbps.max(1);
        self.reach_table().iter().find(|(s, _)| *s >= speed).map_or(Reach::Unsupported, |(_, m)| Reach::Max(*m))
    }

    /// Highest speed the cable is rated for, if it is rated at all.
    pub fn max_speed_mbps(self) -> Option<u32> {
        self.reach_table().last().map(|(s, _)| *s).filter(|s| *s != u32::MAX)
    }

    pub fn is_fibre(self) -> bool {
        matches!(self.family(), CableFamily::Multimode | CableFamily::SingleMode)
    }

    pub fn is_twisted_pair(self) -> bool {
        matches!(self, CableKind::Cat5e | CableKind::Cat6 | CableKind::Cat6A | CableKind::Cat7 | CableKind::Cat8)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copper_reach() {
        assert_eq!(CableKind::Cat6.reach(1_000), Reach::Max(100.0));
        assert_eq!(CableKind::Cat6.reach(10_000), Reach::Max(55.0));
        assert_eq!(CableKind::Cat6A.reach(10_000), Reach::Max(100.0));
        assert_eq!(CableKind::Cat5e.reach(10_000), Reach::Unsupported);
        assert_eq!(CableKind::Cat5e.reach(100), Reach::Max(100.0));
        assert_eq!(CableKind::Cat8.reach(25_000), Reach::Max(30.0));
        assert_eq!(CableKind::Cat6.max_speed_mbps(), Some(10_000));
    }

    #[test]
    fn fibre_and_other_reach() {
        assert_eq!(CableKind::Om3.reach(10_000), Reach::Max(300.0));
        assert_eq!(CableKind::Om1.reach(25_000), Reach::Unsupported);
        assert_eq!(CableKind::Dac.reach(25_000), Reach::Max(5.0));
        assert_eq!(CableKind::Wireless.reach(10_000), Reach::Unlimited);
        assert_eq!(CableKind::Unspecified.reach(1_000), Reach::Unknown);
        assert_eq!(CableKind::Console.reach(0), Reach::Max(15.0));
        assert_eq!(CableKind::Console.max_speed_mbps(), None);
    }
}
