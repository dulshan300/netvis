//! Rack elevations: every Rack area drawn as a column of units. Drag devices
//! to move them, right-click to remove, and place unmounted devices from the
//! list under each rack.

use egui::{Align2, Color32, CornerRadius, FontId, RichText, Sense, Stroke, StrokeKind, Ui, pos2, vec2};
use netvis_core::{AreaId, AreaKind, DeviceId, DeviceKind, Edit, Item, RackMount};

use crate::canvas::{PaletteItem, place_item};
use crate::icons;
use crate::state::State;
use crate::theme;
use crate::widgets::{ERROR, WARN, temp_id};

const UNIT_H: f32 = 17.0;
const GUTTER: f32 = 26.0;
const WIDTH: f32 = 250.0;

enum Action {
    Select(DeviceId),
    Move(DeviceId, AreaId, u8),
    Unmount(DeviceId),
    Place(DeviceId, AreaId),
    AutoPlace(AreaId),
    SelectRack(AreaId),
}

pub fn racks(ui: &mut Ui, st: &mut State) {
    ui.horizontal(|ui| {
        if ui.button(format!("{} Rack", icons::PLUS)).on_hover_text("Add a 42U rack area to the canvas").clicked() {
            let at = st.canvas.view_center();
            place_item(st, PaletteItem::Area(AreaKind::Rack), at, true);
        }
        ui.label(RichText::new("Drag devices to move them · right-click to remove · racks are Rack areas on the canvas").small().weak());
    });
    ui.separator();

    let racks: Vec<AreaId> = st.doc.project.racks().map(|r| r.id).collect();
    if racks.is_empty() {
        ui.label(RichText::new("No racks yet. Add one here, or drop a Rack from Toolbox → Areas & zones.").weak());
        return;
    }

    let mut actions = Vec::new();
    egui::ScrollArea::both().auto_shrink(false).show(ui, |ui| {
        ui.horizontal_top(|ui| {
            for rid in racks {
                ui.vertical(|ui| {
                    ui.set_width(WIDTH);
                    rack_column(ui, st, rid, &mut actions);
                });
                ui.add_space(18.0);
            }
        });
    });

    for a in actions {
        apply(st, a);
    }
}

