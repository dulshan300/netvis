//! Physical-layer reference data: port media, cable reach, transceivers,
//! PoE power levels and vendor-neutral device models.

pub mod cable;
pub mod media;
pub mod models;
pub mod poe;
pub mod transceiver;

pub use cable::{CableFamily, CableKind, Reach};
pub use media::{PortMedia, format_speed};
pub use models::{DeviceModel, PoeSpec, PortGroup};
pub use poe::{POE_MAX_LENGTH_M, PoeStandard};
pub use transceiver::{Connector, Transceiver, TxMedium};

/// Standard rack unit height, in millimetres.
pub const RACK_UNIT_MM: f32 = 44.45;
