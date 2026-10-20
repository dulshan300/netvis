//! NetVis core data model.
//!
//! Everything here is UI-agnostic so it can be unit-tested and shared by the
//! GUI, the renderer and (later) the validation and report crates.

pub mod area;
pub mod device;
pub mod edit;
pub mod geom;
pub mod iface;
pub mod ids;
pub mod io;
pub mod ipam;
pub mod link;
pub mod port;
pub mod project;
pub mod routing;
pub mod select;
pub mod subnet;
pub mod vlan;

pub use netvis_ip as ip;

pub use area::{Area, AreaKind};
pub use device::{Device, DeviceCategory, DeviceKind};
pub use edit::{Edit, History};
pub use geom::{Pos, Rect};
pub use iface::{
    AddrKind, AllowedVlans, Duplex, IfAddr, IfaceKey, InterfaceConfig, L3Config, LogicalInterface, LogicalKind,
    RadioConfig, SwitchportMode, WifiStandard,
};
pub use ids::{AreaId, DeviceId, LinkId, LogicalId, PortId, SsidId, SubnetId, VlanId};
pub use link::{CableKind, Endpoint, Link};
pub use port::{PoeRole, PoeStandard, Port, PortMedia};
pub use project::{Project, ProjectMeta};
pub use routing::{NatKind, NatRule, Routing, Services, StaticRoute};
pub use select::{Item, Selection};
pub use subnet::{AddrRange, DhcpScope, Subnet};
pub use vlan::{Ssid, Vlan, WifiBand, WifiSecurity};
