//! Patch schedule (every run end to end, through patch panels and outlets)
//! and the port map of one device.

use egui::{Align2, Color32, CornerRadius, FontId, RichText, Sense, Stroke, StrokeKind, Ui, pos2, vec2};
use netvis_core::{DeviceId, Edit, Item, LinkId, SwitchportMode};
use netvis_tools::patch::{self, PortState};

use crate::icons;
use crate::state::State;
use crate::widgets::{self, ERROR, OK, WARN, temp_id};

// ---- patch schedule --------------------------------------------------------

#[derive(Debug, Clone, Default)]
struct ScheduleUi {
    filter: String,
    prefix: Option<String>,
    overwrite: bool,
    /// Run whose cables are being edited.
    open: Option<String>,
}

pub fn patch_schedule(ui: &mut Ui, st: &mut State) {
    let id = temp_id("schedule", ());
    let mut s: ScheduleUi = ui.data(|d| d.get_temp(id)).unwrap_or_default();
    let rows = patch::schedule(&st.doc.project);
    let cables: usize = rows.iter().map(|r| r.segments.len()).sum();
    let unlabelled: usize = rows.iter().flat_map(|r| &r.segments).filter(|x| x.label.trim().is_empty()).count();
    let unterminated = rows.iter().filter(|r| r.unterminated).count();
    let spare = rows.iter().filter(|r| r.spare).count();

    ui.horizontal(|ui| {
        ui.add(egui::TextEdit::singleline(&mut s.filter).hint_text(format!("{} Filter runs…", icons::MAGNIFYING_GLASS)).desired_width(200.0));
        ui.menu_button(format!("{} Auto-label", icons::TAG), |ui| {
            let prefix = s.prefix.get_or_insert_with(|| "C-".into());
            ui.horizontal(|ui| {
                ui.label("Prefix");
                ui.add(egui::TextEdit::singleline(prefix).desired_width(60.0));
            });
            ui.checkbox(&mut s.overwrite, "Replace existing labels");
            widgets_hint(ui, "Runs are numbered in schedule order: C-001 for a direct cable, C-002.1, C-002.2… for each cable of a patched run.");
            if ui.button("Label cables").clicked() {
                let (edit, n) = patch::auto_label_edit(&st.doc.project, prefix, s.overwrite);
                if n > 0 {
                    st.apply(edit, "Auto-label cables");
                }
                st.set_status(format!("Labelled {n} cable(s)"));
                ui.close();
            }
        });
        if ui.button(format!("{} Copy CSV", icons::COPY)).clicked() {
            ui.ctx().copy_text(patch::schedule_csv(&rows));
            st.set_status("Patch schedule copied as CSV");
        }
        if ui.button(format!("{} Save CSV…", icons::DOWNLOAD_SIMPLE)).clicked() {
            super::save_csv(st, "patch-schedule", &patch::schedule_csv(&rows));
        }
        let mut summary = format!("{} runs · {cables} cables", rows.len());
        if spare > 0 {
            summary.push_str(&format!(" · {spare} spare"));
        }
        if unlabelled > 0 {
            summary.push_str(&format!(" · {unlabelled} unlabelled"));
        }
        ui.label(RichText::new(summary).small().weak());
        if unterminated > 0 {
            ui.label(RichText::new(format!("{} {unterminated} not patched through", icons::WARNING)).small().color(WARN));
        }
    });
    ui.separator();
    if rows.is_empty() {
        ui.label(RichText::new("No cables yet. Runs through patch panels and wall outlets are traced end to end.").weak());
        ui.data_mut(|d| d.insert_temp(id, s));
        return;
    }

    let needle = s.filter.to_lowercase();
    let mut select: Option<Vec<LinkId>> = None;
    let mut edits: Vec<(Edit, String)> = Vec::new();
    egui::ScrollArea::both().auto_shrink(false).show(ui, |ui| {
        egui::Grid::new("schedule").num_columns(8).spacing([14.0, 4.0]).striped(true).show(ui, |ui| {
            for h in ["Run", "From", "Via", "To", "Cables", "Length", "Service", "Labels"] {
                ui.label(RichText::new(h).small().weak());
            }
            ui.end_row();
            for r in &rows {
                let labels: Vec<&str> = r.segments.iter().map(|x| x.label.as_str()).filter(|x| !x.is_empty()).collect();
                let hay = format!("{} {} {} {} {} {}", r.id, r.a, r.b, r.via, r.service, labels.join(" ")).to_lowercase();
                if !needle.is_empty() && !hay.contains(&needle) {
                    continue;
                }
                let open = s.open.as_deref() == Some(r.id.as_str());
                let icon = if open { icons::CARET_DOWN } else { icons::CARET_RIGHT };
                if ui.selectable_label(open, RichText::new(format!("{icon} {}", r.id)).strong()).on_hover_text("Show the cables of this run and select them").clicked() {
                    s.open = if open { None } else { Some(r.id.clone()) };
                    select = Some(r.circuit.links.clone());
                }
                ui.label(&r.a);
                ui.label(if r.via.is_empty() { RichText::new("direct").weak() } else { RichText::new(&r.via) });
                if r.spare {
                    ui.label(RichText::new(format!("{}  (spare)", r.b)).weak()).on_hover_text("Permanent link with nothing patched to either end");
                } else if r.unterminated {
                    ui.label(RichText::new(format!("{} {}", icons::WARNING, r.b)).color(WARN)).on_hover_text("The run stops at this panel port — patch it through");
                } else {
                    ui.label(&r.b);
                }
                let kinds: Vec<&str> = r.segments.iter().map(|x| x.cable.as_str()).collect();
                ui.label(kinds.join(" + "));
                ui.label(match r.length_m {
                    Some(m) => RichText::new(format!("{m:.0} m")),
                    None if r.segments.iter().any(|x| x.length_m.is_some()) => RichText::new("partly known").weak(),
                    None => RichText::new("—").weak(),
                });
                ui.label(&r.service);
                ui.label(if labels.is_empty() { RichText::new("—").weak() } else { RichText::new(labels.join(", ")).monospace() });
                ui.end_row();

                if open {
                    for seg in &r.segments {
                        ui.label("");
                        ui.label(RichText::new(&seg.from).small());
                        ui.label(RichText::new("→").small().weak());
                        ui.label(RichText::new(&seg.to).small());
                        ui.label(RichText::new(&seg.cable).small());
                        let mut len = seg.length_m;
                        if widgets::parsed_opt(ui, ("seg.len", seg.link), &mut len, 60.0, "m")
                            && let Some(l) = st.doc.project.link(seg.link)
                        {
                            let mut after = l.clone();
                            after.length_m = len.filter(|m: &f32| *m >= 0.0);
                            edits.push((Edit::Link { id: l.id, before: Some(l.clone()), after: Some(after) }, format!("len:{:?}", l.id)));
                        }
                        ui.label("");
                        let mut label = seg.label.clone();
                        if ui.add(egui::TextEdit::singleline(&mut label).hint_text("label").desired_width(110.0)).changed()
                            && let Some(l) = st.doc.project.link(seg.link)
                        {
                            let mut after = l.clone();
                            after.label = label;
                            edits.push((Edit::Link { id: l.id, before: Some(l.clone()), after: Some(after) }, format!("label:{:?}", l.id)));
                        }
                        ui.end_row();
                    }
                }
            }
        });
    });

    for (e, key) in edits {
        st.apply_coalescing(e, "Edit cable", key);
    }
    if let Some(links) = select {
        st.sel.clear();
        for l in links {
            st.sel.add(Item::Link(l));
        }
    }
    ui.data_mut(|d| d.insert_temp(id, s));
}

