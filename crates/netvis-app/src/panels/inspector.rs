//! Context-sensitive property editor for whatever is selected.

use std::collections::HashMap;
use std::net::IpAddr;

use egui::{DragValue, RichText, Ui};
use netvis_core::ip::{self, SubnetInfo};
use netvis_core::port::format_speed;
use netvis_core::{
    AddrKind, AddrRange, AllowedVlans, AreaKind, CableKind, DeviceCategory, DhcpScope, Duplex, Edit, IfAddr, IfaceKey,
    Item, L3Config, LogicalInterface, LogicalKind, NatKind, NatRule, PoeRole, PoeStandard, Port, PortMedia, Project,
    StaticRoute, SwitchportMode, VlanId, WifiBand, WifiSecurity, WifiStandard,
};

use crate::icons;
use crate::state::{Align, Axis, State};
use netvis_core::DeviceKind;
use crate::theme;
use crate::widgets::{self, ERROR, Salt, WARN, rgb, section, temp_id};

pub fn inspector(ui: &mut Ui, st: &mut State) {
    egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| match st.sel.single() {
        Some(Item::Device(id)) => device_inspector(ui, st, id),
        Some(Item::Link(id)) => link_inspector(ui, st, id),
        Some(Item::Area(id)) => area_inspector(ui, st, id),
        Some(Item::Vlan(id)) => vlan_inspector(ui, st, id),
        Some(Item::Subnet(id)) => subnet_inspector(ui, st, id),
        Some(Item::Ssid(id)) => ssid_inspector(ui, st, id),
        None if st.sel.is_empty() => project_inspector(ui, st),
        None => multi_inspector(ui, st),
    });
}

fn heading(ui: &mut Ui, icon: &str, title: &str, subtitle: &str) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(icon).size(22.0));
        ui.vertical(|ui| {
            ui.label(RichText::new(title).strong().size(15.0));
            ui.label(RichText::new(subtitle).small().weak());
        });
    });
    ui.separator();
}

/// Validation findings that involve `item`, shown under the heading, with
/// their one-click fixes.
fn item_issues(ui: &mut Ui, st: &mut State, item: Item) {
    let mine: Vec<_> = st.issues.iter().filter(|i| i.involves(item)).cloned().collect();
    if mine.is_empty() {
        return;
    }
    let mut fix = None;
    egui::Frame::new().inner_margin(6.0).corner_radius(6.0).fill(ui.visuals().faint_bg_color).show(ui, |ui| {
        ui.set_width(ui.available_width());
        for i in mine {
            let color = match i.severity {
                netvis_validate::Severity::Error => ERROR,
                netvis_validate::Severity::Warning => WARN,
                netvis_validate::Severity::Info => ui.visuals().weak_text_color(),
            };
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new(super::severity_icon(i.severity)).color(color));
                ui.label(RichText::new(&i.message).small());
                if let Some(f) = &i.fix
                    && ui.small_button(format!("{} {}", icons::WRENCH, f.label(&st.doc.project))).clicked()
                {
                    fix = Some(f.clone());
                }
            });
        }
    });
    ui.add_space(4.0);
    if let Some(f) = fix {
        st.apply_fix(&f);
    }
}

fn form_grid(ui: &mut Ui, id: impl Salt, add: impl FnOnce(&mut Ui)) {
    egui::Grid::new(id).num_columns(2).spacing([10.0, 6.0]).show(ui, add);
}

/// Text field that reports whether it changed.
fn text_row(ui: &mut Ui, label: &str, value: &mut String) -> bool {
    ui.label(label);
    let changed = ui.add(egui::TextEdit::singleline(value).desired_width(f32::INFINITY)).changed();
    ui.end_row();
    changed
}

fn notes(ui: &mut Ui, value: &mut String) -> bool {
    ui.label("Notes");
    ui.add(egui::TextEdit::multiline(value).desired_rows(3).desired_width(f32::INFINITY)).changed()
}

fn mark(field: &mut Option<String>, name: impl Into<String>) {
    *field = Some(name.into());
}

// ---- project -------------------------------------------------------------

fn project_inspector(ui: &mut Ui, st: &mut State) {
    heading(ui, icons::FILE_PLUS, "Project", "Nothing selected — editing project details");
    let before = st.doc.project.meta.clone();
    let mut m = before.clone();
    let mut field = None;
    form_grid(ui, "meta", |ui| {
        if text_row(ui, "Name", &mut m.name) { mark(&mut field, "name") }
        if text_row(ui, "Client", &mut m.client) { mark(&mut field, "client") }
        if text_row(ui, "Author", &mut m.author) { mark(&mut field, "author") }
        if text_row(ui, "Company", &mut m.company) { mark(&mut field, "company") }
        if text_row(ui, "Revision", &mut m.revision) { mark(&mut field, "revision") }
    });
    ui.label("Description");
    if ui.add(egui::TextEdit::multiline(&mut m.description).desired_rows(3).desired_width(f32::INFINITY)).changed() {
        mark(&mut field, "description");
    }
    if let Some(f) = field.take() {
        let edit = Edit::Meta { before: Box::new(before), after: Box::new(m) };
        st.apply_coalescing(edit, "Edit project details", format!("meta.{f}"));
    }

    section(ui, "Network services");
    let before = st.doc.project.services.clone();
    let mut s = before.clone();
    form_grid(ui, "services", |ui| {
        if text_row(ui, "Domain", &mut s.domain) { mark(&mut field, "domain") }
        ui.label("DNS servers");
        if widgets::ip_list(ui, "svc.dns", &mut s.dns_servers, f32::INFINITY) { mark(&mut field, "dns") }
        ui.end_row();
        ui.label("NTP servers");
        if widgets::list_field(ui, "svc.ntp", &mut s.ntp_servers, f32::INFINITY, "pool.ntp.org, 10.0.10.5") {
            mark(&mut field, "ntp")
        }
        ui.end_row();
        if text_row(ui, "Syslog", &mut s.syslog) { mark(&mut field, "syslog") }
    });
    if notes(ui, &mut s.notes) {
        mark(&mut field, "notes");
    }
    if let Some(f) = field {
        let edit = Edit::Services { before: Box::new(before), after: Box::new(s) };
        st.apply_coalescing(edit, "Edit network services", format!("services.{f}"));
    }

    section(ui, "Summary");
    let p = &st.doc.project;
    form_grid(ui, "summary", |ui| {
        for cat in DeviceCategory::ALL {
            let n = p.devices.iter().filter(|d| d.kind.category() == cat).count();
            ui.label(format!("{}  {}", icons::category(cat), cat.label()));
            ui.label(n.to_string());
            ui.end_row();
        }
        for (icon, label, n) in [
            (icons::LINK, "Links", p.links.len()),
            (icons::RECTANGLE_DASHED, "Areas", p.areas.len()),
            (icons::TAG, "VLANs", p.vlans.len()),
            (icons::TREE_STRUCTURE, "Subnets", p.subnets.len()),
            (icons::WIFI_HIGH, "SSIDs", p.ssids.len()),
            (icons::HASH, "Addresses", p.assignments().len()),
        ] {
            ui.label(format!("{icon}  {label}"));
            ui.label(n.to_string());
            ui.end_row();
        }
    });
}

// ---- device --------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum DevTab {
    #[default]
    General,
    Ports,
    Interfaces,
    Routing,
}

/// Debug builds: preselect the device inspector section (for screenshots).
#[cfg(debug_assertions)]
pub fn debug_set_device_tab(ctx: &egui::Context, name: &str) {
    let tab = match name {
        "ports" => DevTab::Ports,
        "interfaces" => DevTab::Interfaces,
        "routing" => DevTab::Routing,
        _ => DevTab::General,
    };
    ctx.data_mut(|m| m.insert_temp(temp_id("devtab", ()), tab));
}

fn device_inspector(ui: &mut Ui, st: &mut State, id: netvis_core::DeviceId) {
    let Some(head) = st.doc.project.device(id) else { return };
    let sub = match head.primary_address() {
        Some(a) => format!("{} · {}", head.kind.label(), a.net),
        None => format!("{} · {}", head.kind.label(), head.kind.category().label()),
    };
    let (icon, name) = (icons::device(head.kind), head.name.clone());
    heading(ui, icon, &name, &sub);
    // Fixes applied here change the device, so read it afterwards.
    item_issues(ui, st, Item::Device(id));
    let Some(before) = st.doc.project.device(id).cloned() else { return };
    let mut d = before.clone();
    let mut field: Option<String> = None;

    let tab_id = temp_id("devtab", ());
    let mut tab: DevTab = ui.data(|m| m.get_temp(tab_id)).unwrap_or_default();
    ui.horizontal(|ui| {
        for (t, label) in [
            (DevTab::General, "General"),
            (DevTab::Ports, "Ports"),
            (DevTab::Interfaces, "Interfaces"),
            (DevTab::Routing, "Routing"),
        ] {
            ui.selectable_value(&mut tab, t, label);
        }
    });
    ui.separator();

    let p = &st.doc.project;
    let mut model_req: Option<String> = None;
    match tab {
        DevTab::General => {
            general_section(ui, &mut d, &mut field);
            hardware_section(ui, p, &mut d, &mut field, &mut model_req);
        }
        DevTab::Ports => {
            if let Some(key) = ports_section(ui, p, &mut d, &mut field) {
                ui.data_mut(|m| m.insert_temp(temp_id("iface", id), key));
                tab = DevTab::Interfaces;
            }
        }
        DevTab::Interfaces => interfaces_section(ui, p, &mut d, &mut field),
        DevTab::Routing => routing_section(ui, &mut d, &mut field),
    }
    ui.data_mut(|m| m.insert_temp(tab_id, tab));

    if let Some(model) = model_req
        && let Some((edit, removed)) = st.doc.project.apply_model_edit(id, &model)
    {
        st.apply(edit, "Apply device model");
        st.set_status(if removed > 0 { format!("Model applied; {removed} link(s) on removed ports were deleted") } else { "Model applied".to_owned() });
        return;
    }
    if let Some(f) = field {
        let key = format!("{id:?}.{f}");
        st.apply_coalescing(Edit::Device { id, before: Some(before), after: Some(d) }, "Edit device", key);
    }
}

