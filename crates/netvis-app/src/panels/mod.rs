//! Dockable panels.

mod inspector;
mod ipplan;
mod patching;
mod racks;
pub mod tools;
mod vlans;
pub mod vlsm;

#[cfg(debug_assertions)]
pub use inspector::debug_set_device_tab;
pub use inspector::inspector;
pub use ipplan::ip_plan;
pub use patching::{patch_schedule, port_map};
pub use racks::racks;
pub use tools::tools;
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

pub fn issues(ui: &mut Ui, st: &mut State) {
    use netvis_validate::Severity;
    let mut fix_all = false;
    let mut fix: Option<netvis_validate::Fix> = None;
    let filt_id = crate::widgets::temp_id("issues.filter", ());
    let mut show: [bool; 3] = ui.data(|d| d.get_temp(filt_id)).unwrap_or([true, true, true]);
    let counts = st.issue_counts();
    ui.horizontal(|ui| {
        for (i, (sev, color)) in [
            (Severity::Error, crate::widgets::ERROR),
            (Severity::Warning, crate::widgets::WARN),
            (Severity::Info, ui.visuals().weak_text_color()),
        ]
        .into_iter()
        .enumerate()
        {
            let text = RichText::new(format!("{} {} {}", severity_icon(sev), counts[i], sev.label())).color(color);
            ui.toggle_value(&mut show[i], text);
        }
        if st.issues.is_empty() {
            ui.label(RichText::new(format!("{} No problems found", icons::CHECK_CIRCLE)).color(Color32::from_rgb(22, 163, 74)));
        }
        let fixable = st.issues.iter().filter(|i| i.fix.is_some() && i.severity != Severity::Info).count();
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .add_enabled(fixable > 0, egui::Button::new(format!("{} Fix all ({fixable})", icons::WRENCH)))
                .on_hover_text("Apply every one-click fix for errors and warnings as a single undo step")
                .clicked()
            {
                fix_all = true;
            }
        });
    });
    ui.data_mut(|d| d.insert_temp(filt_id, show));
    ui.separator();

    let mut reveal = None;
    egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
        for issue in &st.issues {
            let idx = issue.severity as usize;
            if !show[idx] {
                continue;
            }
            let color = match issue.severity {
                Severity::Error => crate::widgets::ERROR,
                Severity::Warning => crate::widgets::WARN,
                Severity::Info => ui.visuals().weak_text_color(),
            };
            let selected = issue.items.first().is_some_and(|i| st.sel.contains(*i));
            let resp = ui
                .horizontal(|ui| {
                    ui.label(RichText::new(severity_icon(issue.severity)).color(color));
                    if let Some(f) = &issue.fix
                        && ui.small_button(format!("{} {}", icons::WRENCH, f.label(&st.doc.project))).clicked()
                    {
                        fix = Some(f.clone());
                    }
                    ui.add(egui::Label::new(RichText::new(&issue.message)).sense(Sense::click()).truncate())
                })
                .inner;
            let resp = resp.on_hover_text(format!("{}  ·  {}", issue.message, issue.code));
            if selected {
                ui.painter().rect_stroke(resp.rect.expand(2.0), 3.0, egui::Stroke::new(1.0, color), egui::StrokeKind::Outside);
            }
            if resp.clicked()
                && let Some(item) = issue.items.first()
            {
                reveal = Some(*item);
            }
        }
    });
    if let Some(item) = reveal {
        st.reveal(item);
    }
    if let Some(f) = fix {
        st.apply_fix(&f);
    }
    if fix_all {
        st.fix_all();
    }
}

pub fn severity_icon(s: netvis_validate::Severity) -> &'static str {
    match s {
        netvis_validate::Severity::Error => icons::X_CIRCLE,
        netvis_validate::Severity::Warning => icons::WARNING,
        netvis_validate::Severity::Info => icons::INFO,
    }
}

// ---- CSV export -------------------------------------------------------------

/// Save `csv` as `<project>-<suffix>.csv`: a native dialog on Windows, next to
/// the project (or in Documents) elsewhere.
pub(crate) fn save_csv(st: &mut State, suffix: &str, csv: &str) {
    let stem = format!("{}-{suffix}", crate::file_dialog::file_stem_for(&st.doc.project.meta.name));
    let path = match crate::file_dialog::save_csv(&format!("{stem}.csv")) {
        Some(Some(p)) => p,
        Some(None) => return,
        None => st
            .doc
            .path
            .as_ref()
            .and_then(|p| p.parent())
            .map(|d| d.to_path_buf())
            .unwrap_or_else(crate::file_dialog::default_dir)
            .join(format!("{stem}.csv")),
    };
    // Byte-order mark so Excel reads the file as UTF-8 (×, ·, ↔).
    match std::fs::write(&path, format!("\u{feff}{csv}")) {
        Ok(()) => st.set_status(format!("Saved {}", path.display())),
        Err(e) => st.set_status(format!("Could not save {}: {e}", path.display())),
    }
}
