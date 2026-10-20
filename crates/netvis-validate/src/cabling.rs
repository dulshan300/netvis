//! Physical-layer checks on every link: media vs cable, optics, speed, reach
//! and duplex.

use netvis_core::catalog::{CableFamily, Reach, TxMedium};
use netvis_core::catalog::format_speed;
use netvis_core::{CableKind, Duplex, Item, Link, PortMedia, Project};

use crate::{End, Sink};

pub fn check(p: &Project, out: &mut Sink) {
    for l in &p.links {
        check_link(p, l, out);
    }
}

fn check_link(p: &Project, l: &Link, out: &mut Sink) {
    let (Some(a), Some(b)) = (End::resolve(p, l.a), End::resolve(p, l.b)) else {
        out.error("link.dangling", "Link points at a device or port that no longer exists", vec![Item::Link(l.id)]);
        return;
    };
    let items = || vec![Item::Link(l.id), Item::Device(a.dev.id), Item::Device(b.dev.id)];
    let pair = format!("{} ↔ {}", a.label(), b.label());

    // Module sanity per end.
    for e in [&a, &b] {
        match (&e.port.transceiver, e.tx) {
            (Some(id), None) => out.warn("tx.unknown", format!("{}: unknown transceiver “{id}”", e.label()), items()),
            (Some(_), Some(t)) if !e.port.media.is_optical_cage() => {
                out.warn("tx.no-cage", format!("{}: {} fitted to a {} port", e.label(), t.name, e.port.media.label()), items())
            }
            (Some(_), Some(t)) if !t.fits(e.port.media) => out.error(
                "tx.form-factor",
                format!("{}: {} ({}) does not fit a {} cage", e.label(), t.name, t.form.label(), e.port.media.label()),
                items(),
            ),
            _ => {}
        }
    }

    // Media compatibility. Later checks are meaningless if this fails.
    let mut media_ok = true;
    for e in [&a, &b] {
        if let Some(msg) = media_problem(l.cable, e) {
            out.error("cable.media", msg, items());
            media_ok = false;
        }
    }
    if !media_ok {
        return;
    }
    if l.cable == CableKind::Unspecified {
        out.info("cable.unset", format!("{pair}: cable type not set"), items());
    }
    if l.cable.is_fibre() {
        for e in [&a, &b] {
            if e.tx.is_none() && e.port.transceiver.is_none() {
                out.info("tx.missing", format!("{}: no transceiver selected for the {} link", e.label(), l.cable.label()), items());
            }
        }
    }
    if l.cable.family() == CableFamily::DirectAttach {
        for e in [&a, &b] {
            if let Some(t) = e.tx {
                out.warn("tx.on-dac", format!("{}: {} has integrated ends — remove the {} module", e.label(), l.cable.label(), t.name), items());
            }
        }
    }

    // Optics must match each other.
    if let (Some(ta), Some(tb)) = (a.tx, b.tx)
        && (ta.medium != tb.medium || ta.wavelength_nm != tb.wavelength_nm || ta.speed_mbps != tb.speed_mbps)
    {
        out.error("tx.mismatch", format!("{pair}: transceiver mismatch {} ↔ {}", ta.name, tb.name), items());
        return;
    }

    let family = l.cable.family();
    if matches!(family, CableFamily::Wireless | CableFamily::Logical | CableFamily::Console | CableFamily::Coax) {
        check_duplex(&a, &b, &pair, items(), out);
        return;
    }

    // Speed.
    let (sa, sb) = (a.speed(), b.speed());
    let copper_both = [&a, &b].iter().all(|e| e.port.media == PortMedia::Rj45 || e.tx.is_some_and(|t| t.medium == TxMedium::Copper));
    if sa != sb {
        let (slow, fast) = if sa < sb { (&a, &b) } else { (&b, &a) };
        let msg = format!(
            "{pair}: speed mismatch {} vs {}",
            format_speed(sa),
            format_speed(sb)
        );
        if !copper_both {
            out.error("link.speed", format!("{msg} — optics and DACs do not auto-negotiate"), items());
        } else if is_infra(slow.dev.kind) && is_infra(fast.dev.kind) {
            out.warn("link.speed", format!("{msg} — the link negotiates down to {}", format_speed(sa.min(sb))), items());
        } else if is_infra(slow.dev.kind) {
            out.info(
                "link.speed",
                format!("{}: {} can run at {} but {} limits it to {}", pair, fast.dev.name, format_speed(fast.speed()), slow.label(), format_speed(slow.speed())),
                items(),
            );
        }
        // An end device slower than its switch port is normal: nothing to report.
    }
    let speed = sa.min(sb);

    // Reach: optics define fibre reach; copper modules may shorten copper reach.
    let mut reach = l.cable.reach(speed);
    let module_limit = [a.tx, b.tx].into_iter().flatten().filter_map(|t| t.reach_on(l.cable)).reduce(f32::min);
    if let Some(m) = module_limit {
        reach = match reach {
            Reach::Max(c) if !l.cable.is_fibre() => Reach::Max(c.min(m)),
            Reach::Unsupported if !l.cable.is_fibre() => Reach::Unsupported,
            _ => Reach::Max(m),
        };
    }
    match reach {
        Reach::Unsupported => out.error("cable.speed", format!("{pair}: {} is not rated for {}", l.cable.label(), format_speed(speed)), items()),
        Reach::Max(max) => {
            if let Some(len) = l.length_m
                && len > max
            {
                out.error(
                    "cable.reach",
                    format!("{pair}: {len} m exceeds the {max} m limit of {} at {}", l.cable.label(), format_speed(speed)),
                    items(),
                );
            }
        }
        Reach::Unlimited | Reach::Unknown => {}
    }

    check_duplex(&a, &b, &pair, items(), out);
}

