//! Automatic layout of the topology diagram.
//!
//! Devices are put in horizontal layers by their cable distance from the top
//! of the network (WAN clouds, then firewalls and routers), so a run reads
//! top-down: core, distribution, access switch, patch panel, outlet, desk.
//!
//! Areas are kept: every area becomes a column holding its own devices and
//! its child areas side by side, and its rectangle is rebuilt around what it
//! holds. Columns never share x-ranges unless one holds the other, so areas
//! can't overlap. Within a column a layer wraps after [`LayoutOptions::per_row`]
//! devices, and devices are ordered to keep cables short (barycentre sweeps).

use std::cmp::Ordering;
use std::collections::{HashMap, HashSet, VecDeque};

use crate::area::Area;
use crate::device::{DeviceCategory, DeviceKind};
use crate::edit::Edit;
use crate::geom::{Pos, Rect};
use crate::ids::{AreaId, DeviceId};
use crate::project::Project;

#[derive(Debug, Clone, Copy)]
pub struct LayoutOptions {
    /// Most devices side by side in one layer of one column before wrapping.
    pub per_row: usize,
    /// Gap between cards in a row.
    pub gap_x: f32,
    /// Gap between wrapped rows of the same layer.
    pub gap_y: f32,
    /// Gap between layers.
    pub layer_gap: f32,
    /// Space between an area's edge and its contents.
    pub pad: f32,
    /// Extra room at the top of an area for its title strip.
    pub header: f32,
    /// Positions are rounded to this grid.
    pub grid: f32,
}

impl Default for LayoutOptions {
    fn default() -> Self {
        Self { per_row: 6, gap_x: 40.0, gap_y: 40.0, layer_gap: 80.0, pad: 20.0, header: 20.0, grid: 20.0 }
    }
}

/// Which devices to arrange.
#[derive(Debug, Clone, Copy)]
pub enum Scope<'a> {
    /// Everything, rebuilding areas around their contents.
    All,
    /// Only these devices, ignoring areas; the result keeps the group's
    /// top-left corner.
    Devices(&'a [DeviceId]),
}

/// Preferred layer of a device when the cables don't decide it (it is not
/// connected to anything higher). Lower is nearer the top.
fn tier(k: DeviceKind) -> usize {
    use DeviceKind::*;
    match k.category() {
        DeviceCategory::CloudWan => 0,
        _ => match k {
            Modem | Firewall | Router | VpnGateway => 1,
            SwitchL3 | LoadBalancer | WirelessController => 2,
            SwitchL2 | MediaConverter => 3,
            PatchPanel => 4,
            WallOutlet | PoeInjector => 5,
            Ups | Pdu => 4,
            _ => 6,
        },
    }
}

/// Compare names so that "PC-2" sorts before "PC-10".
pub fn natural_cmp(a: &str, b: &str) -> Ordering {
    let (mut a, mut b) = (a.as_bytes(), b.as_bytes());
    loop {
        match (a.first(), b.first()) {
            (None, None) => return Ordering::Equal,
            (None, _) => return Ordering::Less,
            (_, None) => return Ordering::Greater,
            (Some(x), Some(y)) if x.is_ascii_digit() && y.is_ascii_digit() => {
                let na = a.iter().take_while(|c| c.is_ascii_digit()).count();
                let nb = b.iter().take_while(|c| c.is_ascii_digit()).count();
                let (da, db) = (&a[..na], &b[..nb]);
                let strip = |d: &[u8]| -> usize { d.iter().take_while(|c| **c == b'0').count() };
                let (ta, tb) = (&da[strip(da)..], &db[strip(db)..]);
                let o = ta.len().cmp(&tb.len()).then_with(|| ta.cmp(tb));
                if o != Ordering::Equal {
                    return o;
                }
                a = &a[na..];
                b = &b[nb..];
            }
            (Some(x), Some(y)) => {
                let o = x.to_ascii_lowercase().cmp(&y.to_ascii_lowercase());
                if o != Ordering::Equal {
                    return o;
                }
                a = &a[1..];
                b = &b[1..];
            }
        }
    }
}

