//! Topology diagram as SVG: areas, cables and device cards, drawn like the
//! canvas in its light theme. Used for the PDF report and for SVG export.

use std::fmt::Write;

use netvis_core::device::layout;
use netvis_core::style::{cable_dashed, cable_rgb, category_rgb};
use netvis_core::{Device, PortMedia, Project, Rect};

/// Font family used in the SVG. The report registers the bundled DejaVu
/// Sans Condensed under this family name (its stretch is semi-condensed).
pub const SANS: &str = "DejaVu Sans";

#[derive(Debug, Clone, Copy)]
pub struct DiagramOptions {
    /// Second line on each card: the primary IP address instead of the type.
    pub show_addresses: bool,
    pub show_lengths: bool,
    pub margin: f32,
}

impl Default for DiagramOptions {
    fn default() -> Self {
        Self { show_addresses: true, show_lengths: true, margin: 30.0 }
    }
}

fn hex(c: [u8; 3]) -> String {
    format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2])
}

pub fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            c => out.push(c),
        }
    }
    out
}

/// Shorten to at most `max` characters with an ellipsis.
fn clip(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_owned()
    } else {
        format!("{}…", s.chars().take(max.saturating_sub(1)).collect::<String>())
    }
}

/// Width and height of the drawing in SVG user units (1 unit = 1 canvas unit).
pub fn size(p: &Project, opts: &DiagramOptions) -> (f32, f32) {
    match p.bounds() {
        Some(b) => (b.size.x + 2.0 * opts.margin, b.size.y + 2.0 * opts.margin),
        None => (200.0, 100.0),
    }
}

pub fn topology_svg(p: &Project, opts: &DiagramOptions) -> String {
    let (w, h) = size(p, opts);
    let origin = p.bounds().map(|b| b.min).unwrap_or(netvis_core::Pos::new(0.0, 0.0));
    draw(p, opts, Rect::new(origin.x - opts.margin, origin.y - opts.margin, w, h))
}

/// One part of the diagram: the world rectangle `view` plus the margin.
/// Cables leaving it run to the edge. Returns the SVG and its size.
pub fn region_svg(p: &Project, opts: &DiagramOptions, view: Rect) -> (String, f32, f32) {
    let m = opts.margin;
    let r = Rect::new(view.min.x - m, view.min.y - m, view.size.x + 2.0 * m, view.size.y + 2.0 * m);
    (draw(p, opts, r), r.size.x, r.size.y)
}

