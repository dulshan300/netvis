//! IP plan: subnet tree with utilisation, plus entry points for the VLSM
//! planner and auto-assignment.

use egui::{RichText, Ui};
use netvis_core::ip;
use netvis_core::{Edit, Item, Subnet};

use crate::icons;
use crate::state::State;
use crate::widgets::{self, temp_id};

pub fn ip_plan(ui: &mut Ui, st: &mut State) {
    ui.horizontal(|ui| {
        if ui.button(format!("{} Subnet", icons::PLUS)).on_hover_text("Add a subnet").clicked() {
            let net = st.doc.project.suggest_subnet(None);
            let n = st.doc.project.subnets.len() + 1;
            let s = Subnet::new(format!("Subnet {n}"), net).with_defaults(true, false);
            let id = s.id;
            st.apply(Edit::Subnet { id, before: None, after: Some(s) }, "Add subnet");
            st.sel.set_single(Item::Subnet(id));
        }
        if ui.button(format!("{} VLSM planner…", icons::TREE_STRUCTURE)).clicked() {
            st.vlsm = Some(super::vlsm::VlsmPlanner::new(&st.doc.project));
        }
        let scope = if st.sel.devices.is_empty() { "all devices".to_owned() } else { format!("{} selected", st.sel.devices.len()) };
        if ui
            .button(format!("{} Auto-assign ({scope})", icons::LIGHTNING))
            .on_hover_text(
                "Give unaddressed host ports and VLAN interfaces the next free address \
                 in the subnet bound to their VLAN. A host's VLAN comes from the switch port it is cabled to.",
            )
            .clicked()
        {
            st.auto_assign();
        }
        ui.separator();
        let filter_id = temp_id("ipplan.filter", ());
        let mut filter: String = ui.data_mut(|d| d.get_temp(filter_id).unwrap_or_default());
        ui.add(egui::TextEdit::singleline(&mut filter).hint_text(format!("{} Filter", icons::MAGNIFYING_GLASS)).desired_width(160.0));
        ui.data_mut(|d| d.insert_temp(filter_id, filter));
    });

    if let Some((assigned, skipped)) = &st.last_assign {
        let mut clear = false;
        ui.horizontal(|ui| {
            egui::CollapsingHeader::new(
                RichText::new(format!("Last auto-assign: {} assigned, {} skipped", assigned.len(), skipped.len())).small(),
            )
            .id_salt("last-assign")
            .show(ui, |ui| {
                for a in assigned {
                    ui.label(RichText::new(format!("{} {a}", icons::CHECK_CIRCLE)).small().monospace());
                }
                for s in skipped {
                    ui.label(RichText::new(format!("{} {s}", icons::WARNING)).small().color(widgets::WARN));
                }
            });
            clear = ui.small_button(icons::X).on_hover_text("Dismiss").clicked();
        });
        if clear {
            st.last_assign = None;
        }
    }
    ui.separator();

    let filter: String = ui.data(|d| d.get_temp::<String>(temp_id("ipplan.filter", ())).unwrap_or_default()).to_lowercase();
    let p = &st.doc.project;
    if p.subnets.is_empty() {
        ui.label(RichText::new("No subnets yet. Add one, or use the VLSM planner to carve up an address block.").weak());
        return;
    }

    // Order by family, then address, then prefix so parents precede children.
    let mut rows: Vec<&Subnet> = p.subnets.iter().collect();
    rows.sort_by_key(|s| (s.net.addr().is_ipv6(), ip::to_u128(s.net.network()), s.net.prefix_len()));
    let depth = |s: &Subnet| {
        p.subnets.iter().filter(|o| o.id != s.id && o.net.prefix_len() < s.net.prefix_len() && o.net.contains(&s.net.network())).count()
    };

    let mut clicked = None;
    egui::ScrollArea::both().auto_shrink(false).show(ui, |ui| {
        egui::Grid::new("ipplan").num_columns(7).spacing([14.0, 4.0]).striped(true).show(ui, |ui| {
            for h in ["Name", "Network", "VLAN", "Gateway", "DHCP pool", "Utilisation", ""] {
                ui.label(RichText::new(h).small().weak());
            }
            ui.end_row();
            for s in rows {
                let vlan_name = s.vlan.and_then(|v| p.vlan(v)).map(|v| v.label()).unwrap_or_default();
                let hay = format!("{} {} {}", s.name, s.net, vlan_name).to_lowercase();
                if !filter.is_empty() && !hay.contains(&filter) {
                    continue;
                }
                let indent = "    ".repeat(depth(s));
                let selected = st.sel.contains(Item::Subnet(s.id));
                if ui.selectable_label(selected, format!("{indent}{}  {}", icons::TREE_STRUCTURE, s.name)).clicked() {
                    clicked = Some(s.id);
                }
                ui.label(RichText::new(s.net.to_string()).monospace());
                ui.label(widgets::vlan_text(&p.vlans, s.vlan));
                ui.label(RichText::new(s.gateway.map(|g| g.to_string()).unwrap_or("—".into())).monospace());
                ui.label(RichText::new(s.dhcp.as_ref().map(|d| format!("{} – {}", d.start, d.end)).unwrap_or("—".into())).monospace().small());
                let u = p.subnet_usage(s);
                widgets::usage_bar(ui, &u, 120.0);
                ui.label(RichText::new(format!("{:.0}%  ({} free)", u.used_fraction() * 100.0, widgets::fmt_count(u.free))).small());
                ui.end_row();
            }
        });
    });
    if let Some(id) = clicked {
        st.doc.history.break_coalescing();
        st.sel.set_single(Item::Subnet(id));
    }
}