/// Layer of every device: cable hops from the highest-tier device of its
/// connected group, never above the device's own tier minus the hops it is
/// from there (so an isolated UPS still sits with the switches).
fn layers(p: &Project, devs: &[DeviceId]) -> HashMap<DeviceId, usize> {
    let set: HashSet<DeviceId> = devs.iter().copied().collect();
    let mut adj: HashMap<DeviceId, Vec<DeviceId>> = HashMap::new();
    for l in &p.links {
        let (a, b) = (l.a.device, l.b.device);
        if a != b && set.contains(&a) && set.contains(&b) {
            adj.entry(a).or_default().push(b);
            adj.entry(b).or_default().push(a);
        }
    }
    let kind = |d: DeviceId| p.device(d).map(|d| d.kind).unwrap_or(DeviceKind::Pc);
    let mut layer: HashMap<DeviceId, usize> = HashMap::new();
    // Visit groups from their highest-tier member; the start keeps its tier.
    let mut order: Vec<DeviceId> = devs.to_vec();
    order.sort_by_key(|d| tier(kind(*d)));
    for &start in &order {
        if layer.contains_key(&start) {
            continue;
        }
        // Every member of this group with the best tier is a root.
        let mut group = Vec::new();
        let mut seen = HashSet::from([start]);
        let mut q = VecDeque::from([start]);
        while let Some(d) = q.pop_front() {
            group.push(d);
            for n in adj.get(&d).into_iter().flatten() {
                if seen.insert(*n) {
                    q.push_back(*n);
                }
            }
        }
        let best = group.iter().map(|d| tier(kind(*d))).min().unwrap_or(0);
        let mut q: VecDeque<DeviceId> = VecDeque::new();
        for d in group.iter().filter(|d| tier(kind(**d)) == best) {
            layer.insert(*d, best);
            q.push_back(*d);
        }
        while let Some(d) = q.pop_front() {
            let next = layer[&d] + 1;
            for n in adj.get(&d).into_iter().flatten() {
                if !layer.contains_key(n) {
                    layer.insert(*n, next);
                    q.push_back(*n);
                }
            }
        }
    }
    // Close up empty layers.
    let mut used: Vec<usize> = layer.values().copied().collect::<HashSet<_>>().into_iter().collect();
    used.sort_unstable();
    let remap: HashMap<usize, usize> = used.iter().enumerate().map(|(i, l)| (*l, i)).collect();
    layer.values_mut().for_each(|l| *l = remap[l]);
    layer
}

/// A column: an area (or the whole canvas) with its own devices and its
/// child columns.
struct Group {
    area: Option<AreaId>,
    name: String,
    direct: Vec<DeviceId>,
    children: Vec<usize>,
}

/// Area tree by containment, and the innermost area holding each device.
fn groups(p: &Project, devs: &[DeviceId], use_areas: bool) -> Vec<Group> {
    let mut g = vec![Group { area: None, name: String::new(), direct: Vec::new(), children: Vec::new() }];
    if !use_areas {
        g[0].direct = devs.to_vec();
        return g;
    }
    let area_ix: HashMap<AreaId, usize> = p.areas.iter().enumerate().map(|(i, a)| (a.id, i + 1)).collect();
    let smallest = |cands: &mut dyn Iterator<Item = &Area>| -> Option<AreaId> {
        cands.min_by(|a, b| (a.rect.size.x * a.rect.size.y).total_cmp(&(b.rect.size.x * b.rect.size.y))).map(|a| a.id)
    };
    for a in &p.areas {
        g.push(Group { area: Some(a.id), name: a.name.clone(), direct: Vec::new(), children: Vec::new() });
    }
    for (i, a) in p.areas.iter().enumerate() {
        // Parent: the smallest other area that holds this one. Equal
        // rectangles nest by list order so the tree has no cycles.
        let parent = smallest(&mut p.areas.iter().enumerate().filter(|(j, b)| {
            *j != i && b.rect.contains_rect(&a.rect) && (b.rect != a.rect || *j < i)
        }).map(|(_, b)| b));
        let pi = parent.map(|id| area_ix[&id]).unwrap_or(0);
        g[pi].children.push(i + 1);
    }
    for &d in devs {
        let Some(dev) = p.device(d) else { continue };
        let r = dev.rect();
        let c = r.min.offset(r.size.x / 2.0, r.size.y / 2.0);
        let home = smallest(&mut p.areas.iter().filter(|a| a.rect.contains(c)));
        g[home.map(|id| area_ix[&id]).unwrap_or(0)].direct.push(d);
    }
    g
}

