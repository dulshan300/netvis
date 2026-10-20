//! Rack elevation rules: valid rack, in range, no overlaps.

use netvis_core::{AreaKind, Item, Project};

use crate::Sink;

pub fn check(p: &Project, out: &mut Sink) {
    for d in &p.devices {
        let Some(m) = d.hw.rack else { continue };
        let Some(rack) = p.area(m.rack) else {
            out.error("rack.missing", format!("{} is mounted in a rack that no longer exists", d.name), vec![Item::Device(d.id)]);
            continue;
        };
        let items = vec![Item::Device(d.id), Item::Area(rack.id)];
        if rack.kind != AreaKind::Rack {
            out.error("rack.not-rack", format!("{} is mounted in “{}”, which is not a rack", d.name, rack.name), items);
            continue;
        }
        if d.hw.rack_units == 0 {
            out.warn("rack.no-height", format!("{} is placed in {} but has no rack height", d.name, rack.name), items);
            continue;
        }
        let top = m.u as u16 + d.hw.rack_units as u16 - 1;
        if m.u == 0 || top > rack.rack_units as u16 {
            out.error(
                "rack.range",
                format!("{} at U{}–U{top} does not fit {} ({}U)", d.name, m.u, rack.name, rack.rack_units),
                items,
            );
        }
    }

    for rack in p.racks() {
        let contents = p.rack_contents(rack.id);
        for (i, a) in contents.iter().enumerate() {
            for b in &contents[i + 1..] {
                if a.height == 0 || b.height == 0 {
                    continue;
                }
                if a.bottom <= b.top() && b.bottom <= a.top() {
                    out.error(
                        "rack.overlap",
                        format!("{}: {} (U{}–U{}) overlaps {} (U{}–U{})", rack.name, a.device.name, a.bottom, a.top(), b.device.name, b.bottom, b.top()),
                        vec![Item::Device(a.device.id), Item::Device(b.device.id), Item::Area(rack.id)],
                    );
                }
            }
        }
        let unplaced: Vec<_> =
            p.devices_in_area(rack.id).into_iter().filter(|d| d.hw.rack.is_none() && d.hw.rack_units > 0).collect();
        if !unplaced.is_empty() {
            let names: Vec<_> = unplaced.iter().map(|d| d.name.as_str()).collect();
            let mut items = vec![Item::Area(rack.id)];
            items.extend(unplaced.iter().map(|d| Item::Device(d.id)));
            out.info(
                "rack.unplaced",
                format!("{}: {} drawn in the rack but not placed in its elevation ({})", rack.name, names.len(), names.join(", ")),
                items,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use netvis_core::{Area, AreaKind, DeviceKind, Edit, RackMount, Rect};

    use crate::testutil::T;

    #[test]
    fn overlap_range_and_unplaced() {
        let mut t = T::new();
        let rack = Area::new(AreaKind::Rack, "R1", Rect::new(-50.0, -50.0, 2000.0, 400.0));
        let rid = rack.id;
        t.h.apply(&mut t.p, Edit::Area { id: rid, before: None, after: Some(rack) }, "");
        t.p.area_mut(rid).unwrap().rack_units = 10;

        let a = t.dev(DeviceKind::Storage);
        let b = t.dev(DeviceKind::Server);
        let _c = t.dev(DeviceKind::Router);
        assert_eq!(t.codes(), vec!["rack.unplaced"]);

        t.p.device_mut(a).unwrap().hw.rack = Some(RackMount { rack: rid, u: 1 });
        t.p.device_mut(b).unwrap().hw.rack = Some(RackMount { rack: rid, u: 2 });
        assert_eq!(t.codes(), vec!["rack.overlap", "rack.unplaced"]);

        t.p.device_mut(b).unwrap().hw.rack = Some(RackMount { rack: rid, u: 10 });
        assert_eq!(t.codes(), vec!["rack.unplaced"]);
        t.p.device_mut(a).unwrap().hw.rack = Some(RackMount { rack: rid, u: 10 });
        let codes = t.codes();
        assert!(codes.contains(&"rack.range"), "{codes:?}");
    }
}
