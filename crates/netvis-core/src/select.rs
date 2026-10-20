//! A set of selected project items (canvas objects and IP-plan records).

use crate::ids::{AreaId, DeviceId, LinkId, SsidId, SubnetId, VlanId};
use crate::project::Project;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Item {
    Device(DeviceId),
    Link(LinkId),
    Area(AreaId),
    Vlan(VlanId),
    Subnet(SubnetId),
    Ssid(SsidId),
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Selection {
    pub devices: Vec<DeviceId>,
    pub links: Vec<LinkId>,
    pub areas: Vec<AreaId>,
    pub vlans: Vec<VlanId>,
    pub subnets: Vec<SubnetId>,
    pub ssids: Vec<SsidId>,
}

macro_rules! dispatch {
    ($self:ident, $item:expr, |$list:ident, $id:ident| $body:expr) => {
        match $item {
            Item::Device($id) => { let $list = &mut $self.devices; $body }
            Item::Link($id) => { let $list = &mut $self.links; $body }
            Item::Area($id) => { let $list = &mut $self.areas; $body }
            Item::Vlan($id) => { let $list = &mut $self.vlans; $body }
            Item::Subnet($id) => { let $list = &mut $self.subnets; $body }
            Item::Ssid($id) => { let $list = &mut $self.ssids; $body }
        }
    };
}

impl Selection {
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn len(&self) -> usize {
        self.devices.len() + self.links.len() + self.areas.len() + self.vlans.len() + self.subnets.len() + self.ssids.len()
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn contains(&self, item: Item) -> bool {
        match item {
            Item::Device(id) => self.devices.contains(&id),
            Item::Link(id) => self.links.contains(&id),
            Item::Area(id) => self.areas.contains(&id),
            Item::Vlan(id) => self.vlans.contains(&id),
            Item::Subnet(id) => self.subnets.contains(&id),
            Item::Ssid(id) => self.ssids.contains(&id),
        }
    }

    pub fn add(&mut self, item: Item) {
        dispatch!(self, item, |list, id| if !list.contains(&id) {
            list.push(id)
        })
    }

    pub fn remove(&mut self, item: Item) {
        dispatch!(self, item, |list, id| list.retain(|x| *x != id))
    }

    pub fn toggle(&mut self, item: Item) {
        if self.contains(item) { self.remove(item) } else { self.add(item) }
    }

    pub fn set_single(&mut self, item: Item) {
        self.clear();
        self.add(item);
    }

    /// Click semantics: additive toggles, plain click replaces.
    pub fn click(&mut self, item: Item, additive: bool) {
        if additive { self.toggle(item) } else { self.set_single(item) }
    }

    pub fn single(&self) -> Option<Item> {
        if self.len() != 1 {
            return None;
        }
        self.devices
            .first()
            .map(|x| Item::Device(*x))
            .or_else(|| self.links.first().map(|x| Item::Link(*x)))
            .or_else(|| self.areas.first().map(|x| Item::Area(*x)))
            .or_else(|| self.vlans.first().map(|x| Item::Vlan(*x)))
            .or_else(|| self.subnets.first().map(|x| Item::Subnet(*x)))
            .or_else(|| self.ssids.first().map(|x| Item::Ssid(*x)))
    }

    pub fn retain_existing(&mut self, p: &Project) {
        self.devices.retain(|id| p.device(*id).is_some());
        self.links.retain(|id| p.link(*id).is_some());
        self.areas.retain(|id| p.area(*id).is_some());
        self.vlans.retain(|id| p.vlan(*id).is_some());
        self.subnets.retain(|id| p.subnet(*id).is_some());
        self.ssids.retain(|id| p.ssid(*id).is_some());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn click_and_toggle() {
        let (a, b) = (DeviceId::new(), VlanId::new());
        let mut s = Selection::default();
        s.click(Item::Device(a), false);
        s.click(Item::Vlan(b), true);
        assert_eq!(s.len(), 2);
        s.click(Item::Device(a), true);
        assert_eq!(s.single(), Some(Item::Vlan(b)));
        s.click(Item::Device(a), false);
        assert_eq!(s.single(), Some(Item::Device(a)));
    }
}
