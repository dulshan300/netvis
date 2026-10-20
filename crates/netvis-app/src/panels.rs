//! Dockable side panels: toolbox, outline, inspector, issues.

use egui::{Color32, DragValue, RichText, Sense, Ui, vec2};
use netvis_core::port::format_speed;
use netvis_core::{
    AreaKind, CableKind, DeviceCategory, DeviceKind, Edit, PoeRole, PoeStandard, Port, PortMedia,
};

use crate::canvas::{PaletteItem, place_item};
use crate::icons;
use crate::state::{Align, Axis, Item, State};
use crate::theme;

// ---- toolbox -------------------------------------------------------------

pub fn toolbox(ui: &mut Ui, st: &mut State) {
    let filter_id = ui.id().with("filter");
    let mut filter: String = ui.data_mut(|d| d.get_temp(filter_id).unwrap_or_default());
    ui.add(
        egui::TextEdit::singleline(&mut filter)
            .hint_text(format!("{} Search devices…", icons::MAGNIFYING_GLASS))
            .desired_width(f32::INFINITY),
    );
    ui.data_mut(|d| d.insert_temp(filter_id, filter.clone()));
    let needle = filter.to_lowercase();
    ui.add_space(4.0);
    ui.label(RichText::new("Drag onto the canvas, or click to add").small().weak());

    egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
        for cat in DeviceCategory::ALL {
            let kinds: Vec<_> = DeviceKind::ALL
                .into_iter()
                .filter(|k| k.category() == cat && k.label().to_lowercase().contains(&needle))
                .collect();
            if kinds.is_empty() {
                continue;
            }
            egui::CollapsingHeader::new(format!("{}  {}", icons::category(cat), cat.label()))
                .default_open(true)
                .show(ui, |ui| {
                    for kind in kinds {
                        let color = theme::category_color(cat);
                        palette_entry(ui, st, PaletteItem::Device(kind), icons::device(kind), kind.label(), color);
                    }
                });
        }
        let areas: Vec<_> =
            AreaKind::ALL.into_iter().filter(|k| k.label().to_lowercase().contains(&needle)).collect();
        if !areas.is_empty() {
            egui::CollapsingHeader::new(format!("{}  Areas & zones", icons::RECTANGLE_DASHED))
                .default_open(true)
                .show(ui, |ui| {
                    for kind in areas {
                        let [r, g, b] = kind.default_color();
                        let color = Color32::from_rgb(r, g, b);
                        palette_entry(ui, st, PaletteItem::Area(kind), icons::area(kind), kind.label(), color);
                    }
                });
        }
    });
}

fn palette_entry(ui: &mut Ui, st: &mut State, item: PaletteItem, icon: &str, label: &str, color: Color32) {
    let id = ui.id().with(("palette", format!("{item:?}")));
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 28.0), Sense::hover());
    let resp = ui.interact(rect, id, Sense::click_and_drag());
    let visuals = ui.style().interact(&resp);
    if resp.hovered() || resp.dragged() {
        ui.painter().rect_filled(rect, 6.0, visuals.bg_fill);
    }
    let painter = ui.painter();
    painter.text(rect.left_center() + vec2(14.0, 0.0), egui::Align2::CENTER_CENTER, icon, egui::FontId::proportional(16.0), color);
    painter.text(rect.left_center() + vec2(30.0, 0.0), egui::Align2::LEFT_CENTER, label, egui::FontId::proportional(13.5), visuals.text_color());

    if resp.drag_started() {
        egui::DragAndDrop::set_payload(ui.ctx(), item);
    }
    if resp.dragged() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
        if let Some(p) = ui.ctx().pointer_interact_pos() {
            egui::Area::new(id.with("ghost"))
                .fixed_pos(p + vec2(12.0, 8.0))
                .order(egui::Order::Tooltip)
                .interactable(false)
                .show(ui.ctx(), |ui| {
                    egui::Frame::popup(ui.style()).show(ui, |ui| {
                        ui.label(RichText::new(format!("{icon}  {label}")).color(color));
                    });
                });
        }
    } else if resp.clicked() {
        let at = st.canvas.view_center();
        place_item(st, item, at, true);
    }
    resp.on_hover_cursor(egui::CursorIcon::Grab);
}