/// Switching/routing gear, as opposed to end devices and access points.
fn is_infra(kind: netvis_core::DeviceKind) -> bool {
    kind.category() == netvis_core::DeviceCategory::Network && kind != netvis_core::DeviceKind::AccessPoint
}

fn check_duplex(a: &End, b: &End, pair: &str, items: Vec<Item>, out: &mut Sink) {
    let (da, db) = (a.port.cfg.duplex, b.port.cfg.duplex);
    if da != Duplex::Auto && db != Duplex::Auto && da != db {
        out.error("link.duplex", format!("{pair}: duplex mismatch ({} vs {})", da.label(), db.label()), items);
    } else if (da == Duplex::Auto) != (db == Duplex::Auto) && (da == Duplex::Full || db == Duplex::Full) {
        out.warn(
            "link.duplex",
            format!("{pair}: one end is forced to full duplex, the other auto-negotiates — the auto end falls back to half duplex"),
            items,
        );
    }
}

/// Why `cable` cannot terminate on this end, if it can't.
fn media_problem(cable: CableKind, e: &End) -> Option<String> {
    let m = e.port.media;
    let at = e.label();
    if m == PortMedia::Virtual || cable == CableKind::Virtual || cable == CableKind::Unspecified {
        return None;
    }
    let family = cable.family();
    if family == CableFamily::Wireless {
        return (m != PortMedia::Wireless).then(|| format!("{at}: wireless link on a wired {} port", m.label()));
    }
    if m == PortMedia::Wireless {
        return Some(format!("{at}: {} cable on a radio interface", cable.label()));
    }
    match family {
        CableFamily::Copper => match m {
            PortMedia::Rj45 => None,
            _ if m.is_optical_cage() => match e.tx {
                None => Some(format!("{at}: {} cable into an empty {} cage — fit a 1000BASE-T / 10GBASE-T module", cable.label(), m.label())),
                Some(t) if t.medium == TxMedium::Copper => None,
                Some(t) => Some(format!("{at}: {} optic with a {} cable", t.name, cable.label())),
            },
            _ => Some(format!("{at}: {} cable on a {} port", cable.label(), m.label())),
        },
        CableFamily::Multimode | CableFamily::SingleMode => match m {
            PortMedia::Rj45 => Some(format!("{at}: {} fibre into an RJ45 port — use an SFP port or a media converter", cable.label())),
            _ if m.is_optical_cage() => match e.tx {
                None => None,
                Some(t) if t.medium == TxMedium::Copper => Some(format!("{at}: copper {} module with a {} fibre", t.name, cable.label())),
                Some(t) if t.reach_on(cable).is_none() => {
                    Some(format!("{at}: {} is a {} optic but the cable is {}", t.name, t.medium.label().to_lowercase(), cable.label()))
                }
                Some(_) => None,
            },
            _ => Some(format!("{at}: {} fibre on a {} port", cable.label(), m.label())),
        },
        CableFamily::DirectAttach => (!m.is_optical_cage()).then(|| format!("{at}: {} needs an SFP/QSFP cage, not {}", cable.label(), m.label())),
        CableFamily::Coax => (m != PortMedia::Coax).then(|| format!("{at}: coax cable on a {} port", m.label())),
        CableFamily::Console => {
            (!matches!(m, PortMedia::Console | PortMedia::Rj45)).then(|| format!("{at}: console cable on a {} port", m.label()))
        }
        CableFamily::Wireless | CableFamily::Logical => None,
    }
}

