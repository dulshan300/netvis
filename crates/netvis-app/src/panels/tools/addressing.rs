//! Subnet calculator, VLSM, address notation and MAC vendor lookup.

use std::net::IpAddr;

use egui::{DragValue, RichText, Ui};
use netvis_core::catalog::oui;
use netvis_core::ip::{self, IpNet, Ipv6Net, Mac, MacStyle, SubnetInfo, repr};
use netvis_core::{Edit, Subnet};

use super::{hint, kv, kv_grid};
use crate::icons;
use crate::state::State;
use crate::widgets::{self, ERROR, WARN, fmt_count};

// ---- subnet calculator -----------------------------------------------------

#[derive(Debug)]
pub struct SubnetTool {
    input: String,
    split: u8,
    summarize: String,
}

impl Default for SubnetTool {
    fn default() -> Self {
        Self { input: "192.168.10.77/26".into(), split: 28, summarize: "10.1.0.0/24\n10.1.1.0/24\n10.1.2.0/24\n10.1.3.0/24".into() }
    }
}

pub fn subnet(ui: &mut Ui, st: &mut State) {
    let t = &mut st.tools.subnet;
    hint(ui, "Accepts 10.0.0.5/24, 10.0.0.5 255.255.255.0, a bare address, or IPv6. Click any value to copy it.");
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.label("Address / prefix");
        ui.add(egui::TextEdit::singleline(&mut t.input).desired_width(260.0).font(egui::TextStyle::Monospace));
    });
    let parsed = ip::parse_net(&t.input);
    let net = match parsed {
        Ok(n) => n,
        Err(e) => {
            ui.label(RichText::new(e.to_string()).color(ERROR));
            return;
        }
    };
    let info = SubnetInfo::of(&net);
    ui.add_space(6.0);
    kv_grid(ui, "tools.subnet", |ui| {
        if net.addr() != info.network.network() {
            let role = if ip::is_usable_host(&info.network, net.addr()) { "host" } else { "not a usable host" };
            kv(ui, "Address", format!("{}  ({role})", net.addr()));
        }
        kv(ui, "Network", info.network.to_string());
        kv(ui, "Netmask", format!("{}  (/{})", info.netmask, info.network.prefix_len()));
        if let Some(w) = info.wildcard {
            kv(ui, "Wildcard", w.to_string());
        }
        if let Some(b) = info.broadcast {
            kv(ui, "Broadcast", b.to_string());
        }
        kv(ui, "Usable range", format!("{} – {}", info.first_usable, info.last_usable));
        kv(ui, "Usable hosts", fmt_count(info.usable));
        kv(ui, "Total addresses", fmt_count(info.total));
        kv(ui, "Scope", info.scope);
        if let IpNet::V4(_) = net {
            kv(ui, "Mask (binary)", repr::binary(info.netmask));
            kv(ui, "Network (binary)", repr::binary(info.network.network()));
        }
    });

    ui.add_space(4.0);
    let p = &st.doc.project;
    let clash = p.subnets.iter().find(|s| ip::overlaps(&s.net, &info.network)).map(|s| (s.name.clone(), s.net));
    ui.horizontal(|ui| {
        let add = ui.add_enabled(clash.is_none(), egui::Button::new(format!("{} Add to project", icons::PLUS)));
        let add = match clash {
            Some((name, net)) => add.on_disabled_hover_text(format!("Overlaps {name} ({net})")),
            None => add.on_hover_text("Create this subnet with a gateway on the first address and a DHCP scope"),
        };
        if add.clicked() {
            let s = Subnet::new(format!("Subnet {}", info.network), info.network).with_defaults(true, true);
            let id = s.id;
            st.apply(Edit::Subnet { id, before: None, after: Some(s) }, "Add subnet");
            st.sel.set_single(netvis_core::Item::Subnet(id));
            st.set_status(format!("Added {} to the IP plan", info.network));
        }
    });

    // Split.
    let t = &mut st.tools.subnet;
    widgets::section(ui, "Split");
    let width = if net.addr().is_ipv4() { 32 } else { 128 };
    let min = info.network.prefix_len().saturating_add(1).min(width);
    t.split = t.split.clamp(min, width);
    ui.horizontal(|ui| {
        ui.label(format!("{} into", info.network));
        ui.add(DragValue::new(&mut t.split).range(min..=width).prefix("/"));
    });
    match ip::split(&info.network, t.split) {
        Ok(parts) => {
            let each = SubnetInfo::of(&parts[0]).usable;
            hint(ui, &format!("{} subnets of {} usable hosts", parts.len(), fmt_count(each)));
            subnet_list(ui, &parts, 64);
        }
        Err(e) => {
            ui.label(RichText::new(e.to_string()).color(WARN));
        }
    }

    // Summarise.
    widgets::section(ui, "Summarise routes");
    hint(ui, "One network per line (or comma-separated).");
    ui.add(egui::TextEdit::multiline(&mut t.summarize).desired_rows(4).desired_width(260.0).font(egui::TextStyle::Monospace));
    let (nets, bad): (Vec<_>, Vec<_>) =
        t.summarize.split(['\n', ',', ' ']).map(str::trim).filter(|s| !s.is_empty()).map(ip::parse_net).partition(Result::is_ok);
    if !bad.is_empty() {
        ui.label(RichText::new(format!("{} line(s) are not valid networks", bad.len())).color(WARN));
    }
    let nets: Vec<IpNet> = nets.into_iter().map(|n| n.expect("ok").trunc()).collect();
    if !nets.is_empty() {
        kv_grid(ui, "tools.summary", |ui| {
            let agg = ip::summarize(&nets);
            kv(ui, "Exact summary", agg.iter().map(|n| n.to_string()).collect::<Vec<_>>().join(", "));
            match ip::supernet(&nets) {
                Ok(Some(s)) => kv(ui, "Single supernet", format!("{s}  (covers {} addresses)", fmt_count(ip::total_addresses(&s)))),
                Ok(None) => {}
                Err(e) => kv(ui, "Single supernet", e.to_string()),
            }
        });
    }
}

