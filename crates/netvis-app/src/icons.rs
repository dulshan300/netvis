use egui_phosphor::regular as ic;
use netvis_core::{AreaKind, DeviceCategory, DeviceKind};

pub use ic::*;

pub fn device(kind: DeviceKind) -> &'static str {
    use DeviceKind::*;
    match kind {
        Router => ic::ARROWS_OUT_CARDINAL,
        SwitchL2 => ic::NETWORK,
        SwitchL3 => ic::TREE_STRUCTURE,
        Firewall => ic::SHIELD_CHECK,
        AccessPoint => ic::WIFI_HIGH,
        WirelessController => ic::BROADCAST,
        LoadBalancer => ic::SCALES,
        Modem => ic::SWAP,
        VpnGateway => ic::LOCK_KEY,
        Pc => ic::DESKTOP,
        Laptop => ic::LAPTOP,
        Server => ic::HARD_DRIVES,
        VmHost => ic::CUBE,
        Storage => ic::DATABASE,
        Printer => ic::PRINTER,
        IpPhone => ic::PHONE,
        IpCamera => ic::SECURITY_CAMERA,
        IoT => ic::CPU,
        SmartTv => ic::TELEVISION,
        Pos => ic::CASH_REGISTER,
        PatchPanel => ic::DOTS_NINE,
        Ups => ic::BATTERY_CHARGING,
        Pdu => ic::PLUGS,
        PoeInjector => ic::LIGHTNING,
        MediaConverter => ic::ARROWS_HORIZONTAL,
        Internet => ic::GLOBE,
        Isp => ic::CLOUD,
        Mpls => ic::CIRCUITRY,
        SdWan => ic::ARROWS_SPLIT,
        VpnTunnel => ic::LOCK_KEY,
        CloudVpc => ic::CLOUD_ARROW_UP,
    }
}

pub fn category(c: DeviceCategory) -> &'static str {
    match c {
        DeviceCategory::Network => ic::NETWORK,
        DeviceCategory::EndDevice => ic::DESKTOP,
        DeviceCategory::Infrastructure => ic::STACK,
        DeviceCategory::CloudWan => ic::CLOUD,
    }
}

pub fn area(kind: AreaKind) -> &'static str {
    match kind {
        AreaKind::Site => ic::MAP_PIN,
        AreaKind::Building => ic::BUILDING_OFFICE,
        AreaKind::Floor => ic::STACK_SIMPLE,
        AreaKind::Rack => ic::GRID_NINE,
        AreaKind::VlanZone => ic::RECTANGLE_DASHED,
        AreaKind::SecurityZone => ic::SHIELD,
    }
}
