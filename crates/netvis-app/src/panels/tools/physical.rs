//! Transfer time, oversubscription, fibre loss budget and PoE budget.

use egui::{DragValue, RichText, Ui};
use netvis_core::catalog::transceiver::TRANSCEIVERS;
use netvis_core::catalog::{CableKind, PoeStandard, Transceiver, TxMedium, format_speed};
use netvis_core::{DeviceId, Item};
use netvis_tools::bandwidth::{self, RateUnit, SizeUnit};
use netvis_tools::fibre::{self, LossInput};
use netvis_tools::oversub::{self, PortGroup};
use netvis_tools::poe::{self, PoeRow};

use super::{hint, kv, kv_grid, verdict};
use crate::icons;
use crate::state::State;
use crate::widgets::{self, ERROR, OK, WARN};

const SPEEDS: [u32; 9] = [100, 1_000, 2_500, 5_000, 10_000, 25_000, 40_000, 100_000, 400_000];

fn speed_combo(ui: &mut Ui, salt: impl widgets::Salt, value: &mut u32) {
    egui::ComboBox::from_id_salt(salt).width(80.0).selected_text(format_speed(*value)).show_ui(ui, |ui| {
        for s in SPEEDS {
            ui.selectable_value(value, s, format_speed(s));
        }
    });
}

// ---- transfer time ---------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
enum Efficiency {
    Line,
    Tcp1500,
    Tcp9000,
    Custom(f64),
}

impl Efficiency {
    fn value(self) -> f64 {
        match self {
            Efficiency::Line => 1.0,
            Efficiency::Tcp1500 => bandwidth::tcp_efficiency(1500),
            Efficiency::Tcp9000 => bandwidth::tcp_efficiency(9000),
            Efficiency::Custom(f) => f,
        }
    }

    fn label(self) -> String {
        match self {
            Efficiency::Line => "Line rate (100%)".into(),
            Efficiency::Tcp1500 => format!("TCP/IPv4, 1500 MTU ({:.1}%)", self.value() * 100.0),
            Efficiency::Tcp9000 => format!("TCP/IPv4, 9000 jumbo ({:.1}%)", self.value() * 100.0),
            Efficiency::Custom(_) => "Custom".into(),
        }
    }
}

#[derive(Debug)]
pub struct BandwidthTool {
    size: f64,
    size_unit: SizeUnit,
    rate: f64,
    rate_unit: RateUnit,
    eff: Efficiency,
    window_h: f64,
}

impl Default for BandwidthTool {
    fn default() -> Self {
        Self { size: 500.0, size_unit: SizeUnit::GB, rate: 1.0, rate_unit: RateUnit::Gbps, eff: Efficiency::Tcp1500, window_h: 8.0 }
    }
}