// ---- outline -------------------------------------------------------------

pub fn outline(ui: &mut Ui, st: &mut State) {
    let filter_id = ui.id().with("filter");
    let mut filter: String = ui.data_mut(|d| d.get_temp(filter_id).unwrap_or_default());
    ui.add(
        egui::TextEdit::singleline(&mut filter)
            .hint_text(format!("{} Filter…", icons::MAGNIFYING_GLASS))
            .desired_width(f32::INFINITY),
    );
    ui.data_mut(|d| d.insert_temp(filter_id, filter.clone()));
    let needle = filter.to_lowercase();
    let shift = ui.input(|i| i.modifiers.shift);

    let mut clicked: Option<(Item, netvis_core::Pos)> = None;
    egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
        let p = &st.doc.project;
        if !p.areas.is_empty() {
            ui.label(RichText::new("AREAS").small().strong().weak());
            for a in p.areas.iter().filter(|a| a.name.to_lowercase().contains(&needle)) {
                let item = Item::Area(a.id);
                let text = format!("{}  {}", icons::area(a.kind), a.name);
                if ui.selectable_label(st.sel.contains(item), text).clicked() {
                    clicked = Some((item, a.rect.min.offset(a.rect.size.x / 2.0, a.rect.size.y / 2.0)));
                }
            }
            ui.add_space(6.0);
        }
        ui.label(RichText::new(format!("DEVICES ({})", p.devices.len())).small().strong().weak());
        let mut devices: Vec<_> = p
            .devices
            .iter()
            .filter(|d| d.name.to_lowercase().contains(&needle) || d.kind.label().to_lowercase().contains(&needle))
            .collect();
        devices.sort_by_cached_key(|d| d.name.to_lowercase());
        for d in devices {
            let item = Item::Device(d.id);
            let links = p.links_of(d.id).count();
            let text = format!("{}  {}", icons::device(d.kind), d.name);
            let resp = ui
                .selectable_label(st.sel.contains(item), text)
                .on_hover_text(format!("{} · {links} link(s)", d.kind.label()));
            if resp.clicked() {
                let r = d.rect();
                clicked = Some((item, r.min.offset(r.size.x / 2.0, r.size.y / 2.0)));
            }
        }
        if !p.links.is_empty() {
            ui.add_space(6.0);
            ui.label(RichText::new(format!("LINKS ({})", p.links.len())).small().strong().weak());
            for l in &p.links {
                let (Some(a), Some(b)) = (p.device(l.a.device), p.device(l.b.device)) else { continue };
                let text = format!("{}  {} ↔ {}", icons::LINK, a.name, b.name);
                if !text.to_lowercase().contains(&needle) {
                    continue;
                }
                let item = Item::Link(l.id);
                if ui.selectable_label(st.sel.contains(item), text).on_hover_text(l.cable.label()).clicked() {
                    let (ra, rb) = (a.rect(), b.rect());
                    let mid = netvis_core::Pos::new((ra.min.x + rb.min.x) / 2.0, (ra.min.y + rb.min.y) / 2.0);
                    clicked = Some((item, mid));
                }
            }
        }
    });
    if let Some((item, at)) = clicked {
        st.doc.history.break_coalescing();
        st.sel.click(item, shift);
        st.canvas.focus(at);
    }
}

// ---- inspector -----------------------------------------------------------