fn subnet_list(ui: &mut Ui, nets: &[IpNet], show: usize) {
    egui::Grid::new(("tools.split", nets.len())).num_columns(3).spacing([14.0, 2.0]).striped(true).show(ui, |ui| {
        for h in ["Subnet", "Usable range", "Broadcast"] {
            ui.label(RichText::new(h).small().weak());
        }
        ui.end_row();
        for n in nets.iter().take(show) {
            let i = SubnetInfo::of(n);
            ui.label(RichText::new(n.to_string()).monospace());
            ui.label(RichText::new(format!("{} – {}", i.first_usable, i.last_usable)).monospace());
            ui.label(RichText::new(i.broadcast.map(|b| b.to_string()).unwrap_or_else(|| "—".into())).monospace());
            ui.end_row();
        }
    });
    if nets.len() > show {
        hint(ui, &format!("…and {} more", nets.len() - show));
    }
}

// ---- VLSM ------------------------------------------------------------------

#[derive(Debug)]
pub struct VlsmTool {
    parent: String,
    rows: String,
}

impl Default for VlsmTool {
    fn default() -> Self {
        Self {
            parent: "172.16.0.0/22".into(),
            rows: "Staff, 200\nVoice, 100\nCCTV, 60\nManagement, 25\nWAN link, 2".into(),
        }
    }
}

/// `Name, hosts` (or `Name hosts`, or just a number).
fn parse_rows(text: &str) -> (Vec<(String, u32)>, usize) {
    let mut rows = Vec::new();
    let mut bad = 0;
    for (i, line) in text.lines().map(str::trim).filter(|l| !l.is_empty()).enumerate() {
        let (name, n) = match line.rsplit_once([',', '\t', ' ']) {
            Some((name, n)) => (name.trim().trim_end_matches(',').trim().to_owned(), n.trim()),
            None => (format!("Subnet {}", i + 1), line),
        };
        match n.parse::<u32>() {
            Ok(h) if h > 0 => rows.push((if name.is_empty() { format!("Subnet {}", i + 1) } else { name }, h)),
            _ => bad += 1,
        }
    }
    (rows, bad)
}

