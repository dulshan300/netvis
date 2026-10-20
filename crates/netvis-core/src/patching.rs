//! Cable runs through passive patching.
//!
//! Each port of a patch panel or wall outlet joins two cables: the patch cord
//! on the front and the permanent run on the rear. A [`Circuit`] follows a run
//! across those ports from one active port (switch, host…) to the other.

use std::collections::HashSet;

use crate::device::{DeviceCategory, DeviceKind};
use crate::ids::{LinkId, PortId};
use crate::link::{Endpoint, Link};
use crate::project::Project;

/// An end-to-end cable run.
#[derive(Debug, Clone, PartialEq)]
pub struct Circuit {
    /// Network side (switch, router…) when one end is network gear.
    pub a: Endpoint,
    /// The other end. A passive port here means the run is unterminated.
    pub b: Endpoint,
    /// Cables in order from `a` to `b`.
    pub links: Vec<LinkId>,
    /// Passive ports crossed, in order from `a`.
    pub via: Vec<Endpoint>,
}

impl Circuit {
    pub fn is_direct(&self) -> bool {
        self.via.is_empty()
    }

    pub fn reversed(mut self) -> Self {
        std::mem::swap(&mut self.a, &mut self.b);
        self.links.reverse();
        self.via.reverse();
        self
    }

    /// Both ends are active ports.
    pub fn is_terminated(&self, p: &Project) -> bool {
        !p.is_passthrough(self.a) && !p.is_passthrough(self.b)
    }

    /// Total length when every cable has one.
    pub fn length_m(&self, p: &Project) -> Option<f32> {
        self.links.iter().map(|id| p.link(*id).and_then(|l| l.length_m)).sum()
    }

    /// Ports along the run, `a` first: a, via…, b.
    pub fn ports(&self) -> impl Iterator<Item = Endpoint> + '_ {
        std::iter::once(self.a).chain(self.via.iter().copied()).chain(std::iter::once(self.b))
    }
}

fn other_end(l: &Link, from: Endpoint) -> Endpoint {
    if l.a == from { l.b } else { l.a }
}

/// Sort key that puts network gear on the `a` side.
fn end_rank(p: &Project, e: Endpoint) -> (u8, String) {
    match p.device(e.device) {
        None => (9, String::new()),
        Some(d) if d.kind.is_passthrough() => (4, d.name.clone()),
        Some(d) => {
            let r = match d.kind {
                DeviceKind::AccessPoint => 1,
                k if k.category() == DeviceCategory::Network => 0,
                k if k.category() == DeviceCategory::CloudWan => 2,
                _ => 3,
            };
            (r, d.name.clone())
        }
    }
}

impl Project {
    /// Every cable on a port: at most one on an active port, front and rear
    /// on a passive one.
    pub fn port_links(&self, port: PortId) -> impl Iterator<Item = &Link> {
        self.links.iter().filter(move |l| l.touches_port(port))
    }

    pub fn is_passthrough(&self, ep: Endpoint) -> bool {
        self.device(ep.device).is_some_and(|d| d.kind.is_passthrough())
    }

    /// How many cables the port can take.
    pub fn port_capacity(&self, ep: Endpoint) -> usize {
        if self.is_passthrough(ep) { 2 } else { 1 }
    }

    /// Follow `first` away from `from`, crossing passive ports.
    fn walk(&self, from: Endpoint, first: &Link, seen: &mut HashSet<LinkId>) -> (Vec<LinkId>, Vec<Endpoint>, Endpoint) {
        let (mut links, mut via) = (Vec::new(), Vec::new());
        let (mut at, mut cable) = (from, first);
        loop {
            seen.insert(cable.id);
            links.push(cable.id);
            let next = other_end(cable, at);
            if !self.is_passthrough(next) {
                return (links, via, next);
            }
            match self.port_links(next.port).find(|l| !seen.contains(&l.id)) {
                Some(l) => {
                    via.push(next);
                    at = next;
                    cable = l;
                }
                None => return (links, via, next),
            }
        }
    }

    /// The run a cable belongs to, network side first.
    pub fn circuit_of(&self, id: LinkId) -> Option<Circuit> {
        let l = self.link(id)?;
        let mut seen = HashSet::new();
        let (fwd_links, fwd_via, end_b) = self.walk(l.a, l, &mut seen);
        let back = self.is_passthrough(l.a).then(|| self.port_links(l.a.port).find(|x| !seen.contains(&x.id))).flatten();
        let mut c = match back {
            Some(back) => {
                let (bl, bv, end_a) = self.walk(l.a, back, &mut seen);
                let mut links: Vec<LinkId> = bl.into_iter().rev().collect();
                links.extend(fwd_links);
                let mut via: Vec<Endpoint> = bv.into_iter().rev().collect();
                via.push(l.a);
                via.extend(fwd_via);
                Circuit { a: end_a, b: end_b, links, via }
            }
            None => Circuit { a: l.a, b: end_b, links: fwd_links, via: fwd_via },
        };
        if end_rank(self, c.b) < end_rank(self, c.a) {
            c = c.reversed();
        }
        Some(c)
    }

