//! Design-rule checks. `validate` runs every rule and returns the issues found,
//! most severe first. Each issue points at the project items involved.

mod addressing;
mod cabling;
mod fix;
mod names;
mod power;
mod racks;
mod topology;
mod vlans;
mod wireless;

pub use fix::{Fix, fix_all};

use netvis_core::Item;
use netvis_core::catalog::Transceiver;
use netvis_core::catalog::transceiver::transceiver;
use netvis_core::{Device, Endpoint, Port, Project};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Severity {
    Error,
    Warning,
    Info,
}

impl Severity {
    pub fn label(self) -> &'static str {
        match self {
            Severity::Error => "Error",
            Severity::Warning => "Warning",
            Severity::Info => "Info",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Issue {
    pub severity: Severity,
    /// Stable rule identifier, e.g. `cable.reach`.
    pub code: &'static str,
    pub message: String,
    /// Items involved; the first is the primary subject.
    pub items: Vec<Item>,
    /// A one-click remedy, when there is an unambiguous one.
    pub fix: Option<Fix>,
}

impl Issue {
    pub fn involves(&self, item: Item) -> bool {
        self.items.contains(&item)
    }

    pub(crate) fn with_fix(&mut self, fix: Option<Fix>) {
        self.fix = fix;
    }
}

pub(crate) struct Sink(Vec<Issue>);

impl Sink {
    pub fn push(&mut self, severity: Severity, code: &'static str, message: impl Into<String>, items: Vec<Item>) -> &mut Issue {
        self.0.push(Issue { severity, code, message: message.into(), items, fix: None });
        self.0.last_mut().expect("just pushed")
    }
    pub fn error(&mut self, code: &'static str, message: impl Into<String>, items: Vec<Item>) -> &mut Issue {
        self.push(Severity::Error, code, message, items)
    }
    pub fn warn(&mut self, code: &'static str, message: impl Into<String>, items: Vec<Item>) -> &mut Issue {
        self.push(Severity::Warning, code, message, items)
    }
    pub fn info(&mut self, code: &'static str, message: impl Into<String>, items: Vec<Item>) -> &mut Issue {
        self.push(Severity::Info, code, message, items)
    }
}

pub fn validate(p: &Project) -> Vec<Issue> {
    let mut sink = Sink(Vec::new());
    cabling::check(p, &mut sink);
    power::check(p, &mut sink);
    racks::check(p, &mut sink);
    addressing::check(p, &mut sink);
    vlans::check(p, &mut sink);
    names::check(p, &mut sink);
    topology::check(p, &mut sink);
    wireless::check(p, &mut sink);
    let mut v = sink.0;
    v.sort_by_key(|i| i.severity);
    v
}

/// One end of a link, resolved.
pub(crate) struct End<'a> {
    pub dev: &'a Device,
    pub port: &'a Port,
    pub tx: Option<&'static Transceiver>,
}

impl<'a> End<'a> {
    pub fn resolve(p: &'a Project, ep: Endpoint) -> Option<Self> {
        let dev = p.device(ep.device)?;
        let port = dev.port(ep.port)?;
        let tx = port.transceiver.as_deref().and_then(transceiver);
        Some(Self { dev, port, tx })
    }

    pub fn label(&self) -> String {
        format!("{}:{}", self.dev.name, self.port.name)
    }

    /// Line rate actually used: the module's speed if one is fitted.
    pub fn speed(&self) -> u32 {
        self.tx.map_or(self.port.speed_mbps, |t| t.speed_mbps)
    }
}

#[cfg(test)]
mod tests {
    use netvis_core::{AddrKind, CableKind, DeviceKind, IfAddr, PortMedia};

    use crate::testutil::T;

    #[test]
    fn fix_all_resolves_independent_issues_in_one_edit() {
        let mut t = T::new();
        let (a, b) = (t.dev(DeviceKind::Server), t.dev(DeviceKind::Server));
        t.p.device_mut(b).unwrap().name = t.p.device(a).unwrap().name.clone();
        let (pa, pb) = (t.port(a, PortMedia::Rj45), t.port(b, PortMedia::Rj45));
        t.link((a, pa), (b, pb), CableKind::Cat5e, Some(30.0));
        for d in [a, b] {
            t.p.device_mut(d).unwrap().ports[0].speed_mbps = 10_000;
        }
        let a2 = IfAddr { net: "192.168.1.1/24".parse().unwrap(), kind: AddrKind::Static };
        t.p.device_mut(a).unwrap().ports[0].cfg.l3.addresses = vec![a2];
        assert_eq!(t.codes().len(), 2, "{:?}", t.codes());

        let (edit, n) = crate::fix_all(&t.p, None);
        assert_eq!(n, 2);
        t.h.apply(&mut t.p, edit, "fix all");
        assert!(t.codes().is_empty(), "{:?}", t.codes());
        t.h.undo(&mut t.p);
        assert_eq!(t.codes().len(), 2, "one undo step reverts every fix");
    }
}

#[cfg(test)]
pub(crate) mod testutil {
    use netvis_core::{CableKind, DeviceId, DeviceKind, Edit, Endpoint, History, LinkId, PortId, PortMedia, Pos, Project};

    pub struct T {
        pub p: Project,
        pub h: History,
    }

    impl T {
        pub fn new() -> Self {
            Self { p: Project::default(), h: History::default() }
        }

        pub fn dev(&mut self, kind: DeviceKind) -> DeviceId {
            let n = self.p.devices.len() as f32;
            let (id, e) = self.p.add_device_edit(kind, Pos::new(n * 200.0, 0.0));
            self.h.apply(&mut self.p, e, "");
            id
        }

        pub fn port(&self, d: DeviceId, media: PortMedia) -> PortId {
            self.p.first_free_port(d, Some(media)).filter(|id| self.p.device(d).unwrap().port(*id).unwrap().media == media).expect("port of that media")
        }

        pub fn link(&mut self, a: (DeviceId, PortId), b: (DeviceId, PortId), cable: CableKind, len: Option<f32>) -> LinkId {
            let (id, mut e) = self.p.add_link_edit(Endpoint { device: a.0, port: a.1 }, Endpoint { device: b.0, port: b.1 }).unwrap();
            if let Edit::Link { after: Some(l), .. } = &mut e {
                l.cable = cable;
                l.length_m = len;
            }
            self.h.apply(&mut self.p, e, "");
            id
        }

        pub fn set_tx(&mut self, d: DeviceId, port: PortId, tx: &str) {
            let dev = self.p.device_mut(d).unwrap();
            dev.ports.iter_mut().find(|p| p.id == port).unwrap().transceiver = Some(tx.to_owned());
        }

        pub fn codes(&self) -> Vec<&'static str> {
            super::validate(&self.p).into_iter().map(|i| i.code).collect()
        }

        /// Apply the fix of the first issue with `code`; panics if there is none.
        pub fn fix(&mut self, code: &str) {
            let issue = super::validate(&self.p).into_iter().find(|i| i.code == code).expect("issue present");
            let e = issue.fix.expect("issue has a fix").edit(&self.p).expect("fix applies");
            self.h.apply(&mut self.p, e, "fix");
        }
    }
}
