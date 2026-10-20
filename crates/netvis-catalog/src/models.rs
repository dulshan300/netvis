//! Vendor-neutral device models (port layout, rack size, power), loaded from
//! the bundled `data/models.json`. Edit that file to extend the library.

use std::sync::OnceLock;

use serde::Deserialize;

use crate::media::PortMedia;
use crate::poe::PoeStandard;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PoeSpec {
    Pse(PoeStandard),
    Pd(PoeStandard),
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct PortGroup {
    pub prefix: String,
    #[serde(default = "one")]
    pub start: u32,
    pub count: u32,
    pub media: PortMedia,
    /// Defaults to the media's nominal speed.
    #[serde(default)]
    pub speed: Option<u32>,
    #[serde(default)]
    pub poe: Option<PoeSpec>,
}

fn one() -> u32 {
    1
}

impl PortGroup {
    pub fn names(&self) -> impl Iterator<Item = String> + '_ {
        (self.start..self.start + self.count).map(|i| format!("{}{i}", self.prefix))
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct DeviceModel {
    pub id: String,
    pub name: String,
    /// `DeviceKind` variant name in netvis-core, e.g. `SwitchL2`.
    pub kind: String,
    pub ports: Vec<PortGroup>,
    #[serde(default)]
    pub rack_units: u8,
    /// Total PoE power the device can source.
    #[serde(default)]
    pub poe_budget_w: Option<f32>,
    /// Typical draw excluding PoE delivered to others.
    #[serde(default)]
    pub power_w: Option<f32>,
    /// Typical PoE draw when this is a powered device.
    #[serde(default)]
    pub pd_draw_w: Option<f32>,
    /// Output capacity for UPS/PDU models.
    #[serde(default)]
    pub capacity_w: Option<f32>,
    /// `WifiStandard` variant name for radios, e.g. `Wifi6E`.
    #[serde(default)]
    pub wifi: Option<String>,
}

pub fn models() -> &'static [DeviceModel] {
    static MODELS: OnceLock<Vec<DeviceModel>> = OnceLock::new();
    MODELS.get_or_init(|| {
        serde_json::from_str(include_str!("../data/models.json")).expect("bundled models.json is valid")
    })
}

pub fn model(id: &str) -> Option<&'static DeviceModel> {
    models().iter().find(|m| m.id == id)
}

pub fn models_for_kind(kind: &str) -> impl Iterator<Item = &'static DeviceModel> + '_ {
    models().iter().filter(move |m| m.kind == kind)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_models_load_and_are_unique() {
        let all = models();
        assert!(all.len() >= 20);
        let mut ids: Vec<_> = all.iter().map(|m| m.id.as_str()).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), all.len(), "duplicate model ids");
        for m in all {
            assert!(!m.ports.is_empty(), "{} has no ports", m.id);
            let has_pse = m.ports.iter().any(|g| matches!(g.poe, Some(PoeSpec::Pse(_))));
            assert_eq!(has_pse, m.poe_budget_w.is_some(), "{}: PSE ports and budget must go together", m.id);
        }
    }

    #[test]
    fn port_names() {
        let m = model("sw-access-24p-poe").unwrap();
        let names: Vec<_> = m.ports.iter().flat_map(|g| g.names()).collect();
        assert_eq!(names.len(), 28);
        assert_eq!(names[0], "ge1");
        assert_eq!(names[27], "xe4");
    }
}