pub fn inspector(ui: &mut Ui, st: &mut State) {
    egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| match st.sel.single() {
        Some(Item::Device(id)) => device_inspector(ui, st, id),
        Some(Item::Link(id)) => link_inspector(ui, st, id),
        Some(Item::Area(id)) => area_inspector(ui, st, id),
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

fn form_grid(ui: &mut Ui, id: &str, add: impl FnOnce(&mut Ui)) {
    egui::Grid::new(id).num_columns(2).spacing([10.0, 6.0]).striped(false).show(ui, add);
}

/// Text field that reports whether it changed.
fn text_row(ui: &mut Ui, label: &str, value: &mut String) -> bool {
    ui.label(label);
    let changed = ui.add(egui::TextEdit::singleline(value).desired_width(f32::INFINITY)).changed();
    ui.end_row();
    changed
}

fn project_inspector(ui: &mut Ui, st: &mut State) {
    heading(ui, icons::FILE_PLUS, "Project", "Nothing selected — editing project details");
    let before = st.doc.project.meta.clone();
    let mut m = before.clone();
    let mut field = None;
    form_grid(ui, "meta", |ui| {
        if text_row(ui, "Name", &mut m.name) { field = Some("name") }
        if text_row(ui, "Client", &mut m.client) { field = Some("client") }
        if text_row(ui, "Author", &mut m.author) { field = Some("author") }
        if text_row(ui, "Company", &mut m.company) { field = Some("company") }
        if text_row(ui, "Revision", &mut m.revision) { field = Some("revision") }
    });
    ui.label("Description");
    if ui.add(egui::TextEdit::multiline(&mut m.description).desired_rows(4).desired_width(f32::INFINITY)).changed() {
        field = Some("description");
    }
    if let Some(f) = field {
        let edit = Edit::Meta { before: Box::new(before), after: Box::new(m) };
        st.apply_coalescing(edit, "Edit project details", format!("meta.{f}"));
    }

    ui.add_space(10.0);
    ui.label(RichText::new("SUMMARY").small().strong().weak());
    let p = &st.doc.project;
    form_grid(ui, "summary", |ui| {
        for cat in DeviceCategory::ALL {
            let n = p.devices.iter().filter(|d| d.kind.category() == cat).count();
            ui.label(format!("{}  {}", icons::category(cat), cat.label()));
            ui.label(n.to_string());
            ui.end_row();
        }
        ui.label(format!("{}  Links", icons::LINK));
        ui.label(p.links.len().to_string());
        ui.end_row();
        ui.label(format!("{}  Areas", icons::RECTANGLE_DASHED));
        ui.label(p.areas.len().to_string());
        ui.end_row();
    });
}

fn device_inspector(ui: &mut Ui, st: &mut State, id: netvis_core::DeviceId) {
    let Some(before) = st.doc.project.device(id).cloned() else { return };
    let mut d = before.clone();
    let mut field: Option<String> = None;
    let mut set = |f: &str| field = Some(f.to_owned());

    heading(ui, icons::device(d.kind), &before.name, &format!("{} · {}", d.kind.label(), d.kind.category().label()));
    form_grid(ui, "device", |ui| {
        if text_row(ui, "Hostname", &mut d.name) { set("name") }
        if text_row(ui, "Vendor", &mut d.vendor) { set("vendor") }
        if text_row(ui, "Model", &mut d.model) { set("model") }
        ui.label("Position");
        ui.horizontal(|ui| {
            if ui.add(DragValue::new(&mut d.pos.x).prefix("x ").speed(1.0)).changed() { set("pos") }
            if ui.add(DragValue::new(&mut d.pos.y).prefix("y ").speed(1.0)).changed() { set("pos") }
        });
        ui.end_row();
    });
    ui.label("Notes");
    if ui.add(egui::TextEdit::multiline(&mut d.notes).desired_rows(3).desired_width(f32::INFINITY)).changed() {
        set("notes");
    }

    ui.add_space(8.0);
    let used = d.ports.iter().filter(|p| st.doc.project.port_link(p.id).is_some()).count();
    ui.horizontal(|ui| {
        ui.label(RichText::new(format!("PORTS  {used}/{} used", d.ports.len())).small().strong().weak());
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.menu_button(format!("{} Add", icons::PLUS), |ui| {
                for media in PortMedia::ALL {
                    if ui.button(media.label()).clicked() {
                        let n = d.ports.iter().filter(|p| p.media == media).count() + 1;
                        let prefix = match media {
                            PortMedia::Rj45 => "ge",
                            PortMedia::Wireless => "radio",
                            PortMedia::Virtual => "link",
                            PortMedia::Console => "con",
                            PortMedia::Coax => "coax",
                            _ => "xe",
                        };
                        d.ports.push(Port::new(format!("{prefix}{n}-new"), media));
                        set("ports.add");
                        ui.close();
                    }
                }
            });
        });
    });

    let mut remove: Option<usize> = None;
    let project = &st.doc.project;
    egui::Grid::new("ports").num_columns(5).spacing([6.0, 4.0]).striped(true).show(ui, |ui| {
        ui.label(RichText::new("Name").small().weak());
        ui.label(RichText::new("Type").small().weak());
        ui.label(RichText::new("Speed").small().weak());
        ui.label(RichText::new("PoE").small().weak());
        ui.label(RichText::new("Connected").small().weak());
        ui.end_row();
        for (i, port) in d.ports.iter_mut().enumerate() {
            let pid = port.id;
            if ui.add(egui::TextEdit::singleline(&mut port.name).desired_width(48.0)).changed() {
                field = Some(format!("port.{i}.name"));
            }
            egui::ComboBox::from_id_salt(("media", pid))
                .width(58.0)
                .selected_text(port.media.label())
                .show_ui(ui, |ui| {
                    for m in PortMedia::ALL {
                        if ui.selectable_value(&mut port.media, m, m.label()).changed() {
                            port.speed_mbps = m.default_speed_mbps();
                            field = Some(format!("port.{i}.media"));
                        }
                    }
                });
            egui::ComboBox::from_id_salt(("speed", pid))
                .width(44.0)
                .selected_text(format_speed(port.speed_mbps))
                .show_ui(ui, |ui| {
                    for s in [10, 100, 1_000, 2_500, 5_000, 10_000, 25_000, 40_000, 50_000, 100_000] {
                        if ui.selectable_value(&mut port.speed_mbps, s, format_speed(s)).changed() {
                            field = Some(format!("port.{i}.speed"));
                        }
                    }
                });
            egui::ComboBox::from_id_salt(("poe", pid))
                .width(40.0)
                .selected_text(poe_short(port.poe))
                .show_ui(ui, |ui| {
                    let mut opts = vec![PoeRole::None];
                    opts.extend(PoeStandard::ALL.map(PoeRole::Source));
                    opts.extend(PoeStandard::ALL.map(PoeRole::Powered));
                    for o in opts {
                        if ui.selectable_value(&mut port.poe, o, poe_long(o)).changed() {
                            field = Some(format!("port.{i}.poe"));
                        }
                    }
                });
            match project.port_link(pid) {
                Some(l) => {
                    let far = if l.a.port == pid { l.b } else { l.a };
                    let name = project
                        .device(far.device)
                        .map(|fd| format!("{}:{}", fd.name, fd.port(far.port).map(|p| p.name.as_str()).unwrap_or("?")))
                        .unwrap_or_default();
                    let text = RichText::new(&name).color(theme::cable_color(l.cable.family()));
                    let w = ui.available_width().max(60.0);
                    ui.add_sized([w, 18.0], egui::Label::new(text).truncate())
                        .on_hover_text(format!("{name}\n{}", l.cable.label()));
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
        field = Some("ports.remove".into());
    }

    if let Some(f) = field {
        let key = format!("{id:?}.{f}");
        st.apply_coalescing(Edit::Device { id, before: Some(before), after: Some(d) }, "Edit device", key);
    }
}

fn poe_short(r: PoeRole) -> &'static str {
    match r {
        PoeRole::None => "—",
        PoeRole::Source(PoeStandard::Af) | PoeRole::Powered(PoeStandard::Af) => "af",
        PoeRole::Source(PoeStandard::At) | PoeRole::Powered(PoeStandard::At) => "at",
        PoeRole::Source(PoeStandard::BtType3) | PoeRole::Powered(PoeStandard::BtType3) => "bt3",
        PoeRole::Source(PoeStandard::BtType4) | PoeRole::Powered(PoeStandard::BtType4) => "bt4",
    }
}

fn poe_long(r: PoeRole) -> String {
    match r {
        PoeRole::None => "No PoE".into(),
        PoeRole::Source(s) => format!("Source · {} ({} W)", s.label(), s.pse_watts()),
        PoeRole::Powered(s) => format!("Powered · {}", s.label()),
    }
}

fn link_inspector(ui: &mut Ui, st: &mut State, id: netvis_core::LinkId) {
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

    heading(ui, icons::LINK, "Link", l.cable.label());
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
                    field = Some("cable");
                }
            }
        });
        ui.end_row();
        ui.label("Length");
        ui.horizontal(|ui| {
            let mut has = l.length_m.is_some();
            if ui.checkbox(&mut has, "").changed() {
                l.length_m = has.then_some(l.length_m.unwrap_or(3.0));
                field = Some("length");
            }
            if let Some(m) = &mut l.length_m
                && ui.add(DragValue::new(m).suffix(" m").range(0.1..=100_000.0).speed(0.5)).changed()
            {
                field = Some("length");
            }
        });
        ui.end_row();
        if text_row(ui, "Label", &mut l.label) {
            field = Some("label");
        }
    });
    if let Some(f) = field {
        st.apply_coalescing(Edit::Link { id, before: Some(before), after: Some(l) }, "Edit link", format!("{id:?}.{f}"));
    }
}

