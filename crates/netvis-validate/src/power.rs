//! PoE delivery and budgets, and UPS load per rack.

use netvis_core::{DeviceKind, Item, PoeRole, Project};

use crate::Sink;

pub fn check(p: &Project, out: &mut Sink) {
    poe_budgets(p, out);
    poe_delivery(p, out);
    ups_load(p, out);
}

fn poe_budgets(p: &Project, out: &mut Sink) {
    for d in &p.devices {
        let r = p.poe_report(d.id);
        if r.loads.is_empty() {
            continue;
        }
        let mut items = vec![Item::Device(d.id)];
        items.extend(r.loads.iter().map(|l| Item::Device(l.pd.device)));
        match r.budget_w {
            None => {
                out.info("poe.no-budget", format!("{}: powers {} PoE device(s) but has no PoE budget set", d.name, r.loads.len()), items);
            }
            Some(b) if r.demand_w > b => {
                out.error(
                "poe.budget",
                format!("{}: PoE budget exceeded — {:.0} W needed, {:.0} W available", d.name, r.demand_w, b),
                items,
            );
            }
            Some(b) if r.demand_w > 0.9 * b => {
                out.warn(
                "poe.budget",
                format!("{}: PoE budget at {:.0}% ({:.0} of {:.0} W) — little headroom", d.name, r.demand_w / b * 100.0, r.demand_w, b),
                items,
            );
            }
            Some(_) => {}
        }
    }
}

/// Every powered device needs PoE from the far end of its run (patch panels
/// pass power through).
fn poe_delivery(p: &Project, out: &mut Sink) {
    for c in p.circuits().into_iter().filter(|c| c.is_terminated(p)) {
        for (pd_end, pse_end) in [(c.a, c.b), (c.b, c.a)] {
            let (Some(pd_dev), Some(pse_dev)) = (p.device(pd_end.device), p.device(pse_end.device)) else { continue };
            let (Some(pd_port), Some(pse_port)) = (pd_dev.port(pd_end.port), pse_dev.port(pse_end.port)) else { continue };
            let PoeRole::Powered(need) = pd_port.poe else { continue };
            let mut items = vec![Item::Device(pd_dev.id), Item::Device(pse_dev.id)];
            items.extend(c.links.iter().map(|l| Item::Link(*l)));
            let (pd, pse) = (format!("{}:{}", pd_dev.name, pd_port.name), format!("{}:{}", pse_dev.name, pse_port.name));
            match pse_port.poe {
                PoeRole::Source(have) if have < need => {
                    out.warn(
                        "poe.class",
                        format!("{pd} needs {} but {pse} supplies {} — the device may run with reduced features", need.label(), have.label()),
                        items,
                    );
                }
                PoeRole::Source(_) => {}
                // Media converters are not modelled as PoE sources or sinks.
                _ if pse_dev.kind == DeviceKind::MediaConverter => {}
                _ => {
                    out.warn("poe.missing", format!("{pd} expects PoE but {pse} provides none — add PoE or a local power supply"), items);
                }
            }
        }
    }
}

fn ups_load(p: &Project, out: &mut Sink) {
    for rack in p.racks() {
        let power = p.rack_power(rack.id);
        let (load, capacity) = (power.load_w, power.ups_capacity_w);
        if capacity <= 0.0 {
            continue;
        }
        let mut items = vec![Item::Area(rack.id)];
        items.extend(p.rack_contents(rack.id).iter().filter(|m| m.device.kind == DeviceKind::Ups).map(|m| Item::Device(m.device.id)));
        if load > capacity {
            out.error("ups.overload", format!("{}: UPS overloaded — {load:.0} W load on {capacity:.0} W capacity", rack.name), items);
        } else if load > 0.8 * capacity {
            out.warn("ups.load", format!("{}: UPS at {:.0}% ({load:.0} of {capacity:.0} W)", rack.name, load / capacity * 100.0), items);
        }
    }
}

#[cfg(test)]
mod tests {
    use netvis_core::{CableKind, DeviceKind, PoeRole, PoeStandard, PortMedia};

