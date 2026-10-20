//! NetVis design report: a branded PDF with a cover, contents, the vector
//! topology diagram and the plan tables (devices, addressing, VLANs, routing,
//! racks, patch schedule, port maps, bill of materials, design check).
//!
//! ```no_run
//! # let project = netvis_core::Project::default();
//! let pdf = netvis_report::render_pdf(&project, &netvis_report::ReportOptions::default()).unwrap();
//! std::fs::write("report.pdf", pdf).unwrap();
//! ```

pub mod diagram;
pub mod layout;
pub mod pdf;
pub mod report;

pub use pdf::PdfError;
pub use report::{ReportOptions, Section, format_date, today};

/// Lay out and write the report.
pub fn render_pdf(p: &netvis_core::Project, opts: &ReportOptions) -> Result<Vec<u8>, PdfError> {
    let doc = report::build(p, opts);
    let m = &p.meta;
    let info = pdf::PdfInfo { title: format!("{} — network design report", m.name), author: m.author.clone(), date: opts.date };
    let rev = if m.revision.trim().is_empty() { String::new() } else { format!("Revision {} · ", m.revision) };
    let furniture = pdf::Furniture {
        left_header: report::BRAND.to_uppercase(),
        right_header: format!("{} — network design report", m.name),
        left_footer: format!("{rev}{}", format_date(opts.date)),
    };
    pdf::write(&doc, &info, &furniture)
}

#[cfg(test)]
mod tests {
    use netvis_core::{CableKind, DeviceKind, Endpoint, Pos, Project, Subnet};

    use crate::layout::{BOTTOM, FontKind, MARGIN_X, Op, text_width};

    /// Every glyph run stays inside the page margins.
    fn assert_in_bounds(doc: &crate::layout::Doc) {
        for (i, p) in doc.pages.iter().enumerate() {
            for op in &p.ops {
                if let Some((x, y, size, font, text)) = op.clone().into_text() {
                    let right = x + text_width(font, size, &text);
                    assert!(right <= p.width - MARGIN_X + 2.0 || p.plain, "page {}: “{text}” ends at {right:.1} of {:.1}", i + 1, p.width);
                    assert!(y <= p.height - BOTTOM + 2.0 || p.plain, "page {}: “{text}” below the footer", i + 1);
                }
            }
        }
    }

    impl Op {
        fn into_text(self) -> Option<(f32, f32, f32, FontKind, String)> {
            match self {
                Op::Text { x, y, size, font, text, .. } => Some((x, y, size, font, text)),
                _ => None,
            }
        }
    }

    #[test]
    fn empty_project_renders() {
        let p = Project::default();
        let doc = crate::report::build(&p, &Default::default());
        assert!(doc.pages.len() >= 3);
        let pdf = crate::render_pdf(&p, &Default::default()).unwrap();
        assert!(pdf.starts_with(b"%PDF-"));
    }

    #[test]
    fn large_project_paginates_cleanly() {
        let mut p = Project::default();
        p.meta.name = "A very long project name that has to wrap across more than one line on the cover page".into();
        let add = |p: &mut Project, k, x: f32, y: f32| {
            let (id, e) = p.add_device_edit(k, Pos::new(x, y));
            e.apply(p);
            id
        };
        let core = add(&mut p, DeviceKind::SwitchL3, 0.0, 0.0);
        for s in 0..12 {
            let sw = add(&mut p, DeviceKind::SwitchL2, s as f32 * 400.0, 300.0);
            let net = format!("10.{s}.0.0/24").parse().unwrap();
            let sub = Subnet::new(format!("Floor {s} users with a deliberately long subnet name"), net).with_defaults(true, true);
            p.subnets.push(sub);
            let up = (p.first_free_port(sw, Some(netvis_core::PortMedia::SfpPlus)).unwrap(), p.first_free_port(core, None).unwrap());
            let (_, e) = p.add_link_edit(Endpoint { device: sw, port: up.0 }, Endpoint { device: core, port: up.1 }).unwrap();
            e.apply(&mut p);
            for h in 0..20 {
                let pc = add(&mut p, DeviceKind::Pc, s as f32 * 400.0 + h as f32 * 20.0, 600.0);
                p.device_mut(pc).unwrap().name = format!("WORKSTATION-FLOOR{s:02}-DESK{h:03}-EXTRA-LONG");
                let ports = (p.first_free_port(sw, Some(netvis_core::PortMedia::Rj45)), p.first_free_port(pc, None));
                if let (Some(a), Some(b)) = ports {
                    let (_, e) = p.add_link_edit(Endpoint { device: sw, port: a }, Endpoint { device: pc, port: b }).unwrap();
                    e.apply(&mut p);
                    let l = p.links.last_mut().unwrap();
                    l.cable = CableKind::Cat6;
                    l.length_m = Some(20.0 + h as f32);
                    l.label = format!("F{s:02}-D{h:03}");
                }
            }
        }
        let run = p.auto_assign_edit(None);
        run.edit.apply(&mut p);

        let doc = crate::report::build(&p, &Default::default());
        assert!(doc.pages.len() > 20, "{} pages", doc.pages.len());
        assert_in_bounds(&doc);
        let pdf = crate::render_pdf(&p, &Default::default()).unwrap();
        assert!(pdf.len() > 50_000);
    }

    #[test]
    fn campus_sample_renders_within_margins() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../samples/corescale-campus.netvis");
        let p = netvis_core::io::load(&path).unwrap();
        let doc = crate::report::build(&p, &Default::default());
        assert!(doc.pages.len() > 40);
        assert_in_bounds(&doc);
        // Too big for one page: every floor gets a detail page.
        let floors = p.areas.iter().filter(|a| a.kind == netvis_core::AreaKind::Floor).count();
        let details = doc.toc.iter().filter(|e| e.level == 1 && e.title.ends_with("· Floor")).count();
        assert_eq!(details, floors);
        assert!(crate::render_pdf(&p, &Default::default()).is_ok());
    }
}