fn rack_column(ui: &mut Ui, st: &State, rid: AreaId, actions: &mut Vec<Action>) {
    let p = &st.doc.project;
    let Some(rack) = p.area(rid) else { return };
    let units = rack.rack_units.max(1);
    let contents = p.rack_contents(rid);
    let used: u32 = contents.iter().map(|m| m.height as u32).sum();
    let power = p.rack_power(rid);

    ui.horizontal(|ui| {
        if ui.selectable_label(st.sel.contains(Item::Area(rid)), RichText::new(&rack.name).strong()).clicked() {
            actions.push(Action::SelectRack(rid));
        }
        ui.label(RichText::new(format!("{used}/{units}U")).small().weak());
    });
    ui.horizontal(|ui| {
        let mut text = format!("{} {:.0} W", icons::LIGHTNING, power.load_w);
        if power.ups_capacity_w > 0.0 {
            text.push_str(&format!(" / {:.0} W UPS", power.ups_capacity_w));
        }
        let over = power.ups_capacity_w > 0.0 && power.load_w > power.ups_capacity_w;
        let label = RichText::new(text).small();
        ui.label(if over { label.color(ERROR) } else { label.weak() }).on_hover_text(if power.unknown > 0 {
            format!("{} device(s) have no power figure and are not counted", power.unknown)
        } else {
            "Mains draw of mounted equipment plus PoE it delivers".to_owned()
        });
    });

    let (rect, _) = ui.allocate_exact_size(vec2(WIDTH, units as f32 * UNIT_H + 8.0), Sense::hover());
    let pal = theme::Palette::new(ui.visuals().dark_mode);
    let painter = ui.painter_at(rect.expand(2.0));
    let body = egui::Rect::from_min_size(rect.min + vec2(GUTTER, 4.0), vec2(WIDTH - GUTTER, units as f32 * UNIT_H));
    let y_of = |u: u8| body.top() + (units - u) as f32 * UNIT_H; // top edge of unit u

    // Frame and unit grid.
    painter.rect_filled(body.expand(3.0), 4.0, pal.card_border);
    painter.rect_filled(body, 2.0, pal.canvas_bg);
    for u in 1..=units {
        let y = y_of(u);
        let row = egui::Rect::from_min_size(pos2(body.left(), y), vec2(body.width(), UNIT_H));
        if u % 2 == 0 {
            painter.rect_filled(row, 0.0, pal.grid_minor);
        }
        painter.text(pos2(rect.left() + GUTTER - 6.0, y + UNIT_H / 2.0), Align2::RIGHT_CENTER, u.to_string(), FontId::monospace(9.5), pal.subtext);
    }

    // Devices.
    let drag_id = temp_id("rack-drag", rid);
    for m in &contents {
        let d = m.device;
        let h = m.height.max(1);
        let top_u = (m.bottom as u16 + h as u16 - 1).min(units as u16) as u8;
        let r = egui::Rect::from_min_max(pos2(body.left() + 2.0, y_of(top_u) + 1.0), pos2(body.right() - 2.0, y_of(m.bottom.max(1)) + UNIT_H - 1.0));
        let cat = theme::category_color(d.kind.category());
        let selected = st.sel.contains(Item::Device(d.id));
        let out_of_range = m.bottom == 0 || m.bottom as u16 + h as u16 - 1 > units as u16;
        let resp = ui.interact(r, ui.id().with(("rack-dev", d.id)), Sense::click_and_drag());

        painter.rect_filled(r, CornerRadius::same(3), cat.gamma_multiply(if resp.dragged() { 0.12 } else { 0.22 }));
        let stroke = if selected {
            Stroke::new(2.0, pal.accent)
        } else if out_of_range {
            Stroke::new(1.5, ERROR)
        } else {
            Stroke::new(1.0, cat)
        };
        painter.rect_stroke(r, CornerRadius::same(3), stroke, StrokeKind::Inside);
        let text = if h >= 2 && !d.model.is_empty() { format!("{}  {}\n{}", icons::device(d.kind), d.name, d.model) } else { format!("{}  {}", icons::device(d.kind), d.name) };
        let clipped = painter.with_clip_rect(r.shrink(2.0));
        clipped.text(r.left_center() + vec2(6.0, 0.0), Align2::LEFT_CENTER, text, FontId::proportional(11.0), pal.text);
        clipped.text(r.right_center() - vec2(6.0, 0.0), Align2::RIGHT_CENTER, format!("{h}U"), FontId::proportional(9.5), pal.subtext);

        if resp.clicked() {
            actions.push(Action::Select(d.id));
        }
        resp.context_menu(|ui| {
            if ui.button(format!("{} Remove from rack", icons::X)).clicked() {
                actions.push(Action::Unmount(d.id));
                ui.close();
            }
        });

        // Drag: remember where in the block it was grabbed, preview the target.
        if resp.drag_started()
            && let Some(ptr) = resp.interact_pointer_pos()
        {
            let grab_units = ((ptr.y - r.top()) / UNIT_H).floor() as i32;
            ui.data_mut(|m| m.insert_temp(drag_id, (d.id, grab_units)));
        }
        let grab: Option<(DeviceId, i32)> = ui.data(|m| m.get_temp(drag_id));
        if let (Some((gid, grab_units)), Some(ptr)) = (grab.filter(|(g, _)| *g == d.id), resp.interact_pointer_pos())
            && (resp.dragged() || resp.drag_stopped())
        {
            let row_from_top = ((ptr.y - body.top()) / UNIT_H).floor() as i32 - grab_units;
            let target_top = (units as i32 - row_from_top).clamp(h as i32, units as i32);
            let target_bottom = (target_top - h as i32 + 1) as u8;
            let free = slot_free(st, rid, gid, target_bottom, h);
            let ghost = egui::Rect::from_min_max(pos2(body.left() + 2.0, y_of(target_top as u8) + 1.0), pos2(body.right() - 2.0, y_of(target_bottom) + UNIT_H - 1.0));
            let c = if free { Color32::from_rgb(22, 163, 74) } else { ERROR };
            painter.rect_stroke(ghost, CornerRadius::same(3), Stroke::new(2.0, c), StrokeKind::Inside);
            painter.text(ghost.right_top() + vec2(-4.0, -2.0), Align2::RIGHT_BOTTOM, format!("U{target_bottom}"), FontId::proportional(10.0), c);
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
            if resp.drag_stopped() {
                ui.data_mut(|m| m.remove::<(DeviceId, i32)>(drag_id));
                if free && target_bottom != m.bottom {
                    actions.push(Action::Move(gid, rid, target_bottom));
                }
            }
        } else if resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
        }
    }

    // Devices drawn inside the rack area but not yet in the elevation.
    let unplaced: Vec<_> = p.devices_in_area(rid).into_iter().filter(|d| d.hw.rack.is_none()).collect();
    if !unplaced.is_empty() {
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new(format!("Not in elevation ({})", unplaced.len())).small().strong().weak());
            if ui.small_button("Auto-place").on_hover_text("Network gear from the top, servers/storage/UPS from the bottom").clicked() {
                actions.push(Action::AutoPlace(rid));
            }
        });
        for d in unplaced {
            ui.horizontal(|ui| {
                if d.hw.rack_units == 0 {
                    ui.label(RichText::new(format!("{}  {}", icons::device(d.kind), d.name)).small().weak())
                        .on_hover_text("No rack height — set it in the Inspector (General → Hardware)");
                } else {
                    if ui.small_button(icons::PLUS).on_hover_text("Place in the first free slot").clicked() {
                        actions.push(Action::Place(d.id, rid));
                    }
                    if ui.link(RichText::new(format!("{}  {}  ({}U)", icons::device(d.kind), d.name, d.hw.rack_units)).small()).clicked() {
                        actions.push(Action::Select(d.id));
                    }
                }
            });
        }
    }
    if power.ups_capacity_w > 0.0 && power.load_w > 0.8 * power.ups_capacity_w {
        ui.label(RichText::new(format!("{} UPS load {:.0}%", icons::WARNING, power.load_w / power.ups_capacity_w * 100.0)).small().color(WARN));
    }
}

