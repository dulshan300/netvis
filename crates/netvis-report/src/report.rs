//! Report content: cover, contents and the design sections.

use std::collections::BTreeMap;

use netvis_core::style::{ACCENT, cable_rgb};
use netvis_core::link::CableFamily;
use netvis_core::{AreaKind, DeviceCategory, DeviceKind, Item, Project};
use netvis_tools::{bom, patch};
use netvis_validate::Severity;

use crate::diagram::{self, DiagramOptions};
use crate::layout::{
    Cell, Col, Doc, FontKind, MARGIN_X, Op, SUBTEXT, TEXT, bold, col, colored, mono, num, text_width,
};
use crate::layout::{ERROR, OK, WARN};

/// Brand shown on the cover and in the running header.
pub const BRAND: &str = "Corescale";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Section {
    Summary,
    Topology,
    Devices,
    Addressing,
    Vlans,
    Routing,
    Physical,
    Patching,
    PortMaps,
    Bom,
    DesignCheck,
}

impl Section {
    pub const ALL: [Section; 11] = [
        Section::Summary,
        Section::Topology,
        Section::Devices,
        Section::Addressing,
        Section::Vlans,
        Section::Routing,
        Section::Physical,
        Section::Patching,
        Section::PortMaps,
        Section::Bom,
        Section::DesignCheck,
    ];

    /// Stable key for saved preferences.
    pub fn key(self) -> &'static str {
        match self {
            Section::Summary => "summary",
            Section::Topology => "topology",
            Section::Devices => "devices",
            Section::Addressing => "addressing",
            Section::Vlans => "vlans",
            Section::Routing => "routing",
            Section::Physical => "physical",
            Section::Patching => "patching",
            Section::PortMaps => "portmaps",
            Section::Bom => "bom",
            Section::DesignCheck => "check",
        }
    }

    /// One-line description for the export dialog.
    pub fn blurb(self) -> &'static str {
        match self {
            Section::Summary => "Project overview, counts and design-check result",
            Section::Topology => "Vector diagram of the canvas on a landscape page",
            Section::Devices => "Every device with model, location, rack position and address",
            Section::Addressing => "Subnets with usage, and every interface address",
            Section::Vlans => "VLAN database and wireless networks",
            Section::Routing => "DNS, NTP and syslog, static routes, NAT",
            Section::Physical => "Rack elevations, rack power and PoE budgets",
            Section::Patching => "Every cable run end to end through patch panels",
            Section::PortMaps => "Port-by-port tables for switches, routers, firewalls and panels",
            Section::Bom => "Hardware, optics, cabling and racks to order",
            Section::DesignCheck => "All findings from the design rules",
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Section::Summary => "Summary",
            Section::Topology => "Topology",
            Section::Devices => "Device inventory",
            Section::Addressing => "IP addressing",
            Section::Vlans => "VLANs and Wi-Fi",
            Section::Routing => "Routing and services",
            Section::Physical => "Racks and power",
            Section::Patching => "Patch schedule",
            Section::PortMaps => "Port maps",
            Section::Bom => "Bill of materials",
            Section::DesignCheck => "Design check",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ReportOptions {
    pub sections: Vec<Section>,
    /// (year, month 1–12, day)
    pub date: (u16, u8, u8),
    pub diagram: DiagramOptions,
}

impl Default for ReportOptions {
    fn default() -> Self {
        Self { sections: Section::ALL.to_vec(), date: today(), diagram: DiagramOptions::default() }
    }
}

/// Today's date (UTC) from the system clock.
pub fn today() -> (u16, u8, u8) {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    civil_from_days((secs / 86_400) as i64)
}

/// Days since 1970-01-01 to (year, month, day); Howard Hinnant's algorithm.
pub fn civil_from_days(z: i64) -> (u16, u8, u8) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u8;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u8;
    let y = (yoe + era * 400 + i64::from(m <= 2)) as u16;
    (y, m, d)
}

pub fn format_date((y, m, d): (u16, u8, u8)) -> String {
    const MONTHS: [&str; 12] =
        ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];
    format!("{d} {} {y}", MONTHS[(m.clamp(1, 12) - 1) as usize])
}

fn or_dash(s: impl Into<String>) -> String {
    let s = s.into();
    if s.trim().is_empty() { "—".into() } else { s }
}

