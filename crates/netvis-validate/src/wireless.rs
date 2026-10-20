//! Wi-Fi: channel validity and overlap between neighbouring APs, SSID
//! security, and SSID VLANs reaching the APs.

use std::collections::HashMap;

use netvis_core::{AreaKind, DeviceId, DeviceKind, Item, PortId, Project, RadioConfig, SwitchportMode, WifiBand, WifiSecurity, WifiStandard};

use crate::{Fix, Sink};

pub fn check(p: &Project, out: &mut Sink) {
    radios(p, out);
    ssids(p, out);
}

fn valid_channels(band: WifiBand) -> Vec<u16> {
    match band {
        WifiBand::G2_4 => (1..=13).collect(),
        WifiBand::G5 => [36, 40, 44, 48, 52, 56, 60, 64]
            .into_iter()
            .chain((100..=144).step_by(4))
            .chain([149, 153, 157, 161, 165])
            .collect(),
        WifiBand::G6 => (1..=233).step_by(4).collect(),
    }
}

/// Occupied spectrum `[low, high]` in MHz for a radio's channel and width.
fn spectrum(r: &RadioConfig) -> (u32, u32) {
    let ch = r.channel as u32;
    let width = r.width_mhz.max(20) as u32;
    let n = width / 20; // 20 MHz sub-channels
    match r.band {
        WifiBand::G2_4 => {
            let c = 2407 + 5 * ch;
            // 40 MHz on 2.4 GHz bonds upwards (HT40+).
            (c - 11, c + 11 + if width >= 40 { 20 } else { 0 })
        }
        WifiBand::G5 | WifiBand::G6 => {
            let (base_ch, base_f) = match r.band {
                WifiBand::G5 if ch >= 149 => (149, 5000),
                WifiBand::G5 => (36, 5000),
                _ => (1, 5950),
            };
            let block = 4 * n;
            let start = base_ch + (ch.saturating_sub(base_ch) / block) * block;
            let lo = base_f + 5 * start - 10;
            (lo, lo + width)
        }
    }
}

fn overlaps(a: &RadioConfig, b: &RadioConfig) -> bool {
    let (x, y) = (spectrum(a), spectrum(b));
    a.band == b.band && x.0 < y.1 && y.0 < x.1
}

struct Radio<'a> {
    dev: &'a netvis_core::Device,
    port: PortId,
    cfg: &'a RadioConfig,
}

/// Smallest Floor/Building area containing the device, and the switch it hangs off.
fn neighbourhood(p: &Project, d: &netvis_core::Device) -> (Option<netvis_core::AreaId>, Option<DeviceId>) {
    let r = d.rect();
    let c = r.min.offset(r.size.x / 2.0, r.size.y / 2.0);
    let area = p
        .areas
        .iter()
        .filter(|a| matches!(a.kind, AreaKind::Floor | AreaKind::Building) && a.rect.contains(c))
        .min_by(|a, b| (a.rect.size.x * a.rect.size.y).total_cmp(&(b.rect.size.x * b.rect.size.y)))
        .map(|a| a.id);
    let switch = p.links_of(d.id).find_map(|l| {
        let far = l.other_end(d.id)?;
        p.device(far.device).filter(|x| matches!(x.kind, DeviceKind::SwitchL2 | DeviceKind::SwitchL3)).map(|x| x.id)
    });
    (area, switch)
}