pub fn vlsm(ui: &mut Ui, st: &mut State) {
    let t = &mut st.tools.vlsm;
    hint(ui, "Largest subnets are placed first so every block stays aligned.");
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.label("Address block");
        ui.add(egui::TextEdit::singleline(&mut t.parent).desired_width(180.0).font(egui::TextStyle::Monospace));
    });
    ui.label("Subnets — one per line as “Name, hosts”");
    ui.add(egui::TextEdit::multiline(&mut t.rows).desired_rows(6).desired_width(260.0));

    let parent = ip::parse_net(&t.parent).map(|n| n.trunc());
    let (rows, bad) = parse_rows(&t.rows);
    if bad > 0 {
        ui.label(RichText::new(format!("{bad} line(s) skipped — expected “Name, hosts”")).color(WARN));
    }
    let parent = match parent {
        Ok(p) => p,
        Err(e) => {
            ui.label(RichText::new(e.to_string()).color(ERROR));
            return;
        }
    };
    let hosts: Vec<u128> = rows.iter().map(|r| r.1 as u128).collect();
    let plan = match ip::vlsm(&parent, &hosts) {
        Ok(p) => p,
        Err(e) => {
            ui.label(RichText::new(format!("{} {e}", icons::WARNING)).color(ERROR));
            return;
        }
    };

    ui.add_space(6.0);
    let mut tsv = String::from("Name\tHosts\tSubnet\tMask\tGateway\tUsable range\tSpare\n");
    egui::Grid::new("tools.vlsm").num_columns(6).spacing([14.0, 3.0]).striped(true).show(ui, |ui| {
        for h in ["Name", "Needed", "Subnet", "Mask", "Usable range", "Spare"] {
            ui.label(RichText::new(h).small().weak());
        }
        ui.end_row();
        for ((name, need), net) in rows.iter().zip(&plan) {
            let i = SubnetInfo::of(net);
            let spare = i.usable.saturating_sub(*need as u128);
            ui.label(name);
            ui.label(need.to_string());
            ui.label(RichText::new(net.to_string()).monospace());
            ui.label(RichText::new(i.netmask.to_string()).monospace());
            ui.label(RichText::new(format!("{} – {}", i.first_usable, i.last_usable)).monospace());
            ui.label(fmt_count(spare));
            ui.end_row();
            tsv.push_str(&format!(
                "{name}\t{need}\t{net}\t{}\t{}\t{} - {}\t{spare}\n",
                i.netmask, i.first_usable, i.first_usable, i.last_usable
            ));
        }
    });
    let used: u128 = plan.iter().map(ip::total_addresses).sum();
    let total = ip::total_addresses(&parent);
    hint(ui, &format!("Uses {} of {} addresses ({:.1}%)", fmt_count(used), fmt_count(total), used as f64 / total as f64 * 100.0));

    ui.add_space(4.0);
    ui.horizontal(|ui| {
        if ui.button(format!("{} Copy table", icons::COPY)).on_hover_text("Tab-separated, pastes into a spreadsheet").clicked() {
            ui.ctx().copy_text(tsv);
            st.set_status("VLSM table copied");
        }
        if ui.button(format!("{} Create in project…", icons::ARROW_SQUARE_OUT)).on_hover_text("Open the VLSM planner with these rows to add VLAN tags and create the subnets").clicked() {
            let rows = rows.iter().map(|(n, h)| (n.clone(), *h, 0)).collect();
            st.vlsm = Some(crate::panels::vlsm::VlsmPlanner::with_rows(parent, rows));
        }
    });
}

// ---- address notation ------------------------------------------------------