/// Innermost area containing the device (racks excluded; they have a column).
fn location(p: &Project, d: &netvis_core::Device) -> String {
    let r = d.rect();
    let c = r.min.offset(r.size.x / 2.0, r.size.y / 2.0);
    p.areas
        .iter()
        .filter(|a| a.kind != AreaKind::Rack && a.rect.contains(c))
        .min_by(|a, b| (a.rect.size.x * a.rect.size.y).total_cmp(&(b.rect.size.x * b.rect.size.y)))
        .map(|a| a.name.clone())
        .unwrap_or_default()
}

fn rack_position(p: &Project, d: &netvis_core::Device) -> String {
    match d.hw.rack {
        Some(m) => {
            let name = p.area(m.rack).map(|a| a.name.as_str()).unwrap_or("?");
            let h = d.hw.rack_units.max(1);
            if h > 1 { format!("{name} U{}–U{}", m.u, m.u as u16 + h as u16 - 1) } else { format!("{name} U{}", m.u) }
        }
        None => String::new(),
    }
}

fn model_name(d: &netvis_core::Device) -> String {
    d.hw.model
        .as_deref()
        .and_then(netvis_core::catalog::models::model)
        .map(|m| m.name.clone())
        .or_else(|| (!d.model.is_empty()).then(|| d.model.clone()))
        .unwrap_or_default()
}

/// Build the laid-out document (no PDF yet).
pub fn build(p: &Project, opts: &ReportOptions) -> Doc {
    let mut doc = Doc::new();
    let issues = netvis_validate::validate(p);
    cover(&mut doc, p, opts);
    doc.new_plain_page(); // contents, filled in at the end
    let mut n = 0;
    for &s in &opts.sections {
        let has_content = match s {
            Section::Routing => true,
            Section::Physical => p.racks().next().is_some() || p.devices.iter().any(|d| d.hw.poe_budget_w.is_some()),
            Section::Patching => !p.links.is_empty(),
            Section::PortMaps => port_map_devices(p).next().is_some(),
            _ => true,
        };
        if !has_content {
            continue;
        }
        n += 1;
        match s {
            Section::Summary => summary(&mut doc, n, p, &issues, opts),
            Section::Topology => topology(&mut doc, n, p, opts),
            Section::Devices => devices(&mut doc, n, p),
            Section::Addressing => addressing(&mut doc, n, p),
            Section::Vlans => vlans(&mut doc, n, p),
            Section::Routing => routing(&mut doc, n, p),
            Section::Physical => physical(&mut doc, n, p),
            Section::Patching => patching(&mut doc, n, p),
            Section::PortMaps => port_maps(&mut doc, n, p),
            Section::Bom => bill(&mut doc, n, p),
            Section::DesignCheck => design_check(&mut doc, n, p, &issues),
        }
    }
    contents(&mut doc);
    doc
}

fn cover(doc: &mut Doc, p: &Project, opts: &ReportOptions) {
    doc.new_plain_page();
    let (w, h) = crate::layout::A4;
    let m = &p.meta;
    // Brand band.
    doc.push(Op::Rect { x: 0.0, y: 0.0, w, h: 150.0, fill: Some(ACCENT), stroke: None });
    doc.text_at(MARGIN_X, 70.0, 26.0, FontKind::Bold, [255, 255, 255], BRAND.to_uppercase());
    doc.text_at(MARGIN_X, 96.0, 11.0, FontKind::Regular, [219, 234, 254], "Network design");

    doc.text_at(MARGIN_X, 230.0, 12.0, FontKind::Bold, ACCENT, "NETWORK DESIGN REPORT");
    let title_lines = crate::layout::wrap(FontKind::Bold, 28.0, &m.name, w - 2.0 * MARGIN_X);
    let mut y = 268.0;
    for l in title_lines {
        doc.text_at(MARGIN_X, y, 28.0, FontKind::Bold, TEXT, l);
        y += 34.0;
    }
    if !m.description.trim().is_empty() {
        for l in crate::layout::wrap(FontKind::Regular, 11.0, &m.description, w - 2.0 * MARGIN_X) {
            doc.text_at(MARGIN_X, y, 11.0, FontKind::Regular, SUBTEXT, l);
            y += 15.0;
        }
    }

    // Topology preview.
    if !p.devices.is_empty() {
        let dopts = DiagramOptions { show_lengths: false, ..opts.diagram };
        let (dw, dh) = diagram::size(p, &dopts);
        doc.set_y(y + 20.0);
        let box_h = (h - 230.0 - (y + 20.0)).max(120.0);
        doc.svg(diagram::topology_svg(p, &dopts), dw, dh, Some(box_h));
    }

    // Details block.
    let rows = [
        ("Client", or_dash(m.client.clone())),
        ("Prepared by", or_dash([m.author.as_str(), m.company.as_str()].iter().filter(|s| !s.is_empty()).copied().collect::<Vec<_>>().join(", "))),
        ("Revision", or_dash(m.revision.clone())),
        ("Date", format_date(opts.date)),
    ];
    let mut y = h - 170.0;
    doc.push(Op::Line { x1: MARGIN_X, y1: y - 18.0, x2: w - MARGIN_X, y2: y - 18.0, color: crate::layout::RULE, width: 0.8 });
    for (k, v) in rows {
        doc.text_at(MARGIN_X, y, 10.0, FontKind::Regular, SUBTEXT, k);
        doc.text_at(MARGIN_X + 90.0, y, 10.0, FontKind::Bold, TEXT, v);
        y += 18.0;
    }
    doc.text_at(MARGIN_X, h - 40.0, 7.5, FontKind::Regular, SUBTEXT, "Generated with NetVis");
}

