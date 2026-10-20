//! Physical-layer checks. Each cable must suit its two ports; each run
//! (through any patch panels) is checked end to end for optics, speed,
//! channel length and duplex.

use netvis_core::catalog::{CableFamily, Reach, TxMedium};
use netvis_core::catalog::format_speed;
use std::collections::{BTreeSet, HashSet};


use netvis_core::{CableKind, Circuit, Duplex, Item, Link, LinkId, PortMedia, Project};

use crate::{End, Fix, Sink};

pub fn check(p: &Project, out: &mut Sink) {
    modules(p, out);
    let mut bad: HashSet<LinkId> = HashSet::new();
    for l in &p.links {
        if !check_segment(p, l, out) {
            bad.insert(l.id);
        }
    }
    for c in p.circuits() {
        if c.links.iter().all(|id| !bad.contains(id)) {
            check_circuit(p, &c, out);
        }
    }
}

/// Fitted modules must be known and fit their cage (cabled ports only).
fn modules(p: &Project, out: &mut Sink) {
    for d in &p.devices {
        for port in d.ports.iter().filter(|x| x.transceiver.is_some()) {
            let Some(link) = p.port_link(port.id) else { continue };
            let Some(e) = End::resolve(p, netvis_core::Endpoint { device: d.id, port: port.id }) else { continue };
            let items = vec![Item::Link(link.id), Item::Device(d.id)];
            match (&e.port.transceiver, e.tx) {
                (Some(id), None) => {
                    out.warn("tx.unknown", format!("{}: unknown transceiver “{id}”", e.label()), items);
                }
                (Some(_), Some(t)) if !e.port.media.is_optical_cage() => {
                    out.warn("tx.no-cage", format!("{}: {} fitted to a {} port", e.label(), t.name, e.port.media.label()), items);
                }
                (Some(_), Some(t)) if !t.fits(e.port.media) => {
                    out.error(
                        "tx.form-factor",
                        format!("{}: {} ({}) does not fit a {} cage", e.label(), t.name, t.form.label(), e.port.media.label()),
                        items,
                    );
                }
                _ => {}
            }
        }
    }
}

/// Per-cable checks: the cable must suit the ports at both of its ends.
/// Returns false when the run-level checks would be meaningless.
fn check_segment(p: &Project, l: &Link, out: &mut Sink) -> bool {
    let (Some(a), Some(b)) = (End::resolve(p, l.a), End::resolve(p, l.b)) else {
        out.error("link.dangling", "Link points at a device or port that no longer exists", vec![Item::Link(l.id)]);
        return false;
    };
    let items = || vec![Item::Link(l.id), Item::Device(a.dev.id), Item::Device(b.dev.id)];
    let mut ok = true;
    for e in [&a, &b] {
        if let Some(msg) = media_problem(l.cable, e) {
            out.error("cable.media", msg, items());
            ok = false;
        }
    }
    if l.cable.family() == CableFamily::DirectAttach && (a.dev.kind.is_passthrough() || b.dev.kind.is_passthrough()) {
        out.error("cable.media", format!("{} ↔ {}: {} cables have fixed ends and can't be patched through a panel", a.label(), b.label(), l.cable.label()), items());
        ok = false;
    }
    if ok && l.cable == CableKind::Unspecified {
        out.info("cable.unset", format!("{} ↔ {}: cable type not set", a.label(), b.label()), items());
    }
    ok
}