fn radios(p: &Project, out: &mut Sink) {
    let mut all = Vec::new();
    for d in &p.devices {
        for port in &d.ports {
            if let Some(cfg) = &port.cfg.radio
                && port.cfg.enabled
            {
                all.push(Radio { dev: d, port: port.id, cfg });
            }
        }
    }

    for r in &all {
        let who = r.dev.name.as_str();
        let items = vec![Item::Device(r.dev.id)];
        let chans = valid_channels(r.cfg.band);
        if !chans.contains(&r.cfg.channel) {
            out.error("wifi.channel", format!("{who}: channel {} is not valid on {}", r.cfg.channel, r.cfg.band.label()), items.clone())
                .with_fix(Some(Fix::SetChannel { device: r.dev.id, port: r.port, channel: r.cfg.band.default_channel() }));
            continue;
        }
        if !r.cfg.standard.bands().contains(&r.cfg.band) {
            out.error("wifi.band", format!("{who}: {} does not operate on {}", r.cfg.standard.label(), r.cfg.band.label()), items.clone());
        }
        if !r.cfg.band.widths().contains(&r.cfg.width_mhz) || (r.cfg.width_mhz == 320 && r.cfg.standard != WifiStandard::Wifi7) {
            out.warn("wifi.width", format!("{who}: {} MHz channels are not available on {} with {}", r.cfg.width_mhz, r.cfg.band.label(), r.cfg.standard.label()), items.clone());
        }
        if r.cfg.band == WifiBand::G2_4 {
            if r.cfg.width_mhz > 20 {
                out.warn("wifi.24-width", format!("{who}: 40 MHz on 2.4 GHz overlaps most of the band — use 20 MHz"), items.clone());
            } else if ![1, 6, 11].contains(&r.cfg.channel) {
                let fix = Some(Fix::SetChannel { device: r.dev.id, port: r.port, channel: nearest(&[1, 6, 11], r.cfg.channel) });
                out.info("wifi.24-channel", format!("{who}: 2.4 GHz channel {} overlaps 1/6/11 neighbours — prefer 1, 6 or 11", r.cfg.channel), items.clone())
                    .with_fix(fix);
            }
        }
    }

    // Overlap between neighbours (same floor/building, or same switch).
    let hoods: HashMap<DeviceId, _> = all.iter().map(|r| (r.dev.id, neighbourhood(p, r.dev))).collect();
    for (i, a) in all.iter().enumerate() {
        for b in &all[i + 1..] {
            if a.dev.id == b.dev.id || !overlaps(a.cfg, b.cfg) {
                continue;
            }
            let (ha, hb) = (hoods[&a.dev.id], hoods[&b.dev.id]);
            let near = (ha.0.is_some() && ha.0 == hb.0) || (ha.1.is_some() && ha.1 == hb.1);
            if !near {
                continue;
            }
            let neighbours: Vec<&RadioConfig> = all
                .iter()
                .filter(|r| r.dev.id != b.dev.id && {
                    let h = hoods[&r.dev.id];
                    (h.0.is_some() && h.0 == hb.0) || (h.1.is_some() && h.1 == hb.1)
                })
                .map(|r| r.cfg)
                .collect();
            let fix = free_channel(b.cfg, &neighbours).map(|channel| Fix::SetChannel { device: b.dev.id, port: b.port, channel });
            out.warn(
                "wifi.overlap",
                format!(
                    "{} and {} overlap on {} (ch {}/{} MHz vs ch {}/{} MHz)",
                    a.dev.name, b.dev.name, a.cfg.band.label(), a.cfg.channel, a.cfg.width_mhz, b.cfg.channel, b.cfg.width_mhz
                ),
                vec![Item::Device(b.dev.id), Item::Device(a.dev.id)],
            )
            .with_fix(fix);
        }
    }
}

fn nearest(options: &[u16], ch: u16) -> u16 {
    *options.iter().min_by_key(|o| (**o as i32 - ch as i32).abs()).expect("non-empty")
}

/// First channel (aligned to the radio's width) that overlaps none of `others`.
fn free_channel(r: &RadioConfig, others: &[&RadioConfig]) -> Option<u16> {
    let candidates: Vec<u16> = match r.band {
        WifiBand::G2_4 => vec![1, 6, 11],
        _ => valid_channels(r.band),
    };
    candidates.into_iter().find(|ch| {
        let probe = RadioConfig { channel: *ch, ..r.clone() };
        spectrum(&probe).0 == {
            // Only primary channels at the start of their bonding block.
            let aligned = RadioConfig { channel: *ch, ..r.clone() };
            spectrum(&aligned).0
        } && !others.iter().any(|o| overlaps(&probe, o))
    })
}

fn ssids(p: &Project, out: &mut Sink) {
    let mut names: HashMap<&str, usize> = HashMap::new();
    for s in &p.ssids {
        *names.entry(s.name.as_str()).or_default() += 1;
        let items = vec![Item::Ssid(s.id)];
        if s.bands.contains(&WifiBand::G6) && !matches!(s.security, WifiSecurity::Wpa3Personal | WifiSecurity::Wpa3Enterprise | WifiSecurity::Owe) {
            out.warn("wifi.6g-security", format!("SSID {}: 6 GHz requires WPA3 or Enhanced Open, not {}", s.name, s.security.label()), items.clone());
        }
        if s.security == WifiSecurity::Open {
            out.warn("wifi.open", format!("SSID {} is open and unencrypted — consider Enhanced Open (OWE)", s.name), items.clone());
        }
        if s.bands.is_empty() {
            out.warn("wifi.no-band", format!("SSID {} is not broadcast on any band", s.name), items.clone());
        }
        let Some(v) = s.vlan else {
            out.info("wifi.no-vlan", format!("SSID {} is not mapped to a VLAN", s.name), items);
            continue;
        };
        // Each AP's switch port must carry the SSID's VLAN.
        for ap in p.devices.iter().filter(|d| d.kind == DeviceKind::AccessPoint) {
            for port in ap.ports.iter().filter(|x| x.cfg.radio.is_none()) {
                let Some(l) = p.port_link(port.id) else { continue };
                let far = if l.a.port == port.id { l.b } else { l.a };
                let Some(fd) = p.device(far.device) else { continue };
                let Some(fp) = fd.port(far.port) else { continue };
                if fp.cfg.mode == SwitchportMode::Unset || fp.cfg.mode.carries(v) {
                    continue;
                }
                let fix = matches!(fp.cfg.mode, SwitchportMode::Trunk { allowed: netvis_core::AllowedVlans::List(_), .. })
                    .then(|| Fix::AllowVlan { ports: vec![(fd.id, fp.id)], vlan: v });
                out.warn(
                    "wifi.ssid-vlan",
                    format!("{}: uplink {}:{} does not carry {} for SSID {}", ap.name, fd.name, fp.name, p.vlan_label(Some(v)), s.name),
                    vec![Item::Device(ap.id), Item::Ssid(s.id), Item::Device(fd.id)],
                )
                .with_fix(fix);
            }
        }
    }
    for (name, n) in names.into_iter().filter(|(_, n)| *n > 1) {
        let items = p.ssids.iter().filter(|s| s.name == name).map(|s| Item::Ssid(s.id)).collect();
        out.warn("wifi.dup-ssid", format!("SSID “{name}” is defined {n} times"), items);
    }
}