fn contents(doc: &mut Doc) {
    let entries = doc.toc.clone();
    let page = 1;
    let (w, _) = crate::layout::A4;
    let mut ops = vec![Op::Text { x: MARGIN_X, y: 82.0, size: 18.0, font: FontKind::Bold, color: TEXT, text: "Contents".into() }];
    let mut y = 120.0;
    let max_y = crate::layout::A4.1 - 70.0;
    for e in entries.iter().filter(|e| e.level == 0).chain(entries.iter().filter(|e| e.level == 1)).take(0) {
        let _ = e;
    }
    for e in &entries {
        if y > max_y {
            break;
        }
        let (size, font, indent, color) = if e.level == 0 { (11.0, FontKind::Bold, 0.0, TEXT) } else { (9.5, FontKind::Regular, 18.0, SUBTEXT) };
        if e.level == 0 && y > 120.0 {
            y += 6.0;
        }
        let x = MARGIN_X + indent;
        let num = (e.page + 1).to_string();
        let nx = w - MARGIN_X - text_width(font, size, &num);
        ops.push(Op::Text { x, y, size, font, color, text: e.title.clone() });
        // Dot leader.
        let start = x + text_width(font, size, &e.title) + 6.0;
        let dot = text_width(FontKind::Regular, size, ". ");
        let count = ((nx - 6.0 - start) / dot).max(0.0) as usize;
        if count > 0 {
            ops.push(Op::Text { x: start, y, size, font: FontKind::Regular, color: crate::layout::RULE, text: ". ".repeat(count) });
        }
        ops.push(Op::Text { x: nx, y, size, font, color, text: num });
        ops.push(Op::Link { x: MARGIN_X, y: y - size, w: w - 2.0 * MARGIN_X, h: size * 1.4, page: e.page });
        y += size * 1.65;
    }
    doc.pages[page].ops = ops;
}

fn summary(doc: &mut Doc, n: usize, p: &Project, issues: &[netvis_validate::Issue], _opts: &ReportOptions) {
    doc.section(n, Section::Summary.title(), false);
    if !p.meta.description.trim().is_empty() {
        doc.paragraph(&p.meta.description, 10.0, TEXT);
    }
    let count = |c: DeviceCategory| p.devices.iter().filter(|d| d.kind.category() == c).count();
    let cables = p.links.iter().filter(|l| !matches!(l.cable.family(), CableFamily::Logical | CableFamily::Wireless)).count();
    let sites: Vec<&str> = p.areas.iter().filter(|a| a.kind == AreaKind::Site).map(|a| a.name.as_str()).collect();
    let mut rows = vec![("Client", or_dash(p.meta.client.clone()))];
    if !sites.is_empty() {
        rows.push(("Sites", sites.join(", ")));
    }
    rows.extend([
        (
            "Devices",
            format!(
                "{} — {} network, {} end devices, {} infrastructure, {} cloud/WAN",
                p.devices.len(),
                count(DeviceCategory::Network),
                count(DeviceCategory::EndDevice),
                count(DeviceCategory::Infrastructure),
                count(DeviceCategory::CloudWan)
            ),
        ),
        ("Cables", format!("{cables} cables in {} runs", p.circuits().len())),
        ("VLANs", p.vlans.len().to_string()),
        ("Subnets", p.subnets.len().to_string()),
        ("Wireless networks", p.ssids.len().to_string()),
        ("Racks", p.racks().count().to_string()),
    ]);
    doc.kv(&rows);

    let (e, w, i) = issues.iter().fold((0, 0, 0), |(e, w, i), x| match x.severity {
        Severity::Error => (e + 1, w, i),
        Severity::Warning => (e, w + 1, i),
        Severity::Info => (e, w, i + 1),
    });
    doc.subsection("Design check result", 30.0);
    let (text, color) = match (e, w) {
        (0, 0) => (format!("No errors or warnings. {i} informational note(s)."), OK),
        _ => (format!("{e} error(s), {w} warning(s) and {i} note(s) — listed under Design check."), if e > 0 { ERROR } else { WARN }),
    };
    doc.paragraph(&text, 10.0, color);

    doc.subsection("Devices by type", 60.0);
    let mut by_kind: BTreeMap<(u8, &str), usize> = BTreeMap::new();
    for d in &p.devices {
        *by_kind.entry((d.kind.category() as u8, d.kind.label())).or_default() += 1;
    }
    let rows: Vec<Vec<Cell>> = by_kind.into_iter().map(|((_, k), n)| vec![k.into(), n.to_string().into()]).collect();
    doc.table(&[col("Type", 3.0), num("Count", 1.0)], &rows);
}

