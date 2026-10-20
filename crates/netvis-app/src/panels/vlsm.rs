//! VLSM planner: carve an address block into right-sized subnets and create
//! them (with VLANs) in one undoable step.

use egui::{DragValue, RichText};
use netvis_core::ip::{self, IpNet};
use netvis_core::{Edit, Project, Subnet, Vlan};

use crate::icons;
use crate::state::State;
use crate::widgets::{self, ERROR, WARN};

#[derive(Debug, Clone)]
struct Row {
    name: String,
    hosts: u32,
    /// 0 = no VLAN.
    tag: u16,
}

#[derive(Debug, Clone)]
pub struct VlsmPlanner {
    parent: IpNet,
    rows: Vec<Row>,
    gateway_first: bool,
    dhcp: bool,
}

impl VlsmPlanner {
    pub fn new(p: &Project) -> Self {
        // Start from a /16 that does not collide with the existing plan.
        let parent = (0..=255u8)
            .map(|b| IpNet::new([10, b, 0, 0].into(), 16).expect("valid"))
            .find(|n| !p.subnets.iter().any(|s| ip::overlaps(&s.net, n)))
            .unwrap_or_else(|| "10.0.0.0/16".parse().expect("valid"));
        let row = |name: &str, hosts, tag| Row { name: name.into(), hosts, tag };
        Self {
            parent,
            rows: vec![row("Management", 30, 10), row("Staff", 200, 20), row("Voice", 100, 30), row("CCTV", 50, 40), row("Guest", 250, 50)],
            gateway_first: true,
            dhcp: true,
        }
    }
}

impl VlsmPlanner {
    /// Pre-filled from the VLSM calculator: `(name, hosts, vlan tag or 0)`.
    pub fn with_rows(parent: IpNet, rows: Vec<(String, u32, u16)>) -> Self {
        Self {
            parent,
            rows: rows.into_iter().map(|(name, hosts, tag)| Row { name, hosts, tag }).collect(),
            gateway_first: true,
            dhcp: true,
        }
    }
}

