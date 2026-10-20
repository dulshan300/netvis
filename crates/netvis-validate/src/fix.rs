//! One-click remedies attached to issues. Each fix becomes an ordinary
//! undoable [`Edit`].

use std::net::IpAddr;

use netvis_core::{
    AllowedVlans, AreaId, CableKind, DeviceId, Edit, IfAddr, IfaceKey, LinkId, PortId, Project, StpMode, Subnet,
    SubnetId, SwitchportMode, VlanId,
};

#[derive(Debug, Clone, PartialEq)]
pub enum Fix {
    /// Swap one address on an interface for another.
    ReplaceAddress { device: DeviceId, iface: IfaceKey, old: IpAddr, new: IfAddr },
    SetGateway { device: DeviceId, iface: IfaceKey, gateway: IpAddr },
    SetSubnetGateway { subnet: SubnetId, gateway: IpAddr },
    EnableDhcp { subnet: SubnetId },
    RenameDevice { device: DeviceId, name: String },
    SetMode { device: DeviceId, port: PortId, mode: SwitchportMode },
    /// Allow `vlan` on each listed trunk port.
    AllowVlan { ports: Vec<(DeviceId, PortId)>, vlan: VlanId },
    SetChannel { device: DeviceId, port: PortId, channel: u16 },
    SetCable { link: LinkId, cable: CableKind },
    SetTransceivers(Vec<(DeviceId, PortId, String)>),
    MountInRack { rack: AreaId },
    EnableStp { devices: Vec<DeviceId> },
}

impl Fix {
    /// Button text, e.g. `Assign 10.0.20.12`.
    pub fn label(&self, p: &Project) -> String {
        let dev = |id: &DeviceId| p.device(*id).map(|d| d.name.clone()).unwrap_or_default();
        match self {
            Fix::ReplaceAddress { new, old, .. } if new.ip() == *old => format!("Use /{}", new.net.prefix_len()),
            Fix::ReplaceAddress { new, .. } => format!("Assign {}", new.net),
            Fix::SetGateway { gateway, .. } => format!("Set gateway {gateway}"),
            Fix::SetSubnetGateway { gateway, .. } => format!("Set gateway {gateway}"),
            Fix::EnableDhcp { .. } => "Add DHCP scope".into(),
            Fix::RenameDevice { name, .. } => format!("Rename to {name}"),
            Fix::SetMode { device, mode, .. } => match mode {
                SwitchportMode::Access { vlan } => format!("Set {} to access {}", dev(device), p.vlan_label(*vlan)),
                SwitchportMode::Trunk { native, .. } => format!("Set {} native {}", dev(device), p.vlan_label(*native)),
                _ => format!("Reconfigure {}", dev(device)),
            },
            Fix::AllowVlan { ports, vlan } => {
                let tag = p.vlan(*vlan).map(|v| v.tag.to_string()).unwrap_or_default();
                if ports.len() == 1 { format!("Allow VLAN {tag} on trunk") } else { format!("Allow VLAN {tag} on {} trunks", ports.len()) }
            }
            Fix::SetChannel { channel, .. } => format!("Move to channel {channel}"),
            Fix::SetCable { cable, .. } => format!("Use {}", cable.label()),
            Fix::SetTransceivers(v) => match v.first() {
                Some((_, _, id)) => {
                    let name = netvis_core::catalog::transceiver::transceiver(id).map(|t| t.name).unwrap_or(id);
                    format!("Fit {name}")
                }
                None => "Fit transceivers".into(),
            },
            Fix::MountInRack { .. } => "Auto-place in rack".into(),
            Fix::EnableStp { devices } => format!("Enable RSTP on {} switch(es)", devices.len()),
        }
    }