fn topology(doc: &mut Doc, n: usize, p: &Project, opts: &ReportOptions) {
    doc.section(n, Section::Topology.title(), true);
    if p.devices.is_empty() {
        doc.paragraph("No devices yet.", 10.0, SUBTEXT);
        return;
    }
    let (w, h) = diagram::size(p, &opts.diagram);
    let legend_h = 22.0;
    let max_h = doc.remaining() - legend_h;
    doc.svg(diagram::topology_svg(p, &opts.diagram), w, h, Some(max_h));
    // Legend: cable families in use.
    let mut fams: Vec<CableFamily> = Vec::new();
    for l in &p.links {
        if !fams.contains(&l.cable.family()) {
            fams.push(l.cable.family());
        }
    }
    let mut x = MARGIN_X;
    let y = doc.y() + 6.0;
    for f in fams {
        let label = match f {
            CableFamily::Copper => "Copper",
            CableFamily::Multimode => "Multimode fibre",
            CableFamily::SingleMode => "Single-mode fibre",
            CableFamily::DirectAttach => "DAC",
            CableFamily::Coax => "Coax",
            CableFamily::Wireless => "Wireless",
            CableFamily::Console => "Console",
            CableFamily::Logical => "Logical / WAN",
        };
        doc.push(Op::Line { x1: x, y1: y - 3.0, x2: x + 18.0, y2: y - 3.0, color: cable_rgb(f), width: 2.0 });
        doc.text_at(x + 22.0, y, 8.0, FontKind::Regular, SUBTEXT, label);
        x += 30.0 + text_width(FontKind::Regular, 8.0, label);
    }
    doc.set_y(y + 10.0);
}

fn devices(doc: &mut Doc, n: usize, p: &Project) {
    doc.section(n, Section::Devices.title(), false);
    let mut list: Vec<_> = p.devices.iter().filter(|d| d.kind.category() != DeviceCategory::CloudWan).collect();
    list.sort_by_key(|d| (d.kind.category() as u8, d.name.clone()));
    let rows: Vec<Vec<Cell>> = list
        .iter()
        .map(|d| {
            let used = d.ports.iter().filter(|x| p.port_link(x.id).is_some()).count();
            vec![
                bold(d.name.clone()),
                d.kind.label().into(),
                or_dash(model_name(d)).into(),
                or_dash(location(p, d)).into(),
                or_dash(rack_position(p, d)).into(),
                or_dash(d.primary_address().map(|a| a.net.to_string()).unwrap_or_default()).into(),
                format!("{used}/{}", d.ports.len()).into(),
            ]
        })
        .collect();
    doc.table(
        &[col("Name", 1.3), col("Type", 1.1), col("Model", 2.4), col("Location", 1.2), col("Rack", 1.2), mono("Address", 1.5), num("Ports", 0.7)],
        &rows,
    );
}