#[derive(Debug)]
pub struct AddressTool {
    input: String,
    prefix: String,
    mac: String,
}

impl Default for AddressTool {
    fn default() -> Self {
        Self { input: "2001:db8:0:a::1".into(), prefix: "2001:db8:0:a::/64".into(), mac: "00:1b:63:84:45:e6".into() }
    }
}

pub fn address(ui: &mut Ui, st: &mut State) {
    let t = &mut st.tools.address;
    ui.horizontal(|ui| {
        ui.label("Address");
        ui.add(egui::TextEdit::singleline(&mut t.input).desired_width(300.0).font(egui::TextStyle::Monospace));
    });
    let text = t.input.trim();
    let text = text.split('/').next().unwrap_or(text);
    match text.parse::<IpAddr>() {
        Ok(a) => {
            ui.add_space(6.0);
            kv_grid(ui, "tools.addr", |ui| match a {
                IpAddr::V4(v4) => {
                    kv(ui, "Dotted decimal", v4.to_string());
                    kv(ui, "Integer", u32::from(v4).to_string());
                    kv(ui, "Hex", repr::hex(a));
                    kv(ui, "Binary", repr::binary(a));
                    kv(ui, "Reverse DNS", repr::reverse_pointer(a));
                    kv(ui, "IPv4-mapped IPv6", repr::ipv4_mapped(v4).to_string());
                    kv(ui, "Scope", ip::scope(a));
                }
                IpAddr::V6(v6) => {
                    kv(ui, "Compressed", repr::ipv6_compressed(v6));
                    kv(ui, "Expanded", repr::ipv6_expanded(v6));
                    kv(ui, "Reverse DNS", repr::reverse_pointer(a));
                    if let Some(v4) = v6.to_ipv4_mapped() {
                        kv(ui, "Embedded IPv4", v4.to_string());
                    }
                    kv(ui, "Scope", ip::scope(a));
                }
            });
        }
        Err(_) if !text.is_empty() => {
            ui.label(RichText::new("Not a valid IPv4 or IPv6 address").color(ERROR));
        }
        Err(_) => {}
    }

    widgets::section(ui, "SLAAC / EUI-64 address");
    hint(ui, "The address a host picks with stateless autoconfiguration (no privacy extensions).");
    egui::Grid::new("tools.eui").num_columns(2).spacing([10.0, 4.0]).show(ui, |ui| {
        ui.label("/64 prefix");
        ui.add(egui::TextEdit::singleline(&mut t.prefix).desired_width(220.0).font(egui::TextStyle::Monospace));
        ui.end_row();
        ui.label("MAC address");
        ui.add(egui::TextEdit::singleline(&mut t.mac).desired_width(220.0).font(egui::TextStyle::Monospace));
        ui.end_row();
    });
    let prefix = t.prefix.trim().parse::<Ipv6Net>();
    let mac = Mac::parse(&t.mac);
    match (prefix, mac) {
        (Ok(p), Some(m)) => match repr::eui64_address(p, m) {
            Some(a) => kv_grid(ui, "tools.eui.out", |ui| {
                let iid = m.eui64();
                kv(ui, "Interface ID", format!("{:02x}{:02x}:{:02x}{:02x}:{:02x}{:02x}:{:02x}{:02x}", iid[0], iid[1], iid[2], iid[3], iid[4], iid[5], iid[6], iid[7]));
                kv(ui, "Address", a.to_string());
                let ll = repr::eui64_address("fe80::/64".parse().expect("valid"), m).expect("/64");
                kv(ui, "Link-local", ll.to_string());
            }),
            None => {
                ui.label(RichText::new("SLAAC needs a /64 (or shorter) prefix").color(WARN));
            }
        },
        (Err(_), _) => {
            ui.label(RichText::new("Prefix must be IPv6, e.g. 2001:db8:1::/64").color(WARN));
        }
        (_, None) => {
            ui.label(RichText::new("Enter a 48-bit MAC address").color(WARN));
        }
    }
}