fn general_section(ui: &mut Ui, d: &mut netvis_core::Device, field: &mut Option<String>) {
    form_grid(ui, "device", |ui| {
        if text_row(ui, "Hostname", &mut d.name) { mark(field, "name") }
        if text_row(ui, "Vendor", &mut d.vendor) { mark(field, "vendor") }
        if text_row(ui, "Model", &mut d.model) { mark(field, "model") }
        ui.label("DNS override");
        if widgets::ip_list(ui, ("dns", d.id), &mut d.dns, f32::INFINITY) { mark(field, "dns") }
        ui.end_row();
        ui.label("Position");
        ui.horizontal(|ui| {
            if ui.add(DragValue::new(&mut d.pos.x).prefix("x ")).changed() { mark(field, "pos") }
            if ui.add(DragValue::new(&mut d.pos.y).prefix("y ")).changed() { mark(field, "pos") }
        });
        ui.end_row();
    });
    if notes(ui, &mut d.notes) {
        mark(field, "notes");
    }

    let addrs: Vec<_> = d
        .interfaces()
        .flat_map(|i| i.l3.addresses.iter().map(move |a| (i.name.to_owned(), a.net)))
        .collect();
    if !addrs.is_empty() {
        section(ui, "Addresses");
        egui::Grid::new("dev.addrs").num_columns(2).spacing([10.0, 3.0]).show(ui, |ui| {
            for (name, net) in addrs {
                ui.label(RichText::new(name).weak());
                ui.label(RichText::new(net.to_string()).monospace());
                ui.end_row();
            }
        });
    }
}

fn cable_rating(c: CableKind) -> String {
    use netvis_core::catalog::format_speed;
    let steps: Vec<String> = c
        .reach_table()
        .iter()
        .filter(|(s, _)| *s != u32::MAX)
        .map(|(s, m)| if *m >= 1000.0 { format!("{} {:.0} km", format_speed(*s), m / 1000.0) } else { format!("{} {m:.0} m", format_speed(*s)) })
        .collect();
    match c {
        CableKind::Wireless | CableKind::Virtual => "No length limit".into(),
        CableKind::Unspecified => "Set a cable type to check reach".into(),
        CableKind::Os1 | CableKind::Os2 => "Reach set by the optics (10 km LR assumed)".into(),
        CableKind::Console => "RS-232, about 15 m".into(),
        _ if steps.is_empty() => "—".into(),
        _ => steps.join(" · "),
    }
}

fn opt_watts(ui: &mut Ui, salt: impl Salt, v: &mut Option<f32>, hint: &str) -> bool {
    ui.horizontal(|ui| {
        let changed = widgets::parsed_opt(ui, salt, v, 70.0, hint);
        ui.label(RichText::new("W").weak());
        changed
    })
    .inner
}

/// Model, rack placement and power. Sets `model_req` when the user applies a model.
fn hardware_section(
    ui: &mut Ui,
    p: &Project,
    d: &mut netvis_core::Device,
    field: &mut Option<String>,
    model_req: &mut Option<String>,
) {
    section(ui, "Hardware");
    let models: Vec<_> = d.kind.models().collect();
    let pick_id = temp_id("model-pick", d.id);
    let mut pick: Option<String> = ui.data(|m| m.get_temp(pick_id)).or_else(|| d.hw.model.clone());
    if !models.is_empty() {
        ui.horizontal(|ui| {
            let label = pick.as_deref().and_then(netvis_core::catalog::models::model).map(|m| m.name.clone()).unwrap_or("Choose a model…".into());
            egui::ComboBox::from_id_salt(("model", d.id)).width(230.0).selected_text(label).show_ui(ui, |ui| {
                for m in &models {
                    ui.selectable_value(&mut pick, Some(m.id.clone()), &m.name);
                }
            });
            let differs = pick.is_some() && pick != d.hw.model;
            if ui.add_enabled(differs, egui::Button::new("Apply")).on_hover_text("Replace ports, rack size and power figures with the model's").clicked() {
                *model_req = pick.clone();
            }
        });
        if let Some(id) = pick.as_deref()
            && pick != d.hw.model
            && let Some((_, removed)) = p.apply_model_edit(d.id, id)
            && removed > 0
        {
            ui.label(RichText::new(format!("{} Applying removes {removed} link(s) on ports the model doesn't have", icons::WARNING)).small().color(WARN));
        }
    }
    if let Some(id) = &pick {
        ui.data_mut(|m| m.insert_temp(pick_id, id.clone()));
    }

    let has_pse = d.ports.iter().any(|x| matches!(x.poe, PoeRole::Source(_)));
    let has_pd = d.ports.iter().any(|x| matches!(x.poe, PoeRole::Powered(_)));
    let racks: Vec<_> = p.racks().map(|r| (r.id, r.name.clone(), r.rack_units)).collect();
    let id = d.id;
    let kind = d.kind;
    let stp = &mut d.stp;
    let hw = &mut d.hw;
    form_grid(ui, ("hw", id), |ui| {
        ui.label("Rack height");
        if ui.add(DragValue::new(&mut hw.rack_units).range(0..=48).suffix(" U")).on_hover_text("0 = not rack-mounted").changed() {
            mark(field, "hw.units");
        }
        ui.end_row();
        ui.label("Power draw");
        if opt_watts(ui, ("pw", id), &mut hw.power_w, "typical") { mark(field, "hw.power") }
        ui.end_row();
        if has_pse {
            ui.label("PoE budget");
            if opt_watts(ui, ("budget", id), &mut hw.poe_budget_w, "total") { mark(field, "hw.budget") }
            ui.end_row();
        }
        if has_pd {
            ui.label("PoE draw");
            if opt_watts(ui, ("pd", id), &mut hw.pd_draw_w, "class max") { mark(field, "hw.pd") }
            ui.end_row();
        }
        if matches!(kind, DeviceKind::SwitchL2 | DeviceKind::SwitchL3) {
            ui.label("Spanning tree");
            egui::ComboBox::from_id_salt(("stp", id)).selected_text(stp.label()).show_ui(ui, |ui| {
                for m in netvis_core::StpMode::ALL {
                    if ui.selectable_value(stp, m, m.label()).changed() {
                        mark(field, "stp");
                    }
                }
            });
            ui.end_row();
        }
        if matches!(kind, DeviceKind::Ups | DeviceKind::Pdu) {
            ui.label("Capacity");
            if opt_watts(ui, ("cap", id), &mut hw.capacity_w, "output") { mark(field, "hw.cap") }
            ui.end_row();
        }
        if hw.rack_units > 0 {
            ui.label("Rack");
            ui.horizontal(|ui| {
                let current = hw.rack.and_then(|m| racks.iter().find(|r| r.0 == m.rack)).map(|r| r.1.clone()).unwrap_or("— not racked —".into());
                let mut rack = hw.rack.map(|m| m.rack);
                egui::ComboBox::from_id_salt(("rack", id)).width(120.0).selected_text(current).show_ui(ui, |ui| {
                    ui.selectable_value(&mut rack, None, "— not racked —");
                    for (rid, name, _) in &racks {
                        ui.selectable_value(&mut rack, Some(*rid), name);
                    }
                    if racks.is_empty() {
                        ui.label(RichText::new("Add a Rack area first").small().weak());
                    }
                });
                if rack != hw.rack.map(|m| m.rack) {
                    hw.rack = rack.map(|r| {
                        let u = p.rack_free_slot(r, hw.rack_units, true, Some(id)).unwrap_or(1);
                        netvis_core::RackMount { rack: r, u }
                    });
                    mark(field, "hw.rack");
                }
                if let Some(m) = &mut hw.rack {
                    let units = racks.iter().find(|r| r.0 == m.rack).map_or(42, |r| r.2);
                    if ui.add(DragValue::new(&mut m.u).range(1..=units).prefix("U")).on_hover_text("Lowest unit occupied").changed() {
                        mark(field, "hw.rack");
                    }
                }
            });
            ui.end_row();
        }
    });

    if has_pse {
        let r = p.poe_report(id);
        section(ui, &format!("PoE · {} of {} ports powering devices", r.loads.len(), r.pse_ports));
        if let Some(b) = r.budget_w {
            let frac = (r.demand_w / b).min(1.0);
            let color = if r.demand_w > b { ERROR } else if frac > 0.9 { WARN } else { egui::Color32::from_rgb(22, 163, 74) };
            ui.horizontal(|ui| {
                ui.add(egui::ProgressBar::new(frac).fill(color).desired_width(160.0).desired_height(10.0));
                ui.label(RichText::new(format!("{:.0} / {:.0} W  ({:.0}%)", r.demand_w, b, r.demand_w / b * 100.0)).small());
            });
        } else {
            ui.label(RichText::new(format!("{:.0} W demanded · no budget set", r.demand_w)).small().color(WARN));
        }
        egui::Grid::new(("poe", id)).num_columns(3).spacing([10.0, 2.0]).show(ui, |ui| {
            for l in &r.loads {
                let port = d.port(l.pse_port).map(|x| x.name.clone()).unwrap_or_default();
                let pd = p.device(l.pd.device).map(|x| x.name.clone()).unwrap_or_default();
                ui.label(RichText::new(port).small().monospace());
                ui.label(RichText::new(format!("{pd}  ({})", l.pd_class.short())).small());
                ui.label(RichText::new(format!("{:.1} W", l.demand_w)).small().monospace());
                ui.end_row();
            }
        });
    }
}