#[cfg(test)]
mod tests {
    use netvis_core::{CableKind, DeviceKind, PortMedia};

    use crate::testutil::T;

    #[test]
    fn clean_links_have_no_issues() {
        let mut t = T::new();
        let (a, b) = (t.dev(DeviceKind::SwitchL2), t.dev(DeviceKind::SwitchL2));
        let (pa, pb) = (t.port(a, PortMedia::SfpPlus), t.port(b, PortMedia::SfpPlus));
        t.link((a, pa), (b, pb), CableKind::Om4, Some(120.0));
        t.set_tx(a, pa, "sfpp-10g-sr");
        t.set_tx(b, pb, "sfpp-10g-sr");
        let pc = t.dev(DeviceKind::Pc);
        let (pa, pp) = (t.port(a, PortMedia::Rj45), t.port(pc, PortMedia::Rj45));
        t.link((a, pa), (pc, pp), CableKind::Cat6, Some(80.0));
        assert!(t.codes().is_empty(), "{:?}", crate::validate(&t.p));
    }

    #[test]
    fn fibre_into_rj45_and_copper_into_empty_cage() {
        let mut t = T::new();
        let (a, b) = (t.dev(DeviceKind::SwitchL2), t.dev(DeviceKind::Pc));
        let (pa, pb) = (t.port(a, PortMedia::Rj45), t.port(b, PortMedia::Rj45));
        t.link((a, pa), (b, pb), CableKind::Om3, None);
        let c = t.dev(DeviceKind::Server);
        let (pa, pc) = (t.port(a, PortMedia::SfpPlus), t.port(c, PortMedia::Rj45));
        t.link((a, pa), (c, pc), CableKind::Cat6A, None);
        let codes = t.codes();
        assert_eq!(codes.iter().filter(|c| **c == "cable.media").count(), 3, "{codes:?}");
    }

    #[test]
    fn copper_reach_depends_on_speed() {
        let mut t = T::new();
        let (a, b) = (t.dev(DeviceKind::Server), t.dev(DeviceKind::Server));
        let (pa, pb) = (t.port(a, PortMedia::Rj45), t.port(b, PortMedia::Rj45));
        t.link((a, pa), (b, pb), CableKind::Cat6, Some(70.0));
        assert!(t.codes().is_empty(), "1G over 70 m Cat6 is fine");
        for d in [a, b] {
            t.p.device_mut(d).unwrap().ports[0].speed_mbps = 10_000;
        }
        assert_eq!(t.codes(), vec!["cable.reach"], "10G over Cat6 is limited to 55 m");
        t.p.links[0].cable = CableKind::Cat5e;
        assert_eq!(t.codes(), vec!["cable.speed"]);
    }