pub fn bandwidth(ui: &mut Ui, st: &mut State) {
    let t = &mut st.tools.bandwidth;
    egui::Grid::new("tools.bw.in").num_columns(2).spacing([10.0, 6.0]).show(ui, |ui| {
        ui.label("Data size");
        ui.horizontal(|ui| {
            ui.add(DragValue::new(&mut t.size).range(0.0..=1e9).speed(1.0).max_decimals(2));
            egui::ComboBox::from_id_salt("tools.bw.su").width(60.0).selected_text(t.size_unit.label()).show_ui(ui, |ui| {
                for u in SizeUnit::ALL {
                    ui.selectable_value(&mut t.size_unit, u, u.label());
                }
            });
        });
        ui.end_row();
        ui.label("Link speed");
        ui.horizontal(|ui| {
            ui.add(DragValue::new(&mut t.rate).range(0.001..=1e6).speed(0.1).max_decimals(3));
            egui::ComboBox::from_id_salt("tools.bw.ru").width(70.0).selected_text(t.rate_unit.label()).show_ui(ui, |ui| {
                for u in RateUnit::ALL {
                    ui.selectable_value(&mut t.rate_unit, u, u.label());
                }
            });
        });
        ui.end_row();
        ui.label("Efficiency");
        ui.horizontal(|ui| {
            let mut kind = t.eff;
            egui::ComboBox::from_id_salt("tools.bw.eff").width(220.0).selected_text(kind.label()).show_ui(ui, |ui| {
                for e in [Efficiency::Line, Efficiency::Tcp1500, Efficiency::Tcp9000] {
                    let l = e.label();
                    ui.selectable_value(&mut kind, e, l);
                }
                if ui.selectable_label(matches!(kind, Efficiency::Custom(_)), "Custom").clicked() {
                    kind = Efficiency::Custom(0.8);
                }
            });
            t.eff = kind;
            if let Efficiency::Custom(f) = &mut t.eff {
                let mut pct = *f * 100.0;
                ui.add(DragValue::new(&mut pct).range(1.0..=100.0).suffix(" %"));
                *f = pct / 100.0;
            }
        });
        ui.end_row();
    });
    hint(ui, "TCP efficiency counts Ethernet preamble, header, FCS and inter-frame gap plus 40 bytes of IP/TCP headers. Real transfers also lose time to latency, loss and disk speed.");

    let bytes = t.size * t.size_unit.bytes();
    let bps = t.rate * t.rate_unit.bps();
    let eff = t.eff.value();
    ui.add_space(6.0);
    kv_grid(ui, "tools.bw.out", |ui| {
        if let Some(s) = bandwidth::transfer_secs(bytes, bps, eff) {
            kv(ui, "Transfer time", bandwidth::format_duration(s));
            kv(ui, "Goodput", bandwidth::format_bps(bps * eff));
        }
    });

    widgets::section(ui, "Same transfer on other links");
    egui::Grid::new("tools.bw.table").num_columns(2).spacing([18.0, 2.0]).striped(true).show(ui, |ui| {
        for s in SPEEDS {
            ui.label(format_speed(s));
            let secs = bandwidth::transfer_secs(bytes, s as f64 * 1e6, eff).unwrap_or(f64::INFINITY);
            ui.label(RichText::new(bandwidth::format_duration(secs)).monospace());
            ui.end_row();
        }
    });

    widgets::section(ui, "Bandwidth needed for a backup window");
    ui.horizontal(|ui| {
        ui.label("Move the data above within");
        ui.add(DragValue::new(&mut t.window_h).range(0.01..=720.0).speed(0.1).suffix(" h"));
    });
    if let Some(need) = bandwidth::required_bps(bytes, t.window_h * 3600.0, eff) {
        kv_grid(ui, "tools.bw.need", |ui| kv(ui, "Line rate needed", bandwidth::format_bps(need)));
    }
}

// ---- oversubscription ------------------------------------------------------

#[derive(Debug)]
pub struct OversubTool {
    down: Vec<PortGroup>,
    up: Vec<PortGroup>,
}

impl Default for OversubTool {
    fn default() -> Self {
        Self { down: vec![PortGroup { count: 48, mbps: 1_000 }], up: vec![PortGroup { count: 2, mbps: 10_000 }] }
    }
}

fn group_rows(ui: &mut Ui, salt: &str, rows: &mut Vec<PortGroup>) {
    let mut remove = None;
    for (i, g) in rows.iter_mut().enumerate() {
        ui.horizontal(|ui| {
            ui.add(DragValue::new(&mut g.count).range(0..=10_000).suffix(" ×"));
            speed_combo(ui, (salt, i), &mut g.mbps);
            if ui.small_button(icons::TRASH).clicked() {
                remove = Some(i);
            }
        });
    }
    if let Some(i) = remove {
        rows.remove(i);
    }
    if ui.small_button(format!("{} Ports", icons::PLUS)).clicked() {
        rows.push(PortGroup { count: 1, mbps: 1_000 });
    }
}

fn ratio_text(r: Option<f64>) -> String {
    r.map(|r| format!("{r:.1}:1")).unwrap_or_else(|| "—".into())
}