/// Draw the planner if open. Closes itself on Create/Cancel.
pub fn show(ctx: &egui::Context, st: &mut State) {
    let Some(mut planner) = st.vlsm.take() else { return };
    let mut keep = true;
    let mut create = false;

    let resp = egui::Modal::new(egui::Id::new("vlsm")).show(ctx, |ui| {
        ui.set_width(620.0);
        ui.heading(format!("{} VLSM planner", icons::TREE_STRUCTURE));
        ui.label(RichText::new("Largest subnets are placed first so blocks stay aligned and nothing is wasted.").small().weak());
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.label("Address block");
            widgets::net_field(ui, "vlsm.parent", &mut planner.parent, 160.0, true);
            ui.label(RichText::new(format!("{} usable", widgets::fmt_count(ip::usable_count(&planner.parent)))).small().weak());
        });
        ui.add_space(6.0);

        let hosts: Vec<u128> = planner.rows.iter().map(|r| r.hosts as u128).collect();
        let plan = ip::vlsm(&planner.parent, &hosts);
        let p = &st.doc.project;

        let mut remove = None;
        egui::Grid::new("vlsm.rows").num_columns(6).spacing([10.0, 4.0]).striped(true).show(ui, |ui| {
            for h in ["Name", "Hosts", "VLAN", "Subnet", "Usable", ""] {
                ui.label(RichText::new(h).small().weak());
            }
            ui.end_row();
            for (i, r) in planner.rows.iter_mut().enumerate() {
                ui.add(egui::TextEdit::singleline(&mut r.name).desired_width(120.0));
                ui.add(DragValue::new(&mut r.hosts).range(1..=16_000_000));
                ui.horizontal(|ui| {
                    ui.add(DragValue::new(&mut r.tag).range(0..=4094).custom_formatter(|v, _| if v == 0.0 { "—".into() } else { format!("{v}") }));
                    if r.tag > 0 {
                        let existing = p.vlans.iter().find(|v| v.tag == r.tag);
                        let (t, c) = match existing {
                            Some(v) => (format!("uses {}", v.name), ui.visuals().weak_text_color()),
                            None => ("new".to_owned(), egui::Color32::from_rgb(22, 163, 74)),
                        };
                        ui.label(RichText::new(t).small().color(c));
                    }
                });
                match &plan {
                    Ok(nets) => {
                        let net = nets[i];
                        let clash = p.subnets.iter().any(|s| ip::overlaps(&s.net, &net));
                        let text = RichText::new(net.to_string()).monospace();
                        ui.label(if clash { text.color(WARN) } else { text })
                            .on_hover_text(if clash { "Overlaps an existing subnet" } else { "" });
                        ui.label(widgets::fmt_count(ip::usable_count(&net)));
                    }
                    Err(_) => {
                        ui.label("—");
                        ui.label("");
                    }
                }
                if ui.small_button(icons::TRASH).clicked() {
                    remove = Some(i);
                }
                ui.end_row();
            }
        });
        if let Some(i) = remove {
            planner.rows.remove(i);
        }
        if ui.small_button(format!("{} Row", icons::PLUS)).clicked() {
            let tag = planner.rows.iter().map(|r| r.tag).max().unwrap_or(0) + 10;
            planner.rows.push(Row { name: format!("Subnet {}", planner.rows.len() + 1), hosts: 50, tag: tag.min(4094) });
        }

        ui.add_space(6.0);
        ui.checkbox(&mut planner.gateway_first, "Gateway on the first usable address");
        ui.checkbox(&mut planner.dhcp, "DHCP scope over the upper half of each subnet");

        let ok = plan.is_ok() && !planner.rows.is_empty();
        if let Err(e) = &plan {
            ui.label(RichText::new(format!("{} {e}", icons::WARNING)).color(ERROR));
        } else if let Ok(nets) = &plan {
            let used: u128 = nets.iter().map(ip::total_addresses).sum();
            let total = ip::total_addresses(&planner.parent);
            ui.label(RichText::new(format!("Uses {} of {} addresses ({:.1}%)", widgets::fmt_count(used), widgets::fmt_count(total), used as f64 / total as f64 * 100.0)).small().weak());
        }

        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if ui.add_enabled(ok, egui::Button::new(RichText::new(format!("Create {} subnets", planner.rows.len())).strong())).clicked() {
                create = true;
                keep = false;
            }
            if ui.button("Cancel").clicked() {
                keep = false;
            }
        });
    });
    if resp.should_close() {
        keep = false;
    }

    if create {
        apply(&planner, st);
    }
    if keep {
        st.vlsm = Some(planner);
    }
}

fn apply(planner: &VlsmPlanner, st: &mut State) {
    let hosts: Vec<u128> = planner.rows.iter().map(|r| r.hosts as u128).collect();
    let Ok(nets) = ip::vlsm(&planner.parent, &hosts) else { return };
    let p = &st.doc.project;
    let mut edits = Vec::new();
    let mut new_vlans: Vec<Vlan> = Vec::new();
    for (row, net) in planner.rows.iter().zip(nets) {
        let vlan = (row.tag > 0).then(|| {
            if let Some(v) = p.vlans.iter().chain(&new_vlans).find(|v| v.tag == row.tag) {
                return v.id;
            }
            let v = Vlan::new(row.tag, row.name.clone());
            let id = v.id;
            new_vlans.push(v);
            id
        });
        let mut s = Subnet::new(row.name.clone(), net).with_defaults(planner.gateway_first, planner.dhcp);
        s.vlan = vlan;
        edits.push(Edit::Subnet { id: s.id, before: None, after: Some(s) });
    }
    let nv = new_vlans.len();
    let ns = edits.len();
    for v in new_vlans {
        edits.insert(0, Edit::Vlan { id: v.id, before: None, after: Some(v) });
    }
    st.apply(Edit::Batch(edits), "VLSM plan");
    st.set_status(format!("Created {ns} subnet(s) and {nv} VLAN(s) from {}", planner.parent));
}
