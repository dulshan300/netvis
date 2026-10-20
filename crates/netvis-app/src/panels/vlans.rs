//! VLAN database and wireless networks (SSIDs).

use egui::{RichText, Ui};
use netvis_core::{Edit, Item, Ssid, Vlan};

use crate::icons;
use crate::state::State;
use crate::widgets::{self, rgb};

pub fn vlans(ui: &mut Ui, st: &mut State) {
    let mut clicked = None;
    ui.columns(2, |cols| {
        vlan_table(&mut cols[0], st, &mut clicked);
        ssid_table(&mut cols[1], st, &mut clicked);
    });
    if let Some(item) = clicked {
        st.doc.history.break_coalescing();
        st.sel.set_single(item);
    }
}

fn vlan_table(ui: &mut Ui, st: &mut State, clicked: &mut Option<Item>) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(format!("{}  VLANs ({})", icons::TAG, st.doc.project.vlans.len())).strong());
        if ui.small_button(format!("{} VLAN", icons::PLUS)).clicked() {
            let tag = st.doc.project.next_vlan_tag();
            let v = Vlan::new(tag, format!("VLAN{tag}"));
            let id = v.id;
            st.apply(Edit::Vlan { id, before: None, after: Some(v) }, "Add VLAN");
            *clicked = Some(Item::Vlan(id));
        }
    });
    let p = &st.doc.project;
    let mut vlans: Vec<_> = p.vlans.iter().collect();
    vlans.sort_by_key(|v| v.tag);
    egui::ScrollArea::vertical().id_salt("vlan-scroll").auto_shrink(false).show(ui, |ui| {
        egui::Grid::new("vlans").num_columns(4).spacing([12.0, 4.0]).striped(true).show(ui, |ui| {
            for h in ["ID", "Name", "Subnet", "Ports"] {
                ui.label(RichText::new(h).small().weak());
            }
            ui.end_row();
            for v in vlans {
                let dup = p.vlans.iter().filter(|x| x.tag == v.tag).count() > 1;
                ui.horizontal(|ui| {
                    widgets::swatch(ui, rgb(v.color));
                    let tag = RichText::new(v.tag.to_string()).monospace();
                    ui.label(if dup { tag.color(widgets::ERROR) } else { tag });
                });
                if ui.selectable_label(st.sel.contains(Item::Vlan(v.id)), &v.name).on_hover_text(&v.purpose).clicked() {
                    *clicked = Some(Item::Vlan(v.id));
                }
                let u = p.vlan_usage(v.id);
                let subnet = u.subnets.first().and_then(|s| p.subnet(*s)).map(|s| s.net.to_string());
                match subnet {
                    Some(s) => ui.label(RichText::new(s).monospace().small()),
                    None => ui.label(RichText::new("none").small().color(widgets::WARN)),
                };
                ui.label(RichText::new(format!("{} access · {} trunk", u.access_ports, u.trunk_ports)).small().weak());
                ui.end_row();
            }
        });
        if p.vlans.is_empty() {
            ui.label(RichText::new("No VLANs yet.").weak());
        }
    });
}

fn ssid_table(ui: &mut Ui, st: &mut State, clicked: &mut Option<Item>) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(format!("{}  Wireless networks ({})", icons::WIFI_HIGH, st.doc.project.ssids.len())).strong());
        if ui.small_button(format!("{} SSID", icons::PLUS)).clicked() {
            let n = st.doc.project.ssids.len() + 1;
            let s = Ssid::new(format!("Corescale-{n}"));
            let id = s.id;
            st.apply(Edit::Ssid { id, before: None, after: Some(s) }, "Add SSID");
            *clicked = Some(Item::Ssid(id));
        }
    });
    let p = &st.doc.project;
    egui::ScrollArea::vertical().id_salt("ssid-scroll").auto_shrink(false).show(ui, |ui| {
        egui::Grid::new("ssids").num_columns(4).spacing([12.0, 4.0]).striped(true).show(ui, |ui| {
            for h in ["SSID", "VLAN", "Security", "Bands"] {
                ui.label(RichText::new(h).small().weak());
            }
            ui.end_row();
            for s in &p.ssids {
                let name = if s.hidden { format!("{} (hidden)", s.name) } else { s.name.clone() };
                if ui.selectable_label(st.sel.contains(Item::Ssid(s.id)), name).clicked() {
                    *clicked = Some(Item::Ssid(s.id));
                }
                ui.label(widgets::vlan_text(&p.vlans, s.vlan));
                ui.label(RichText::new(s.security.label()).small());
                let bands: Vec<_> = s.bands.iter().map(|b| b.label()).collect();
                ui.label(RichText::new(bands.join(" · ")).small().weak());
                ui.end_row();
            }
        });
        if p.ssids.is_empty() {
            ui.label(RichText::new("No SSIDs yet.").weak());
        }
    });
}