pub fn oversub(ui: &mut Ui, st: &mut State) {
    let t = &mut st.tools.oversub;
    ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            widgets::section(ui, "Edge / downlink ports");
            group_rows(ui, "down", &mut t.down);
        });
        ui.add_space(24.0);
        ui.vertical(|ui| {
            widgets::section(ui, "Uplinks");
            group_rows(ui, "up", &mut t.up);
        });
    });
    ui.add_space(6.0);
    let r = oversub::ratio(&t.down, &t.up);
    kv_grid(ui, "tools.os.out", |ui| {
        kv(ui, "Edge capacity", bandwidth::format_bps(oversub::capacity_mbps(&t.down) * 1e6));
        kv(ui, "Uplink capacity", bandwidth::format_bps(oversub::capacity_mbps(&t.up) * 1e6));
        kv(ui, "Oversubscription", ratio_text(r));
    });
    hint(ui, "Common guidance: access ≤ 20:1, distribution ≤ 4:1, core 1:1 (non-blocking).");

    widgets::section(ui, "Switches in this project");
    let report = oversub::switch_report(&st.doc.project);
    if report.is_empty() {
        hint(ui, "No switches yet.");
        return;
    }
    hint(ui, "Uplinks are links towards a higher tier (L2 → L3 → router/firewall). Click a name to select it.");
    let mut reveal = None;
    egui::Grid::new("tools.os.project").num_columns(6).spacing([16.0, 3.0]).striped(true).show(ui, |ui| {
        for h in ["Switch", "Edge ports", "Uplinks", "If all ports used", "Cabled now", "Guideline"] {
            ui.label(RichText::new(h).small().weak());
        }
        ui.end_row();
        for s in &report {
            if ui.link(&s.device.name).clicked() {
                reveal = Some(s.device.id);
            }
            ui.label(format!("{} ({})", s.edge_ports, bandwidth::format_bps(s.edge_mbps * 1e6)));
            ui.label(if s.uplink_ports > 0 {
                RichText::new(format!("{} ({})", s.uplink_ports, bandwidth::format_bps(s.uplink_mbps * 1e6)))
            } else {
                RichText::new("none found").weak()
            });
            let g = s.guideline();
            let weak = ui.visuals().weak_text_color();
            let color = |r: Option<f64>| match r {
                Some(r) if r > g * 1.5 => ERROR,
                Some(r) if r > g => WARN,
                Some(_) => OK,
                None => weak,
            };
            ui.label(RichText::new(ratio_text(s.ratio())).monospace().weak())
                .on_hover_text("Worst case: every edge port in use at line rate");
            ui.label(RichText::new(ratio_text(s.connected_ratio())).monospace().color(color(s.connected_ratio())));
            ui.label(RichText::new(format!("≤ {g:.0}:1")).weak());
            ui.end_row();
        }
    });
    if let Some(d) = reveal {
        st.reveal(Item::Device(d));
        st.actions.push(crate::state::Action::ShowCanvas);
    }
}

// ---- fibre loss ------------------------------------------------------------

#[derive(Debug)]
pub struct FibreTool {
    optic: &'static str,
    cable: CableKind,
    input: LossInput,
    /// Mated pairs assumed for project links.
    project_connectors: u32,
}

impl Default for FibreTool {
    fn default() -> Self {
        Self {
            optic: "sfpp-10g-lr",
            cable: CableKind::Os2,
            input: LossInput {
                length_m: 2_000.0,
                db_per_km: 0.4,
                connectors: 2,
                connector_db: fibre::CONNECTOR_DB,
                splices: 2,
                splice_db: fibre::SPLICE_DB,
                margin_db: 1.0,
            },
            project_connectors: 2,
        }
    }
}

fn fibre_optics() -> impl Iterator<Item = &'static Transceiver> {
    TRANSCEIVERS.iter().filter(|t| t.medium != TxMedium::Copper)
}