fn draw(p: &Project, opts: &DiagramOptions, view: Rect) -> String {
    let (w, h) = (view.size.x, view.size.y);
    let (ox, oy) = (-view.min.x, -view.min.y);
    let visible = |r: &Rect| r.intersects(&view);
    let mut s = String::new();
    let _ = write!(
        s,
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{w:.1}" height="{h:.1}" viewBox="0 0 {w:.1} {h:.1}" font-family="{SANS}"><rect width="100%" height="100%" fill="#ffffff"/><g transform="translate({ox:.2} {oy:.2})">"##
    );

    for a in p.areas.iter().filter(|a| visible(&a.rect)) {
        let r = a.rect;
        let c = hex(a.color);
        let _ = write!(
            s,
            r##"<rect x="{:.1}" y="{:.1}" width="{:.1}" height="{:.1}" rx="6" fill="{c}" fill-opacity="0.07" stroke="{c}" stroke-opacity="0.7" stroke-width="1.2"/><rect x="{:.1}" y="{:.1}" width="{:.1}" height="18" rx="6" fill="{c}" fill-opacity="0.16"/><text x="{:.1}" y="{:.1}" font-size="10" font-weight="bold" fill="#1f2937">{}</text><text x="{:.1}" y="{:.1}" font-size="8" fill="#64748b" text-anchor="end">{}</text>"##,
            r.min.x,
            r.min.y,
            r.size.x,
            r.size.y,
            r.min.x,
            r.min.y,
            r.size.x,
            r.min.x + 8.0,
            r.min.y + 12.5,
            escape(&a.name),
            r.max().x - 8.0,
            r.min.y + 12.5,
            a.kind.label()
        );
    }

    let mut labels = String::new();
    for l in &p.links {
        let (Some(da), Some(db)) = (p.device(l.a.device), p.device(l.b.device)) else { continue };
        let (Some(a), Some(b)) = (da.port_center_by_id(l.a.port), db.port_center_by_id(l.b.port)) else { continue };
        // Only cables that start or end in view; ones just passing over
        // a detail page are noise.
        if !view.contains(a) && !view.contains(b) {
            continue;
        }
        let fam = l.cable.family();
        let dash = if cable_dashed(fam) { r#" stroke-dasharray="6 4""# } else { "" };
        let _ = write!(
            s,
            r#"<line x1="{:.1}" y1="{:.1}" x2="{:.1}" y2="{:.1}" stroke="{}" stroke-width="1.6"{dash}/>"#,
            a.x,
            a.y,
            b.x,
            b.y,
            hex(cable_rgb(fam))
        );
        let (mx, my) = ((a.x + b.x) / 2.0, (a.y + b.y) / 2.0);
        if opts.show_lengths
            && let Some(m) = l.length_m
            && view.contains(netvis_core::Pos::new(mx, my))
        {
            let text = format!("{m:.0} m");
            let tw = text.chars().count() as f32 * 4.6 + 8.0;
            let _ = write!(
                labels,
                r##"<rect x="{:.1}" y="{:.1}" width="{tw:.1}" height="12" rx="3" fill="#ffffff" stroke="#cbd5e1" stroke-width="0.6"/><text x="{mx:.1}" y="{:.1}" font-size="8" fill="#334155" text-anchor="middle">{text}</text>"##,
                mx - tw / 2.0,
                my - 6.0,
                my + 2.8
            );
        }
    }
    s.push_str(&labels);

    for d in p.devices.iter().filter(|d| visible(&d.rect())) {
        device_card(&mut s, p, d, opts);
    }
    s.push_str("</g></svg>");
    s
}

fn device_card(s: &mut String, p: &Project, d: &Device, opts: &DiagramOptions) {
    let r = d.rect();
    let cat = hex(category_rgb(d.kind.category()));
    let (x, y) = (r.min.x, r.min.y);
    let _ = write!(
        s,
        r##"<rect x="{x:.1}" y="{y:.1}" width="{:.1}" height="{:.1}" rx="5" fill="#ffffff" stroke="#d0d5dd" stroke-width="1"/><rect x="{x:.1}" y="{:.1}" width="3" height="{:.1}" fill="{cat}"/><rect x="{:.1}" y="{:.1}" width="28" height="28" rx="5" fill="{cat}" fill-opacity="0.12"/><text x="{:.1}" y="{:.1}" font-size="8.5" font-weight="bold" fill="{cat}" text-anchor="middle">{}</text>"##,
        r.size.x,
        r.size.y,
        y + 4.0,
        r.size.y - 8.0,
        x + 9.0,
        y + 9.0,
        x + 23.0,
        y + 26.0,
        escape(d.kind.name_prefix())
    );
    let sub = match (opts.show_addresses, d.primary_address()) {
        (true, Some(a)) => a.net.to_string(),
        _ => d.kind.label().to_owned(),
    };
    let chars = ((r.size.x - 50.0) / 5.6).max(6.0) as usize;
    let _ = write!(
        s,
        r##"<text x="{:.1}" y="{:.1}" font-size="10.5" font-weight="bold" fill="#111827">{}</text><text x="{:.1}" y="{:.1}" font-size="8" fill="#64748b">{}</text>"##,
        x + 44.0,
        y + 20.0,
        escape(&clip(&d.name, chars)),
        x + 44.0,
        y + 33.0,
        escape(&clip(&sub, chars + 4))
    );
    let half = layout::PORT_SIZE / 2.0;
    for (i, port) in d.ports.iter().enumerate() {
        let c = d.port_center(i);
        let fill = p.port_link(port.id).map(|l| hex(cable_rgb(l.cable.family()))).unwrap_or_else(|| "#f1f3f6".into());
        if matches!(port.media, PortMedia::Wireless | PortMedia::Virtual | PortMedia::Coax) {
            let _ = write!(s, r##"<circle cx="{:.1}" cy="{:.1}" r="{half:.1}" fill="{fill}" stroke="#969eaa" stroke-width="0.6"/>"##, c.x, c.y);
        } else {
            let _ = write!(
                s,
                r##"<rect x="{:.1}" y="{:.1}" width="{:.1}" height="{:.1}" rx="1" fill="{fill}" stroke="#969eaa" stroke-width="0.6"/>"##,
                c.x - half,
                c.y - half,
                layout::PORT_SIZE,
                layout::PORT_SIZE
            );
        }
    }
}

/// Front elevation of a rack: unit numbers and mounted devices. Returns the
/// SVG and its width and height.
pub fn rack_svg(p: &Project, rack: netvis_core::AreaId) -> (String, f32, f32) {
    const U: f32 = 12.0;
    const W: f32 = 260.0;
    const GUTTER: f32 = 26.0;
    let Some(r) = p.area(rack) else { return (String::new(), 1.0, 1.0) };
    let units = r.rack_units.max(1) as f32;
    let (w, h) = (W + GUTTER + 8.0, units * U + 16.0);
    let mut s = String::new();
    let _ = write!(
        s,
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{w:.1}" height="{h:.1}" viewBox="0 0 {w:.1} {h:.1}" font-family="{SANS}"><rect width="100%" height="100%" fill="#ffffff"/><rect x="{GUTTER}" y="4" width="{W}" height="{:.1}" rx="3" fill="#f8fafc" stroke="#64748b" stroke-width="1.5"/>"##,
        units * U + 8.0
    );
    let top = |u: f32| 8.0 + (units - u) * U;
    for u in 1..=r.rack_units.max(1) {
        let y = top(u as f32);
        let _ = write!(
            s,
            r##"<line x1="{:.1}" y1="{y:.1}" x2="{:.1}" y2="{y:.1}" stroke="#e2e8f0" stroke-width="0.5"/><text x="{:.1}" y="{:.1}" font-size="6.5" fill="#94a3b8" text-anchor="end">{u}</text>"##,
            GUTTER + 4.0,
            GUTTER + W - 4.0,
            GUTTER - 4.0,
            y + U * 0.72
        );
    }
    for m in p.rack_contents(rack) {
        let bottom = m.bottom.clamp(1, r.rack_units.max(1)) as f32;
        let height = m.height.max(1) as f32;
        let top_u = (bottom + height - 1.0).min(units);
        let (y0, y1) = (top(top_u) + 1.0, top(bottom) + U - 1.0);
        let c = hex(category_rgb(m.device.kind.category()));
        let _ = write!(
            s,
            r##"<rect x="{:.1}" y="{y0:.1}" width="{:.1}" height="{:.1}" rx="2" fill="{c}" fill-opacity="0.18" stroke="{c}" stroke-width="0.8"/><text x="{:.1}" y="{:.1}" font-size="7.5" font-weight="bold" fill="#111827">{}</text><text x="{:.1}" y="{:.1}" font-size="6.5" fill="#64748b" text-anchor="end">{}U</text>"##,
            GUTTER + 6.0,
            W - 12.0,
            y1 - y0,
            GUTTER + 12.0,
            (y0 + y1) / 2.0 + 2.6,
            escape(&clip(&m.device.name, 40)),
            GUTTER + W - 10.0,
            (y0 + y1) / 2.0 + 2.4,
            m.height
        );
    }
    s.push_str("</svg>");
    (s, w, h)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_and_clips() {
        assert_eq!(escape("A&B <x>"), "A&amp;B &lt;x&gt;");
        assert_eq!(clip("abcdef", 4), "abc…");
        assert_eq!(clip("abc", 4), "abc");
    }
}
