//! Pluggable optics and copper modules.

use crate::cable::CableKind;
use crate::media::PortMedia;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TxMedium {
    Multimode,
    SingleMode,
    Copper,
}

impl TxMedium {
    pub fn label(self) -> &'static str {
        match self {
            TxMedium::Multimode => "Multimode fibre",
            TxMedium::SingleMode => "Single-mode fibre",
            TxMedium::Copper => "Twisted pair",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Connector {
    DuplexLc,
    Mpo12,
    Rj45,
}

impl Connector {
    pub fn label(self) -> &'static str {
        match self {
            Connector::DuplexLc => "Duplex LC",
            Connector::Mpo12 => "MPO-12",
            Connector::Rj45 => "RJ45",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transceiver {
    /// Stable identifier stored in project files.
    pub id: &'static str,
    pub name: &'static str,
    /// Module form factor.
    pub form: PortMedia,
    pub speed_mbps: u32,
    pub medium: TxMedium,
    pub connector: Connector,
    /// Nominal wavelength; 0 for copper.
    pub wavelength_nm: u16,
    /// Supported cable types with their reach in metres.
    pub reach: &'static [(CableKind, f32)],
}

const MMF_1G: &[(CableKind, f32)] =
    &[(CableKind::Om1, 275.0), (CableKind::Om2, 550.0), (CableKind::Om3, 550.0), (CableKind::Om4, 550.0), (CableKind::Om5, 550.0)];
const MMF_10G: &[(CableKind, f32)] =
    &[(CableKind::Om1, 33.0), (CableKind::Om2, 82.0), (CableKind::Om3, 300.0), (CableKind::Om4, 400.0), (CableKind::Om5, 400.0)];
const MMF_25G: &[(CableKind, f32)] = &[(CableKind::Om3, 70.0), (CableKind::Om4, 100.0), (CableKind::Om5, 100.0)];
const MMF_40G: &[(CableKind, f32)] = &[(CableKind::Om3, 100.0), (CableKind::Om4, 150.0), (CableKind::Om5, 150.0)];
const MMF_100G: &[(CableKind, f32)] = &[(CableKind::Om3, 70.0), (CableKind::Om4, 100.0), (CableKind::Om5, 100.0)];
const SMF_2K: &[(CableKind, f32)] = &[(CableKind::Os1, 2_000.0), (CableKind::Os2, 2_000.0)];
const SMF_10K: &[(CableKind, f32)] = &[(CableKind::Os1, 10_000.0), (CableKind::Os2, 10_000.0)];
const SMF_40K: &[(CableKind, f32)] = &[(CableKind::Os2, 40_000.0)];
const SMF_70K: &[(CableKind, f32)] = &[(CableKind::Os2, 70_000.0)];
const CU_1G: &[(CableKind, f32)] =
    &[(CableKind::Cat5e, 100.0), (CableKind::Cat6, 100.0), (CableKind::Cat6A, 100.0), (CableKind::Cat7, 100.0), (CableKind::Cat8, 100.0)];
const CU_10G: &[(CableKind, f32)] =
    &[(CableKind::Cat6, 30.0), (CableKind::Cat6A, 30.0), (CableKind::Cat7, 30.0), (CableKind::Cat8, 30.0)];

macro_rules! tx {
    ($id:literal, $name:literal, $form:ident, $speed:expr, $medium:ident, $conn:ident, $nm:expr, $reach:expr) => {
        Transceiver {
            id: $id,
            name: $name,
            form: PortMedia::$form,
            speed_mbps: $speed,
            medium: TxMedium::$medium,
            connector: Connector::$conn,
            wavelength_nm: $nm,
            reach: $reach,
        }
    };
}

pub const TRANSCEIVERS: &[Transceiver] = &[
    tx!("sfp-1g-sx", "1000BASE-SX", Sfp, 1_000, Multimode, DuplexLc, 850, MMF_1G),
    tx!("sfp-1g-lx", "1000BASE-LX", Sfp, 1_000, SingleMode, DuplexLc, 1310, SMF_10K),
    tx!("sfp-1g-zx", "1000BASE-ZX", Sfp, 1_000, SingleMode, DuplexLc, 1550, SMF_70K),
    tx!("sfp-1g-t", "1000BASE-T SFP", Sfp, 1_000, Copper, Rj45, 0, CU_1G),
    tx!("sfpp-10g-sr", "10GBASE-SR", SfpPlus, 10_000, Multimode, DuplexLc, 850, MMF_10G),
    tx!("sfpp-10g-lr", "10GBASE-LR", SfpPlus, 10_000, SingleMode, DuplexLc, 1310, SMF_10K),
    tx!("sfpp-10g-er", "10GBASE-ER", SfpPlus, 10_000, SingleMode, DuplexLc, 1550, SMF_40K),
    tx!("sfpp-10g-t", "10GBASE-T SFP+", SfpPlus, 10_000, Copper, Rj45, 0, CU_10G),
    tx!("sfp28-25g-sr", "25GBASE-SR", Sfp28, 25_000, Multimode, DuplexLc, 850, MMF_25G),
    tx!("sfp28-25g-lr", "25GBASE-LR", Sfp28, 25_000, SingleMode, DuplexLc, 1310, SMF_10K),
    tx!("qsfpp-40g-sr4", "40GBASE-SR4", QsfpPlus, 40_000, Multimode, Mpo12, 850, MMF_40G),
    tx!("qsfpp-40g-lr4", "40GBASE-LR4", QsfpPlus, 40_000, SingleMode, DuplexLc, 1310, SMF_10K),
    tx!("qsfp28-100g-sr4", "100GBASE-SR4", Qsfp28, 100_000, Multimode, Mpo12, 850, MMF_100G),
    tx!("qsfp28-100g-cwdm4", "100GBASE-CWDM4", Qsfp28, 100_000, SingleMode, DuplexLc, 1310, SMF_2K),
    tx!("qsfp28-100g-lr4", "100GBASE-LR4", Qsfp28, 100_000, SingleMode, DuplexLc, 1310, SMF_10K),
];

pub fn transceiver(id: &str) -> Option<&'static Transceiver> {
    TRANSCEIVERS.iter().find(|t| t.id == id)
}

impl Transceiver {
    /// Reach on the given cable, or `None` if the module cannot use it.
    pub fn reach_on(&self, cable: CableKind) -> Option<f32> {
        self.reach.iter().find(|(c, _)| *c == cable).map(|(_, m)| *m)
    }

    /// Whether the module physically fits the cage. SFP modules work in
    /// SFP+/SFP28 cages, SFP+ in SFP28, and QSFP+ in QSFP28.
    pub fn fits(&self, cage: PortMedia) -> bool {
        use PortMedia::*;
        match self.form {
            Sfp => matches!(cage, Sfp | SfpPlus | Sfp28),
            SfpPlus => matches!(cage, SfpPlus | Sfp28),
            Sfp28 => cage == Sfp28,
            QsfpPlus => matches!(cage, QsfpPlus | Qsfp28),
            Qsfp28 => cage == Qsfp28,
            _ => false,
        }
    }

    /// Short summary such as `10G · MMF 850 nm · Duplex LC`.
    pub fn summary(&self) -> String {
        let medium = match self.medium {
            TxMedium::Multimode => format!("MMF {} nm", self.wavelength_nm),
            TxMedium::SingleMode => format!("SMF {} nm", self.wavelength_nm),
            TxMedium::Copper => "Copper".to_owned(),
        };
        format!("{} · {medium} · {}", crate::media::format_speed(self.speed_mbps), self.connector.label())
    }
}

pub fn for_cage(cage: PortMedia) -> impl Iterator<Item = &'static Transceiver> {
    TRANSCEIVERS.iter().filter(move |t| t.fits(cage))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_unique_and_resolvable() {
        let mut ids: Vec<_> = TRANSCEIVERS.iter().map(|t| t.id).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), TRANSCEIVERS.len());
        assert_eq!(transceiver("sfpp-10g-sr").unwrap().reach_on(CableKind::Om4), Some(400.0));
        assert_eq!(transceiver("sfpp-10g-sr").unwrap().reach_on(CableKind::Os2), None);
    }

    #[test]
    fn backwards_compatible_cages() {
        let sx = transceiver("sfp-1g-sx").unwrap();
        assert!(sx.fits(PortMedia::SfpPlus));
        let sr = transceiver("sfpp-10g-sr").unwrap();
        assert!(!sr.fits(PortMedia::Sfp));
        assert!(for_cage(PortMedia::Qsfp28).any(|t| t.id == "qsfpp-40g-sr4"));
        assert!(for_cage(PortMedia::Rj45).next().is_none());
    }
}