pub fn fibre(ui: &mut Ui, st: &mut State) {
    let t = &mut st.tools.fibre;
    let optic = netvis_core::catalog::transceiver::transceiver(t.optic).expect("catalogue id");
    egui::Grid::new("tools.fibre.in").num_columns(2).spacing([10.0, 5.0]).show(ui, |ui| {
        ui.label("Optic");
        let mut pick = t.optic;
        egui::ComboBox::from_id_salt("tools.fibre.optic").width(240.0).selected_text(format!("{} — {}", optic.name, optic.summary())).show_ui(ui, |ui| {
            for o in fibre_optics() {
                ui.selectable_value(&mut pick, o.id, format!("{} — {}", o.name, o.summary()));
            }
        });
        ui.end_row();
        let optic = netvis_core::catalog::transceiver::transceiver(pick).expect("catalogue id");
        if pick != t.optic || optic.reach_on(t.cable).is_none() {
            t.optic = pick;
            t.cable = optic.reach.iter().map(|r| r.0).max_by(|a, b| optic.reach_on(*a).partial_cmp(&optic.reach_on(*b)).expect("finite")).expect("fibre optic has reach");
            t.input.db_per_km = fibre::attenuation_db_per_km(t.cable, optic.wavelength_nm).unwrap_or(t.input.db_per_km);
        }

        ui.label("Fibre");
        let before = t.cable;
        egui::ComboBox::from_id_salt("tools.fibre.cable").width(240.0).selected_text(t.cable.label()).show_ui(ui, |ui| {
            for (c, reach) in optic.reach {
                ui.selectable_value(&mut t.cable, *c, format!("{} — up to {}", c.label(), fmt_len(*reach)));
            }
        });
        if t.cable != before {
            t.input.db_per_km = fibre::attenuation_db_per_km(t.cable, optic.wavelength_nm).unwrap_or(t.input.db_per_km);
        }
        ui.end_row();

        ui.label("Length");
        ui.add(DragValue::new(&mut t.input.length_m).range(0.0..=200_000.0).speed(5.0).suffix(" m"));
        ui.end_row();
        ui.label("Attenuation");
        ui.add(DragValue::new(&mut t.input.db_per_km).range(0.1..=10.0).speed(0.01).max_decimals(2).suffix(" dB/km"));
        ui.end_row();
        ui.label("Connector pairs");
        ui.horizontal(|ui| {
            ui.add(DragValue::new(&mut t.input.connectors).range(0..=20));
            ui.label("×");
            ui.add(DragValue::new(&mut t.input.connector_db).range(0.0..=2.0).speed(0.01).max_decimals(2).suffix(" dB"));
        });
        ui.end_row();
        ui.label("Splices");
        ui.horizontal(|ui| {
            ui.add(DragValue::new(&mut t.input.splices).range(0..=100));
            ui.label("×");
            ui.add(DragValue::new(&mut t.input.splice_db).range(0.0..=1.0).speed(0.01).max_decimals(2).suffix(" dB"));
        });
        ui.end_row();
        ui.label("Safety margin");
        ui.add(DragValue::new(&mut t.input.margin_db).range(0.0..=10.0).speed(0.1).max_decimals(1).suffix(" dB"));
        ui.end_row();
    });
    hint(ui, "Defaults: TIA-568.3 maximum connector (0.75 dB) and splice (0.3 dB) loss, typical cabled attenuation. Use measured values when you have them.");

    let optic = netvis_core::catalog::transceiver::transceiver(t.optic).expect("catalogue id");
    let b = fibre::budget(&t.input);
    let limit = fibre::optic_limit_db(optic, t.cable);
    let reach = optic.reach_on(t.cable).unwrap_or(0.0);
    ui.add_space(6.0);
    kv_grid(ui, "tools.fibre.out", |ui| {
        kv(ui, "Fibre", format!("{:.2} dB", b.fibre_db));
        kv(ui, "Connectors", format!("{:.2} dB", b.connector_db));
        kv(ui, "Splices", format!("{:.2} dB", b.splice_db));
        kv(ui, "Margin", format!("{:.2} dB", b.margin_db));
        kv(ui, "Total", format!("{:.2} dB", b.total_db()));
        if let Some(l) = limit {
            kv(ui, "Optic allows", format!("{l:.2} dB"));
            if let Some(m) = fibre::max_length_m(&t.input, l) {
                kv(ui, "Longest run", format!("{} by loss · {} by the standard", fmt_len(m), fmt_len(reach)));
            }
        }
    });
    if let Some(l) = limit {
        let head = b.headroom(l);
        let too_long = t.input.length_m > reach;
        let text = if too_long {
            format!("Longer than the {} reach of {} on {}", fmt_len(reach), optic.name, t.cable.label())
        } else if head < 0.0 {
            format!("Over budget by {:.2} dB", -head)
        } else {
            format!("Within budget — {head:.2} dB headroom")
        };
        verdict(ui, head >= 0.0 && !too_long, head < 0.5, text);
    }
    hint(ui, "Optic limits are the IEEE 802.3 maximum channel insertion loss (1000BASE-ZX: typical module budget). Check the module datasheet for marginal links.");

    // Project fibre links.
    let t = &mut st.tools.fibre;
    widgets::section(ui, "Fibre links in this project");
    ui.horizontal(|ui| {
        ui.label("Assume");
        ui.add(DragValue::new(&mut t.project_connectors).range(0..=12));
        ui.label("connector pairs per link and the margin above");
    });
    let rows = fibre::project_links(&st.doc.project, t.project_connectors, t.input.margin_db);
    if rows.is_empty() {
        hint(ui, "No fibre links yet.");
        return;
    }
    let mut reveal = None;
    egui::Grid::new("tools.fibre.project").num_columns(6).spacing([14.0, 3.0]).striped(true).show(ui, |ui| {
        for h in ["Link", "Fibre", "Length", "Optic", "Loss", "Allowed"] {
            ui.label(RichText::new(h).small().weak());
        }
        ui.end_row();
        for r in &rows {
            if ui.link(&r.label).clicked() {
                reveal = Some(r.link);
            }
            ui.label(r.cable.label());
            ui.label(r.length_m.map(fmt_len).unwrap_or_else(|| "—".into()));
            ui.label(r.optic.map(|o| o.name).unwrap_or("—"));
            match (r.budget, r.limit_db) {
                (Some(b), Some(l)) => {
                    let c = if b.total_db() > l { ERROR } else if l - b.total_db() < 0.5 { WARN } else { OK };
                    ui.label(RichText::new(format!("{:.2} dB", b.total_db())).color(c));
                    ui.label(format!("{l:.2} dB"));
                }
                _ => {
                    let why = if r.optic.is_none() { "fit an optic" } else { "set a length" };
                    ui.label(RichText::new(why).weak());
                    ui.label("");
                }
            }
            ui.end_row();
        }
    });
    if let Some(l) = reveal {
        st.reveal(Item::Link(l));
        st.actions.push(crate::state::Action::ShowCanvas);
    }
}