    #[test]
    fn optic_mismatches() {
        let mut t = T::new();
        let (a, b) = (t.dev(DeviceKind::SwitchL3), t.dev(DeviceKind::SwitchL3));
        let (pa, pb) = (t.port(a, PortMedia::SfpPlus), t.port(b, PortMedia::SfpPlus));
        t.link((a, pa), (b, pb), CableKind::Om4, Some(500.0));
        t.set_tx(a, pa, "sfpp-10g-sr");
        t.set_tx(b, pb, "sfpp-10g-sr");
        assert_eq!(t.codes(), vec!["cable.reach"], "SR on OM4 reaches 400 m");

        t.set_tx(b, pb, "sfpp-10g-lr");
        let codes = t.codes();
        assert!(codes.contains(&"cable.media"), "LR on multimode: {codes:?}");

        t.p.links[0].cable = CableKind::Os2;
        t.set_tx(a, pa, "sfp-1g-lx");
        let codes = t.codes();
        assert!(codes.contains(&"tx.mismatch"), "{codes:?}");

        t.set_tx(a, pa, "qsfp28-100g-sr4");
        assert!(t.codes().contains(&"tx.form-factor"));
    }

    #[test]
    fn speed_mismatch_severity_depends_on_media() {
        let mut t = T::new();
        let (a, b) = (t.dev(DeviceKind::Server), t.dev(DeviceKind::Server));
        let (pa, pb) = (t.port(a, PortMedia::Rj45), t.port(b, PortMedia::Rj45));
        t.link((a, pa), (b, pb), CableKind::Cat6A, None);
        t.p.device_mut(a).unwrap().ports[0].speed_mbps = 10_000;
        assert!(t.codes().is_empty(), "a slower end device is normal");

        let (s1, s2) = (t.dev(DeviceKind::SwitchL2), t.dev(DeviceKind::SwitchL2));
        let (p1, p2) = (t.port(s1, PortMedia::Rj45), t.port(s2, PortMedia::Rj45));
        t.link((s1, p1), (s2, p2), CableKind::Cat6A, None);
        t.p.device_mut(s1).unwrap().ports[0].speed_mbps = 10_000;
        let issues = crate::validate(&t.p);
        assert_eq!((issues[0].code, issues[0].severity), ("link.speed", crate::Severity::Warning));

        let ap = t.dev(DeviceKind::AccessPoint);
        let (p1, p2) = (t.port(s1, PortMedia::Rj45), t.port(ap, PortMedia::Rj45));
        t.link((s1, p1), (ap, p2), CableKind::Cat6A, None);
        assert!(crate::validate(&t.p).iter().any(|i| i.code == "link.speed" && i.severity == crate::Severity::Info), "AP bottlenecked by a 1G port");

        let (pa, pb) = (t.port(a, PortMedia::SfpPlus), t.port(b, PortMedia::SfpPlus));
        t.link((a, pa), (b, pb), CableKind::Dac, Some(3.0));
        t.set_tx(a, pa, "sfp-1g-sx");
        let issues = crate::validate(&t.p);
        assert!(issues.iter().any(|i| i.code == "link.speed" && i.severity == crate::Severity::Error));
        assert!(issues.iter().any(|i| i.code == "tx.on-dac"));
    }

    #[test]
    fn duplex_mismatch() {
        use netvis_core::Duplex;
        let mut t = T::new();
        let (a, b) = (t.dev(DeviceKind::Router), t.dev(DeviceKind::Router));
        let (pa, pb) = (t.port(a, PortMedia::Rj45), t.port(b, PortMedia::Rj45));
        t.link((a, pa), (b, pb), CableKind::Cat6, None);
        t.p.device_mut(a).unwrap().ports[0].cfg.duplex = Duplex::Full;
        assert_eq!(crate::validate(&t.p)[0].severity, crate::Severity::Warning);
        t.p.device_mut(b).unwrap().ports[0].cfg.duplex = Duplex::Half;
        assert_eq!(crate::validate(&t.p)[0].severity, crate::Severity::Error);
    }

    #[test]
    fn wireless_rules() {
        let mut t = T::new();
        let (ap, lt) = (t.dev(DeviceKind::AccessPoint), t.dev(DeviceKind::Laptop));
        let (pa, pb) = (t.port(ap, PortMedia::Wireless), t.port(lt, PortMedia::Wireless));
        t.link((ap, pa), (lt, pb), CableKind::Wireless, None);
        assert!(t.codes().is_empty());
        t.p.links[0].cable = CableKind::Cat6;
        assert!(t.codes().contains(&"cable.media"));
    }
}