fn area_inspector(ui: &mut Ui, st: &mut State, id: netvis_core::AreaId) {
    let Some(before) = st.doc.project.area(id).cloned() else { return };
    let mut a = before.clone();
    let mut field = None;
    heading(ui, icons::area(a.kind), &before.name, a.kind.label());
    form_grid(ui, "area", |ui| {
        if text_row(ui, "Name", &mut a.name) {
            field = Some("name");
        }
        ui.label("Kind");
        egui::ComboBox::from_id_salt("area-kind").selected_text(a.kind.label()).show_ui(ui, |ui| {
            for k in AreaKind::ALL {
                if ui.selectable_value(&mut a.kind, k, k.label()).changed() {
                    field = Some("kind");
                }
            }
        });
        ui.end_row();
        ui.label("Colour");
        if ui.color_edit_button_srgb(&mut a.color).changed() {
            field = Some("color");
        }
        ui.end_row();
        ui.label("Position");
        ui.horizontal(|ui| {
            if ui.add(DragValue::new(&mut a.rect.min.x).prefix("x ")).changed() { field = Some("rect") }
            if ui.add(DragValue::new(&mut a.rect.min.y).prefix("y ")).changed() { field = Some("rect") }
        });
        ui.end_row();
        ui.label("Size");
        ui.horizontal(|ui| {
            let min = netvis_core::Area::MIN_SIZE..=100_000.0;
            if ui.add(DragValue::new(&mut a.rect.size.x).prefix("w ").range(min.clone())).changed() { field = Some("rect") }
            if ui.add(DragValue::new(&mut a.rect.size.y).prefix("h ").range(min)).changed() { field = Some("rect") }
        });
        ui.end_row();
    });
    ui.label("Notes");
    if ui.add(egui::TextEdit::multiline(&mut a.notes).desired_rows(3).desired_width(f32::INFINITY)).changed() {
        field = Some("notes");
    }
    if let Some(f) = field {
        st.apply_coalescing(Edit::Area { id, before: Some(before), after: Some(a) }, "Edit area", format!("{id:?}.{f}"));
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
    ui.label(RichText::new("ALIGN").small().strong().weak());
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
    ui.label(RichText::new("DISTRIBUTE").small().strong().weak());
    ui.add_enabled_ui(n >= 3, |ui| {
        ui.horizontal(|ui| {
            if ui.button(format!("{} Horizontally", icons::ARROWS_HORIZONTAL)).clicked() {
                st.distribute(Axis::Horizontal);
            }
            if ui.button(format!("{} Vertically", icons::ARROWS_VERTICAL)).clicked() {
                st.distribute(Axis::Vertical);
            }
        });
    });
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        if ui.add_enabled(n > 0, egui::Button::new(format!("{} Duplicate", icons::COPY))).clicked() {
            st.duplicate_selection();
        }
        if ui.button(format!("{} Delete", icons::TRASH)).clicked() {
            st.delete_selection();
        }
    });
}

// ---- issues --------------------------------------------------------------

pub fn issues(ui: &mut Ui, _st: &mut State) {
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.label(RichText::new(icons::LIST_CHECKS).size(18.0).weak());
        ui.label(RichText::new("Validation engine not enabled yet").weak());
    });
    ui.label(
        RichText::new(
            "IP conflicts, VLAN mismatches, cable and PoE checks will be listed here, \
             each linked to the device or cable involved.",
        )
        .small()
        .weak(),
    );
}