fn mode_text(p: &Project, m: &SwitchportMode) -> RichText {
    match m {
        SwitchportMode::Unset => RichText::new("—").weak(),
        SwitchportMode::Routed => RichText::new("L3"),
        SwitchportMode::Access { vlan } => match vlan.and_then(|v| p.vlan(v)) {
            Some(v) => RichText::new(format!("A {}", v.tag)).color(rgb(v.color)),
            None => RichText::new("A ?").color(WARN),
        },
        SwitchportMode::Trunk { .. } => RichText::new("Trunk").strong(),
    }
}

/// Returns an interface to open in the Interfaces tab, if the user asked.
fn ports_section(ui: &mut Ui, p: &Project, d: &mut netvis_core::Device, field: &mut Option<String>) -> Option<IfaceKey> {
    let used = d.ports.iter().filter(|x| p.port_link(x.id).is_some()).count();
    let mut open = None;
    ui.horizontal(|ui| {
        section(ui, &format!("Ports  {used}/{} used", d.ports.len()));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.menu_button(format!("{} Add", icons::PLUS), |ui| {
                for media in PortMedia::ALL {
                    if ui.button(media.label()).clicked() {
                        let n = d.ports.iter().filter(|x| x.media == media).count() + 1;
                        let prefix = match media {
                            PortMedia::Rj45 => "ge",
                            PortMedia::Wireless => "radio",
                            PortMedia::Virtual => "link",
                            PortMedia::Console => "con",
                            PortMedia::Coax => "coax",
                            _ => "xe",
                        };
                        d.ports.push(Port::new(format!("{prefix}{n}-new"), media));
                        mark(field, "ports.add");
                        ui.close();
                    }
                }
            });
        });
    });

    let mut remove: Option<usize> = None;
    egui::Grid::new("ports").num_columns(5).spacing([6.0, 4.0]).striped(true).show(ui, |ui| {
        for h in ["Name", "Type", "Speed", "Mode", "Connected"] {
            ui.label(RichText::new(h).small().weak());
        }
        ui.end_row();
        for (i, port) in d.ports.iter_mut().enumerate() {
            let pid = port.id;
            if ui.add(egui::TextEdit::singleline(&mut port.name).desired_width(56.0)).changed() {
                mark(field, format!("port.{i}.name"));
            }
            egui::ComboBox::from_id_salt(("media", pid)).width(58.0).selected_text(port.media.label()).show_ui(ui, |ui| {
                for m in PortMedia::ALL {
                    if ui.selectable_value(&mut port.media, m, m.label()).changed() {
                        port.speed_mbps = m.default_speed_mbps();
                        mark(field, format!("port.{i}.media"));
                    }
                }
            });
            speed_combo(ui, pid, &mut port.speed_mbps, field, i);
            let mode = egui::Button::new(mode_text(p, &port.cfg.mode)).small().frame(false);
            if ui.add(mode).on_hover_text("Configure switchport and addressing").clicked() {
                open = Some(IfaceKey::Port(pid));
            }
            match crate::widgets::run_summary(p, netvis_core::Endpoint { device: d.id, port: pid }) {
                Some(r) => {
                    let text = RichText::new(&r.far).color(theme::cable_color(r.cable.family()));
                    let w = ui.available_width().max(60.0);
                    let via = if r.via.is_empty() { String::new() } else { format!("\nvia {}", r.via) };
                    ui.add_sized([w, 18.0], egui::Label::new(text).truncate())
                        .on_hover_text(format!("{}{via}\n{}", r.far, r.cables));
                }
                None => {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("—").weak());
                        if ui.small_button(icons::TRASH).on_hover_text("Remove port").clicked() {
                            remove = Some(i);
                        }
                    });
                }
            }
            ui.end_row();
        }
    });
    if let Some(i) = remove {
        d.ports.remove(i);
        mark(field, "ports.remove");
    }
    open
}

fn speed_combo(ui: &mut Ui, salt: netvis_core::PortId, speed: &mut u32, field: &mut Option<String>, i: usize) {
    egui::ComboBox::from_id_salt(("speed", salt)).width(44.0).selected_text(format_speed(*speed)).show_ui(ui, |ui| {
        for s in [10, 100, 1_000, 2_500, 5_000, 10_000, 25_000, 40_000, 50_000, 100_000] {
            if ui.selectable_value(speed, s, format_speed(s)).changed() {
                mark(field, format!("port.{i}.speed"));
            }
        }
    });
}

fn interfaces_section(ui: &mut Ui, p: &Project, d: &mut netvis_core::Device, field: &mut Option<String>) {
    let sel_id = temp_id("iface", d.id);
    let exists = |d: &netvis_core::Device, k: IfaceKey| d.iface_name(k).is_some();
    let mut key: Option<IfaceKey> = ui
        .data(|m| m.get_temp::<IfaceKey>(sel_id))
        .filter(|k| exists(d, *k))
        .or_else(|| {
            // Open on something interesting: an addressed interface, else a cabled port.
            d.interfaces()
                .find(|i| i.l3.is_addressed())
                .map(|i| i.key)
                .or_else(|| d.ports.iter().find(|x| p.port_link(x.id).is_some()).map(|x| IfaceKey::Port(x.id)))
                .or_else(|| d.interfaces().next().map(|i| i.key))
        });

    ui.horizontal(|ui| {
        let current = key.and_then(|k| d.iface_name(k)).unwrap_or("—").to_owned();
        egui::ComboBox::from_id_salt(("iface", d.id)).width(150.0).selected_text(current).show_ui(ui, |ui| {
            for i in d.interfaces() {
                let addr = i.l3.addresses.first().map(|a| format!("  {}", a.net)).unwrap_or_default();
                let dhcp = if i.l3.dhcp_client { "  DHCP" } else { "" };
                ui.selectable_value(&mut key, Some(i.key), format!("{}{addr}{dhcp}", i.name));
            }
        });
        ui.menu_button(format!("{} Add", icons::PLUS), |ui| {
            let n = d.logical.len() + 1;
            let first_port = d.ports.first().map(|x| (x.id, x.name.clone()));
            let mut add = |l: LogicalInterface| {
                key = Some(IfaceKey::Logical(l.id));
                d.logical.push(l);
                mark(field, "logical.add");
            };
            if ui.button("VLAN interface (SVI)").clicked() {
                add(LogicalInterface::new(format!("Vlan{n}"), LogicalKind::Svi { vlan: None }));
                ui.close();
            }
            if ui.button("Loopback").clicked() {
                add(LogicalInterface::new(format!("Loopback{}", n - 1), LogicalKind::Loopback));
                ui.close();
            }
            if ui.add_enabled(first_port.is_some(), egui::Button::new("Sub-interface (802.1Q)")).clicked() {
                let (pid, pname) = first_port.clone().expect("enabled only with ports");
                add(LogicalInterface::new(format!("{pname}.{n}"), LogicalKind::SubInterface { parent: Some(pid), vlan: None }));
                ui.close();
            }
            if ui.button("Tunnel").clicked() {
                add(LogicalInterface::new(format!("Tunnel{}", n - 1), LogicalKind::Tunnel));
                ui.close();
            }
        });
    });
    ui.separator();

    match key {
        Some(IfaceKey::Port(pid)) => port_detail(ui, p, d, pid, field),
        Some(IfaceKey::Logical(lid)) => {
            if logical_detail(ui, p, d, lid, field) {
                key = None;
            }
        }
        None => {
            ui.label(RichText::new("This device has no interfaces.").weak());
        }
    }
    match key {
        Some(k) => ui.data_mut(|m| {
            m.insert_temp(sel_id, k);
        }),
        None => ui.data_mut(|m| {
            m.remove::<IfaceKey>(sel_id);
        }),
    }
}

