//! NetVis core data model.
//!
//! Everything here is UI-agnostic so it can be unit-tested and shared by the
//! GUI, the renderer and (later) the validation and report crates.

pub mod area;
pub mod device;
pub mod edit;
pub mod geom;
pub mod ids;
pub mod io;
pub mod link;
pub mod port;
pub mod project;

pub use area::{Area, AreaKind};
pub use device::{Device, DeviceCategory, DeviceKind};
pub use edit::{Edit, History};
pub use geom::{Pos, Rect};
pub use ids::{AreaId, DeviceId, LinkId, PortId};
pub use link::{CableKind, Endpoint, Link};
pub use port::{PoeRole, PoeStandard, Port, PortMedia};
pub use project::{Project, ProjectMeta};
