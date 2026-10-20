//! PoE budget planning: how much a switch must reserve, and what is actually
//! drawn once cable loss is included.

use netvis_catalog::{POE_MAX_LENGTH_M, PoeStandard};
use netvis_core::{DeviceId, Project};

#[derive(Debug, Clone, PartialEq)]
pub struct PoeRow {
    pub label: String,
    pub qty: u32,
    pub class: PoeStandard,
    /// Typical draw at the device; `None` = the class maximum.
    pub draw_w: Option<f32>,
}

impl PoeRow {
    pub fn draw(&self) -> f32 {
        self.draw_w.unwrap_or_else(|| self.class.pd_watts()).min(self.class.pd_watts())
    }
}

/// Estimated I²R cable loss for a device drawing `draw_w` over `length_m`.
/// The standard's worst case (PSE minus PD power at 100 m) is scaled by
/// length and by the square of the load.
pub fn cable_loss_w(class: PoeStandard, draw_w: f32, length_m: f32) -> f32 {
    let worst = class.pse_watts() - class.pd_watts();
    let load = (draw_w / class.pd_watts()).clamp(0.0, 1.0);
    worst * (length_m / POE_MAX_LENGTH_M).clamp(0.0, 1.0) * load * load
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct PoeTotals {
    pub ports: u32,
    /// What a switch allocating by class reserves (PSE power per class).
    pub reserved_w: f32,
    /// Expected draw at the PSE, including cable loss.
    pub drawn_w: f32,
}

pub fn totals(rows: &[PoeRow], length_m: f32) -> PoeTotals {
    rows.iter().fold(PoeTotals::default(), |mut t, r| {
        let q = r.qty as f32;
        t.ports += r.qty;
        t.reserved_w += q * r.class.pse_watts();
        t.drawn_w += q * (r.draw() + cable_loss_w(r.class, r.draw(), length_m));
        t
    })
}

/// Rows describing the powered devices currently fed by a switch.
pub fn rows_from_switch(p: &Project, switch: DeviceId) -> Vec<PoeRow> {
    let mut rows: Vec<PoeRow> = Vec::new();
    for load in p.poe_report(switch).loads {
        let Some(d) = p.device(load.pd.device) else { continue };
        let label = d.kind.label().to_owned();
        let draw = Some(load.draw_w);
        match rows.iter_mut().find(|r| r.label == label && r.class == load.pd_class && r.draw_w == draw) {
            Some(r) => r.qty += 1,
            None => rows.push(PoeRow { label, qty: 1, class: load.pd_class, draw_w: draw }),
        }
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reserve_versus_draw() {
        let rows = vec![
            PoeRow { label: "AP".into(), qty: 4, class: PoeStandard::At, draw_w: Some(18.0) },
            PoeRow { label: "Phone".into(), qty: 10, class: PoeStandard::Af, draw_w: None },
        ];
        let t = totals(&rows, 0.0);
        assert_eq!(t.ports, 14);
        assert!((t.reserved_w - (4.0 * 30.0 + 10.0 * 15.4)).abs() < 1e-3);
        assert!((t.drawn_w - (4.0 * 18.0 + 10.0 * 12.95)).abs() < 1e-3);
        // Full-load class device at 100 m loses exactly the standard's allowance.
        assert!((cable_loss_w(PoeStandard::At, 25.5, 100.0) - 4.5).abs() < 1e-4);
        assert!(totals(&rows, 50.0).drawn_w > t.drawn_w);
    }
}
