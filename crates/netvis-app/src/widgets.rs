//! Small reusable form widgets.

use std::fmt::Display;
use std::net::IpAddr;
use std::str::FromStr;

use egui::{Color32, Id, RichText, Stroke, Ui, vec2};
use netvis_core::ip::IpNet;
use netvis_core::ipam::Usage;
use netvis_core::{Vlan, VlanId};

/// Anything usable as an egui id salt.
pub trait Salt: std::hash::Hash + std::fmt::Debug {}
impl<T: std::hash::Hash + std::fmt::Debug> Salt for T {}

pub const WARN: Color32 = Color32::from_rgb(217, 119, 6);
pub const ERROR: Color32 = Color32::from_rgb(220, 38, 38);
pub const OK: Color32 = Color32::from_rgb(22, 163, 74);

/// Text field bound to a typed value. The text is kept in temp memory while it
/// does not parse, so the user can type freely; the value only changes when
/// the text is valid. External changes (undo) reset the buffer.
pub fn parsed_with<T>(
    ui: &mut Ui,
    salt: impl Salt,
    value: &mut T,
    width: f32,
    hint: &str,
    parse: impl Fn(&str) -> Option<T>,
    format: impl Fn(&T) -> String,
) -> bool
where
    T: Clone + PartialEq + Send + Sync + 'static,
{
    let id = ui.id().with(("parsed", salt));
    let mut buf: (String, T) = ui
        .data(|d| d.get_temp::<(String, T)>(id))
        .filter(|(_, v)| v == value)
        .unwrap_or_else(|| (format(value), value.clone()));
    let valid = parse(&buf.0).is_some();
    let mut edit = egui::TextEdit::singleline(&mut buf.0).desired_width(width).hint_text(hint);
    if !valid {
        edit = edit.text_color(ERROR);
    }
    let resp = ui.add(edit);
    let mut changed = false;
    if resp.changed()
        && let Some(v) = parse(&buf.0)
        && v != *value
    {
        *value = v.clone();
        buf.1 = v;
        changed = true;
    }
    if resp.lost_focus() && parse(&buf.0).is_none() {
        // Revert invalid text when leaving the field.
        buf.0 = format(value);
    }
    if !valid {
        resp.on_hover_text("Invalid value — not applied");
    }
    ui.data_mut(|d| d.insert_temp(id, buf));
    changed
}

pub fn parsed<T>(ui: &mut Ui, salt: impl Salt, value: &mut T, width: f32, hint: &str) -> bool
where
    T: FromStr + Display + Clone + PartialEq + Send + Sync + 'static,
{
    parsed_with(ui, salt, value, width, hint, |s| s.trim().parse().ok(), |v| v.to_string())
}

/// Optional value: empty text means `None`.
pub fn parsed_opt<T>(ui: &mut Ui, salt: impl Salt, value: &mut Option<T>, width: f32, hint: &str) -> bool
where
    T: FromStr + Display + Clone + PartialEq + Send + Sync + 'static,
{
    parsed_with(
        ui,
        salt,
        value,
        width,
        hint,
        |s| if s.trim().is_empty() { Some(None) } else { s.trim().parse().ok().map(Some) },
        |v| v.as_ref().map(|x| x.to_string()).unwrap_or_default(),
    )
}

/// Interface address or prefix; accepts `a/len` and `a mask`.
pub fn net_field(ui: &mut Ui, salt: impl Salt, value: &mut IpNet, width: f32, trunc: bool) -> bool {
    parsed_with(
        ui,
        salt,
        value,
        width,
        "10.0.0.1/24",
        |s| netvis_core::ip::parse_net(s).ok().map(|n| if trunc { n.trunc() } else { n }),
        |n| n.to_string(),
    )
}

/// Comma- or space-separated list.
pub fn list_field<T>(ui: &mut Ui, salt: impl Salt, value: &mut Vec<T>, width: f32, hint: &str) -> bool
where
    T: FromStr + Display + Clone + PartialEq + Send + Sync + 'static,
{
    parsed_with(
        ui,
        salt,
        value,
        width,
        hint,
        |s| s.split([',', ' ']).filter(|x| !x.trim().is_empty()).map(|x| x.trim().parse().ok()).collect(),
        |v| v.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(", "),
    )
}