fn addressing(doc: &mut Doc, n: usize, p: &Project) {
    doc.section(n, Section::Addressing.title(), false);
    doc.subsection("Subnets", 60.0);
    if p.subnets.is_empty() {
        doc.paragraph("No subnets planned.", 10.0, SUBTEXT);
    } else {
        let mut subnets: Vec<_> = p.subnets.iter().collect();
        subnets.sort_by_key(|s| (s.net.addr().is_ipv6(), netvis_core::ip::to_u128(s.net.network()), s.net.prefix_len()));
        let rows: Vec<Vec<Cell>> = subnets
            .iter()
            .map(|s| {
                let u = p.subnet_usage(s);
                let used = u.usable.saturating_sub(u.free);
                let dhcp = s.dhcp.as_ref().map(|d| format!("{} – {}", d.start, d.end)).unwrap_or_default();
                vec![
                    bold(s.name.clone()),
                    s.net.to_string().into(),
                    or_dash(s.vlan.map(|v| p.vlan_label(Some(v))).unwrap_or_default()).into(),
                    or_dash(s.gateway.map(|g| g.to_string()).unwrap_or_default()).into(),
                    or_dash(dhcp).into(),
                    format!("{used}/{} ({:.0}%)", u.usable, u.used_fraction() * 100.0).into(),
                ]
            })
            .collect();
        doc.table(&[col("Name", 1.4), mono("Network", 1.4), col("VLAN", 1.2), mono("Gateway", 1.2), mono("DHCP range", 2.2), num("Used", 1.1)], &rows);
    }

    doc.subsection("Interface addresses", 60.0);
    let mut assigned = p.assignments();
    assigned.sort_by_key(|a| (a.addr.ip().is_ipv6(), netvis_core::ip::to_u128(a.addr.ip())));
    if assigned.is_empty() {
        doc.paragraph("No static addresses or reservations yet.", 10.0, SUBTEXT);
        return;
    }
    let rows: Vec<Vec<Cell>> = assigned
        .iter()
        .filter_map(|a| {
            let d = p.device(a.device)?;
            Some(vec![
                a.addr.net.to_string().into(),
                bold(d.name.clone()),
                d.iface_name(a.iface).unwrap_or("?").into(),
                or_dash(p.subnet_containing(a.addr.ip()).map(|s| s.name.clone()).unwrap_or_default()).into(),
                a.addr.kind.label().into(),
            ])
        })
        .collect();
    doc.table(&[mono("Address", 1.6), col("Device", 1.4), col("Interface", 1.0), col("Subnet", 1.4), col("Type", 1.2)], &rows);
}

fn vlans(doc: &mut Doc, n: usize, p: &Project) {
    doc.section(n, Section::Vlans.title(), false);
    doc.subsection("VLANs", 60.0);
    let mut vl: Vec<_> = p.vlans.iter().collect();
    vl.sort_by_key(|v| v.tag);
    if vl.is_empty() {
        doc.paragraph("No VLANs defined.", 10.0, SUBTEXT);
    } else {
        let rows: Vec<Vec<Cell>> = vl
            .iter()
            .map(|v| {
                let u = p.vlan_usage(v.id);
                let subnets: Vec<String> = u.subnets.iter().filter_map(|s| p.subnet(*s)).map(|s| s.net.to_string()).collect();
                vec![
                    Cell { text: v.tag.to_string(), color: Some(v.color), bold: true },
                    bold(v.name.clone()),
                    or_dash(v.purpose.clone()).into(),
                    or_dash(subnets.join(", ")).into(),
                    u.access_ports.to_string().into(),
                    u.trunk_ports.to_string().into(),
                ]
            })
            .collect();
        doc.table(&[num("Tag", 0.6), col("Name", 1.3), col("Purpose", 2.4), mono("Subnet", 1.6), num("Access", 0.8), num("Trunks", 0.8)], &rows);
    }
    doc.subsection("Wireless networks", 50.0);
    if p.ssids.is_empty() {
        doc.paragraph("No SSIDs defined.", 10.0, SUBTEXT);
        return;
    }
    let aps = p.devices.iter().filter(|d| d.kind == DeviceKind::AccessPoint).count();
    let rows: Vec<Vec<Cell>> = p
        .ssids
        .iter()
        .map(|s| {
            vec![
                bold(s.name.clone()),
                or_dash(s.vlan.map(|v| p.vlan_label(Some(v))).unwrap_or_default()).into(),
                s.security.label().into(),
                s.bands.iter().map(|b| b.label()).collect::<Vec<_>>().join(", ").into(),
                if s.hidden { "Hidden" } else { "Broadcast" }.into(),
            ]
        })
        .collect();
    doc.table(&[col("SSID", 1.5), col("VLAN", 1.3), col("Security", 1.6), col("Bands", 1.4), col("Visibility", 0.9)], &rows);
    doc.paragraph(&format!("Served by {aps} access point(s)."), 9.0, SUBTEXT);
}

