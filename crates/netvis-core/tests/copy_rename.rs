use netvis_core::{Area, AreaKind, DeviceKind, Edit, Endpoint, Pos, Project, RackMount, Rect};

fn add(p: &mut Project, k: DeviceKind, name: &str, x: f32, y: f32) -> netvis_core::DeviceId {
    let (id, mut e) = p.add_device_edit(k, Pos::new(x, y));
    if let Edit::Device { after: Some(d), .. } = &mut e {
        d.name = name.into();
    }
    e.apply(p);
    id
}

#[test]
fn copying_a_floor_renames_and_keeps_rack_positions() {
    let mut p = Project::default();
    let floor = Area::new(AreaKind::Floor, "L1 · Retail", Rect::new(0.0, 0.0, 1000.0, 400.0));
    let rack = Area::new(AreaKind::Rack, "ELV L1", Rect::new(20.0, 30.0, 400.0, 300.0));
    let (fid, rid) = (floor.id, rack.id);
    p.areas.push(floor);
    p.areas.push(rack);
    let sw = add(&mut p, DeviceKind::SwitchL2, "SW-L1-SEC", 40.0, 60.0);
    p.device_mut(sw).unwrap().hw.rack = Some(RackMount { rack: rid, u: 20 });
    let cam = add(&mut p, DeviceKind::IpCamera, "CAM-L1-01", 600.0, 60.0);
    let outside = add(&mut p, DeviceKind::SwitchL3, "CORE", 0.0, 900.0);
    for (a, b) in [(sw, cam), (sw, outside)] {
        let ea = Endpoint { device: a, port: p.first_free_port(a, None).unwrap() };
        let eb = Endpoint { device: b, port: p.first_free_port(b, None).unwrap() };
        p.add_link_edit(ea, eb).unwrap().1.apply(&mut p);
    }
    p.links[0].label = "C-L1-001".into();

    let (res, edit) = p.copy_with_rename_edit(&[fid], &[], Pos::new(0.0, -500.0), "L1", "L2");
    edit.apply(&mut p);
    assert_eq!((res.devices.len(), res.areas.len(), res.renumbered), (2, 2, 0));
    let names: Vec<&str> = p.devices.iter().map(|d| d.name.as_str()).collect();
    assert!(names.contains(&"SW-L2-SEC") && names.contains(&"CAM-L2-01"));
    assert!(p.areas.iter().any(|a| a.name == "L2 · Retail"));
    let new_rack = p.areas.iter().find(|a| a.name == "ELV L2").unwrap().id;
    let new_sw = p.devices.iter().find(|d| d.name == "SW-L2-SEC").unwrap();
    assert_eq!(new_sw.hw.rack, Some(RackMount { rack: new_rack, u: 20 }));
    assert_eq!(new_sw.pos, Pos::new(40.0, -440.0));
    // The camera cable is copied and relabelled; the uplink to the core isn't.
    assert_eq!(p.links.len(), 3);
    assert!(p.links.iter().any(|l| l.label == "C-L2-001"));

    // Copying again with the same rename can't reuse the names.
    let (res, _) = p.copy_with_rename_edit(&[fid], &[], Pos::new(0.0, -1000.0), "L1", "L2");
    assert_eq!(res.renumbered, 2);
}