fn slot_free(st: &State, rack: AreaId, device: DeviceId, bottom: u8, height: u8) -> bool {
    let p = &st.doc.project;
    let units = p.area(rack).map_or(0, |a| a.rack_units);
    if bottom == 0 || bottom as u16 + height as u16 - 1 > units as u16 {
        return false;
    }
    p.rack_contents(rack)
        .iter()
        .filter(|m| m.device.id != device)
        .all(|m| m.top() < bottom || m.bottom > bottom + height - 1)
}

fn set_mount(st: &mut State, id: DeviceId, mount: Option<RackMount>, label: &str) {
    let Some(before) = st.doc.project.device(id).cloned() else { return };
    let mut after = before.clone();
    after.hw.rack = mount;
    st.apply(Edit::Device { id, before: Some(before), after: Some(after) }, label);
}

fn apply(st: &mut State, a: Action) {
    match a {
        Action::Select(id) => {
            st.doc.history.break_coalescing();
            st.sel.set_single(Item::Device(id));
        }
        Action::SelectRack(id) => st.reveal(Item::Area(id)),
        Action::Move(id, rack, u) => set_mount(st, id, Some(RackMount { rack, u }), "Move in rack"),
        Action::Unmount(id) => set_mount(st, id, None, "Remove from rack"),
        Action::Place(id, rack) => {
            let Some(d) = st.doc.project.device(id) else { return };
            let from_top = !matches!(d.kind, DeviceKind::Server | DeviceKind::VmHost | DeviceKind::Storage | DeviceKind::Ups);
            match st.doc.project.rack_free_slot(rack, d.hw.rack_units, from_top, Some(id)) {
                Some(u) => set_mount(st, id, Some(RackMount { rack, u }), "Place in rack"),
                None => st.set_status(format!("No {}U gap left in this rack", d.hw.rack_units)),
            }
        }
        Action::AutoPlace(rack) => {
            let (edit, n, no_room) = st.doc.project.auto_mount_edit(rack);
            st.apply(edit, "Auto-place in rack");
            st.set_status(if no_room.is_empty() {
                format!("Placed {n} device(s)")
            } else {
                format!("Placed {n}; no room for {}", no_room.join(", "))
            });
        }
    }
}