fn port_detail(ui: &mut Ui, p: &Project, d: &mut netvis_core::Device, pid: netvis_core::PortId, field: &mut Option<String>) {
    let dev_id = d.id;
    let vlan_hint = p.untagged_vlan_at(dev_id, pid);
    let link = p.port_link(pid).cloned();
    let Some((i, port)) = d.ports.iter_mut().enumerate().find(|(_, x)| x.id == pid) else { return };
    let f = |s: &str| format!("port.{i}.{s}");

    form_grid(ui, ("port", pid), |ui| {
        if text_row(ui, "Name", &mut port.name) { mark(field, f("name")) }
        ui.label("Enabled");
        if ui.checkbox(&mut port.cfg.enabled, "").changed() { mark(field, f("enabled")) }
        ui.end_row();
        if text_row(ui, "Description", &mut port.description) { mark(field, f("desc")) }
        ui.label("Type / speed");
        ui.horizontal(|ui| {
            egui::ComboBox::from_id_salt(("pmedia", pid)).width(70.0).selected_text(port.media.label()).show_ui(ui, |ui| {
                for m in PortMedia::ALL {
                    if ui.selectable_value(&mut port.media, m, m.label()).changed() {
                        port.speed_mbps = m.default_speed_mbps();
                        if m == PortMedia::Wireless && port.cfg.radio.is_none() {
                            port.cfg.radio = Some(Default::default());
                        }
                        mark(field, f("media"));
                    }
                }
            });
            speed_combo(ui, pid, &mut port.speed_mbps, field, i);
            egui::ComboBox::from_id_salt(("duplex", pid)).width(52.0).selected_text(port.cfg.duplex.label()).show_ui(ui, |ui| {
                for x in Duplex::ALL {
                    if ui.selectable_value(&mut port.cfg.duplex, x, x.label()).changed() {
                        mark(field, f("duplex"));
                    }
                }
            });
        });
        ui.end_row();
        if port.media.is_optical_cage() {
            ui.label("Transceiver");
            let current = port.transceiver.as_deref().and_then(netvis_core::catalog::transceiver::transceiver);
            let text = match (&port.transceiver, current) {
                (None, _) => "— none / DAC —".to_owned(),
                (Some(_), Some(t)) => t.name.to_owned(),
                (Some(id), None) => format!("unknown: {id}"),
            };
            egui::ComboBox::from_id_salt(("tx", pid)).width(170.0).selected_text(text).show_ui(ui, |ui| {
                if ui.selectable_value(&mut port.transceiver, None, "— none / DAC —").changed() { mark(field, f("tx")) }
                for t in netvis_core::catalog::transceiver::for_cage(port.media) {
                    if ui.selectable_value(&mut port.transceiver, Some(t.id.to_owned()), t.name).on_hover_text(t.summary()).changed() {
                        mark(field, f("tx"));
                    }
                }
            });
            ui.end_row();
        }
        ui.label("MTU");
        if widgets::parsed_opt(ui, ("mtu", pid), &mut port.cfg.mtu, 70.0, "1500") { mark(field, f("mtu")) }
        ui.end_row();
        ui.label("LAG group").on_hover_text("Port-channel number; parallel links in the same group are one logical link");
        if widgets::parsed_opt(ui, ("lag", pid), &mut port.cfg.lag, 70.0, "none") { mark(field, f("lag")) }
        ui.end_row();
        if text_row(ui, "MAC", &mut port.cfg.mac) { mark(field, f("mac")) }
        ui.label("PoE");
        egui::ComboBox::from_id_salt(("poe", pid)).selected_text(poe_long(port.poe)).show_ui(ui, |ui| {
            let mut opts = vec![PoeRole::None];
            opts.extend(PoeStandard::ALL.map(PoeRole::Source));
            opts.extend(PoeStandard::ALL.map(PoeRole::Powered));
            for o in opts {
                if ui.selectable_value(&mut port.poe, o, poe_long(o)).changed() {
                    mark(field, f("poe"));
                }
            }
        });
        ui.end_row();
        ui.label("Connected");
        match &link {
            Some(l) => {
                let far = if l.a.port == pid { l.b } else { l.a };
                let fd = p.device(far.device);
                let fp = fd.and_then(|x| x.port(far.port));
                ui.vertical(|ui| {
                    ui.label(
                        RichText::new(format!(
                            "{}:{}",
                            fd.map(|x| x.name.as_str()).unwrap_or("?"),
                            fp.map(|x| x.name.as_str()).unwrap_or("?")
                        ))
                        .color(theme::cable_color(l.cable.family())),
                    );
                    if let Some(fp) = fp
                        && fp.cfg.mode != SwitchportMode::Unset
                    {
                        ui.label(RichText::new(format!("far end: {}", mode_summary(p, &fp.cfg.mode))).small().weak());
                    }
                });
            }
            None => {
                ui.label(RichText::new("Not connected").weak());
            }
        }
        ui.end_row();
    });

    section(ui, "Switchport");
    switchport_editor(ui, p, &mut port.cfg.mode, pid, field, &f("mode"));

    if let Some(radio) = &mut port.cfg.radio {
        section(ui, "Radio");
        radio_editor(ui, radio, pid, field, &f("radio"));
    }

    if port.cfg.mode.is_switchport() {
        ui.add_space(4.0);
        ui.label(RichText::new("Switchports carry VLANs; put the IP address on a VLAN interface (SVI).").small().weak());
    } else {
        l3_editor(ui, p, &mut port.cfg.l3, ("l3", pid), vlan_hint, field, &f("l3"));
    }
}

fn mode_summary(p: &Project, m: &SwitchportMode) -> String {
    match m {
        SwitchportMode::Unset => "host".into(),
        SwitchportMode::Routed => "routed".into(),
        SwitchportMode::Access { vlan } => format!("access {}", p.vlan_label(*vlan)),
        SwitchportMode::Trunk { native, allowed } => {
            let allowed = match allowed {
                AllowedVlans::All => "all".to_owned(),
                AllowedVlans::List(l) => {
                    l.iter().filter_map(|v| p.vlan(*v)).map(|v| v.tag.to_string()).collect::<Vec<_>>().join(",")
                }
            };
            format!("trunk, native {}, allowed {allowed}", p.vlan_label(*native))
        }
    }
}

fn switchport_editor(
    ui: &mut Ui,
    p: &Project,
    mode: &mut SwitchportMode,
    salt: netvis_core::PortId,
    field: &mut Option<String>,
    key: &str,
) {
    ui.horizontal(|ui| {
        let current = std::mem::discriminant(mode);
        for (label, template) in [
            ("Host", SwitchportMode::Unset),
            ("Access", SwitchportMode::Access { vlan: None }),
            ("Trunk", SwitchportMode::Trunk { native: None, allowed: AllowedVlans::All }),
            ("Routed", SwitchportMode::Routed),
        ] {
            let on = std::mem::discriminant(&template) == current;
            if ui.selectable_label(on, label).clicked() && !on {
                *mode = template;
                mark(field, key);
            }
        }
    });
    match mode {
        SwitchportMode::Access { vlan } => {
            form_grid(ui, ("acc", salt), |ui| {
                ui.label("VLAN");
                if widgets::vlan_combo(ui, ("acc", salt), &p.vlans, vlan, 160.0) { mark(field, key) }
                ui.end_row();
            });
        }
        SwitchportMode::Trunk { native, allowed } => {
            form_grid(ui, ("trunk", salt), |ui| {
                ui.label("Native VLAN");
                if widgets::vlan_combo(ui, ("native", salt), &p.vlans, native, 160.0) { mark(field, key) }
                ui.end_row();
                ui.label("Allowed");
                ui.horizontal(|ui| {
                    let all = matches!(allowed, AllowedVlans::All);
                    if ui.radio(all, "All").clicked() && !all {
                        *allowed = AllowedVlans::All;
                        mark(field, key);
                    }
                    if ui.radio(!all, "Selected").clicked() && all {
                        *allowed = AllowedVlans::List(p.vlans.iter().map(|v| v.id).collect());
                        mark(field, key);
                    }
                });
                ui.end_row();
            });
            if let AllowedVlans::List(list) = allowed {
                let mut vlans: Vec<_> = p.vlans.iter().collect();
                vlans.sort_by_key(|v| v.tag);
                ui.horizontal_wrapped(|ui| {
                    for v in vlans {
                        let mut on = list.contains(&v.id);
                        if ui.checkbox(&mut on, RichText::new(v.tag.to_string()).color(rgb(v.color))).on_hover_text(&v.name).changed() {
                            if on { list.push(v.id) } else { list.retain(|x| *x != v.id) }
                            mark(field, key);
                        }
                    }
                });
            }
        }
        SwitchportMode::Unset | SwitchportMode::Routed => {}
    }
}

