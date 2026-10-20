//! IT tools: calculators and reference tables. Each tool works stand-alone;
//! most also read the open project (and a few can write to it).

mod addressing;
mod physical;
mod reference;

use egui::{RichText, Ui};

use crate::icons;
use crate::state::State;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tool {
    #[default]
    Subnet,
    Vlsm,
    Address,
    Mac,
    Bandwidth,
    Oversub,
    Fibre,
    Poe,
    Ports,
    Bom,
}

impl Tool {
    const GROUPS: [(&'static str, &'static [Tool]); 3] = [
        ("Addressing", &[Tool::Subnet, Tool::Vlsm, Tool::Address, Tool::Mac]),
        ("Capacity & physical", &[Tool::Bandwidth, Tool::Oversub, Tool::Fibre, Tool::Poe]),
        ("Reference", &[Tool::Ports, Tool::Bom]),
    ];

    fn label(self) -> (&'static str, &'static str) {
        match self {
            Tool::Subnet => (icons::CALCULATOR, "Subnet calculator"),
            Tool::Vlsm => (icons::TREE_STRUCTURE, "VLSM"),
            Tool::Address => (icons::BINARY, "IPv4 / IPv6 notation"),
            Tool::Mac => (icons::FINGERPRINT, "MAC vendor lookup"),
            Tool::Bandwidth => (icons::GAUGE, "Transfer time"),
            Tool::Oversub => (icons::FUNNEL, "Oversubscription"),
            Tool::Fibre => (icons::LIGHTBULB_FILAMENT, "Fibre loss budget"),
            Tool::Poe => (icons::LIGHTNING, "PoE budget"),
            Tool::Ports => (icons::LIST_NUMBERS, "Port reference"),
            Tool::Bom => (icons::CLIPBOARD_TEXT, "Bill of materials"),
        }
    }

    #[cfg(debug_assertions)]
    pub fn from_key(key: &str) -> Option<Tool> {
        Some(match key {
            "subnet" => Tool::Subnet,
            "vlsm" => Tool::Vlsm,
            "address" => Tool::Address,
            "mac" => Tool::Mac,
            "bandwidth" => Tool::Bandwidth,
            "oversub" => Tool::Oversub,
            "fibre" => Tool::Fibre,
            "poe" => Tool::Poe,
            "ports" => Tool::Ports,
            "bom" => Tool::Bom,
            _ => return None,
        })
    }
}

/// Inputs of every tool, kept for the session so switching tools is cheap.
#[derive(Debug, Default)]
pub struct ToolsState {
    pub tool: Tool,
    subnet: addressing::SubnetTool,
    vlsm: addressing::VlsmTool,
    address: addressing::AddressTool,
    mac: addressing::MacTool,
    bandwidth: physical::BandwidthTool,
    oversub: physical::OversubTool,
    fibre: physical::FibreTool,
    poe: physical::PoeTool,
    ports: reference::PortsTool,
    bom: reference::BomTool,
}

pub fn tools(ui: &mut Ui, st: &mut State) {
    let mut tool = st.tools.tool;
    egui::Panel::left("tools.nav").resizable(false).exact_size(186.0).frame(egui::Frame::NONE).show(ui, |ui| {
        egui::ScrollArea::vertical().id_salt("tools.nav").show(ui, |ui| {
            for (group, list) in Tool::GROUPS {
                crate::widgets::section(ui, group);
                for &t in list {
                    let (icon, name) = t.label();
                    if ui.selectable_label(tool == t, format!("{icon}  {name}")).clicked() {
                        tool = t;
                    }
                }
            }
        });
    });
    st.tools.tool = tool;

    egui::CentralPanel::default().frame(egui::Frame::new().inner_margin(egui::Margin { left: 12, ..Default::default() })).show(ui, |ui| {
        egui::ScrollArea::vertical().id_salt(("tools.body", tool as u8)).auto_shrink(false).show(ui, |ui| {
            let (icon, name) = tool.label();
            ui.heading(format!("{icon}  {name}"));
            match tool {
                Tool::Subnet => addressing::subnet(ui, st),
                Tool::Vlsm => addressing::vlsm(ui, st),
                Tool::Address => addressing::address(ui, st),
                Tool::Mac => addressing::mac(ui, st),
                Tool::Bandwidth => physical::bandwidth(ui, st),
                Tool::Oversub => physical::oversub(ui, st),
                Tool::Fibre => physical::fibre(ui, st),
                Tool::Poe => physical::poe(ui, st),
                Tool::Ports => reference::ports(ui, st),
                Tool::Bom => reference::bom(ui, st),
            }
        });
    });
}

// ---- shared bits -----------------------------------------------------------

fn hint(ui: &mut Ui, text: &str) {
    ui.label(RichText::new(text).small().weak());
}

/// Key/value row in a two-column grid; the value is monospace and copies on
/// click.
fn kv(ui: &mut Ui, key: &str, value: impl Into<String>) {
    let value = value.into();
    ui.label(RichText::new(key).weak());
    let resp = ui.add(egui::Label::new(RichText::new(&value).monospace()).sense(egui::Sense::click())).on_hover_text("Click to copy");
    if resp.clicked() {
        ui.ctx().copy_text(value);
    }
    ui.end_row();
}

fn kv_grid(ui: &mut Ui, salt: &str, body: impl FnOnce(&mut Ui)) {
    egui::Grid::new(salt).num_columns(2).spacing([18.0, 4.0]).striped(true).show(ui, body);
}

/// Green / amber / red verdict line.
fn verdict(ui: &mut Ui, ok: bool, warn: bool, text: impl Into<String>) {
    let (icon, color) = if !ok {
        (icons::X_CIRCLE, crate::widgets::ERROR)
    } else if warn {
        (icons::WARNING, crate::widgets::WARN)
    } else {
        (icons::CHECK_CIRCLE, crate::widgets::OK)
    };
    ui.label(RichText::new(format!("{icon} {}", text.into())).color(color).strong());
}