fn fmt_len(m: f32) -> String {
    if m >= 1000.0 { format!("{:.1} km", m / 1000.0) } else { format!("{m:.0} m") }
}

// ---- PoE budget ------------------------------------------------------------

#[derive(Debug)]
pub struct PoeTool {
    budget_w: f32,
    length_m: f32,
    rows: Vec<PoeRow>,
}

impl Default for PoeTool {
    fn default() -> Self {
        let row = |label: &str, qty, class, draw| PoeRow { label: label.into(), qty, class, draw_w: draw };
        Self {
            budget_w: 370.0,
            length_m: 50.0,
            rows: vec![
                row("Wi-Fi 6 AP", 6, PoeStandard::At, Some(18.0)),
                row("Desk phone", 16, PoeStandard::Af, Some(4.5)),
                row("Dome camera", 4, PoeStandard::Af, Some(7.0)),
            ],
        }
    }
}

pub fn poe(ui: &mut Ui, st: &mut State) {
    let p = &st.doc.project;
    let t = &mut st.tools.poe;
    let switches: Vec<(DeviceId, String, Option<f32>)> =
        p.devices.iter().filter(|d| d.hw.poe_budget_w.is_some() || !p.poe_report(d.id).loads.is_empty()).map(|d| (d.id, d.name.clone(), d.hw.poe_budget_w)).collect();
    ui.horizontal(|ui| {
        ui.label("PSE budget");
        ui.add(DragValue::new(&mut t.budget_w).range(0.0..=10_000.0).speed(1.0).suffix(" W"));
        ui.label("Average cable length");
        ui.add(DragValue::new(&mut t.length_m).range(0.0..=100.0).speed(1.0).suffix(" m"));
        if !switches.is_empty() {
            egui::ComboBox::from_id_salt("tools.poe.from").selected_text(format!("{} Load from switch", icons::DOWNLOAD_SIMPLE)).show_ui(ui, |ui| {
                for (id, name, budget) in &switches {
                    if ui.selectable_label(false, name).clicked() {
                        t.rows = poe::rows_from_switch(p, *id);
                        if let Some(b) = budget {
                            t.budget_w = *b;
                        }
                    }
                }
            });
        }
    });
    ui.add_space(4.0);

    let mut remove = None;
    egui::Grid::new("tools.poe.rows").num_columns(5).spacing([10.0, 4.0]).striped(true).show(ui, |ui| {
        for h in ["Device", "Qty", "Class", "Typical draw", ""] {
            ui.label(RichText::new(h).small().weak());
        }
        ui.end_row();
        for (i, r) in t.rows.iter_mut().enumerate() {
            ui.add_sized([140.0, 20.0], egui::TextEdit::singleline(&mut r.label));
            ui.add(DragValue::new(&mut r.qty).range(0..=1000));
            egui::ComboBox::from_id_salt(("tools.poe.class", i)).width(120.0).selected_text(r.class.label()).show_ui(ui, |ui| {
                for c in PoeStandard::ALL {
                    ui.selectable_value(&mut r.class, c, c.label());
                }
            });
            ui.horizontal(|ui| {
                let mut known = r.draw_w.is_some();
                if ui.checkbox(&mut known, "").on_hover_text("Untick to assume the class maximum").changed() {
                    r.draw_w = known.then(|| r.class.pd_watts() * 0.6);
                }
                match &mut r.draw_w {
                    Some(w) => {
                        ui.add(DragValue::new(w).range(0.1..=r.class.pd_watts()).speed(0.1).max_decimals(1).suffix(" W"));
                    }
                    None => {
                        ui.label(RichText::new(format!("max {:.1} W", r.class.pd_watts())).weak());
                    }
                }
            });
            if ui.small_button(icons::TRASH).clicked() {
                remove = Some(i);
            }
            ui.end_row();
        }
    });
    if let Some(i) = remove {
        t.rows.remove(i);
    }
    if ui.small_button(format!("{} Device type", icons::PLUS)).clicked() {
        t.rows.push(PoeRow { label: "Device".into(), qty: 1, class: PoeStandard::At, draw_w: None });
    }

    let tot = poe::totals(&t.rows, t.length_m);
    ui.add_space(6.0);
    kv_grid(ui, "tools.poe.out", |ui| {
        kv(ui, "PoE ports", tot.ports.to_string());
        kv(ui, "Reserved by class", format!("{:.0} W of {:.0} W ({:.0}%)", tot.reserved_w, t.budget_w, pct(tot.reserved_w, t.budget_w)));
        kv(ui, "Expected draw", format!("{:.0} W incl. cable loss ({:.0}%)", tot.drawn_w, pct(tot.drawn_w, t.budget_w)));
        kv(ui, "Headroom (draw)", format!("{:.0} W", t.budget_w - tot.drawn_w));
    });
    let ok = tot.drawn_w <= t.budget_w;
    let text = if !ok {
        "Expected draw exceeds the budget — split the load or use a larger PSE".to_owned()
    } else if tot.reserved_w > t.budget_w {
        "Fits by actual draw, but a switch that reserves by class will refuse some ports — use LLDP power negotiation or a bigger budget".to_owned()
    } else if tot.drawn_w > 0.8 * t.budget_w {
        "Fits, with little headroom".to_owned()
    } else {
        "Fits comfortably".to_owned()
    };
    verdict(ui, ok, tot.reserved_w > t.budget_w || tot.drawn_w > 0.8 * t.budget_w, text);

    widgets::section(ui, "IEEE 802.3 power levels");
    egui::Grid::new("tools.poe.ref").num_columns(4).spacing([16.0, 2.0]).striped(true).show(ui, |ui| {
        for h in ["Standard", "At the PSE port", "At the device (100 m)", "Pairs"] {
            ui.label(RichText::new(h).small().weak());
        }
        ui.end_row();
        for c in PoeStandard::ALL {
            ui.label(c.label());
            ui.label(format!("{:.1} W", c.pse_watts()));
            ui.label(format!("{:.2} W", c.pd_watts()));
            ui.label(if c >= PoeStandard::BtType3 { "4" } else { "2" });
            ui.end_row();
        }
    });
}

fn pct(a: f32, b: f32) -> f32 {
    if b > 0.0 { a / b * 100.0 } else { 0.0 }
}
