//! Branding, colours and egui style setup.

use egui::{Color32, CornerRadius, FontDefinitions, Stroke, Theme, ThemePreference, Visuals};
use netvis_core::link::CableFamily;
use netvis_core::{DeviceCategory, PortMedia};
use serde::{Deserialize, Serialize};

pub const BRAND_NAME: &str = "Corescale";
pub const APP_NAME: &str = "NetVis";

/// Primary brand accent. Placeholder until Corescale's brand palette is supplied.
pub const ACCENT: Color32 = Color32::from_rgb(37, 99, 235);
pub const ACCENT_DARK_MODE: Color32 = Color32::from_rgb(96, 145, 250);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ThemeChoice {
    #[default]
    System,
    Light,
    Dark,
}

impl ThemeChoice {
    pub fn label(self) -> &'static str {
        match self {
            ThemeChoice::System => "Follow system",
            ThemeChoice::Light => "Light",
            ThemeChoice::Dark => "Dark",
        }
    }

    fn preference(self) -> ThemePreference {
        match self {
            ThemeChoice::System => ThemePreference::System,
            ThemeChoice::Light => ThemePreference::Light,
            ThemeChoice::Dark => ThemePreference::Dark,
        }
    }
}

pub fn install(ctx: &egui::Context, choice: ThemeChoice) {
    let mut fonts = FontDefinitions::default();
    egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
    // The default UI font has no arrows (→ ↔); fall back to the bundled
    // monospace font, which does.
    let mono = fonts.families.get(&egui::FontFamily::Monospace).and_then(|f| f.first().cloned());
    if let (Some(mono), Some(prop)) = (mono, fonts.families.get_mut(&egui::FontFamily::Proportional))
        && !prop.contains(&mono)
    {
        prop.push(mono);
    }
    ctx.set_fonts(fonts);

    ctx.set_visuals_of(Theme::Light, visuals(false));
    ctx.set_visuals_of(Theme::Dark, visuals(true));
    for theme in [Theme::Light, Theme::Dark] {
        ctx.style_mut_of(theme, |s| {
            use egui::{FontFamily::*, FontId, TextStyle};
            s.text_styles = [
                (TextStyle::Small, FontId::new(11.0, Proportional)),
                (TextStyle::Body, FontId::new(13.5, Proportional)),
                (TextStyle::Button, FontId::new(13.5, Proportional)),
                (TextStyle::Heading, FontId::new(18.0, Proportional)),
                (TextStyle::Monospace, FontId::new(12.5, Monospace)),
            ]
            .into();
            s.spacing.item_spacing = egui::vec2(8.0, 6.0);
            s.spacing.button_padding = egui::vec2(8.0, 4.0);
            s.spacing.interact_size.y = 24.0;
        });
    }
    apply_choice(ctx, choice);
}

pub fn apply_choice(ctx: &egui::Context, choice: ThemeChoice) {
    ctx.set_theme(choice.preference());
}

fn visuals(dark: bool) -> Visuals {
    let mut v = if dark { Visuals::dark() } else { Visuals::light() };
    let accent = if dark { ACCENT_DARK_MODE } else { ACCENT };
    let r = CornerRadius::same(6);

    v.selection.bg_fill = accent.gamma_multiply(if dark { 0.55 } else { 0.25 });
    v.selection.stroke = Stroke::new(1.0, accent);
    v.hyperlink_color = accent;
    v.window_corner_radius = CornerRadius::same(10);
    v.menu_corner_radius = CornerRadius::same(8);
    for w in [
        &mut v.widgets.noninteractive,
        &mut v.widgets.inactive,
        &mut v.widgets.hovered,
        &mut v.widgets.active,
        &mut v.widgets.open,
    ] {
        w.corner_radius = r;
    }
    // Visible outlines on idle inputs so form fields don't read as plain labels.
    let outline = if dark { Color32::from_rgb(62, 68, 80) } else { Color32::from_rgb(206, 211, 219) };
    v.widgets.inactive.bg_stroke = Stroke::new(1.0, outline);
    if dark {
        v.panel_fill = Color32::from_rgb(24, 26, 31);
        v.window_fill = Color32::from_rgb(30, 33, 39);
        v.extreme_bg_color = Color32::from_rgb(17, 19, 23);
        v.faint_bg_color = Color32::from_rgb(32, 35, 42);
    } else {
        v.panel_fill = Color32::from_rgb(246, 247, 249);
        v.window_fill = Color32::WHITE;
        v.extreme_bg_color = Color32::WHITE;
        v.faint_bg_color = Color32::from_rgb(240, 242, 245);
    }
    v
}

/// Colours used to paint the topology canvas.
pub struct Palette {
    pub canvas_bg: Color32,
    pub grid_minor: Color32,
    pub grid_major: Color32,
    pub card: Color32,
    pub card_border: Color32,
    pub text: Color32,
    pub subtext: Color32,
    pub accent: Color32,
    pub port_free: Color32,
    pub port_stroke: Color32,
    pub poe: Color32,
}

impl Palette {
    pub fn new(dark: bool) -> Self {
        if dark {
            Self {
                canvas_bg: Color32::from_rgb(19, 21, 26),
                grid_minor: Color32::from_rgb(30, 33, 40),
                grid_major: Color32::from_rgb(40, 44, 53),
                card: Color32::from_rgb(34, 38, 46),
                card_border: Color32::from_rgb(58, 64, 76),
                text: Color32::from_rgb(232, 235, 240),
                subtext: Color32::from_rgb(150, 158, 172),
                accent: ACCENT_DARK_MODE,
                port_free: Color32::from_rgb(48, 53, 63),
                port_stroke: Color32::from_rgb(100, 108, 122),
                poe: Color32::from_rgb(245, 158, 11),
            }
        } else {
            Self {
                canvas_bg: Color32::from_rgb(250, 251, 252),
                grid_minor: Color32::from_rgb(236, 239, 243),
                grid_major: Color32::from_rgb(222, 226, 232),
                card: Color32::WHITE,
                card_border: Color32::from_rgb(208, 213, 221),
                text: Color32::from_rgb(17, 24, 39),
                subtext: Color32::from_rgb(100, 110, 125),
                accent: ACCENT,
                port_free: Color32::from_rgb(241, 243, 246),
                port_stroke: Color32::from_rgb(150, 158, 170),
                poe: Color32::from_rgb(217, 119, 6),
            }
        }
    }
}

pub fn category_color(c: DeviceCategory) -> Color32 {
    let [r, g, b] = netvis_core::style::category_rgb(c);
    Color32::from_rgb(r, g, b)
}

pub fn cable_color(f: CableFamily) -> Color32 {
    let [r, g, b] = netvis_core::style::cable_rgb(f);
    Color32::from_rgb(r, g, b)
}

pub fn media_is_round(m: PortMedia) -> bool {
    matches!(m, PortMedia::Wireless | PortMedia::Virtual | PortMedia::Coax)
}
