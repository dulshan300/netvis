//! Built-in starting points: File → New from template.

use std::sync::OnceLock;

use egui::RichText;
use netvis_core::{Project, io};

use crate::icons;
use crate::state::{Action, State};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    Basic,
    Intermediate,
    Advanced,
}

impl Level {
    fn label(self) -> &'static str {
        match self {
            Level::Basic => "Basic",
            Level::Intermediate => "Intermediate",
            Level::Advanced => "Advanced",
        }
    }

    fn color(self) -> egui::Color32 {
        match self {
            Level::Basic => egui::Color32::from_rgb(22, 163, 74),
            Level::Intermediate => egui::Color32::from_rgb(37, 99, 235),
            Level::Advanced => egui::Color32::from_rgb(147, 51, 234),
        }
    }
}

pub struct Template {
    pub key: &'static str,
    pub title: &'static str,
    pub level: Level,
    pub icon: &'static str,
    pub summary: &'static str,
    /// User-guide page that walks through it, if any.
    pub guide: Option<&'static str>,
    data: &'static str,
}

macro_rules! file {
    ($path:literal) => {
        include_str!(concat!("../../../", $path))
    };
}

pub const ALL: &[Template] = &[
    Template {
        key: "home",
        title: "Home network",
        level: Level::Basic,
        icon: icons::HOUSE,
        summary: "Internet router, a small PoE switch, one Wi-Fi AP and a few wired devices on one LAN.",
        guide: None,
        data: file!("templates/home.netvis"),
    },
    Template {
        key: "small-office",
        title: "Small office",
        level: Level::Basic,
        icon: icons::BRIEFCASE,
        summary: "10–20 people. Firewall routing staff, voice, guest and management VLANs; one PoE switch and patch panel in a wall cabinet; desks with phones.",
        guide: None,
        data: file!("templates/small-office.netvis"),
    },
    Template {
        key: "retail-store",
        title: "Retail store",
        level: Level::Basic,
        icon: icons::STOREFRONT,
        summary: "Tills on their own VLAN for card payments, IP cameras and a recorder, back office, and customer Wi-Fi that only reaches the internet.",
        guide: None,
        data: file!("templates/retail-store.netvis"),
    },
    Template {
        key: "cctv-system",
        title: "IP CCTV system",
        level: Level::Basic,
        icon: icons::SECURITY_CAMERA,
        summary: "Stand-alone surveillance network: 20 cameras on two PoE switches with fibre uplinks, a recording server, storage and a monitoring PC.",
        guide: None,
        data: file!("templates/cctv-system.netvis"),
    },
    Template {
        key: "small-company",
        title: "Small company",
        level: Level::Intermediate,
        icon: icons::BUILDING_OFFICE,
        summary: "50–80 people on two floors. L3 core routing six VLANs, a server rack, fibre uplinks to each floor, desks on wall outlets, Wi-Fi.",
        guide: None,
        data: file!("templates/small-company.netvis"),
    },
    Template {
        key: "corescale-hq",
        title: "Head office (Corescale HQ)",
        level: Level::Intermediate,
        icon: icons::BUILDINGS,
        summary: "Firewall, core, DMZ, server rack, two floors with a fibre riser and patched desks.",
        guide: None,
        data: file!("samples/corescale-hq.netvis"),
    },
    Template {
        key: "shopping-mall",
        title: "Shopping mall (tutorial)",
        level: Level::Advanced,
        icon: icons::SHOPPING_BAG,
        summary: "Nine floors from BF to L6: IP CCTV, door access, Wi-Fi and background music, an ELV room per floor and a fibre backbone to the GF server room.",
        guide: Some("tutorial-shopping-mall"),
        data: file!("samples/shopping-mall.netvis"),
    },
    Template {
        key: "corescale-campus",
        title: "Campus (Corescale Campus)",
        level: Level::Advanced,
        icon: icons::GRAPH,
        summary: "Data centre with dual ISPs and a core pair, VM hosts and storage, two buildings and five floors of patched access.",
        guide: None,
        data: file!("samples/corescale-campus.netvis"),
    },
];

/// A fresh copy of the template's project.
pub fn load(t: &Template) -> Result<Project, io::IoError> {
    io::from_str(t.data)
}

/// Device and cable counts, parsed once.
fn counts() -> &'static [(usize, usize)] {
    static C: OnceLock<Vec<(usize, usize)>> = OnceLock::new();
    C.get_or_init(|| ALL.iter().map(|t| load(t).map(|p| (p.devices.len(), p.links.len())).unwrap_or((0, 0))).collect())
}

/// The gallery, while `st.templates_open`.
pub fn show(ctx: &egui::Context, st: &mut State) {
    if !st.templates_open {
        return;
    }
    let mut close = false;
    let resp = egui::Modal::new(egui::Id::new("templates")).show(ctx, |ui| {
        ui.set_width(760.0);
        ui.heading(format!("{} New from template", icons::SQUARES_FOUR));
        ui.label(RichText::new("Start from a working design and change it to suit. Every template passes all design checks.").weak());
        ui.add_space(6.0);
        egui::ScrollArea::vertical().max_height(520.0).show(ui, |ui| {
            egui::Grid::new("templates.grid").num_columns(2).spacing([10.0, 10.0]).show(ui, |ui| {
                for (i, t) in ALL.iter().enumerate() {
                    let (devices, cables) = counts()[i];
                    egui::Frame::group(ui.style()).inner_margin(10.0).show(ui, |ui| {
                        ui.vertical(|ui| {
                            ui.set_width(350.0);
                            ui.set_min_height(112.0);
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(t.icon).size(22.0).color(t.level.color()));
                                ui.vertical(|ui| {
                                    ui.label(RichText::new(t.title).strong().size(14.0));
                                    ui.horizontal(|ui| {
                                        ui.label(RichText::new(t.level.label()).small().color(t.level.color()));
                                        ui.label(RichText::new(format!("· {devices} devices · {cables} cables")).small().weak());
                                    });
                                });
                            });
                            ui.add_space(2.0);
                            ui.label(RichText::new(t.summary).small());
                            ui.horizontal(|ui| {
                                if ui.button(RichText::new(format!("{} Use this template", icons::PLUS)).strong()).clicked() {
                                    st.actions.push(Action::NewFromTemplate(i));
                                    close = true;
                                }
                                if let Some(page) = t.guide
                                    && ui.button(format!("{} Read the tutorial", icons::BOOK_OPEN)).clicked()
                                {
                                    st.actions.push(Action::ShowGuide(Some(page)));
                                    close = true;
                                }
                            });
                        });
                    });
                    if i % 2 == 1 {
                        ui.end_row();
                    }
                }
            });
        });
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            if ui.button(format!("{} Blank project", icons::FILE_PLUS)).clicked() {
                st.actions.push(Action::New);
                close = true;
            }
            if ui.button("Cancel").clicked() {
                close = true;
            }
        });
    });
    if close || resp.should_close() {
        st.templates_open = false;
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn every_template_loads_and_passes_checks() {
        for t in super::ALL {
            let p = super::load(t).unwrap_or_else(|e| panic!("{}: {e}", t.key));
            assert!(!p.devices.is_empty(), "{}", t.key);
            let issues: Vec<_> = netvis_validate::validate(&p).into_iter().map(|i| format!("{} {}", i.code, i.message)).collect();
            assert!(issues.is_empty(), "{}: {issues:#?}", t.key);
        }
    }
}
