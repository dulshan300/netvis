//! Colours shared by the canvas and the exported diagrams, as `[r, g, b]`.

use crate::device::DeviceCategory;
use crate::link::CableFamily;

pub fn category_rgb(c: DeviceCategory) -> [u8; 3] {
    match c {
        DeviceCategory::Network => [37, 99, 235],
        DeviceCategory::EndDevice => [22, 163, 74],
        DeviceCategory::Infrastructure => [100, 116, 139],
        DeviceCategory::CloudWan => [147, 51, 234],
    }
}

pub fn cable_rgb(f: CableFamily) -> [u8; 3] {
    match f {
        CableFamily::Copper => [59, 130, 246],
        CableFamily::Multimode => [6, 182, 212],
        CableFamily::SingleMode => [234, 179, 8],
        CableFamily::DirectAttach => [168, 85, 247],
        CableFamily::Coax => [180, 120, 70],
        CableFamily::Wireless => [34, 197, 94],
        CableFamily::Console | CableFamily::Logical => [148, 163, 184],
    }
}

/// Wireless and logical links are drawn dashed.
pub fn cable_dashed(f: CableFamily) -> bool {
    matches!(f, CableFamily::Wireless | CableFamily::Logical)
}

/// Corescale brand accent.
pub const ACCENT: [u8; 3] = [37, 99, 235];