    use crate::testutil::T;

    #[test]
    fn budget_and_class_checks() {
        let mut t = T::new();
        let sw = t.dev(DeviceKind::SwitchL2);
        t.p.device_mut(sw).unwrap().hw.poe_budget_w = Some(70.0);
        for _ in 0..2 {
            let ap = t.dev(DeviceKind::AccessPoint);
            let (a, b) = (t.port(sw, PortMedia::Rj45), t.port(ap, PortMedia::Rj45));
            t.link((sw, a), (ap, b), CableKind::Cat6, None);
        }
        assert!(poe(&t).is_empty(), "60 of 70 W is fine: {:?}", poe(&t));

        let cam = t.dev(DeviceKind::IpCamera);
        let (a, b) = (t.port(sw, PortMedia::Rj45), t.port(cam, PortMedia::Rj45));
        t.link((sw, a), (cam, b), CableKind::Cat6, None);
        let issues = crate::validate(&t.p);
        assert!(issues.iter().any(|i| i.code == "poe.budget" && i.severity == crate::Severity::Error));

        // A bt PD on an at port.
        t.p.device_mut(cam).unwrap().ports[0].poe = PoeRole::Powered(PoeStandard::BtType3);
        assert!(t.codes().contains(&"poe.class"));
    }

    fn poe(t: &T) -> Vec<&'static str> {
        t.codes().into_iter().filter(|c| c.starts_with("poe")).collect()
    }

    #[test]
    fn pd_power_is_traced_through_patch_panels() {
        let mut t = T::new();
        let router = t.dev(DeviceKind::Router);
        let cam = t.dev(DeviceKind::IpCamera);
        let (a, b) = (t.port(router, PortMedia::Rj45), t.port(cam, PortMedia::Rj45));
        t.link((router, a), (cam, b), CableKind::Cat6, None);
        assert_eq!(t.codes(), vec!["poe.missing"]);

        // Camera → panel → PoE switch: powered, and counted in the budget.
        let (pp, cam2, sw) = (t.dev(DeviceKind::PatchPanel), t.dev(DeviceKind::IpCamera), t.dev(DeviceKind::SwitchL2));
        let pp_port = t.port(pp, PortMedia::Rj45);
        let cam_port = t.port(cam2, PortMedia::Rj45);
        t.link((pp, pp_port), (cam2, cam_port), CableKind::Cat6, Some(60.0));
        assert_eq!(t.codes(), vec!["patch.unterminated", "poe.missing"], "unpatched outlet");
        let sw_port = t.port(sw, PortMedia::Rj45);
        t.link((sw, sw_port), (pp, pp_port), CableKind::Cat6, Some(2.0));
        assert_eq!(t.codes(), vec!["poe.missing"], "the router-fed camera still has no power");
        assert_eq!(t.p.poe_report(sw).loads.len(), 1);
    }

    #[test]
    fn ups_overload() {
        use netvis_core::{Area, AreaKind, Edit, RackMount, Rect};
        let mut t = T::new();
        let rack = Area::new(AreaKind::Rack, "R1", Rect::new(5000.0, 0.0, 10.0, 10.0));
        let rid = rack.id;
        t.h.apply(&mut t.p, Edit::Area { id: rid, before: None, after: Some(rack) }, "");
        let ups = t.dev(DeviceKind::Ups);
        let srv = t.dev(DeviceKind::Server);
        for (d, u) in [(ups, 1), (srv, 3)] {
            t.p.device_mut(d).unwrap().hw.rack = Some(RackMount { rack: rid, u });
        }
        t.p.device_mut(ups).unwrap().hw.capacity_w = Some(1000.0);
        t.p.device_mut(srv).unwrap().hw.power_w = Some(850.0);
        assert_eq!(t.codes(), vec!["ups.load"]);
        t.p.device_mut(srv).unwrap().hw.power_w = Some(1200.0);
        assert_eq!(t.codes(), vec!["ups.overload"]);
    }
}
