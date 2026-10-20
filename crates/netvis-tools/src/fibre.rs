//! Fibre link loss budgets.
//!
//! Attenuation, connector and splice figures are planning values (TIA-568.3
//! maxima for connectors and splices, typical cabled-fibre attenuation).
//! Optic limits are the IEEE 802.3 maximum channel insertion loss for the
//! standard; real modules are often better, so check the datasheet when a
//! link is close.

use netvis_catalog::{CableFamily, CableKind, Transceiver, TxMedium};
use netvis_core::{LinkId, Project};

/// Mated connector pair (TIA-568.3 maximum).
pub const CONNECTOR_DB: f32 = 0.75;
/// Fusion or mechanical splice (TIA-568.3 maximum).
pub const SPLICE_DB: f32 = 0.3;

/// Cabled attenuation in dB/km at the wavelength.
pub fn attenuation_db_per_km(cable: CableKind, wavelength_nm: u16) -> Option<f32> {
    let mm = match cable.family() {
        CableFamily::Multimode => true,
        CableFamily::SingleMode => false,
        _ => return None,
    };
    Some(match (mm, wavelength_nm) {
        (true, 0..=1000) => 3.0,
        (true, _) => 1.5,
        (false, _) if cable == CableKind::Os1 => 1.0,
        (false, 0..=1400) => 0.4,
        (false, _) => 0.3,
    })
}

/// Maximum channel insertion loss for an optic on the given fibre, in dB.
pub fn optic_limit_db(t: &Transceiver, cable: CableKind) -> Option<f32> {
    if t.medium == TxMedium::Copper || t.reach_on(cable).is_none() {
        return None;
    }
    let om3 = matches!(cable, CableKind::Om1 | CableKind::Om2 | CableKind::Om3);
    Some(match t.id {
        "sfp-1g-sx" => 3.56,
        "sfp-1g-lx" => 6.0, // 1000BASE-LX10
        "sfp-1g-zx" => 23.0, // not an IEEE standard; typical module budget
        "sfpp-10g-sr" if om3 => 2.6,
        "sfpp-10g-sr" => 2.9,
        "sfpp-10g-lr" => 6.2,
        "sfpp-10g-er" => 11.0,
        "sfp28-25g-sr" | "qsfp28-100g-sr4" if om3 => 1.8,
        "sfp28-25g-sr" | "qsfp28-100g-sr4" => 1.9,
        "sfp28-25g-lr" => 6.3,
        "qsfpp-40g-sr4" if om3 => 1.9,
        "qsfpp-40g-sr4" => 1.5,
        "qsfpp-40g-lr4" => 6.7,
        "qsfp28-100g-cwdm4" => 5.0,
        "qsfp28-100g-lr4" => 6.3,
        _ => return None,
    })
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LossInput {
    pub length_m: f32,
    pub db_per_km: f32,
    pub connectors: u32,
    pub connector_db: f32,
    pub splices: u32,
    pub splice_db: f32,
    /// Safety margin reserved for repairs and ageing.
    pub margin_db: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LossBudget {
    pub fibre_db: f32,
    pub connector_db: f32,
    pub splice_db: f32,
    pub margin_db: f32,
}

impl LossBudget {
    /// Expected link loss including the safety margin.
    pub fn total_db(&self) -> f32 {
        self.fibre_db + self.connector_db + self.splice_db + self.margin_db
    }

    /// Headroom left against an optic limit (negative = over budget).
    pub fn headroom(&self, limit_db: f32) -> f32 {
        limit_db - self.total_db()
    }
}

pub fn budget(i: &LossInput) -> LossBudget {
    LossBudget {
        fibre_db: i.length_m / 1000.0 * i.db_per_km,
        connector_db: i.connectors as f32 * i.connector_db,
        splice_db: i.splices as f32 * i.splice_db,
        margin_db: i.margin_db,
    }
}

/// Longest run that stays inside `limit_db` with the same connectors,
/// splices and margin.
pub fn max_length_m(i: &LossInput, limit_db: f32) -> Option<f32> {
    if i.db_per_km <= 0.0 {
        return None;
    }
    let fixed = budget(&LossInput { length_m: 0.0, ..*i }).total_db();
    let left = limit_db - fixed;
    (left > 0.0).then(|| left / i.db_per_km * 1000.0)
}

/// A fibre link in the project with a known length and optic.
#[derive(Debug, Clone, PartialEq)]
pub struct ProjectFibre {
    pub link: LinkId,
    pub label: String,
    pub cable: CableKind,
    pub length_m: Option<f32>,
    pub optic: Option<&'static Transceiver>,
    pub budget: Option<LossBudget>,
    pub limit_db: Option<f32>,
}

/// Loss budget for every fibre link, assuming `connectors` mated pairs and
/// no splices (patch panel at each end = 2).
pub fn project_links(p: &Project, connectors: u32, margin_db: f32) -> Vec<ProjectFibre> {
    let mut out = Vec::new();
    for l in p.links.iter().filter(|l| l.cable.is_fibre()) {
        let name = |e: netvis_core::Endpoint| {
            p.device(e.device)
                .map(|d| format!("{}:{}", d.name, d.port(e.port).map(|x| x.name.as_str()).unwrap_or("?")))
                .unwrap_or_else(|| "?".into())
        };
        let optic = [l.a, l.b].iter().find_map(|e| {
            let port = p.device(e.device)?.port(e.port)?;
            netvis_catalog::transceiver::transceiver(port.transceiver.as_deref()?)
        });
        let limit_db = optic.and_then(|t| optic_limit_db(t, l.cable));
        let budget = match (l.length_m, optic.and_then(|t| attenuation_db_per_km(l.cable, t.wavelength_nm))) {
            (Some(len), Some(att)) => Some(budget(&LossInput {
                length_m: len,
                db_per_km: att,
                connectors,
                connector_db: CONNECTOR_DB,
                splices: 0,
                splice_db: SPLICE_DB,
                margin_db,
            })),
            _ => None,
        };
        out.push(ProjectFibre {
            link: l.id,
            label: format!("{} ↔ {}", name(l.a), name(l.b)),
            cable: l.cable,
            length_m: l.length_m,
            optic,
            budget,
            limit_db,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use netvis_catalog::transceiver::transceiver;

    #[test]
    fn budget_math() {
        let i = LossInput {
            length_m: 300.0,
            db_per_km: 3.0,
            connectors: 2,
            connector_db: 0.5,
            splices: 0,
            splice_db: SPLICE_DB,
            margin_db: 0.0,
        };
        let b = budget(&i);
        assert!((b.total_db() - 1.9).abs() < 1e-5);
        let sr = transceiver("sfpp-10g-sr").unwrap();
        let lim = optic_limit_db(sr, CableKind::Om3).unwrap();
        assert!((b.headroom(lim) - 0.7).abs() < 1e-5);
        assert!((max_length_m(&i, lim).unwrap() - 533.333).abs() < 0.01);
        assert_eq!(optic_limit_db(sr, CableKind::Os2), None);
        assert_eq!(attenuation_db_per_km(CableKind::Os2, 1550), Some(0.3));
        assert_eq!(attenuation_db_per_km(CableKind::Cat6, 850), None);
    }

    #[test]
    fn every_fibre_optic_has_a_limit() {
        for t in netvis_catalog::transceiver::TRANSCEIVERS.iter().filter(|t| t.medium != TxMedium::Copper) {
            for (c, _) in t.reach {
                assert!(optic_limit_db(t, *c).is_some(), "{} on {c:?}", t.id);
            }
        }
    }
}