fn routing(doc: &mut Doc, n: usize, p: &Project) {
    doc.section(n, Section::Routing.title(), false);
    let s = &p.services;
    doc.subsection("Network services", 40.0);
    doc.kv(&[
        ("Domain", or_dash(s.domain.clone())),
        ("DNS servers", or_dash(s.dns_servers.iter().map(|a| a.to_string()).collect::<Vec<_>>().join(", "))),
        ("NTP servers", or_dash(s.ntp_servers.join(", "))),
        ("Syslog", or_dash(s.syslog.clone())),
    ]);
    let routes: Vec<Vec<Cell>> = p
        .devices
        .iter()
        .flat_map(|d| d.routing.static_routes.iter().map(move |r| (d, r)))
        .map(|(d, r)| {
            vec![
                bold(d.name.clone()),
                r.dest.to_string().into(),
                or_dash(r.next_hop.map(|h| h.to_string()).unwrap_or_default()).into(),
                or_dash(r.interface.clone()).into(),
                r.distance.to_string().into(),
                or_dash(r.description.clone()).into(),
            ]
        })
        .collect();
    doc.subsection("Static routes", 40.0);
    if routes.is_empty() {
        doc.paragraph("No static routes.", 10.0, SUBTEXT);
    } else {
        doc.table(&[col("Device", 1.2), mono("Destination", 1.4), mono("Next hop", 1.3), col("Interface", 1.0), num("AD", 0.5), col("Description", 2.2)], &routes);
    }
    let nat: Vec<Vec<Cell>> = p
        .devices
        .iter()
        .flat_map(|d| d.routing.nat.iter().map(move |r| (d, r)))
        .map(|(d, r)| {
            vec![
                bold(d.name.clone()),
                r.kind.label().into(),
                or_dash(r.inside.clone()).into(),
                or_dash(r.outside.clone()).into(),
                or_dash(r.service.clone()).into(),
                or_dash(r.description.clone()).into(),
            ]
        })
        .collect();
    doc.subsection("NAT", 40.0);
    if nat.is_empty() {
        doc.paragraph("No NAT rules.", 10.0, SUBTEXT);
    } else {
        doc.table(&[col("Device", 1.1), col("Type", 1.0), col("Inside", 1.5), col("Outside", 1.6), col("Service", 0.8), col("Description", 1.8)], &nat);
    }
}

fn physical(doc: &mut Doc, n: usize, p: &Project) {
    doc.section(n, Section::Physical.title(), false);
    for rack in p.racks() {
        let contents = p.rack_contents(rack.id);
        let used: u32 = contents.iter().map(|m| m.height as u32).sum();
        let power = p.rack_power(rack.id);
        doc.subsection(&format!("{} ({}U)", rack.name, rack.rack_units), 80.0);
        let mut line = format!("{used} of {}U used · {:.0} W load", rack.rack_units, power.load_w);
        if power.ups_capacity_w > 0.0 {
            line.push_str(&format!(" on {:.0} W of UPS capacity ({:.0}%)", power.ups_capacity_w, power.load_w / power.ups_capacity_w * 100.0));
        }
        if power.unknown > 0 {
            line.push_str(&format!(" · {} device(s) without a power figure", power.unknown));
        }
        doc.paragraph(&line, 9.0, SUBTEXT);
        if !contents.is_empty() {
            let (svg, w, h) = diagram::rack_svg(p, rack.id);
            doc.ensure(160.0);
            doc.svg(svg, w, h, Some(300.0));
        }
        let mut sorted: Vec<_> = contents.iter().collect();
        sorted.sort_by_key(|m| std::cmp::Reverse(m.bottom));
        let rows: Vec<Vec<Cell>> = sorted
            .iter()
            .map(|m| {
                let units = if m.height > 1 { format!("U{}–U{}", m.bottom, m.top()) } else { format!("U{}", m.bottom) };
                let over = m.bottom == 0 || m.top() > rack.rack_units;
                vec![
                    if over { colored(units, ERROR) } else { bold(units) },
                    m.device.name.clone().into(),
                    or_dash(model_name(m.device)).into(),
                    format!("{}U", m.height).into(),
                    m.device.hw.power_w.map(|w| format!("{w:.0} W")).unwrap_or_else(|| "—".into()).into(),
                ]
            })
            .collect();
        if rows.is_empty() {
            doc.paragraph("Nothing mounted yet.", 9.0, SUBTEXT);
        } else {
            doc.table(&[col("Position", 1.0), col("Device", 1.3), col("Model", 2.8), num("Height", 0.6), num("Power", 0.7)], &rows);
        }
    }

    let poe: Vec<Vec<Cell>> = p
        .devices
        .iter()
        .filter_map(|d| {
            let r = p.poe_report(d.id);
            (r.pse_ports > 0).then(|| {
                let status = match r.budget_w {
                    Some(b) if r.demand_w > b => colored("Over budget", ERROR),
                    Some(b) if r.demand_w > 0.9 * b => colored("Little headroom", WARN),
                    Some(_) => colored("OK", OK),
                    None => "No budget set".into(),
                };
                vec![
                    bold(d.name.clone()),
                    r.loads.len().to_string().into(),
                    format!("{:.0} W", r.demand_w).into(),
                    r.budget_w.map(|b| format!("{b:.0} W")).unwrap_or_else(|| "—".into()).into(),
                    status,
                ]
            })
        })
        .collect();
    if !poe.is_empty() {
        doc.subsection("PoE budgets", 50.0);
        doc.table(&[col("Switch", 1.4), num("Powered devices", 1.0), num("Demand", 0.8), num("Budget", 0.8), col("Status", 1.2)], &poe);
    }
}