fn radio_editor(ui: &mut Ui, r: &mut netvis_core::RadioConfig, salt: netvis_core::PortId, field: &mut Option<String>, key: &str) {
    form_grid(ui, ("radio", salt), |ui| {
        ui.label("Standard");
        egui::ComboBox::from_id_salt(("std", salt)).selected_text(r.standard.label()).show_ui(ui, |ui| {
            for s in WifiStandard::ALL {
                if ui.selectable_value(&mut r.standard, s, s.label()).changed() {
                    if !s.bands().contains(&r.band) {
                        r.band = s.bands()[0];
                        r.channel = r.band.default_channel();
                    }
                    mark(field, key);
                }
            }
        });
        ui.end_row();
        ui.label("Band");
        egui::ComboBox::from_id_salt(("band", salt)).selected_text(r.band.label()).show_ui(ui, |ui| {
            for b in r.standard.bands() {
                if ui.selectable_value(&mut r.band, *b, b.label()).changed() {
                    r.channel = b.default_channel();
                    if !b.widths().contains(&r.width_mhz) {
                        r.width_mhz = 20;
                    }
                    mark(field, key);
                }
            }
        });
        ui.end_row();
        ui.label("Channel / width");
        ui.horizontal(|ui| {
            if ui.add(DragValue::new(&mut r.channel).range(1..=233)).changed() { mark(field, key) }
            egui::ComboBox::from_id_salt(("width", salt)).width(70.0).selected_text(format!("{} MHz", r.width_mhz)).show_ui(ui, |ui| {
                for w in r.band.widths() {
                    if ui.selectable_value(&mut r.width_mhz, *w, format!("{w} MHz")).changed() {
                        mark(field, key);
                    }
                }
            });
        });
        ui.end_row();
        ui.label("TX power (dBm)");
        if widgets::parsed_opt(ui, ("txp", salt), &mut r.tx_power_dbm, 60.0, "auto") { mark(field, key) }
        ui.end_row();
    });
}

/// IP addressing for one interface, with subnet-aware hints.
fn l3_editor(
    ui: &mut Ui,
    p: &Project,
    l3: &mut L3Config,
    salt: impl Salt + Copy,
    vlan_hint: Option<VlanId>,
    field: &mut Option<String>,
    key: &str,
) {
    section(ui, "IP addressing");
    if ui.checkbox(&mut l3.dhcp_client, "Obtain address via DHCP").changed() {
        mark(field, key);
    }

    let mut counts: HashMap<IpAddr, usize> = HashMap::new();
    for a in p.assignments() {
        *counts.entry(a.addr.ip()).or_default() += 1;
    }

    let mut remove = None;
    egui::Grid::new(("addrs", salt)).num_columns(3).spacing([6.0, 4.0]).show(ui, |ui| {
        for (i, a) in l3.addresses.iter_mut().enumerate() {
            if widgets::net_field(ui, (salt, i), &mut a.net, 130.0, false) { mark(field, key) }
            egui::ComboBox::from_id_salt(("kind", salt, i)).width(80.0).selected_text(short_kind(a.kind)).show_ui(ui, |ui| {
                for k in [AddrKind::Static, AddrKind::Reservation] {
                    if ui.selectable_value(&mut a.kind, k, k.label()).changed() { mark(field, key) }
                }
            });
            if ui.small_button(icons::TRASH).on_hover_text("Remove address").clicked() {
                remove = Some(i);
            }
            ui.end_row();
            let (text, color) = address_status(p, a, counts.get(&a.ip()).copied().unwrap_or(0));
            ui.label(RichText::new(text).small().color(color));
            ui.end_row();
        }
    });
    if let Some(i) = remove {
        l3.addresses.remove(i);
        mark(field, key);
    }

    // Assign from a planned subnet.
    let pick_id = temp_id("assign-subnet", salt);
    let default = l3
        .addresses
        .first()
        .and_then(|a| p.subnet_containing(a.ip()))
        .or_else(|| vlan_hint.and_then(|v| p.subnet_for_vlan(v)))
        .or_else(|| p.subnets.iter().find(|s| p.child_subnets(s).is_empty()))
        .map(|s| s.id);
    let mut pick: Option<netvis_core::SubnetId> =
        ui.data(|m| m.get_temp(pick_id)).filter(|id: &netvis_core::SubnetId| p.subnet(*id).is_some()).or(default);
    ui.horizontal(|ui| {
        let label = pick.and_then(|id| p.subnet(id)).map(|s| format!("{} {}", s.name, s.net)).unwrap_or("No subnets".into());
        egui::ComboBox::from_id_salt(("pick", salt)).width(150.0).selected_text(label).show_ui(ui, |ui| {
            for s in &p.subnets {
                ui.selectable_value(&mut pick, Some(s.id), format!("{}  {}", s.name, s.net));
            }
        });
        let subnet = pick.and_then(|id| p.subnet(id));
        let next = subnet.and_then(|s| p.next_free_ip(s));
        let tip = next.map(|a| format!("Assign {a}")).unwrap_or("Subnet is full or not selected".into());
        if ui.add_enabled(next.is_some(), egui::Button::new(format!("{} Next free", icons::LIGHTNING))).on_hover_text(tip).clicked()
            && let (Some(s), Some(a)) = (subnet, next)
        {
            l3.addresses.push(IfAddr { net: ip::IpNet::new(a, s.net.prefix_len()).expect("valid"), kind: AddrKind::Static });
            if l3.gateway.is_none() {
                l3.gateway = s.gateway.filter(|g| *g != a);
            }
            mark(field, key);
        }
        if ui.button(icons::PLUS).on_hover_text("Add an address manually").clicked() {
            let net = subnet.map(|s| s.net).unwrap_or("192.168.1.10/24".parse().expect("valid"));
            l3.addresses.push(IfAddr { net, kind: AddrKind::Static });
            mark(field, key);
        }
    });
    ui.data_mut(|m| match pick {
        Some(id) => {
            m.insert_temp(pick_id, id);
        }
        None => {
            m.remove::<netvis_core::SubnetId>(pick_id);
        }
    });

    form_grid(ui, ("gw", salt), |ui| {
        ui.label("Default gateway");
        ui.horizontal(|ui| {
            if widgets::parsed_opt(ui, ("gw", salt), &mut l3.gateway, 130.0, "none") { mark(field, key) }
            let suggestion = l3
                .addresses
                .first()
                .and_then(|a| p.subnet_containing(a.ip()))
                .and_then(|s| s.gateway)
                .filter(|g| Some(*g) != l3.gateway && l3.addresses.iter().all(|a| a.ip() != *g));
            if let Some(g) = suggestion
                && ui.small_button(format!("Use {g}")).clicked()
            {
                l3.gateway = Some(g);
                mark(field, key);
            }
        });
        ui.end_row();
    });
}

fn short_kind(k: AddrKind) -> &'static str {
    match k {
        AddrKind::Static => "Static",
        AddrKind::Reservation => "Reserved",
    }
}

fn address_status(p: &Project, a: &IfAddr, uses: usize) -> (String, egui::Color32) {
    let addr = a.ip();
    if uses > 1 {
        return (format!("{} Duplicate — used {uses}×", icons::WARNING), ERROR);
    }
    let Some(s) = p.subnet_containing(addr) else {
        return (format!("{} Not in any planned subnet", icons::WARNING), WARN);
    };
    if !ip::is_usable_host(&s.net, addr) {
        return (format!("{} Network/broadcast address of {}", icons::WARNING, s.net), ERROR);
    }
    if a.net.prefix_len() != s.net.prefix_len() {
        return (format!("{} Prefix /{} differs from {} ({})", icons::WARNING, a.net.prefix_len(), s.name, s.net), WARN);
    }
    if a.kind == AddrKind::Static && s.dhcp.as_ref().is_some_and(|d| d.range().contains(addr)) {
        return (format!("{} Inside the DHCP pool of {}", icons::WARNING, s.name), WARN);
    }
    let role = if s.gateway == Some(addr) { " · gateway" } else { "" };
    (format!("{} {}{role}", icons::CHECK_CIRCLE, s.name), egui::Color32::from_rgb(22, 163, 74))
}

/// Returns true if the interface was deleted.
fn logical_detail(
    ui: &mut Ui,
    p: &Project,
    d: &mut netvis_core::Device,
    lid: netvis_core::LogicalId,
    field: &mut Option<String>,
) -> bool {
    let ports: Vec<_> = d.ports.iter().map(|x| (x.id, x.name.clone())).collect();
    let Some((i, l)) = d.logical.iter_mut().enumerate().find(|(_, x)| x.id == lid) else { return false };
    let key = format!("logical.{i}");
    let mut delete = false;

    form_grid(ui, ("logical", lid), |ui| {
        ui.label("Kind");
        ui.label(l.kind.label());
        ui.end_row();
        if text_row(ui, "Name", &mut l.name) { mark(field, &key) }
        ui.label("Enabled");
        if ui.checkbox(&mut l.enabled, "").changed() { mark(field, &key) }
        ui.end_row();
        if text_row(ui, "Description", &mut l.description) { mark(field, &key) }
        match &mut l.kind {
            LogicalKind::Svi { vlan } => {
                ui.label("VLAN");
                if widgets::vlan_combo(ui, ("svi", lid), &p.vlans, vlan, 160.0) { mark(field, &key) }
                ui.end_row();
            }
            LogicalKind::SubInterface { parent, vlan } => {
                ui.label("Parent port");
                let current = parent.and_then(|pid| ports.iter().find(|(id, _)| *id == pid)).map(|(_, n)| n.clone());
                egui::ComboBox::from_id_salt(("parent", lid)).selected_text(current.unwrap_or("—".into())).show_ui(ui, |ui| {
                    for (pid, name) in &ports {
                        if ui.selectable_value(parent, Some(*pid), name).changed() { mark(field, &key) }
                    }
                });
                ui.end_row();
                ui.label("802.1Q VLAN");
                if widgets::vlan_combo(ui, ("subif", lid), &p.vlans, vlan, 160.0) { mark(field, &key) }
                ui.end_row();
            }
            LogicalKind::Loopback | LogicalKind::Tunnel => {}
        }
        ui.label("MTU");
        if widgets::parsed_opt(ui, ("lmtu", lid), &mut l.mtu, 70.0, "default") { mark(field, &key) }
        ui.end_row();
    });
    let hint = l.kind.vlan();
    l3_editor(ui, p, &mut l.l3, ("ll3", lid), hint, field, &key);

    ui.add_space(8.0);
    if ui.button(format!("{} Delete interface", icons::TRASH)).clicked() {
        delete = true;
    }
    if delete {
        d.logical.retain(|x| x.id != lid);
        mark(field, "logical.remove");
    }
    delete
}