#[cfg(test)]
mod tests {
    use netvis_core::{AllowedVlans, Area, AreaKind, CableKind, DeviceKind, Edit, PortMedia, Rect, Ssid, SwitchportMode, Vlan, WifiBand, WifiSecurity};

    use super::*;
    use crate::testutil::T;

    fn radio(band: WifiBand, ch: u16, w: u16) -> RadioConfig {
        RadioConfig { band, channel: ch, width_mhz: w, ..RadioConfig::default() }
    }

    #[test]
    fn spectrum_overlap() {
        assert!(!overlaps(&radio(WifiBand::G2_4, 1, 20), &radio(WifiBand::G2_4, 6, 20)));
        assert!(overlaps(&radio(WifiBand::G2_4, 1, 20), &radio(WifiBand::G2_4, 3, 20)));
        assert!(overlaps(&radio(WifiBand::G5, 36, 80), &radio(WifiBand::G5, 44, 20)), "44 is inside 36–48");
        assert!(!overlaps(&radio(WifiBand::G5, 36, 80), &radio(WifiBand::G5, 52, 80)));
        assert!(!overlaps(&radio(WifiBand::G5, 149, 80), &radio(WifiBand::G5, 132, 40)));
        assert!(!overlaps(&radio(WifiBand::G5, 36, 20), &radio(WifiBand::G2_4, 1, 20)));
        assert_eq!(free_channel(&radio(WifiBand::G5, 36, 80), &[&radio(WifiBand::G5, 36, 80)]), Some(52));
    }

    #[test]
    fn neighbouring_aps_and_ssid_vlans() {
        let mut t = T::new();
        let floor = Area::new(AreaKind::Floor, "L1", Rect::new(-100.0, -100.0, 2000.0, 600.0));
        t.h.apply(&mut t.p, Edit::Area { id: floor.id, before: None, after: Some(floor) }, "");
        let (a, b) = (t.dev(DeviceKind::AccessPoint), t.dev(DeviceKind::AccessPoint));
        assert_eq!(t.codes(), vec!["wifi.overlap"], "both default to ch 36 / 80 MHz on one floor");
        t.fix("wifi.overlap");
        assert!(t.codes().is_empty(), "{:?}", t.codes());
        t.p.device_mut(a).unwrap().ports[1].cfg.radio.as_mut().unwrap().channel = 37;
        assert_eq!(t.codes(), vec!["wifi.channel"]);
        t.fix("wifi.channel");

        let guest = Vlan::new(50, "Guest");
        let gid = guest.id;
        t.h.apply(&mut t.p, Edit::Vlan { id: gid, before: None, after: Some(guest) }, "");
        let mut s = Ssid::new("Guest");
        s.vlan = Some(gid);
        s.security = WifiSecurity::Open;
        s.bands.push(WifiBand::G6);
        t.h.apply(&mut t.p, Edit::Ssid { id: s.id, before: None, after: Some(s) }, "");
        let sw = t.dev(DeviceKind::SwitchL2);
        let (ps, pa) = (t.port(sw, PortMedia::Rj45), t.port(b, PortMedia::Rj45));
        t.link((sw, ps), (b, pa), CableKind::Cat6A, None);
        t.p.device_mut(sw).unwrap().ports[0].cfg.mode = SwitchportMode::Trunk { native: None, allowed: AllowedVlans::List(vec![]) };
        let codes = t.codes();
        for c in ["wifi.6g-security", "wifi.open", "wifi.ssid-vlan"] {
            assert!(codes.contains(&c), "{c} missing from {codes:?}");
        }
        t.fix("wifi.ssid-vlan");
        assert!(!t.codes().contains(&"wifi.ssid-vlan"));
        let _ = b;
    }
}
