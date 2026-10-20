//! Dockable panels.

mod inspector;
mod ipplan;
mod vlans;
pub mod vlsm;

#[cfg(debug_assertions)]
pub use inspector::debug_set_device_tab;
pub use inspector::inspector;
pub use ipplan::ip_plan;
pub use vlans::vlans;

use egui::{Color32, RichText, Sense, Ui, vec2};
use netvis_core::{AreaKind, DeviceCategory, DeviceKind};

use crate::canvas::{PaletteItem, place_item};
use crate::icons;
use crate::state::{Item, State};
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