/// Run-level checks between the two active ends: optics, speed, reach of
/// the whole channel, duplex, and unterminated runs.
fn check_circuit(p: &Project, c: &Circuit, out: &mut Sink) {
    let segs: Vec<&Link> = c.links.iter().filter_map(|id| p.link(*id)).collect();
    let mut items: Vec<Item> = c.links.iter().map(|l| Item::Link(*l)).collect();
    items.extend(c.ports().map(|e| Item::Device(e.device)));
    items.dedup();
    let items = || items.clone();

    if !c.is_terminated(p) {
        let (Some(a), Some(b)) = (End::resolve(p, c.a), End::resolve(p, c.b)) else { return };
        if a.dev.kind.is_passthrough() {
            return; // a spare permanent link: normal
        }
        let msg = if is_infra(a.dev.kind) {
            format!("{} is patched to {}, which goes no further", a.label(), b.label())
        } else {
            format!("{} is cabled to {} but the run stops there — patch it through to a switch", a.label(), b.label())
        };
        if is_infra(a.dev.kind) {
            out.info("patch.unterminated", msg, items());
        } else {
            out.warn("patch.unterminated", msg, items());
        }
        return;
    }
    let (Some(a), Some(b)) = (End::resolve(p, c.a), End::resolve(p, c.b)) else { return };
    let pair = if c.is_direct() {
        format!("{} ↔ {}", a.label(), b.label())
    } else {
        let via: Vec<String> = c.via.iter().filter_map(|e| End::resolve(p, *e)).map(|e| e.label()).collect();
        format!("{} ↔ {} (via {})", a.label(), b.label(), via.join(", "))
    };

    // One kind of medium end to end.
    let families: HashSet<CableFamily> =
        segs.iter().map(|l| l.cable.family()).filter(|f| *f != CableFamily::Logical).collect();
    if families.len() > 1 {
        let kinds: BTreeSet<&str> = segs.iter().map(|l| l.cable.label()).collect();
        out.error("cable.mixed", format!("{pair}: the run mixes {}", kinds.into_iter().collect::<Vec<_>>().join(" and ")), items());
        return;
    }
    // The end ports see the cable next to them.
    let (cable_a, cable_b) = (segs[0].cable, segs[segs.len() - 1].cable);
    if !c.is_direct() {
        for (e, cable) in [(&a, cable_b), (&b, cable_a)] {
            // An optic at one end must also suit the fibre at the other end.
            if let Some(msg) = media_problem(cable, e) {
                out.error("cable.media", msg, items());
                return;
            }
        }
    }

    if segs.iter().any(|l| l.cable.is_fibre()) {
        let cable = cable_a;
        let missing: Vec<&End> = [&a, &b].into_iter().filter(|e| e.tx.is_none() && e.port.transceiver.is_none()).collect();
        if !missing.is_empty() {
            let names: Vec<_> = missing.iter().map(|e| e.label()).collect();
            let picks: Option<Vec<_>> = missing
                .iter()
                .map(|e| suggest_optic(e, cable).map(|t| (e.dev.id, e.port.id, t.id.to_owned())))
                .collect();
            out.info("tx.missing", format!("{}: no transceiver selected for the {} link", names.join(", "), cable.label()), items())
                .with_fix(picks.filter(|v| !v.is_empty()).map(Fix::SetTransceivers));
        }
    }
    if cable_a.family() == CableFamily::DirectAttach {
        for e in [&a, &b] {
            if let Some(t) = e.tx {
                out.warn("tx.on-dac", format!("{}: {} has integrated ends — remove the {} module", e.label(), cable_a.label(), t.name), items());
            }
        }
    }

    // Optics must match each other.
    if let (Some(ta), Some(tb)) = (a.tx, b.tx)
        && (ta.medium != tb.medium || ta.wavelength_nm != tb.wavelength_nm || ta.speed_mbps != tb.speed_mbps)
    {
        let fix = (ta.fits(b.port.media) && ta.reach_on(cable_b).is_some())
            .then(|| Fix::SetTransceivers(vec![(b.dev.id, b.port.id, ta.id.to_owned())]));
        out.error("tx.mismatch", format!("{pair}: transceiver mismatch {} ↔ {}", ta.name, tb.name), items()).with_fix(fix);
        return;
    }

    let family = cable_a.family();
    if matches!(family, CableFamily::Wireless | CableFamily::Logical | CableFamily::Console | CableFamily::Coax) {
        check_duplex(&a, &b, &pair, items(), out);
        return;
    }

    // Speed.
    let (sa, sb) = (a.speed(), b.speed());
    let copper_both = [&a, &b].iter().all(|e| e.port.media == PortMedia::Rj45 || e.tx.is_some_and(|t| t.medium == TxMedium::Copper));
    if sa != sb {
        let (slow, fast) = if sa < sb { (&a, &b) } else { (&b, &a) };
        let msg = format!("{pair}: speed mismatch {} vs {}", format_speed(sa), format_speed(sb));
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

    // Every cable must carry the speed; the channel as a whole must be in reach.
    let module_limit = |cable: CableKind| [a.tx, b.tx].into_iter().flatten().filter_map(|t| t.reach_on(cable)).reduce(f32::min);
    let mut channel: Option<(f32, CableKind)> = None;
    let mut unrated = false;
    for l in &segs {
        let mut reach = l.cable.reach(speed);
        let module = module_limit(l.cable);
        if let Some(m) = module {
            reach = match reach {
                Reach::Max(c) if !l.cable.is_fibre() => Reach::Max(c.min(m)),
                Reach::Unsupported if !l.cable.is_fibre() => Reach::Unsupported,
                _ => Reach::Max(m),
            };
        }
        // Copper that is too slow or too long at 10G: Cat6A carries 10G to 100 m.
        let upgrade = (l.cable.is_twisted_pair()
            && speed <= 10_000
            && module.is_none()
            && l.cable != CableKind::Cat6A
            && (segs.len() > 1 || l.length_m.is_none_or(|m| m <= 100.0)))
        .then_some(Fix::SetCable { link: l.id, cable: CableKind::Cat6A });
        match reach {
            Reach::Unsupported => {
                let what = if c.is_direct() { pair.clone() } else { format!("{pair}, cable {}", segment_label(p, l)) };
                out.error("cable.speed", format!("{what}: {} is not rated for {}", l.cable.label(), format_speed(speed)), items())
                    .with_fix(upgrade);
                unrated = true;
            }
            Reach::Max(max) => {
                if channel.is_none_or(|(m, _)| max < m) {
                    channel = Some((max, l.cable));
                }
                if c.is_direct()
                    && let Some(len) = l.length_m
                    && len > max
                {
                    out.error(
                        "cable.reach",
                        format!("{pair}: {len} m exceeds the {max} m limit of {} at {}", l.cable.label(), format_speed(speed)),
                        items(),
                    )
                    .with_fix(upgrade);
                }
            }
            Reach::Unlimited | Reach::Unknown => {}
        }
    }
    if !c.is_direct() && !unrated && let Some((max, cable)) = channel {
        // Lengths that are known give a lower bound for the channel.
        let len: f32 = segs.iter().filter_map(|l| l.length_m).sum();
        if len > max {
            out.error(
                "cable.reach",
                format!("{pair}: the channel is {len:.0} m over {} cables — more than the {max} m limit of {} at {}", segs.len(), cable.label(), format_speed(speed)),
                items(),
            );
        }
    }

    check_duplex(&a, &b, &pair, items(), out);
}

fn segment_label(p: &Project, l: &Link) -> String {
    let name = |e: netvis_core::Endpoint| End::resolve(p, e).map(|e| e.label()).unwrap_or_else(|| "?".into());
    format!("{} ↔ {}", name(l.a), name(l.b))
}

/// A module for an empty cage that suits the fibre and the port speed:
/// SR-class on multimode, LR-class on single-mode.
fn suggest_optic(e: &End, cable: CableKind) -> Option<&'static netvis_core::catalog::Transceiver> {
    netvis_core::catalog::transceiver::for_cage(e.port.media)
        .filter(|t| t.speed_mbps == e.port.speed_mbps && t.reach_on(cable).is_some())
        .max_by(|x, y| {
            // Prefer the shortest-reach module that does the job (cheapest optic).
            let (rx, ry) = (x.reach_on(cable).unwrap_or(0.0), y.reach_on(cable).unwrap_or(0.0));
            ry.total_cmp(&rx)
        })
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
            PortMedia::Lc | PortMedia::Mpo => None,
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
        t.fix("cable.speed");
        assert_eq!(t.p.links[0].cable, CableKind::Cat6A);
        assert!(t.codes().is_empty());
    }

    #[test]
    fn missing_optics_are_suggested() {
        let mut t = T::new();
        let (a, b) = (t.dev(DeviceKind::SwitchL3), t.dev(DeviceKind::SwitchL2));
        let (pa, pb) = (t.port(a, PortMedia::SfpPlus), t.port(b, PortMedia::SfpPlus));
        t.link((a, pa), (b, pb), CableKind::Os2, Some(2_000.0));
        assert_eq!(t.codes(), vec!["tx.missing"]);
        t.fix("tx.missing");
        assert!(t.codes().is_empty(), "{:?}", t.codes());
        let tx = t.p.device(a).unwrap().port(pa).unwrap().transceiver.clone();
        assert_eq!(tx.as_deref(), Some("sfpp-10g-lr"), "LR, not the pricier ER");
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

    /// Port `i` of `d` (for patch-panel ports that already have one cable).
    fn nth(t: &T, d: netvis_core::DeviceId, i: usize) -> netvis_core::PortId {
        t.p.device(d).unwrap().ports[i].id
    }

    #[test]
    fn copper_channel_through_panels() {
        let mut t = T::new();
        let (sw, pp, wo, pc) = (t.dev(DeviceKind::SwitchL2), t.dev(DeviceKind::PatchPanel), t.dev(DeviceKind::WallOutlet), t.dev(DeviceKind::Pc));
        let sp = t.port(sw, PortMedia::Rj45);
        t.link((sw, sp), (pp, nth(&t, pp, 0)), CableKind::Cat6, Some(3.0));
        t.link((pp, nth(&t, pp, 0)), (wo, nth(&t, wo, 0)), CableKind::Cat6A, Some(90.0));
        t.link((wo, nth(&t, wo, 0)), (pc, nth(&t, pc, 0)), CableKind::Cat6, Some(5.0));
        assert!(t.codes().is_empty(), "98 m channel is fine: {:?}", crate::validate(&t.p));
        assert_eq!(t.p.untagged_vlan_at(pc, nth(&t, pc, 0)), None);

        t.p.links[2].length_m = Some(9.0);
        let issues = crate::validate(&t.p);
        assert_eq!(t.codes(), vec!["cable.reach"], "102 m channel");
        assert!(issues[0].message.contains("via PP-01:1, WO-01:A"), "{}", issues[0].message);

        // 10G on the run: the Cat6 cords are fine at that length, Cat5e is not.
        t.p.links[2].length_m = Some(5.0);
        t.p.links[0].cable = CableKind::Cat5e;
        for d in [sw, pc] {
            t.p.device_mut(d).unwrap().ports[0].speed_mbps = 10_000;
        }
        let codes = t.codes();
        assert!(codes.contains(&"cable.speed"), "{codes:?}");
        t.fix("cable.speed");
        assert_eq!(t.p.links[0].cable, CableKind::Cat6A);
    }

    #[test]
    fn fibre_through_lc_panels() {
        let mut t = T::new();
        let (a, b) = (t.dev(DeviceKind::SwitchL3), t.dev(DeviceKind::SwitchL2));
        let (fa, fb) = (t.dev(DeviceKind::PatchPanel), t.dev(DeviceKind::PatchPanel));
        for pp in [fa, fb] {
            for port in &mut t.p.device_mut(pp).unwrap().ports {
                port.media = PortMedia::Lc;
            }
        }
        let (pa, pb) = (t.port(a, PortMedia::SfpPlus), t.port(b, PortMedia::SfpPlus));
        t.link((a, pa), (fa, nth(&t, fa, 0)), CableKind::Os2, Some(2.0));
        t.link((fa, nth(&t, fa, 0)), (fb, nth(&t, fb, 0)), CableKind::Os2, Some(1_500.0));
        t.link((fb, nth(&t, fb, 0)), (b, pb), CableKind::Os2, Some(2.0));
        assert_eq!(t.codes(), vec!["tx.missing"], "one finding for the whole run");
        t.fix("tx.missing");
        assert!(t.codes().is_empty(), "{:?}", crate::validate(&t.p));

        // A multimode trunk between single-mode cords.
        t.p.links[1].cable = CableKind::Om4;
        assert_eq!(t.codes(), vec!["cable.mixed"]);
        t.p.links[1].cable = CableKind::Os2;
        // Copper into an LC adapter.
        t.p.links[2].cable = CableKind::Cat6;
        assert!(t.codes().contains(&"cable.media"));
        // DAC through a panel.
        t.p.links[2].cable = CableKind::Dac;
        assert!(t.codes().contains(&"cable.media"));
    }

    #[test]
    fn unterminated_runs() {
        let mut t = T::new();
        let (pp, pc, sw) = (t.dev(DeviceKind::PatchPanel), t.dev(DeviceKind::Pc), t.dev(DeviceKind::SwitchL2));
        t.link((pp, nth(&t, pp, 0)), (pc, nth(&t, pc, 0)), CableKind::Cat6, None);
        let issues = crate::validate(&t.p);
        assert_eq!(issues.len(), 1);
        assert_eq!((issues[0].code, issues[0].severity), ("patch.unterminated", crate::Severity::Warning));
        // A spare permanent link between two panels is not a problem.
        let pp2 = t.dev(DeviceKind::PatchPanel);
        t.link((pp, nth(&t, pp, 1)), (pp2, nth(&t, pp2, 1)), CableKind::Cat6A, None);
        assert_eq!(crate::validate(&t.p).len(), 1);
        let sp = t.port(sw, PortMedia::Rj45);
        t.link((sw, sp), (pp, nth(&t, pp, 0)), CableKind::Cat6, None);
        assert!(t.codes().is_empty(), "{:?}", crate::validate(&t.p));
    }
}