fn routing_section(ui: &mut Ui, d: &mut netvis_core::Device, field: &mut Option<String>) {
    let id = d.id;
    let r = &mut d.routing;
    ui.horizontal(|ui| {
        section(ui, "Static routes");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.small_button(format!("{} Route", icons::PLUS)).clicked() {
                r.static_routes.push(StaticRoute::default());
                mark(field, "routes.add");
            }
        });
    });
    let mut remove = None;
    if r.static_routes.is_empty() {
        ui.label(RichText::new("No static routes").weak().small());
    }
    for (i, route) in r.static_routes.iter_mut().enumerate() {
        egui::Frame::group(ui.style()).show(ui, |ui| {
            egui::Grid::new(("route", id, i)).num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
                ui.label("Destination");
                ui.horizontal(|ui| {
                    if widgets::net_field(ui, ("rdst", id, i), &mut route.dest, 130.0, true) { mark(field, format!("route.{i}")) }
                    if ui.small_button(icons::TRASH).clicked() {
                        remove = Some(i);
                    }
                });
                ui.end_row();
                ui.label("Next hop");
                if widgets::parsed_opt(ui, ("rnh", id, i), &mut route.next_hop, 130.0, "10.0.0.1") { mark(field, format!("route.{i}")) }
                ui.end_row();
                ui.label("Interface");
                if ui.add(egui::TextEdit::singleline(&mut route.interface).desired_width(130.0).hint_text("optional")).changed() {
                    mark(field, format!("route.{i}"));
                }
                ui.end_row();
                ui.label("Distance");
                if ui.add(DragValue::new(&mut route.distance).range(1..=255)).changed() { mark(field, format!("route.{i}")) }
                ui.end_row();
                if text_row(ui, "Description", &mut route.description) { mark(field, format!("route.{i}")) }
            });
        });
    }
    if let Some(i) = remove {
        r.static_routes.remove(i);
        mark(field, "routes.remove");
    }

    ui.horizontal(|ui| {
        section(ui, "NAT rules");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.small_button(format!("{} Rule", icons::PLUS)).clicked() {
                r.nat.push(NatRule::default());
                mark(field, "nat.add");
            }
        });
    });
    if r.nat.is_empty() {
        ui.label(RichText::new("No NAT rules").weak().small());
    }
    let mut remove = None;
    for (i, n) in r.nat.iter_mut().enumerate() {
        egui::Frame::group(ui.style()).show(ui, |ui| {
            egui::Grid::new(("nat", id, i)).num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
                ui.label("Type");
                ui.horizontal(|ui| {
                    egui::ComboBox::from_id_salt(("natk", id, i)).selected_text(n.kind.label()).show_ui(ui, |ui| {
                        for k in NatKind::ALL {
                            if ui.selectable_value(&mut n.kind, k, k.label()).changed() { mark(field, format!("nat.{i}")) }
                        }
                    });
                    if ui.small_button(icons::TRASH).clicked() {
                        remove = Some(i);
                    }
                });
                ui.end_row();
                if text_row(ui, "Inside", &mut n.inside) { mark(field, format!("nat.{i}")) }
                if text_row(ui, "Outside", &mut n.outside) { mark(field, format!("nat.{i}")) }
                if text_row(ui, "Service", &mut n.service) { mark(field, format!("nat.{i}")) }
                if text_row(ui, "Description", &mut n.description) { mark(field, format!("nat.{i}")) }
            });
        });
    }
    if let Some(i) = remove {
        r.nat.remove(i);
        mark(field, "nat.remove");
    }

    section(ui, "Dynamic routing notes");
    if ui
        .add(egui::TextEdit::multiline(&mut r.dynamic).desired_rows(4).desired_width(f32::INFINITY).hint_text("OSPF area 0: 10.0.0.0/16 …\nBGP AS 65001, peer 203.0.113.1 …"))
        .changed()
    {
        mark(field, "dynamic");
    }
}

fn poe_long(r: PoeRole) -> String {
    match r {
        PoeRole::None => "No PoE".into(),
        PoeRole::Source(s) => format!("Source · {} ({} W)", s.label(), s.pse_watts()),
        PoeRole::Powered(s) => format!("Powered · {}", s.label()),
    }
}

// ---- link / area / multi -------------------------------------------------

fn link_inspector(ui: &mut Ui, st: &mut State, id: netvis_core::LinkId) {
    heading(ui, icons::LINK, "Link", st.doc.project.link(id).map_or("", |l| l.cable.label()));
    item_issues(ui, st, Item::Link(id));
    let p = &st.doc.project;
    let Some(before) = p.link(id).cloned() else { return };
    let mut l = before.clone();
    let mut field = None;
    let end = |ep: netvis_core::Endpoint| {
        p.device(ep.device)
            .map(|d| {
                let port = d.port(ep.port);
                format!(
                    "{}:{}  ({} {})",
                    d.name,
                    port.map(|x| x.name.as_str()).unwrap_or("?"),
                    port.map(|x| x.media.label()).unwrap_or(""),
                    port.map(|x| format_speed(x.speed_mbps)).unwrap_or_default(),
                )
            })
            .unwrap_or_else(|| "?".into())
    };
    let (a, b) = (end(l.a), end(l.b));

    form_grid(ui, "link", |ui| {
        ui.label("End A");
        ui.label(a);
        ui.end_row();
        ui.label("End B");
        ui.label(b);
        ui.end_row();
        ui.label("Cable");
        egui::ComboBox::from_id_salt("cable").selected_text(l.cable.label()).show_ui(ui, |ui| {
            for c in CableKind::ALL {
                let text = RichText::new(c.label()).color(theme::cable_color(c.family()));
                if ui.selectable_value(&mut l.cable, c, text).changed() {
                    mark(&mut field, "cable");
                }
            }
        });
        ui.end_row();
        ui.label("Length");
        ui.horizontal(|ui| {
            let mut has = l.length_m.is_some();
            if ui.checkbox(&mut has, "").changed() {
                l.length_m = has.then_some(l.length_m.unwrap_or(3.0));
                mark(&mut field, "length");
            }
            if let Some(m) = &mut l.length_m
                && ui.add(DragValue::new(m).suffix(" m").range(0.1..=100_000.0).speed(0.5)).changed()
            {
                mark(&mut field, "length");
            }
        });
        ui.end_row();
        if text_row(ui, "Label", &mut l.label) {
            mark(&mut field, "label");
        }
        ui.label("Rated");
        ui.label(RichText::new(cable_rating(l.cable)).small());
        ui.end_row();
        for (end, ep) in [("Module A", l.a), ("Module B", l.b)] {
            let tx = p.device(ep.device).and_then(|d| d.port(ep.port)).and_then(|x| x.transceiver.as_deref());
            if let Some(tx) = tx {
                ui.label(end);
                let t = netvis_core::catalog::transceiver::transceiver(tx);
                ui.label(RichText::new(t.map(|t| format!("{} — {}", t.name, t.summary())).unwrap_or_else(|| tx.to_owned())).small());
                ui.end_row();
            }
        }
        // VLANs carried, derived from the two port configs.
        let modes: Vec<String> = [l.a, l.b]
            .iter()
            .filter_map(|ep| p.device(ep.device).and_then(|d| d.port(ep.port)).map(|x| (x, ep)))
            .filter(|(x, _)| x.cfg.mode != SwitchportMode::Unset)
            .map(|(x, ep)| format!("{}: {}", p.device(ep.device).map(|d| d.name.as_str()).unwrap_or("?"), mode_summary(p, &x.cfg.mode)))
            .collect();
        if !modes.is_empty() {
            ui.label("Switching");
            ui.vertical(|ui| {
                for m in modes {
                    ui.label(RichText::new(m).small());
                }
            });
            ui.end_row();
        }
    });
    if let Some(f) = field {
        st.apply_coalescing(Edit::Link { id, before: Some(before), after: Some(l) }, "Edit link", format!("{id:?}.{f}"));
    }
}