/// Rows and columns for `n` cards with at most `per_row` across, balanced
/// so the last column isn't nearly empty.
fn grid_of(n: usize, per_row: usize) -> (usize, usize) {
    let rows = n.div_ceil(per_row).max(1);
    (rows, n.div_ceil(rows))
}

/// Cells: the devices of one group's own column in one layer.
type CellKey = (usize, usize);

struct Placed {
    pos: HashMap<DeviceId, Pos>,
    areas: HashMap<AreaId, Rect>,
}

/// Compute new positions. The caller turns them into an edit.
fn place(p: &Project, devs: &[DeviceId], use_areas: bool, o: &LayoutOptions) -> Placed {
    let layer = layers(p, devs);
    let nlayers = layer.values().max().map_or(0, |m| m + 1);
    let gs = groups(p, devs, use_areas);
    let size = |d: DeviceId| p.device(d).map(|d| d.size()).unwrap_or(Pos::new(132.0, 60.0));
    let name = |d: DeviceId| p.device(d).map(|d| d.name.as_str()).unwrap_or("");
    let snap = |v: f32| (v / o.grid).ceil() * o.grid;
    let per_row = o.per_row.max(1);

    // Devices of each cell, first ordered by name.
    let mut cells: HashMap<CellKey, Vec<DeviceId>> = HashMap::new();
    for (gi, g) in gs.iter().enumerate() {
        for &d in &g.direct {
            cells.entry((gi, layer[&d])).or_default().push(d);
        }
    }
    for v in cells.values_mut() {
        v.sort_by(|a, b| natural_cmp(name(*a), name(*b)));
    }

    // Each group's own column: slot width and how many slots across.
    let slot_w = |gi: usize| -> f32 {
        gs[gi].direct.iter().map(|d| size(*d).x).fold(0.0, f32::max) + o.gap_x
    };
    let across = |gi: usize| -> usize {
        (0..nlayers).map(|l| cells.get(&(gi, l)).map_or(0, |v| grid_of(v.len(), per_row).1)).max().unwrap_or(0)
    };
    let min_layer = |gi: usize| -> usize {
        fn walk(gs: &[Group], layer: &HashMap<DeviceId, usize>, gi: usize) -> usize {
            let own = gs[gi].direct.iter().map(|d| layer[d]).min().unwrap_or(usize::MAX);
            gs[gi].children.iter().map(|c| walk(gs, layer, *c)).fold(own, usize::min)
        }
        walk(&gs, &layer, gi)
    };

    // Layer bands: tall enough for the most-wrapped cell.
    let mut row_h = vec![0.0f32; nlayers];
    let mut rows = vec![0usize; nlayers];
    for (&(_, l), v) in &cells {
        row_h[l] = v.iter().map(|d| size(*d).y).fold(row_h[l], f32::max);
        rows[l] = rows[l].max(v.len().div_ceil(per_row));
    }
    let mut band_y = vec![0.0f32; nlayers];
    let mut y = 0.0;
    for l in 0..nlayers {
        band_y[l] = y;
        let h = rows[l] as f32 * (row_h[l] + o.gap_y) - o.gap_y;
        y = snap(y + h.max(0.0) + o.layer_gap);
    }

    // Lay the columns out left to right, recursively. The own column of
    // a group is one more column among its children, ordered by the
    // highest layer they reach so the top of the network sits left.
    enum Col {
        Own(usize),
        Child(usize),
    }
    let mut own_x: HashMap<usize, f32> = HashMap::new();
    let mut group_x: HashMap<usize, (f32, f32)> = HashMap::new();
    #[allow(clippy::too_many_arguments)]
    fn layout_x(
        gi: usize,
        x0: f32,
        gs: &[Group],
        o: &LayoutOptions,
        cols: &dyn Fn(usize) -> Vec<Col>,
        own_w: &dyn Fn(usize) -> f32,
        own_x: &mut HashMap<usize, f32>,
        group_x: &mut HashMap<usize, (f32, f32)>,
    ) -> f32 {
        let pad = if gs[gi].area.is_some() { o.pad } else { 0.0 };
        let mut x = x0 + pad;
        let mut first = true;
        for c in cols(gi) {
            if !first {
                x += o.gap_x;
            }
            first = false;
            match c {
                Col::Own(g) => {
                    own_x.insert(g, x);
                    x += own_w(g);
                }
                Col::Child(c) => x = layout_x(c, x, gs, o, cols, own_w, own_x, group_x),
            }
        }
        let w = (x + pad - x0).max(Area::MIN_SIZE + 2.0 * o.pad);
        group_x.insert(gi, (x0, x0 + w));
        x0 + w
    }
    let cols = |gi: usize| -> Vec<Col> {
        let mut v: Vec<(usize, String, Col)> = Vec::new();
        if !gs[gi].direct.is_empty() {
            v.push((gs[gi].direct.iter().map(|d| layer[d]).min().unwrap_or(0), String::new(), Col::Own(gi)));
        }
        for &c in &gs[gi].children {
            // Empty areas are left where they are.
            if min_layer(c) != usize::MAX {
                v.push((min_layer(c), gs[c].name.clone(), Col::Child(c)));
            }
        }
        v.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| natural_cmp(&a.1, &b.1)));
        v.into_iter().map(|(_, _, c)| c).collect()
    };
    let own_w = |gi: usize| -> f32 { snap(across(gi) as f32 * slot_w(gi) - o.gap_x) };
    layout_x(0, 0.0, &gs, o, &cols, &own_w, &mut own_x, &mut group_x);

    // Order inside cells: sweep down and up, sorting each cell by the mean
    // x of the ports its cables reach in the layer before it (above on the
    // way down, below on the way up). Ports, not card centres, so outlets
    // follow the order of the panel ports they are patched to.
    let port_dx = |e: &crate::link::Endpoint| -> f32 {
        p.device(e.device).and_then(|d| d.port_center_by_id(e.port).map(|c| c.x - d.pos.x)).unwrap_or(0.0)
    };
    let mut nbrs: HashMap<DeviceId, Vec<(DeviceId, f32)>> = HashMap::new();
    for l in &p.links {
        let (a, b) = (l.a.device, l.b.device);
        if a != b && layer.contains_key(&a) && layer.contains_key(&b) && layer[&a] != layer[&b] {
            nbrs.entry(a).or_default().push((b, port_dx(&l.b)));
            nbrs.entry(b).or_default().push((a, port_dx(&l.a)));
        }
    }
    let cell_pos = |k: CellKey, v: &[DeviceId], pos: &mut HashMap<DeviceId, Pos>| {
        let (gi, l) = k;
        let w = slot_w(gi);
        // Column-major, so x grows with the order and a desk's PC and
        // phone end up stacked.
        let (rows, _) = grid_of(v.len(), per_row);
        for (i, d) in v.iter().enumerate() {
            let (r, c) = (i % rows, i / rows);
            pos.insert(*d, Pos::new(own_x[&gi] + c as f32 * w, band_y[l] + r as f32 * (row_h[l] + o.gap_y)));
        }
    };
    let mut pos: HashMap<DeviceId, Pos> = HashMap::new();
    for (k, v) in &cells {
        cell_pos(*k, v, &mut pos);
    }
    // Mostly x, with y as a tie-break so that within a wrapped column the
    // upper neighbour comes first (cells fill column by column).
    let key = |q: Pos, dx: f32| q.x + dx + q.y * 0.01;
    let mut keys: Vec<CellKey> = cells.keys().copied().collect();
    keys.sort_by_key(|k| (k.1, k.0));
    // An odd count so the last sweep runs downwards.
    for sweep in 0..7 {
        let down = sweep % 2 == 0;
        let mut order = keys.clone();
        if !down {
            order.reverse();
        }
        for k in order {
            let v = cells.get_mut(&k).unwrap();
            let bary: HashMap<DeviceId, f32> = v
                .iter()
                .map(|d| {
                    let all = nbrs.get(d).map(Vec::as_slice).unwrap_or(&[]);
                    let before: Vec<_> =
                        all.iter().filter(|(n, _)| if down { layer[n] < k.1 } else { layer[n] > k.1 }).collect();
                    let use_: Vec<_> = if before.is_empty() { all.iter().collect() } else { before };
                    let b = if use_.is_empty() {
                        key(pos[d], 0.0)
                    } else {
                        use_.iter().map(|(n, dx)| key(pos[n], *dx)).sum::<f32>() / use_.len() as f32
                    };
                    (*d, b)
                })
                .collect();
            let rank: HashMap<DeviceId, usize> = v.iter().enumerate().map(|(i, d)| (*d, i)).collect();
            v.sort_by(|a, b| bary[a].total_cmp(&bary[b]).then(rank[a].cmp(&rank[b])));
            cell_pos(k, v, &mut pos);
        }
    }

    // Area rectangles around their contents, innermost first.
    let mut areas: HashMap<AreaId, Rect> = HashMap::new();
    fn rect_of(
        gi: usize,
        gs: &[Group],
        pos: &HashMap<DeviceId, Pos>,
        size: &dyn Fn(DeviceId) -> Pos,
        group_x: &HashMap<usize, (f32, f32)>,
        o: &LayoutOptions,
        out: &mut HashMap<AreaId, Rect>,
    ) -> Option<Rect> {
        let mut inner: Option<Rect> = None;
        let mut add = |r: Rect| inner = Some(inner.map_or(r, |i| i.union(&r)));
        for d in &gs[gi].direct {
            add(Rect { min: pos[d], size: size(*d) });
        }
        for &c in &gs[gi].children {
            if let Some(r) = rect_of(c, gs, pos, size, group_x, o, out) {
                add(r);
            }
        }
        let inner = inner?;
        let area = gs[gi].area?;
        let (x0, x1) = group_x[&gi];
        let top = inner.min.y - o.pad - o.header;
        let bottom = inner.max().y + o.pad;
        let r = Rect::new(x0, top, x1 - x0, bottom - top);
        out.insert(area, r);
        Some(r)
    }
    rect_of(0, &gs, &pos, &size, &group_x, o, &mut areas);
    Placed { pos, areas }
}

