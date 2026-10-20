//! Edit → Duplicate with rename…: copy a floor (or any selection) and
//! replace text in the copies' names, e.g. L1 → L2.

use egui::RichText;
use netvis_core::{Pos, Rect};

use crate::icons;
use crate::state::State;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Place {
    Above,
    Below,
    Right,
}

#[derive(Debug, Clone)]
pub struct CopyDialog {
    pub find: String,
    pub replace: String,
    pub place: Place,
}

impl CopyDialog {
    /// Suggest the first word of a selected area's name ("L1 · Retail" → "L1").
    pub fn new(st: &State) -> Self {
        let find = st
            .sel
            .areas
            .first()
            .and_then(|a| st.doc.project.area(*a))
            .and_then(|a| a.name.split([' ', '·']).find(|w| !w.is_empty()).map(str::to_owned))
            .unwrap_or_default();
        Self { find, replace: String::new(), place: Place::Above }
    }
}

/// Bounds of the selected areas and devices.
fn selection_bounds(st: &State) -> Option<Rect> {
    let p = &st.doc.project;
    let areas = p.areas.iter().filter(|a| st.sel.areas.contains(&a.id)).map(|a| a.rect);
    let devs = p.devices.iter().filter(|d| st.sel.devices.contains(&d.id)).map(|d| d.rect());
    areas.chain(devs).reduce(|a, b| a.union(&b))
}

pub fn show(ctx: &egui::Context, st: &mut State) {
    let Some(mut dlg) = st.copy_dialog.take() else {
        return;
    };
    let mut keep = true;
    let mut run = false;
    let resp = egui::Modal::new(egui::Id::new("copy-rename")).show(ctx, |ui| {
        ui.set_width(420.0);
        ui.heading(format!("{} Duplicate with rename", icons::COPY));
        let (na, nd) = (st.sel.areas.len(), st.sel.devices.len());
        ui.label(
            RichText::new(format!(
                "Copies {na} area(s) with everything inside them and {nd} other device(s), with the cables between them. Cables to devices outside the selection aren't copied."
            ))
            .small()
            .weak(),
        );
        ui.add_space(6.0);
        egui::Grid::new("copy.grid").num_columns(2).spacing([8.0, 6.0]).show(ui, |ui| {
            ui.label("Find in names");
            ui.add(egui::TextEdit::singleline(&mut dlg.find).hint_text("e.g. L1").desired_width(200.0));
            ui.end_row();
            ui.label("Replace with");
            ui.add(egui::TextEdit::singleline(&mut dlg.replace).hint_text("e.g. L2").desired_width(200.0));
            ui.end_row();
            ui.label("Place the copy");
            ui.horizontal(|ui| {
                ui.radio_value(&mut dlg.place, Place::Above, "Above");
                ui.radio_value(&mut dlg.place, Place::Below, "Below");
                ui.radio_value(&mut dlg.place, Place::Right, "Right");
            });
            ui.end_row();
        });
        if !dlg.find.is_empty() {
            let example = st
                .doc
                .project
                .devices
                .iter()
                .filter(|d| d.name.contains(&dlg.find))
                .find(|d| st.sel.devices.contains(&d.id) || st.sel.areas.iter().any(|a| st.doc.project.area(*a).is_some_and(|a| a.rect.contains(d.rect().min))));
            if let Some(d) = example {
                ui.label(RichText::new(format!("{} → {}", d.name, d.name.replace(&dlg.find, &dlg.replace))).small().monospace());
            }
        }
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if ui.add_enabled(na + nd > 0, egui::Button::new(RichText::new(format!("{} Duplicate", icons::COPY)).strong())).clicked() {
                run = true;
            }
            if ui.button("Cancel").clicked() {
                keep = false;
            }
        });
    });
    if resp.should_close() {
        keep = false;
    }
    if run {
        keep = false;
        let Some(b) = selection_bounds(st) else {
            return;
        };
        let gap = 40.0;
        let delta = match dlg.place {
            Place::Above => Pos::new(0.0, -(b.size.y + gap)),
            Place::Below => Pos::new(0.0, b.size.y + gap),
            Place::Right => Pos::new(b.size.x + gap, 0.0),
        };
        let (areas, devices) = (st.sel.areas.clone(), st.sel.devices.clone());
        let (res, edit) = st.doc.project.copy_with_rename_edit(&areas, &devices, delta, &dlg.find, &dlg.replace);
        st.apply(edit, "Duplicate with rename");
        st.sel.clear();
        st.sel.devices = res.devices.clone();
        st.sel.areas = res.areas.clone();
        let mut msg = format!("Copied {} device(s) and {} area(s)", res.devices.len(), res.areas.len());
        if res.renumbered > 0 {
            msg += &format!("; {} name(s) were taken and got a numbered name instead", res.renumbered);
        }
        st.set_status(msg);
    }
    if keep {
        st.copy_dialog = Some(dlg);
    }
}