fn area_inspector(ui: &mut Ui, st: &mut State, id: netvis_core::AreaId) {
    let Some(head) = st.doc.project.area(id) else { return };
    let (icon, name, kind) = (icons::area(head.kind), head.name.clone(), head.kind.label());
    heading(ui, icon, &name, kind);
    item_issues(ui, st, Item::Area(id));
    let Some(before) = st.doc.project.area(id).cloned() else { return };
    let mut a = before.clone();
    let mut field = None;
    let mut auto_place = false;
    form_grid(ui, "area", |ui| {
        if text_row(ui, "Name", &mut a.name) {
            mark(&mut field, "name");
        }
        ui.label("Kind");
        egui::ComboBox::from_id_salt("area-kind").selected_text(a.kind.label()).show_ui(ui, |ui| {
            for k in AreaKind::ALL {
                if ui.selectable_value(&mut a.kind, k, k.label()).changed() {
                    mark(&mut field, "kind");
                }
            }
        });
        ui.end_row();
        ui.label("Colour");
        if ui.color_edit_button_srgb(&mut a.color).changed() {
            mark(&mut field, "color");
        }
        ui.end_row();
        ui.label("Position");
        ui.horizontal(|ui| {
            if ui.add(DragValue::new(&mut a.rect.min.x).prefix("x ")).changed() { mark(&mut field, "rect") }
            if ui.add(DragValue::new(&mut a.rect.min.y).prefix("y ")).changed() { mark(&mut field, "rect") }
        });
        ui.end_row();
        ui.label("Size");
        ui.horizontal(|ui| {
            let min = netvis_core::Area::MIN_SIZE..=100_000.0;
            if ui.add(DragValue::new(&mut a.rect.size.x).prefix("w ").range(min.clone())).changed() { mark(&mut field, "rect") }
            if ui.add(DragValue::new(&mut a.rect.size.y).prefix("h ").range(min)).changed() { mark(&mut field, "rect") }
        });
        ui.end_row();
        if a.kind == AreaKind::Rack {
            ui.label("Rack height");
            ui.horizontal(|ui| {
                if ui.add(DragValue::new(&mut a.rack_units).range(1..=60).suffix(" U")).changed() { mark(&mut field, "units") }
                if ui.button("Auto-place devices").on_hover_text("Place rack-mountable devices drawn inside this area").clicked() {
                    auto_place = true;
                }
            });
            ui.end_row();
        }
    });
    if notes(ui, &mut a.notes) {
        mark(&mut field, "notes");
    }
    if let Some(f) = field {
        st.apply_coalescing(Edit::Area { id, before: Some(before), after: Some(a) }, "Edit area", format!("{id:?}.{f}"));
    }
    if auto_place {
        let (edit, n, no_room) = st.doc.project.auto_mount_edit(id);
        st.apply(edit, "Auto-place in rack");
        st.set_status(if no_room.is_empty() { format!("Placed {n} device(s)") } else { format!("Placed {n}; no room for {}", no_room.join(", ")) });
    }
}

fn multi_inspector(ui: &mut Ui, st: &mut State) {
    let s = &st.sel;
    heading(
        ui,
        icons::SELECTION,
        &format!("{} items selected", s.len()),
        &format!("{} devices · {} links · {} areas", s.devices.len(), s.links.len(), s.areas.len()),
    );
    let n = s.devices.len();
    section(ui, "Align");
    ui.add_enabled_ui(n >= 2, |ui| {
        ui.horizontal_wrapped(|ui| {
            for (icon, tip, how) in [
                (icons::ALIGN_LEFT, "Align left", Align::Left),
                (icons::ALIGN_CENTER_HORIZONTAL, "Align centres horizontally", Align::CenterX),
                (icons::ALIGN_RIGHT, "Align right", Align::Right),
                (icons::ALIGN_TOP, "Align top", Align::Top),
                (icons::ALIGN_CENTER_VERTICAL, "Align centres vertically", Align::CenterY),
                (icons::ALIGN_BOTTOM, "Align bottom", Align::Bottom),
            ] {
                if ui.button(RichText::new(icon).size(16.0)).on_hover_text(tip).clicked() {
                    st.align(how);
                }
            }
        });
    });
    section(ui, "Distribute");
    ui.add_enabled_ui(n >= 2, |ui| {
        ui.horizontal(|ui| {
            if ui.button(format!("{} Horizontally", icons::ARROWS_HORIZONTAL)).on_hover_text("Equal gaps between the selected devices; overlapping devices are spread apart").clicked() {
                st.distribute(Axis::Horizontal);
            }
            if ui.button(format!("{} Vertically", icons::ARROWS_VERTICAL)).on_hover_text("Equal gaps between the selected devices; overlapping devices are spread apart").clicked() {
                st.distribute(Axis::Vertical);
            }
        });
    });
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        if ui.add_enabled(n > 0, egui::Button::new(format!("{} Duplicate", icons::COPY))).clicked() {
            st.duplicate_selection();
        }
        if ui.add_enabled(n > 0, egui::Button::new(format!("{} Auto-assign IPs", icons::LIGHTNING))).clicked() {
            st.auto_assign();
        }
        if ui.button(format!("{} Delete", icons::TRASH)).clicked() {
            st.delete_selection();
        }
    });
}

// ---- VLAN / subnet / SSID ------------------------------------------------

fn vlan_inspector(ui: &mut Ui, st: &mut State, id: VlanId) {
    let p = &st.doc.project;
    let Some(before) = p.vlan(id).cloned() else { return };
    let mut v = before.clone();
    let mut field = None;
    heading(ui, icons::TAG, &before.label(), "VLAN");
    form_grid(ui, "vlan", |ui| {
        ui.label("VLAN ID");
        ui.horizontal(|ui| {
            if ui.add(DragValue::new(&mut v.tag).range(netvis_core::Vlan::MIN_TAG..=netvis_core::Vlan::MAX_TAG)).changed() {
                mark(&mut field, "tag");
            }
            let dup = p.vlans.iter().filter(|x| x.id != id && x.tag == v.tag).count();
            if dup > 0 {
                ui.label(RichText::new(format!("{} tag also used by {dup} other VLAN(s)", icons::WARNING)).small().color(WARN));
            }
        });
        ui.end_row();
        if text_row(ui, "Name", &mut v.name) { mark(&mut field, "name") }
        if text_row(ui, "Purpose", &mut v.purpose) { mark(&mut field, "purpose") }
        ui.label("Colour");
        if ui.color_edit_button_srgb(&mut v.color).changed() { mark(&mut field, "color") }
        ui.end_row();
    });
    if notes(ui, &mut v.notes) {
        mark(&mut field, "notes");
    }

    let u = p.vlan_usage(id);
    section(ui, "Used by");
    form_grid(ui, "vlan.use", |ui| {
        for (label, n) in [("Access ports", u.access_ports), ("Trunks carrying it", u.trunk_ports), ("VLAN interfaces", u.logical), ("SSIDs", u.ssids)] {
            ui.label(label);
            ui.label(n.to_string());
            ui.end_row();
        }
    });
    let mut go = None;
    section(ui, "Subnets");
    if u.subnets.is_empty() {
        ui.label(RichText::new("No subnet bound — auto-assign cannot address hosts in this VLAN").small().color(WARN));
    }
    for sid in &u.subnets {
        if let Some(s) = p.subnet(*sid)
            && ui.link(format!("{}  {}", s.name, s.net)).clicked()
        {
            go = Some(*sid);
        }
    }
    if ui.button(format!("{} New subnet for this VLAN", icons::PLUS)).clicked() {
        let net = p.suggest_subnet(Some(v.tag));
        let mut s = netvis_core::Subnet::new(v.name.clone(), net).with_defaults(true, true);
        s.vlan = Some(id);
        let sid = s.id;
        st.apply(Edit::Subnet { id: sid, before: None, after: Some(s) }, "Add subnet");
        st.sel.set_single(Item::Subnet(sid));
        return;
    }
    if let Some(sid) = go {
        st.sel.set_single(Item::Subnet(sid));
        return;
    }
    if let Some(f) = field {
        st.apply_coalescing(Edit::Vlan { id, before: Some(before), after: Some(v) }, "Edit VLAN", format!("{id:?}.{f}"));
    }
}