fn widgets_hint(ui: &mut Ui, text: &str) {
    ui.label(RichText::new(text).small().weak());
}

// ---- port map --------------------------------------------------------------

const PORT_W: f32 = 34.0;
const PORT_H: f32 = 24.0;

pub fn port_map(ui: &mut Ui, st: &mut State) {
    let p = &st.doc.project;
    let pick_id = temp_id("portmap.device", ());
    // Follow the selection; otherwise keep the last device shown.
    let selected = match st.sel.single() {
        Some(Item::Device(d)) if p.device(d).is_some_and(|d| !d.ports.is_empty()) => Some(d),
        _ => None,
    };
    let mut device: Option<DeviceId> = selected.or_else(|| ui.data(|d| d.get_temp(pick_id))).filter(|d| p.device(*d).is_some());
    if device.is_none() {
        device = p
            .devices
            .iter()
            .find(|d| matches!(d.kind, netvis_core::DeviceKind::SwitchL2 | netvis_core::DeviceKind::SwitchL3))
            .or_else(|| p.devices.iter().find(|d| !d.ports.is_empty()))
            .map(|d| d.id);
    }
    let Some(dev_id) = device else {
        ui.label(RichText::new("Add a device to see its port map.").weak());
        return;
    };

    let mut sorted: Vec<(DeviceId, String)> = p
        .devices
        .iter()
        .filter(|d| !d.ports.is_empty())
        .map(|d| (d.id, format!("{}  {}", icons::device(d.kind), d.name)))
        .collect();
    sorted.sort_by_key(|(id, label)| (p.device(*id).map(|d| d.kind.category() as u8), label.clone()));
    let mut choice = dev_id;
    let rows = patch::port_map(p, dev_id);
    let (current, dev_name) = {
        let d = p.device(dev_id).expect("checked");
        (format!("{}  {}", icons::device(d.kind), d.name), d.name.clone())
    };
    let mut save = false;
    ui.horizontal(|ui| {
        egui::ComboBox::from_id_salt("portmap.pick").width(200.0).selected_text(current).show_ui(ui, |ui| {
            for (id, label) in &sorted {
                ui.selectable_value(&mut choice, *id, label);
            }
        });
        let used = rows.iter().filter(|r| r.state != PortState::Free).count();
        ui.label(RichText::new(format!("{used} of {} ports in use", rows.len())).small().weak());
        if ui.button(format!("{} Copy CSV", icons::COPY)).clicked() {
            ui.ctx().copy_text(patch::port_map_csv(&rows));
        }
        save = ui.button(format!("{} Save CSV…", icons::DOWNLOAD_SIMPLE)).clicked();
    });
    if save {
        super::save_csv(st, &format!("ports-{dev_name}"), &patch::port_map_csv(&rows));
    }
    let p = &st.doc.project;
    let d = p.device(dev_id).expect("checked");
    ui.data_mut(|m| m.insert_temp(pick_id, choice));
    if choice != dev_id {
        st.sel.set_single(Item::Device(choice));
        return;
    }
    ui.add_space(6.0);

    // Faceplate: copper and cages in two rows, odd ports on top like most switches.
    let mut reveal = None;
    let pal = crate::theme::Palette::new(ui.visuals().dark_mode);
    egui::ScrollArea::both().auto_shrink(false).show(ui, |ui| {
        let per_row = rows.len().div_ceil(2).max(1);
        let two_rows = rows.len() > 8;
        let cols = if two_rows { per_row } else { rows.len() };
        let (rect, _) = ui.allocate_exact_size(
            vec2(cols as f32 * (PORT_W + 4.0) + 16.0, if two_rows { 2.0 * PORT_H + 26.0 } else { PORT_H + 20.0 }),
            Sense::hover(),
        );
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 6.0, pal.card);
        painter.rect_stroke(rect, 6.0, Stroke::new(1.0, pal.card_border), StrokeKind::Inside);
        for (i, r) in rows.iter().enumerate() {
            let (col, row) = if two_rows { (i / 2, i % 2) } else { (i, 0) };
            let min = rect.min + vec2(8.0 + col as f32 * (PORT_W + 4.0), 8.0 + row as f32 * (PORT_H + 8.0));
            let pr = egui::Rect::from_min_size(min, vec2(PORT_W, PORT_H));
            let port = d.ports.iter().find(|x| x.id == r.endpoint.port);
            let vlan = port.and_then(|x| match x.cfg.mode {
                SwitchportMode::Access { vlan } => vlan.and_then(|v| p.vlan(v)).map(|v| widgets::rgb(v.color)),
                _ => None,
            });
            let trunk = port.is_some_and(|x| matches!(x.cfg.mode, SwitchportMode::Trunk { .. }));
            let fill = match r.state {
                PortState::Up => OK.gamma_multiply(0.35),
                PortState::Unterminated => WARN.gamma_multiply(0.35),
                PortState::OneSide => OK.gamma_multiply(0.12),
                PortState::Disabled => ERROR.gamma_multiply(0.25),
                PortState::Free => Color32::TRANSPARENT,
            };
            let resp = ui.interact(pr, ui.id().with(("port", r.endpoint.port)), Sense::click());
            painter.rect_filled(pr, CornerRadius::same(3), fill);
            let stroke = if resp.hovered() { Stroke::new(2.0, pal.accent) } else { Stroke::new(1.0, pal.subtext) };
            painter.rect_stroke(pr, CornerRadius::same(3), stroke, StrokeKind::Inside);
            if let Some(c) = vlan {
                painter.rect_filled(egui::Rect::from_min_max(pos2(pr.left() + 3.0, pr.bottom() - 5.0), pos2(pr.right() - 3.0, pr.bottom() - 2.0)), 1.0, c);
            } else if trunk {
                // Striped bar for trunks.
                for k in 0..4 {
                    let x0 = pr.left() + 3.0 + k as f32 * (PORT_W - 6.0) / 4.0;
                    let seg = egui::Rect::from_min_max(pos2(x0, pr.bottom() - 5.0), pos2(x0 + (PORT_W - 6.0) / 8.0, pr.bottom() - 2.0));
                    painter.rect_filled(seg, 0.0, pal.text);
                }
            }
            let short: String = r.name.chars().rev().take(4).collect::<Vec<_>>().into_iter().rev().collect();
            painter.text(pr.center() + vec2(-2.0, -1.0), Align2::CENTER_CENTER, short, FontId::monospace(9.0), pal.text);
            if r.poe.starts_with("PSE") {
                painter.text(pr.right_top() + vec2(-1.5, 1.0), Align2::RIGHT_TOP, icons::LIGHTNING, FontId::proportional(7.0), WARN);
            }
            let tip = port_tip(r);
            let resp = resp.on_hover_text(tip);
            if resp.clicked() && r.state != PortState::Free {
                reveal = p.peer(r.endpoint).map(|e| e.device);
            }
        }
        legend(ui);
        ui.add_space(6.0);

        egui::Grid::new("portmap.rows").num_columns(10).spacing([12.0, 3.0]).striped(true).show(ui, |ui| {
            for h in ["Port", "State", "Media", "Speed", "Mode / VLAN", "Connected to", "Via", "Cables", "Labels", "PoE"] {
                ui.label(RichText::new(h).small().weak());
            }
            ui.end_row();
            for r in &rows {
                ui.label(RichText::new(&r.name).monospace());
                let c = match r.state {
                    PortState::Up => OK,
                    PortState::Unterminated => WARN,
                    PortState::OneSide => ui.visuals().text_color(),
                    PortState::Disabled => ERROR,
                    PortState::Free => ui.visuals().weak_text_color(),
                };
                ui.label(RichText::new(r.state.label()).color(c));
                ui.label(&r.media);
                ui.label(&r.speed);
                ui.label(&r.mode);
                if r.far.is_empty() {
                    ui.label("");
                } else if ui.link(&r.far).clicked() {
                    reveal = p.peer(r.endpoint).map(|e| e.device);
                }
                ui.label(RichText::new(&r.via).weak());
                ui.label(&r.cables);
                ui.label(RichText::new(&r.labels).monospace());
                ui.label(&r.poe);
                ui.end_row();
            }
        });
    });
    if let Some(dev) = reveal {
        st.reveal(Item::Device(dev));
    }
}

fn port_tip(r: &patch::PortRow) -> String {
    let mut lines = vec![format!("{} — {} {}", r.name, r.media, r.speed), r.state.label().to_owned()];
    for (k, v) in [("Mode", &r.mode), ("To", &r.far), ("Via", &r.via), ("Cables", &r.cables), ("Labels", &r.labels), ("PoE", &r.poe), ("Note", &r.description)] {
        if !v.is_empty() {
            lines.push(format!("{k}: {v}"));
        }
    }
    if let Some(run) = &r.run {
        lines.push(format!("Run {run}"));
    }
    lines.join("\n")
}

fn legend(ui: &mut Ui) {
    ui.horizontal(|ui| {
        for (c, t) in [(OK, "connected"), (OK.gamma_multiply(0.3), "one side only"), (WARN, "patched, not terminated"), (ERROR, "shut down")] {
            widgets::swatch(ui, c.gamma_multiply(0.6));
            ui.label(RichText::new(t).small().weak());
            ui.add_space(6.0);
        }
        ui.label(RichText::new("· bar = access VLAN colour, striped = trunk · ⚡ = PoE source · click a port to jump to the far end").small().weak());
    });
}