/// The edit that arranges the diagram; a no-op when nothing moves.
pub fn auto_layout(p: &Project, scope: Scope, o: &LayoutOptions) -> Edit {
    let (devs, use_areas): (Vec<DeviceId>, bool) = match scope {
        Scope::All => (p.devices.iter().map(|d| d.id).collect(), true),
        Scope::Devices(ids) => (p.devices.iter().filter(|d| ids.contains(&d.id)).map(|d| d.id).collect(), false),
    };
    if devs.is_empty() {
        return Edit::Batch(Vec::new());
    }
    let placed = place(p, &devs, use_areas, o);

    // Keep the top-left corner where it was so the view doesn't jump.
    let old = devs
        .iter()
        .filter_map(|d| p.device(*d))
        .map(|d| d.rect())
        .chain(if use_areas { p.areas.iter().filter(|a| placed.areas.contains_key(&a.id)).map(|a| a.rect).collect() } else { Vec::new() })
        .reduce(|a, b| a.union(&b))
        .map(|r| r.min)
        .unwrap_or_default();
    let new = placed
        .pos
        .values()
        .copied()
        .chain(placed.areas.values().map(|r| r.min))
        .fold(Pos::new(f32::MAX, f32::MAX), |a, b| Pos::new(a.x.min(b.x), a.y.min(b.y)));
    let shift = Pos::new(old.x - new.x, old.y - new.y).snapped(o.grid);

    let mut edits = Vec::new();
    for a in &p.areas {
        if let Some(r) = placed.areas.get(&a.id) {
            let mut after = a.clone();
            after.rect = Rect { min: r.min.offset(shift.x, shift.y), size: r.size };
            if after != *a {
                edits.push(Edit::Area { id: a.id, before: Some(a.clone()), after: Some(after) });
            }
        }
    }
    // Areas left empty go in a row under everything else.
    if use_areas {
        let bottom = placed.pos.iter().map(|(d, q)| q.y + p.device(*d).map_or(0.0, |d| d.size().y)).chain(placed.areas.values().map(|r| r.max().y)).fold(0.0, f32::max) + shift.y;
        let mut x = old.x;
        for a in p.areas.iter().filter(|a| !placed.areas.contains_key(&a.id)) {
            let mut after = a.clone();
            after.rect.min = Pos::new(x, bottom + o.layer_gap).snapped(o.grid);
            x += a.rect.size.x + o.gap_x;
            if after != *a {
                edits.push(Edit::Area { id: a.id, before: Some(a.clone()), after: Some(after) });
            }
        }
    }
    for d in &p.devices {
        if let Some(q) = placed.pos.get(&d.id) {
            let mut after = d.clone();
            after.pos = q.offset(shift.x, shift.y);
            if after != *d {
                edits.push(Edit::Device { id: d.id, before: Some(d.clone()), after: Some(after) });
            }
        }
    }
    Edit::Batch(edits)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AreaKind, Endpoint, Project};

    fn link(p: &mut Project, a: DeviceId, b: DeviceId) {
        let ea = Endpoint { device: a, port: p.first_free_port(a, None).unwrap() };
        let eb = Endpoint { device: b, port: p.first_free_port(b, None).unwrap() };
        let (_, e) = p.add_link_edit(ea, eb).unwrap();
        e.apply(p);
    }

    fn add(p: &mut Project, k: DeviceKind, name: &str, x: f32, y: f32) -> DeviceId {
        let (id, mut e) = p.add_device_edit(k, Pos::new(x, y));
        if let Edit::Device { after: Some(d), .. } = &mut e {
            d.name = name.into();
        }
        e.apply(p);
        id
    }

    #[test]
    fn natural_order() {
        let mut v = vec!["PC-10", "pc-2", "PC-1", "AP"];
        v.sort_by(|a, b| natural_cmp(a, b));
        assert_eq!(v, ["AP", "PC-1", "pc-2", "PC-10"]);
        assert_eq!(natural_cmp("x01", "x1"), Ordering::Equal);
    }

    #[test]
    fn layers_follow_cables_top_down() {
        let mut p = Project::default();
        // Scrambled on purpose.
        let pc = add(&mut p, DeviceKind::Pc, "PC", 0.0, 0.0);
        let sw = add(&mut p, DeviceKind::SwitchL2, "SW", 500.0, 900.0);
        let fw = add(&mut p, DeviceKind::Firewall, "FW", 100.0, 400.0);
        let net = add(&mut p, DeviceKind::Internet, "Internet", 900.0, 300.0);
        link(&mut p, net, fw);
        link(&mut p, fw, sw);
        link(&mut p, sw, pc);
        auto_layout(&p, Scope::All, &LayoutOptions::default()).apply(&mut p);
        let y = |d| p.device(d).unwrap().pos.y;
        assert!(y(net) < y(fw) && y(fw) < y(sw) && y(sw) < y(pc));
        // Top-left kept.
        let b = p.bounds().unwrap();
        assert_eq!((b.min.x, b.min.y), (0.0, 0.0));
        // Running it again changes nothing.
        assert!(auto_layout(&p, Scope::All, &LayoutOptions::default()).is_noop());
    }

    #[test]
    fn wraps_and_never_overlaps() {
        let mut p = Project::default();
        let sw = add(&mut p, DeviceKind::SwitchL2, "SW", 0.0, 0.0);
        let pcs: Vec<_> = (0..20).map(|i| add(&mut p, DeviceKind::Pc, &format!("PC-{i}"), 0.0, 0.0)).collect();
        for pc in &pcs {
            link(&mut p, sw, *pc);
        }
        let o = LayoutOptions::default();
        auto_layout(&p, Scope::All, &o).apply(&mut p);
        let rects: Vec<_> = p.devices.iter().map(|d| d.rect()).collect();
        for (i, a) in rects.iter().enumerate() {
            for b in &rects[i + 1..] {
                assert!(!a.intersects(b));
            }
        }
        let ys: HashSet<i32> = pcs.iter().map(|d| p.device(*d).unwrap().pos.y as i32).collect();
        assert_eq!(ys.len(), 20usize.div_ceil(o.per_row));
    }

    #[test]
    fn areas_keep_their_devices_and_stay_apart() {
        let mut p = Project::default();
        let core = add(&mut p, DeviceKind::SwitchL3, "CORE", 400.0, 0.0);
        let mut areas = Vec::new();
        for (i, name) in ["Floor 1", "Floor 2"].iter().enumerate() {
            let x = i as f32 * 300.0;
            let a = Area::new(AreaKind::Floor, *name, Rect::new(x, 200.0, 280.0, 400.0));
            areas.push(a.id);
            p.areas.push(a);
            let sw = add(&mut p, DeviceKind::SwitchL2, &format!("SW-{i}"), x + 20.0, 240.0);
            link(&mut p, core, sw);
            for j in 0..3 {
                let pc = add(&mut p, DeviceKind::Pc, &format!("PC-{i}-{j}"), x + 20.0, 400.0);
                link(&mut p, sw, pc);
            }
        }
        let before: Vec<Vec<DeviceId>> =
            areas.iter().map(|a| p.devices_in_area(*a).iter().map(|d| d.id).collect()).collect();
        auto_layout(&p, Scope::All, &LayoutOptions::default()).apply(&mut p);
        for (a, ids) in areas.iter().zip(&before) {
            let now: HashSet<DeviceId> = p.devices_in_area(*a).iter().map(|d| d.id).collect();
            assert_eq!(now, ids.iter().copied().collect::<HashSet<_>>());
            let r = p.area(*a).unwrap().rect;
            for d in ids {
                assert!(r.contains_rect(&p.device(*d).unwrap().rect()));
            }
        }
        let (r0, r1) = (p.area(areas[0]).unwrap().rect, p.area(areas[1]).unwrap().rect);
        assert!(!r0.intersects(&r1));
        assert!(!p.device(core).unwrap().rect().intersects(&r0));
    }

    #[test]
    fn selection_scope_moves_only_the_selection() {
        let mut p = Project::default();
        let a = add(&mut p, DeviceKind::SwitchL2, "A", 100.0, 100.0);
        let b = add(&mut p, DeviceKind::Pc, "B", 100.0, 100.0);
        let c = add(&mut p, DeviceKind::Pc, "C", 900.0, 900.0);
        link(&mut p, a, b);
        auto_layout(&p, Scope::Devices(&[a, b]), &LayoutOptions::default()).apply(&mut p);
        assert_eq!(p.device(c).unwrap().pos, Pos::new(900.0, 900.0));
        assert_eq!(p.device(a).unwrap().pos, Pos::new(100.0, 100.0));
        assert!(p.device(b).unwrap().pos.y > 100.0);
    }
}