fn subnet_inspector(ui: &mut Ui, st: &mut State, id: netvis_core::SubnetId) {
    let p = &st.doc.project;
    let Some(before) = p.subnet(id).cloned() else { return };
    let mut s = before.clone();
    let mut field = None;
    let mut go_device = None;
    heading(ui, icons::TREE_STRUCTURE, &before.name, &before.net.to_string());

    form_grid(ui, "subnet", |ui| {
        if text_row(ui, "Name", &mut s.name) { mark(&mut field, "name") }
        ui.label("Network");
        if widgets::net_field(ui, ("snet", id), &mut s.net, 160.0, true) { mark(&mut field, "net") }
        ui.end_row();
        ui.label("VLAN");
        if widgets::vlan_combo(ui, ("svlan", id), &p.vlans, &mut s.vlan, 160.0) { mark(&mut field, "vlan") }
        ui.end_row();
        ui.label("Gateway");
        ui.horizontal(|ui| {
            if widgets::parsed_opt(ui, ("sgw", id), &mut s.gateway, 130.0, "none") { mark(&mut field, "gateway") }
            if let Some(g) = s.gateway
                && !ip::is_usable_host(&s.net, g)
            {
                ui.label(RichText::new(icons::WARNING).color(ERROR)).on_hover_text("Gateway is not a usable host in this subnet");
            }
        });
        ui.end_row();
    });

    let info = SubnetInfo::of(&s.net);
    egui::CollapsingHeader::new(RichText::new("Calculator").small().strong()).default_open(true).show(ui, |ui| {
        egui::Grid::new("calc").num_columns(2).spacing([10.0, 2.0]).show(ui, |ui| {
            let mut row = |k: &str, v: String| {
                ui.label(RichText::new(k).small().weak());
                ui.label(RichText::new(v).small().monospace());
                ui.end_row();
            };
            row("Netmask", info.netmask.to_string());
            if let Some(w) = info.wildcard {
                row("Wildcard", w.to_string());
            }
            if let Some(b) = info.broadcast {
                row("Broadcast", b.to_string());
            }
            row("Host range", format!("{} – {}", info.first_usable, info.last_usable));
            row("Usable hosts", widgets::fmt_count(info.usable));
            row("Scope", info.scope.to_owned());
        });
    });

    let usage = p.subnet_usage(&s);
    ui.horizontal(|ui| {
        widgets::usage_bar(ui, &usage, 160.0);
        ui.label(
            RichText::new(format!("{} / {} used", widgets::fmt_count(usable_used(&usage)), widgets::fmt_count(usage.usable)))
                .small(),
        );
    });
    if let Some(next) = p.next_free_ip(&s) {
        ui.label(RichText::new(format!("Next free: {next}")).small().weak());
    }

    section(ui, "DHCP");
    let mut enabled = s.dhcp.is_some();
    if ui.checkbox(&mut enabled, "DHCP scope").changed() {
        s.dhcp = if enabled { netvis_core::Subnet::new("", s.net).with_defaults(false, true).dhcp.or_else(|| fallback_scope(&s)) } else { None };
        mark(&mut field, "dhcp");
    }
    if let Some(d) = &mut s.dhcp {
        form_grid(ui, "dhcp", |ui| {
            ui.label("Range");
            ui.horizontal(|ui| {
                if widgets::parsed(ui, ("dstart", id), &mut d.start, 110.0, "start") { mark(&mut field, "dhcp") }
                ui.label("–");
                if widgets::parsed(ui, ("dend", id), &mut d.end, 110.0, "end") { mark(&mut field, "dhcp") }
            });
            ui.end_row();
            ui.label("");
            let ok = s.net.contains(&d.start) && s.net.contains(&d.end) && ip::range_len(d.start, d.end) > 0;
            if ok {
                ui.label(RichText::new(format!("{} addresses", widgets::fmt_count(ip::range_len(d.start, d.end)))).small().weak());
            } else {
                ui.label(RichText::new(format!("{} Range must lie inside {} with start ≤ end", icons::WARNING, s.net)).small().color(ERROR));
            }
            ui.end_row();
            ui.label("Lease (hours)");
            if ui.add(DragValue::new(&mut d.lease_hours).range(1..=8760)).changed() { mark(&mut field, "dhcp") }
            ui.end_row();
            ui.label("DNS for clients");
            if widgets::ip_list(ui, ("ddns", id), &mut d.dns, f32::INFINITY) { mark(&mut field, "dhcp") }
            ui.end_row();
            ui.label("Served by");
            let server = d.server.and_then(|x| p.device(x)).map(|x| x.name.clone()).unwrap_or("—".into());
            egui::ComboBox::from_id_salt(("dsrv", id)).selected_text(server).show_ui(ui, |ui| {
                if ui.selectable_value(&mut d.server, None, "—").changed() { mark(&mut field, "dhcp") }
                for dev in p.devices.iter().filter(|x| x.kind.category() == DeviceCategory::Network || x.kind == netvis_core::DeviceKind::Server) {
                    if ui.selectable_value(&mut d.server, Some(dev.id), &dev.name).changed() { mark(&mut field, "dhcp") }
                }
            });
            ui.end_row();
        });
    }

    ui.horizontal(|ui| {
        section(ui, "Reserved ranges");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.small_button(format!("{} Range", icons::PLUS)).clicked() {
                let a = ip::nth_usable(&s.net, 1).unwrap_or(s.net.addr());
                let b = ip::nth_usable(&s.net, 9).unwrap_or(a);
                s.reserved.push(AddrRange { start: a, end: b, label: "Infrastructure".into() });
                mark(&mut field, "reserved");
            }
        });
    });
    let mut remove = None;
    egui::Grid::new("reserved").num_columns(4).spacing([6.0, 4.0]).show(ui, |ui| {
        for (i, r) in s.reserved.iter_mut().enumerate() {
            if widgets::parsed(ui, ("rs", id, i), &mut r.start, 100.0, "start") { mark(&mut field, "reserved") }
            if widgets::parsed(ui, ("re", id, i), &mut r.end, 100.0, "end") { mark(&mut field, "reserved") }
            if ui.add(egui::TextEdit::singleline(&mut r.label).desired_width(80.0).hint_text("label")).changed() {
                mark(&mut field, "reserved");
            }
            if ui.small_button(icons::TRASH).clicked() {
                remove = Some(i);
            }
            ui.end_row();
        }
    });
    if let Some(i) = remove {
        s.reserved.remove(i);
        mark(&mut field, "reserved");
    }
    if notes(ui, &mut s.notes) {
        mark(&mut field, "notes");
    }

    let assigned = p.assignments_in(&s);
    section(ui, &format!("Addresses ({})", assigned.len()));
    let mut counts: HashMap<IpAddr, usize> = HashMap::new();
    for a in &assigned {
        *counts.entry(a.addr.ip()).or_default() += 1;
    }
    egui::Grid::new("subnet.addrs").num_columns(3).spacing([10.0, 3.0]).striped(true).show(ui, |ui| {
        if let Some(g) = s.gateway
            && !assigned.iter().any(|a| a.addr.ip() == g)
        {
            ui.label(RichText::new(g.to_string()).monospace());
            ui.label(RichText::new("gateway (not on any interface)").weak().small());
            ui.label("");
            ui.end_row();
        }
        for a in &assigned {
            let dup = counts[&a.addr.ip()] > 1;
            let text = RichText::new(a.addr.ip().to_string()).monospace();
            ui.label(if dup { text.color(ERROR) } else { text });
            let who = p
                .device(a.device)
                .map(|d| format!("{}:{}", d.name, d.iface_name(a.iface).unwrap_or("?")))
                .unwrap_or_default();
            if ui.link(who).clicked() {
                go_device = Some(a.device);
            }
            ui.label(RichText::new(short_kind(a.addr.kind)).small().weak());
            ui.end_row();
        }
    });

    if let Some(f) = field {
        st.apply_coalescing(Edit::Subnet { id, before: Some(before), after: Some(s) }, "Edit subnet", format!("{id:?}.{f}"));
    }
    if let Some(d) = go_device {
        st.sel.set_single(Item::Device(d));
        if let Some(dev) = st.doc.project.device(d) {
            let r = dev.rect();
            st.canvas.focus(r.min.offset(r.size.x / 2.0, r.size.y / 2.0));
        }
    }
}

fn usable_used(u: &netvis_core::ipam::Usage) -> u128 {
    u.usable - u.free
}

/// Small subnets get a pool covering every host after the first.
fn fallback_scope(s: &netvis_core::Subnet) -> Option<DhcpScope> {
    let start = ip::nth_usable(&s.net, 1)?;
    let (_, end) = ip::usable_range(&s.net);
    Some(DhcpScope { start, end, lease_hours: 24, dns: Vec::new(), server: None })
}

fn ssid_inspector(ui: &mut Ui, st: &mut State, id: netvis_core::SsidId) {
    let p = &st.doc.project;
    let Some(before) = p.ssid(id).cloned() else { return };
    let mut s = before.clone();
    let mut field = None;
    heading(ui, icons::WIFI_HIGH, &before.name, "Wireless network (SSID)");
    form_grid(ui, "ssid", |ui| {
        if text_row(ui, "SSID", &mut s.name) { mark(&mut field, "name") }
        ui.label("VLAN");
        if widgets::vlan_combo(ui, ("ssidv", id), &p.vlans, &mut s.vlan, 160.0) { mark(&mut field, "vlan") }
        ui.end_row();
        ui.label("Security");
        egui::ComboBox::from_id_salt(("sec", id)).selected_text(s.security.label()).show_ui(ui, |ui| {
            for x in WifiSecurity::ALL {
                if ui.selectable_value(&mut s.security, x, x.label()).changed() { mark(&mut field, "sec") }
            }
        });
        ui.end_row();
        ui.label("Bands");
        ui.horizontal(|ui| {
            for b in WifiBand::ALL {
                let mut on = s.bands.contains(&b);
                if ui.checkbox(&mut on, b.label()).changed() {
                    if on { s.bands.push(b) } else { s.bands.retain(|x| *x != b) }
                    mark(&mut field, "bands");
                }
            }
        });
        ui.end_row();
        ui.label("Hidden");
        if ui.checkbox(&mut s.hidden, "Don't broadcast SSID").changed() { mark(&mut field, "hidden") }
        ui.end_row();
    });
    if s.bands.contains(&WifiBand::G6) && !matches!(s.security, WifiSecurity::Wpa3Personal | WifiSecurity::Wpa3Enterprise | WifiSecurity::Owe) {
        ui.label(RichText::new(format!("{} 6 GHz requires WPA3 or Enhanced Open", icons::WARNING)).small().color(WARN));
    }
    if notes(ui, &mut s.notes) {
        mark(&mut field, "notes");
    }
    if let Some(f) = field {
        st.apply_coalescing(Edit::Ssid { id, before: Some(before), after: Some(s) }, "Edit SSID", format!("{id:?}.{f}"));
    }
}
