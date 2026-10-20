//! File → Export PDF report…: pick sections, render, save, open.

use egui::RichText;
use netvis_report::{ReportOptions, Section};

use crate::icons;
use crate::state::State;
use crate::widgets::{ERROR, WARN};

#[derive(Debug, Clone)]
pub struct ExportDialog {
    include: Vec<(Section, bool)>,
    addresses: bool,
    lengths: bool,
    error: Option<String>,
}

impl ExportDialog {
    pub fn new(st: &State) -> Self {
        let skip = &st.settings.report_skip;
        Self {
            include: Section::ALL.iter().map(|s| (*s, !skip.iter().any(|k| k == s.key()))).collect(),
            addresses: st.settings.show_addresses,
            lengths: true,
            error: None,
        }
    }

    fn options(&self) -> ReportOptions {
        let mut o = ReportOptions { sections: self.include.iter().filter(|(_, on)| *on).map(|(s, _)| *s).collect(), ..Default::default() };
        o.diagram.show_addresses = self.addresses;
        o.diagram.show_lengths = self.lengths;
        o
    }
}

pub fn show(ctx: &egui::Context, st: &mut State) {
    let Some(mut dlg) = st.export.take() else { return };
    let mut keep = true;
    let mut export = false;

    let resp = egui::Modal::new(egui::Id::new("export-report")).show(ctx, |ui| {
        ui.set_width(460.0);
        ui.heading(format!("{} Export PDF report", icons::FILE_PDF));
        let m = &st.doc.project.meta;
        ui.label(
            RichText::new(format!(
                "Corescale design report for “{}” · revision {} · {}",
                m.name,
                if m.revision.is_empty() { "—" } else { &m.revision },
                netvis_report::format_date(netvis_report::today())
            ))
            .small()
            .weak(),
        );
        ui.label(RichText::new("Cover, client and revision come from the project details (Inspector with nothing selected).").small().weak());
        ui.add_space(6.0);

        egui::Grid::new("export.sections").num_columns(2).spacing([8.0, 3.0]).show(ui, |ui| {
            for (s, on) in &mut dlg.include {
                ui.checkbox(on, RichText::new(s.title()).strong());
                ui.label(RichText::new(s.blurb()).small().weak());
                ui.end_row();
            }
        });
        ui.horizontal(|ui| {
            if ui.small_button("All").clicked() {
                dlg.include.iter_mut().for_each(|(_, on)| *on = true);
            }
            if ui.small_button("None").clicked() {
                dlg.include.iter_mut().for_each(|(_, on)| *on = false);
            }
        });
        ui.add_space(4.0);
        ui.checkbox(&mut dlg.addresses, "Show IP addresses on the diagram");
        ui.checkbox(&mut dlg.lengths, "Show cable lengths on the diagram");
        if cfg!(windows) {
            ui.checkbox(&mut st.settings.report_open_after, "Open the PDF when done");
        }

        let issues = st.issues.iter().filter(|i| i.severity == netvis_validate::Severity::Error).count();
        if issues > 0 {
            ui.label(RichText::new(format!("{} The design has {issues} error(s); they are listed in the Design check section.", icons::WARNING)).small().color(WARN));
        }
        if let Some(e) = &dlg.error {
            ui.label(RichText::new(e).color(ERROR));
        }

        ui.add_space(8.0);
        let any = dlg.include.iter().any(|(_, on)| *on);
        ui.horizontal(|ui| {
            if ui.add_enabled(any, egui::Button::new(RichText::new(format!("{} Export…", icons::FILE_PDF)).strong())).clicked() {
                export = true;
            }
            if ui.button("Cancel").clicked() {
                keep = false;
            }
        });
    });
    if resp.should_close() {
        keep = false;
    }

    if export {
        st.settings.report_skip = dlg.include.iter().filter(|(_, on)| !*on).map(|(s, _)| s.key().to_owned()).collect();
        match netvis_report::render_pdf(&st.doc.project, &dlg.options()) {
            Ok(bytes) => {
                let kb = bytes.len() / 1024;
                if let Some(path) = super::export_file(st, "report", "PDF document", "pdf", &bytes) {
                    st.set_status(format!("Exported report ({kb} KB) to {}", path.display()));
                    if st.settings.report_open_after {
                        crate::file_dialog::open_with_default_app(&path);
                    }
                    keep = false;
                }
                // Cancelled save dialog: stay open.
            }
            Err(e) => dlg.error = Some(format!("Could not create the report: {e}")),
        }
    }
    if keep {
        st.export = Some(dlg);
    }
}

/// File → Export diagram as SVG.
pub fn export_svg(st: &mut State) {
    let opts = netvis_report::diagram::DiagramOptions { show_addresses: st.settings.show_addresses, ..Default::default() };
    let svg = netvis_report::diagram::topology_svg(&st.doc.project, &opts);
    if let Some(path) = super::export_file(st, "topology", "SVG image", "svg", svg.as_bytes()) {
        st.set_status(format!("Exported diagram to {}", path.display()));
    }
}
