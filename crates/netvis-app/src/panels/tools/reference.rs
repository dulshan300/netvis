//! Port reference and bill of materials.

use egui::{DragValue, RichText, Ui};
use netvis_core::catalog::ports::{PORTS, Proto};
use netvis_tools::bom::{self, BomOptions, BomSection};

use super::hint;
use crate::icons;
use crate::state::State;
use crate::widgets::{self, WARN};

#[derive(Debug, Default)]
pub struct PortsTool {
    filter: String,
    protocols: bool,
}

pub fn ports(ui: &mut Ui, st: &mut State) {
    let t = &mut st.tools.ports;
    ui.horizontal(|ui| {
        ui.add(egui::TextEdit::singleline(&mut t.filter).hint_text(format!("{} Port number or name…", icons::MAGNIFYING_GLASS)).desired_width(240.0));
        ui.checkbox(&mut t.protocols, "IP protocol numbers");
    });
    ui.add_space(4.0);
    let list: Vec<_> = PORTS.iter().filter(|p| (p.proto == Proto::Ip) == t.protocols && p.matches(&t.filter)).collect();
    if list.is_empty() {
        hint(ui, "No match in the reference list.");
        return;
    }
    egui::Grid::new("tools.ports").num_columns(4).spacing([16.0, 3.0]).striped(true).show(ui, |ui| {
        for h in [if t.protocols { "Number" } else { "Port" }, "Transport", "Service", "Use"] {
            ui.label(RichText::new(h).small().weak());
        }
        ui.end_row();
        for p in list {
            ui.label(RichText::new(p.port_label()).monospace().strong());
            ui.label(p.proto.label());
            ui.label(p.name);
            ui.horizontal(|ui| {
                ui.label(p.description);
                if p.insecure {
                    ui.label(RichText::new(format!("{} clear text / risky", icons::WARNING)).small().color(WARN))
                        .on_hover_text("Prefer the encrypted alternative or restrict to a management network");
                }
            });
            ui.end_row();
        }
    });
}

#[derive(Debug, Default)]
pub struct BomTool {
    opts: BomOptions,
}

pub fn bom(ui: &mut Ui, st: &mut State) {
    let t = &mut st.tools.bom;
    ui.horizontal(|ui| {
        let mut slack = t.opts.slack * 100.0;
        ui.label("Cable slack");
        ui.add(DragValue::new(&mut slack).range(0.0..=100.0).suffix(" %"));
        t.opts.slack = slack / 100.0;
        ui.checkbox(&mut t.opts.include_generic, "Include devices without a model");
    });
    let opts = t.opts;
    let lines = bom::build(&st.doc.project, opts);
    if lines.is_empty() {
        hint(ui, "Nothing to list yet — add devices, links and racks to the project.");
        return;
    }
    ui.horizontal(|ui| {
        if ui.button(format!("{} Copy CSV", icons::COPY)).clicked() {
            ui.ctx().copy_text(bom::to_csv(&lines));
            st.set_status("Bill of materials copied as CSV");
        }
        if ui.button(format!("{} Save CSV…", icons::DOWNLOAD_SIMPLE)).clicked() {
            crate::panels::save_csv(st, "bom", &bom::to_csv(&lines));
        }
    });
    ui.add_space(4.0);

    let mut section = None;
    egui::Grid::new("tools.bom").num_columns(4).spacing([16.0, 3.0]).striped(true).show(ui, |ui| {
        for l in &lines {
            if section != Some(l.section) {
                section = Some(l.section);
                ui.label(RichText::new(l.section.label().to_uppercase()).small().strong().weak());
                ui.label(RichText::new("Qty").small().weak());
                ui.label(RichText::new(if l.section == BomSection::Cabling { "Order" } else { "" }).small().weak());
                ui.label("");
                ui.end_row();
            }
            ui.label(&l.item);
            ui.label(RichText::new(l.qty.to_string()).monospace().strong());
            ui.label(l.length_m.map(|m| format!("{m:.0} m")).unwrap_or_default());
            ui.label(RichText::new(&l.notes).weak());
            ui.end_row();
        }
    });
    hint(ui, "Cabling counts one cable per link; links without a length need measuring before ordering.");

    // Rack space and power summary.
    let p = &st.doc.project;
    let racks: Vec<_> = p.racks().collect();
    if !racks.is_empty() {
        widgets::section(ui, "Rack space and power");
        egui::Grid::new("tools.bom.racks").num_columns(4).spacing([16.0, 3.0]).striped(true).show(ui, |ui| {
            for h in ["Rack", "Used", "Free", "Power"] {
                ui.label(RichText::new(h).small().weak());
            }
            ui.end_row();
            for r in racks {
                let used: u32 = p.rack_contents(r.id).iter().map(|m| m.height as u32).sum();
                let pw = p.rack_power(r.id);
                ui.label(&r.name);
                ui.label(format!("{used}U"));
                ui.label(format!("{}U", (r.rack_units as u32).saturating_sub(used)));
                ui.label(if pw.ups_capacity_w > 0.0 {
                    format!("{:.0} W of {:.0} W UPS", pw.load_w, pw.ups_capacity_w)
                } else {
                    format!("{:.0} W", pw.load_w)
                });
                ui.end_row();
            }
        });
    }
}