pub fn ip_list(ui: &mut Ui, salt: impl Salt, value: &mut Vec<IpAddr>, width: f32) -> bool {
    list_field(ui, salt, value, width, "e.g. 1.1.1.1, 9.9.9.9")
}

pub fn rgb(c: [u8; 3]) -> Color32 {
    Color32::from_rgb(c[0], c[1], c[2])
}

pub fn swatch(ui: &mut Ui, color: Color32) {
    let (r, _) = ui.allocate_exact_size(vec2(10.0, 10.0), egui::Sense::hover());
    ui.painter().rect_filled(r, 3.0, color);
}

pub fn vlan_text(vlans: &[Vlan], id: Option<VlanId>) -> RichText {
    match id {
        None => RichText::new("—").weak(),
        Some(id) => match vlans.iter().find(|v| v.id == id) {
            Some(v) => RichText::new(v.label()).color(rgb(v.color)),
            None => RichText::new("(deleted)").color(ERROR),
        },
    }
}

/// Pick a VLAN (or none) from the project's VLAN database.
pub fn vlan_combo(ui: &mut Ui, salt: impl Salt, vlans: &[Vlan], value: &mut Option<VlanId>, width: f32) -> bool {
    let mut changed = false;
    egui::ComboBox::from_id_salt(("vlan", salt)).width(width).selected_text(vlan_text(vlans, *value)).show_ui(ui, |ui| {
        changed |= ui.selectable_value(value, None, "— none —").changed();
        let mut sorted: Vec<_> = vlans.iter().collect();
        sorted.sort_by_key(|v| v.tag);
        for v in sorted {
            changed |= ui.selectable_value(value, Some(v.id), RichText::new(v.label()).color(rgb(v.color))).changed();
        }
        if vlans.is_empty() {
            ui.label(RichText::new("No VLANs yet — add them in the VLANs tab").weak().small());
        }
    });
    changed
}

/// Stacked bar: assigned | reserved | DHCP pool | free.
pub fn usage_bar(ui: &mut Ui, u: &Usage, width: f32) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(width, 10.0), egui::Sense::hover());
    let p = ui.painter();
    let bg = ui.visuals().faint_bg_color;
    p.rect_filled(rect, 3.0, bg);
    if u.usable > 0 {
        let mut x = rect.left();
        for (n, c) in [
            (u.allocated, Color32::from_rgb(139, 92, 246)),
            (u.assigned, Color32::from_rgb(37, 99, 235)),
            (u.reserved, Color32::from_rgb(100, 116, 139)),
            (u.dhcp_pool, Color32::from_rgb(16, 185, 129)),
        ] {
            let w = (n as f64 / u.usable as f64 * rect.width() as f64) as f32;
            if w > 0.0 {
                let seg = egui::Rect::from_min_size(egui::pos2(x, rect.top()), vec2(w.max(1.0), rect.height()));
                p.rect_filled(seg, 2.0, c);
                x += w;
            }
        }
    }
    p.rect_stroke(rect, 3.0, Stroke::new(1.0, ui.visuals().widgets.inactive.bg_stroke.color), egui::StrokeKind::Inside);
    resp.on_hover_text(format!(
        "{} usable\n{} in child subnets · {} assigned · {} reserved · {} DHCP pool · {} free",
        fmt_count(u.usable),
        fmt_count(u.allocated),
        fmt_count(u.assigned),
        fmt_count(u.reserved),
        fmt_count(u.dhcp_pool),
        fmt_count(u.free)
    ))
}

/// Compact count: exact up to a million, then powers of two for IPv6.
pub fn fmt_count(n: u128) -> String {
    if n < 1_000_000 {
        n.to_string()
    } else {
        format!("≈2^{}", 128 - n.leading_zeros() - 1)
    }
}

/// Small uppercase section label.
pub fn section(ui: &mut Ui, text: &str) {
    ui.add_space(6.0);
    ui.label(RichText::new(text.to_uppercase()).small().strong().weak());
}

/// Id for state that should persist per entity across frames.
pub fn temp_id(name: &str, salt: impl Salt) -> Id {
    Id::new(("netvis", name)).with(salt)
}