fn patching(doc: &mut Doc, n: usize, p: &Project) {
    doc.section(n, Section::Patching.title(), true);
    doc.paragraph(
        "Every cable run end to end. Runs through patch panels and wall outlets list the panel ports they pass; lengths are the sum of all cables in the run.",
        9.0,
        SUBTEXT,
    );
    let rows: Vec<Vec<Cell>> = patch::schedule(p)
        .into_iter()
        .map(|r| {
            let cables: Vec<String> = r
                .segments
                .iter()
                .map(|s| match s.length_m {
                    Some(m) => format!("{} {m:.0} m", s.cable),
                    None => s.cable.clone(),
                })
                .collect();
            let labels: Vec<&str> = r.segments.iter().map(|s| s.label.as_str()).filter(|s| !s.is_empty()).collect();
            let to = if r.spare {
                colored(format!("{} (spare)", r.b), SUBTEXT)
            } else if r.unterminated {
                colored(format!("{} — not patched", r.b), WARN)
            } else {
                r.b.clone().into()
            };
            vec![
                bold(r.id.clone()),
                r.a.clone().into(),
                or_dash(r.via.clone()).into(),
                to,
                cables.join(" + ").into(),
                r.length_m.map(|m| format!("{m:.0} m")).unwrap_or_else(|| "—".into()).into(),
                or_dash(r.service.clone()).into(),
                or_dash(labels.join(", ")).into(),
            ]
        })
        .collect();
    doc.table(
        &[col("Run", 0.6), col("From", 1.5), col("Via", 1.8), col("To", 1.6), col("Cables", 2.2), num("Length", 0.6), col("Service", 1.5), mono("Labels", 1.1)],
        &rows,
    );
}

fn port_map_devices(p: &Project) -> impl Iterator<Item = &netvis_core::Device> {
    p.devices.iter().filter(|d| {
        matches!(d.kind, DeviceKind::SwitchL2 | DeviceKind::SwitchL3 | DeviceKind::Router | DeviceKind::Firewall | DeviceKind::PatchPanel)
            && d.ports.iter().any(|x| p.port_link(x.id).is_some())
    })
}