    pub fn edit(&self, p: &Project) -> Option<Edit> {
        let with_device = |id: DeviceId, f: &dyn Fn(&mut netvis_core::Device)| -> Option<Edit> {
            let before = p.device(id)?.clone();
            let mut after = before.clone();
            f(&mut after);
            Some(Edit::Device { id, before: Some(before), after: Some(after) })
        };
        let with_subnet = |id: SubnetId, f: &dyn Fn(&mut Subnet)| -> Option<Edit> {
            let before = p.subnet(id)?.clone();
            let mut after = before.clone();
            f(&mut after);
            Some(Edit::Subnet { id, before: Some(before), after: Some(after) })
        };
        match self {
            Fix::ReplaceAddress { device, iface, old, new } => with_device(*device, &|d| {
                if let Some(l3) = d.l3_mut(*iface)
                    && let Some(a) = l3.addresses.iter_mut().find(|a| a.ip() == *old)
                {
                    *a = *new;
                    if l3.gateway.is_some_and(|g| !new.net.contains(&g)) {
                        l3.gateway = p.subnet_containing(new.ip()).and_then(|s| s.gateway).filter(|g| *g != new.ip());
                    }
                }
            }),
            Fix::SetGateway { device, iface, gateway } => with_device(*device, &|d| {
                if let Some(l3) = d.l3_mut(*iface) {
                    l3.gateway = Some(*gateway);
                }
            }),
            Fix::SetSubnetGateway { subnet, gateway } => with_subnet(*subnet, &|s| s.gateway = Some(*gateway)),
            Fix::EnableDhcp { subnet } => with_subnet(*subnet, &|s| {
                s.dhcp = Subnet::new("", s.net).with_defaults(false, true).dhcp;
            }),
            Fix::RenameDevice { device, name } => with_device(*device, &|d| d.name = name.clone()),
            Fix::SetMode { device, port, mode } => with_device(*device, &|d| {
                if let Some(x) = d.ports.iter_mut().find(|x| x.id == *port) {
                    x.cfg.mode = mode.clone();
                }
            }),
            Fix::AllowVlan { ports, vlan } => {
                let mut edits = Vec::new();
                let mut devices: Vec<DeviceId> = ports.iter().map(|(d, _)| *d).collect();
                devices.dedup();
                for d in devices {
                    let mine: Vec<PortId> = ports.iter().filter(|(x, _)| *x == d).map(|(_, p)| *p).collect();
                    edits.push(with_device(d, &|dev| {
                        for x in dev.ports.iter_mut().filter(|x| mine.contains(&x.id)) {
                            if let SwitchportMode::Trunk { allowed: AllowedVlans::List(l), .. } = &mut x.cfg.mode
                                && !l.contains(vlan)
                            {
                                l.push(*vlan);
                            }
                        }
                    })?);
                }
                Some(Edit::Batch(edits))
            }
            Fix::SetChannel { device, port, channel } => with_device(*device, &|d| {
                if let Some(r) = d.ports.iter_mut().find(|x| x.id == *port).and_then(|x| x.cfg.radio.as_mut()) {
                    r.channel = *channel;
                }
            }),
            Fix::SetCable { link, cable } => {
                let before = p.link(*link)?.clone();
                let after = netvis_core::Link { cable: *cable, ..before.clone() };
                Some(Edit::Link { id: *link, before: Some(before), after: Some(after) })
            }
            Fix::SetTransceivers(list) => {
                let mut edits = Vec::new();
                for (d, port, tx) in list {
                    edits.push(with_device(*d, &|dev| {
                        if let Some(x) = dev.ports.iter_mut().find(|x| x.id == *port) {
                            x.transceiver = Some(tx.clone());
                        }
                    })?);
                }
                Some(Edit::Batch(edits))
            }
            Fix::MountInRack { rack } => Some(p.auto_mount_edit(*rack).0),
            Fix::EnableStp { devices } => {
                Some(Edit::Batch(devices.iter().filter_map(|d| with_device(*d, &|dev| dev.stp = StpMode::Rstp)).collect()))
            }
        }
    }
}

/// Apply every available fix (optionally only for the given rule codes),
/// re-validating after each so later fixes see earlier ones. Returns one
/// combined edit and the number of fixes applied.
pub fn fix_all(p: &Project, codes: Option<&[&str]>) -> (Edit, usize) {
    let mut work = p.clone();
    let mut edits = Vec::new();
    let mut tried: Vec<(String, Fix)> = Vec::new();
    for _ in 0..500 {
        let issues = crate::validate(&work);
        let next = issues.into_iter().find(|i| {
            i.fix.is_some()
                && codes.is_none_or(|c| c.contains(&i.code))
                && !tried.iter().any(|(m, f)| *m == i.message && Some(f) == i.fix.as_ref())
        });
        let Some(issue) = next else { break };
        let fix = issue.fix.expect("filtered");
        tried.push((issue.message, fix.clone()));
        if let Some(e) = fix.edit(&work) {
            e.apply(&mut work);
            edits.push(e);
        }
    }
    let n = edits.len();
    (Edit::Batch(edits), n)
}