    /// Every run in the project, sorted by the `a` end's device and port.
    pub fn circuits(&self) -> Vec<Circuit> {
        let mut seen: HashSet<LinkId> = HashSet::new();
        let mut out = Vec::new();
        for l in &self.links {
            if seen.contains(&l.id) {
                continue;
            }
            if let Some(c) = self.circuit_of(l.id) {
                seen.extend(c.links.iter().copied());
                out.push(c);
            }
        }
        let key = |e: Endpoint| {
            let d = self.device(e.device);
            let name = d.map(|d| d.name.clone()).unwrap_or_default();
            let idx = d.and_then(|d| d.ports.iter().position(|x| x.id == e.port)).unwrap_or(usize::MAX);
            (name, idx)
        };
        out.sort_by_cached_key(|c| (end_rank(self, c.a).0, key(c.a), key(c.b)));
        out
    }

    /// The run starting at `ep`, oriented so that `a == ep`.
    pub fn circuit_from(&self, ep: Endpoint) -> Option<Circuit> {
        let l = self.port_links(ep.port).next()?;
        let c = self.circuit_of(l.id)?;
        Some(if c.a == ep { c } else { c.reversed() })
    }

    /// The active port at the far end of the run from `ep`, through any
    /// patch panels. `None` if the port is not cabled or the run stops at an
    /// unpatched panel.
    pub fn peer(&self, ep: Endpoint) -> Option<Endpoint> {
        let c = self.circuit_from(ep)?;
        (!self.is_passthrough(c.b) && c.b != ep).then_some(c.b)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geom::Pos;

    fn add(p: &mut Project, kind: DeviceKind) -> crate::ids::DeviceId {
        let (id, e) = p.add_device_edit(kind, Pos::new(0.0, 0.0));
        e.apply(p);
        id
    }

    fn port(p: &Project, d: crate::ids::DeviceId, i: usize) -> Endpoint {
        Endpoint { device: d, port: p.device(d).unwrap().ports[i].id }
    }

    fn cable(p: &mut Project, a: (crate::ids::DeviceId, usize), b: (crate::ids::DeviceId, usize)) -> LinkId {
        let (a, b) = (port(p, a.0, a.1), port(p, b.0, b.1));
        let (id, e) = p.add_link_edit(a, b).expect("valid link");
        e.apply(p);
        id
    }

    #[test]
    fn traces_through_panel_and_outlet() {
        let mut p = Project::default();
        let (sw, pp, wo, pc) =
            (add(&mut p, DeviceKind::SwitchL2), add(&mut p, DeviceKind::PatchPanel), add(&mut p, DeviceKind::WallOutlet), add(&mut p, DeviceKind::Pc));
        // Built out of order: the horizontal run first, then the patch cords.
        let rear = cable(&mut p, (pp, 4), (wo, 0));
        let user = cable(&mut p, (pc, 0), (wo, 0));
        assert_eq!(p.peer(port(&p, pc, 0)), None, "not patched to a switch yet");
        let front = cable(&mut p, (sw, 4), (pp, 4));

        let c = p.circuit_of(rear).unwrap();
        assert_eq!(c.a, port(&p, sw, 4), "switch side first");
        assert_eq!(c.b, port(&p, pc, 0));
        assert_eq!(c.links, vec![front, rear, user]);
        assert_eq!(c.via, vec![port(&p, pp, 4), port(&p, wo, 0)]);
        assert!(c.is_terminated(&p));
        assert_eq!(p.peer(port(&p, pc, 0)), Some(port(&p, sw, 4)));
        assert_eq!(p.peer(port(&p, sw, 4)), Some(port(&p, pc, 0)));
        assert_eq!(p.circuits().len(), 1);

        // A third cable on a panel port is refused; a second one on a switch port too.
        assert!(p.check_link(port(&p, pp, 4), port(&p, pp, 5)).is_err());
        assert!(p.check_link(port(&p, sw, 4), port(&p, pp, 6)).is_err());
        assert_eq!(p.first_free_port(pp, None), Some(port(&p, pp, 0).port));
    }

    #[test]
    fn direct_links_and_unterminated_runs() {
        let mut p = Project::default();
        let (a, b, pp) = (add(&mut p, DeviceKind::SwitchL2), add(&mut p, DeviceKind::Server), add(&mut p, DeviceKind::PatchPanel));
        cable(&mut p, (b, 0), (a, 0));
        cable(&mut p, (a, 1), (pp, 0));
        let cs = p.circuits();
        assert_eq!(cs.len(), 2);
        assert!(cs[0].is_direct() && cs[0].a == port(&p, a, 0));
        assert!(!cs[1].is_terminated(&p));
        assert_eq!(cs[1].b, port(&p, pp, 0), "panel end last");
        assert_eq!(p.peer(port(&p, a, 1)), None);
    }
}