fn port_maps(doc: &mut Doc, n: usize, p: &Project) {
    doc.section(n, Section::PortMaps.title(), true);
    let mut list: Vec<_> = port_map_devices(p).collect();
    list.sort_by_key(|d| (d.kind == DeviceKind::PatchPanel, d.name.clone()));
    let cols: Vec<Col> = vec![
        mono("Port", 0.9),
        col("State", 1.0),
        col("Media", 1.0),
        num("Speed", 0.5),
        col("Mode / VLAN", 1.5),
        col("Connected to", 1.8),
        col("Via", 1.6),
        col("Cables", 2.0),
        mono("Labels", 0.9),
        col("PoE", 0.6),
    ];
    for (i, d) in list.into_iter().enumerate() {
        if i > 0 {
            doc.space(6.0);
        }
        let rows = patch::port_map(p, d.id);
        let used = rows.iter().filter(|r| r.state != patch::PortState::Free).count();
        doc.subsection(&format!("{} — {}", d.name, or_dash(model_name(d))), 60.0);
        doc.paragraph(&format!("{used} of {} ports in use · {}", rows.len(), or_dash(rack_position(p, d))), 8.5, SUBTEXT);
        let mut body: Vec<Vec<Cell>> = Vec::new();
        // Consecutive free ports of the same media and speed collapse into one row.
        let mut i = 0;
        while i < rows.len() {
            let r = &rows[i];
            if r.state == patch::PortState::Free {
                let mut j = i;
                while j + 1 < rows.len() && rows[j + 1].state == patch::PortState::Free && rows[j + 1].media == r.media && rows[j + 1].speed == r.speed {
                    j += 1;
                }
                if j > i {
                    let mut row: Vec<Cell> = vec![
                        bold(format!("{}–{}", r.name, rows[j].name)),
                        colored(format!("Free ({} ports)", j - i + 1), SUBTEXT),
                        r.media.clone().into(),
                        r.speed.clone().into(),
                    ];
                    row.extend(std::iter::repeat_with(Cell::default).take(5));
                    row.push(r.poe.clone().into());
                    body.push(row);
                    i = j + 1;
                    continue;
                }
            }
            body.push(port_row(r));
            i += 1;
        }
        doc.table(&cols, &body);
    }
}

fn port_row(r: &patch::PortRow) -> Vec<Cell> {
    let state = match r.state {
        patch::PortState::Up => colored(r.state.label(), OK),
        patch::PortState::Unterminated => colored(r.state.label(), WARN),
        patch::PortState::Disabled => colored(r.state.label(), ERROR),
        _ => colored(r.state.label(), SUBTEXT),
    };
    vec![
        bold(r.name.clone()),
        state,
        r.media.clone().into(),
        r.speed.clone().into(),
        r.mode.clone().into(),
        r.far.clone().into(),
        r.via.clone().into(),
        r.cables.clone().into(),
        r.labels.clone().into(),
        r.poe.clone().into(),
    ]
}

fn bill(doc: &mut Doc, n: usize, p: &Project) {
    doc.section(n, Section::Bom.title(), false);
    let lines = bom::build(p, bom::BomOptions::default());
    if lines.is_empty() {
        doc.paragraph("Nothing to list yet.", 10.0, SUBTEXT);
        return;
    }
    let mut section = None;
    let mut rows: Vec<Vec<Cell>> = Vec::new();
    for l in &lines {
        let sec = if section != Some(l.section) {
            section = Some(l.section);
            bold(l.section.label())
        } else {
            "".into()
        };
        rows.push(vec![
            sec,
            l.item.clone().into(),
            bold(l.qty.to_string()),
            l.length_m.map(|m| format!("{m:.0} m")).unwrap_or_default().into(),
            l.notes.clone().into(),
        ]);
    }
    doc.table(&[col("Section", 0.8), col("Item", 2.6), num("Qty", 0.5), num("Order", 0.7), col("Notes", 2.4)], &rows);
    doc.paragraph("Cable quantities count one cable per link and add 10% slack to measured lengths. Links without a length need measuring before ordering.", 8.5, SUBTEXT);
}

fn design_check(doc: &mut Doc, n: usize, p: &Project, issues: &[netvis_validate::Issue]) {
    doc.section(n, Section::DesignCheck.title(), false);
    if issues.is_empty() {
        doc.paragraph("The design passes every check: addressing, VLANs, Wi-Fi, names, topology, cabling, power and racks.", 10.0, OK);
        return;
    }
    doc.paragraph("Findings from the NetVis design rules at the time of export.", 9.0, SUBTEXT);
    let rows: Vec<Vec<Cell>> = issues
        .iter()
        .map(|i| {
            let color = match i.severity {
                Severity::Error => ERROR,
                Severity::Warning => WARN,
                Severity::Info => SUBTEXT,
            };
            let what: Vec<String> = i
                .items
                .iter()
                .filter_map(|it| match it {
                    Item::Device(d) => p.device(*d).map(|d| d.name.clone()),
                    _ => None,
                })
                .take(4)
                .collect();
            vec![colored(i.severity.label(), color), i.code.into(), i.message.clone().into(), or_dash(what.join(", ")).into()]
        })
        .collect();
    doc.table(&[col("Severity", 0.8), mono("Rule", 1.2), col("Finding", 4.0), col("Devices", 1.4)], &rows);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(20_746), (2026, 10, 20));
        assert_eq!(civil_from_days(11_016), (2000, 2, 29));
        assert_eq!(format_date((2026, 10, 20)), "20 October 2026");
    }
}