// ---- MAC lookup ------------------------------------------------------------

#[derive(Debug)]
pub struct MacTool {
    input: String,
    upper: bool,
}

impl Default for MacTool {
    fn default() -> Self {
        Self { input: "00:00:0c:9f:f0:01".into(), upper: false }
    }
}

pub fn mac(ui: &mut Ui, st: &mut State) {
    let t = &mut st.tools.mac;
    hint(ui, &format!("Full MAC, OUI prefix (first 6 hex digits) or part of a vendor name. Bundled IEEE registry: {} entries.", oui::count()));
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.label(format!("{} Lookup", icons::MAGNIFYING_GLASS));
        ui.add(egui::TextEdit::singleline(&mut t.input).desired_width(260.0).font(egui::TextStyle::Monospace));
        ui.checkbox(&mut t.upper, "Upper-case");
    });
    ui.add_space(6.0);
    let text = t.input.trim();
    if let Some(m) = Mac::parse(text) {
        kv_grid(ui, "tools.mac", |ui| {
            for style in MacStyle::ALL {
                kv(ui, style.label(), m.format(style, t.upper));
            }
            kv(ui, "OUI", oui::format_oui(m.oui()));
            let vendor = if m.is_local() {
                "Locally administered — randomised or software-assigned, no vendor".to_owned()
            } else {
                oui::vendor(m.oui()).unwrap_or("Not in the MA-L registry (may be an MA-M/MA-S block)").to_owned()
            };
            kv(ui, "Vendor", vendor);
            let cast = if m.is_broadcast() {
                "Broadcast"
            } else if m.is_multicast() {
                "Multicast (group)"
            } else {
                "Unicast"
            };
            kv(ui, "Type", format!("{cast} · {}", if m.is_local() { "locally administered" } else { "globally unique" }));
        });
    } else if let Some(o) = ip::mac::parse_oui(text) {
        kv_grid(ui, "tools.oui", |ui| {
            kv(ui, "OUI", oui::format_oui(o));
            kv(ui, "Vendor", oui::vendor(o).unwrap_or("Not in the MA-L registry"));
        });
    } else if text.chars().count() >= 2 {
        let hits = oui::search(text, 60);
        let total = oui::search_count(text);
        hint(ui, &format!("{total} OUI(s) registered to names containing “{text}”{}", if total > hits.len() { format!(" — first {}", hits.len()) } else { String::new() }));
        egui::Grid::new("tools.oui.search").num_columns(2).spacing([14.0, 2.0]).striped(true).show(ui, |ui| {
            for (o, name) in hits {
                ui.label(RichText::new(oui::format_oui(o)).monospace());
                ui.label(name);
                ui.end_row();
            }
        });
    }

    // MACs recorded in the project.
    let p = &st.doc.project;
    let rows: Vec<(String, Mac)> = p
        .devices
        .iter()
        .flat_map(|d| d.ports.iter().map(move |port| (d, port)))
        .filter_map(|(d, port)| Some((format!("{}:{}", d.name, port.name), Mac::parse(&port.cfg.mac)?)))
        .collect();
    if !rows.is_empty() {
        widgets::section(ui, "MAC addresses in this project");
        egui::Grid::new("tools.mac.project").num_columns(3).spacing([14.0, 2.0]).striped(true).show(ui, |ui| {
            for (who, m) in rows {
                ui.label(who);
                ui.label(RichText::new(m.format(MacStyle::Colon, t.upper)).monospace());
                let v = if m.is_local() { "locally administered" } else { oui::vendor(m.oui()).unwrap_or("unknown") };
                ui.label(RichText::new(v).weak());
                ui.end_row();
            }
        });
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn vlsm_rows() {
        let (rows, bad) = super::parse_rows("Staff, 200\nVoice 100\n  \n60\nnope, x");
        assert_eq!(rows, vec![("Staff".into(), 200), ("Voice".into(), 100), ("Subnet 3".into(), 60)]);
        assert_eq!(bad, 1);
    }
}
