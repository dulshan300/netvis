//! Transfer time, required bandwidth and protocol efficiency.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SizeUnit {
    B,
    KB,
    MB,
    GB,
    TB,
    KiB,
    MiB,
    GiB,
    TiB,
}

impl SizeUnit {
    pub const ALL: [SizeUnit; 9] = [
        SizeUnit::B,
        SizeUnit::KB,
        SizeUnit::MB,
        SizeUnit::GB,
        SizeUnit::TB,
        SizeUnit::KiB,
        SizeUnit::MiB,
        SizeUnit::GiB,
        SizeUnit::TiB,
    ];

    pub fn bytes(self) -> f64 {
        match self {
            SizeUnit::B => 1.0,
            SizeUnit::KB => 1e3,
            SizeUnit::MB => 1e6,
            SizeUnit::GB => 1e9,
            SizeUnit::TB => 1e12,
            SizeUnit::KiB => 1024.0,
            SizeUnit::MiB => 1024f64.powi(2),
            SizeUnit::GiB => 1024f64.powi(3),
            SizeUnit::TiB => 1024f64.powi(4),
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            SizeUnit::B => "B",
            SizeUnit::KB => "KB",
            SizeUnit::MB => "MB",
            SizeUnit::GB => "GB",
            SizeUnit::TB => "TB",
            SizeUnit::KiB => "KiB",
            SizeUnit::MiB => "MiB",
            SizeUnit::GiB => "GiB",
            SizeUnit::TiB => "TiB",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RateUnit {
    Kbps,
    Mbps,
    Gbps,
}

impl RateUnit {
    pub const ALL: [RateUnit; 3] = [RateUnit::Kbps, RateUnit::Mbps, RateUnit::Gbps];

    pub fn bps(self) -> f64 {
        match self {
            RateUnit::Kbps => 1e3,
            RateUnit::Mbps => 1e6,
            RateUnit::Gbps => 1e9,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            RateUnit::Kbps => "kbit/s",
            RateUnit::Mbps => "Mbit/s",
            RateUnit::Gbps => "Gbit/s",
        }
    }
}

/// Ethernet framing per packet that carries no payload: preamble + SFD (8),
/// header (14), FCS (4) and the inter-frame gap (12).
pub const ETHERNET_OVERHEAD: f64 = 38.0;

/// Fraction of line rate left for TCP payload at the given IP MTU, with
/// 20-byte IPv4 and 20-byte TCP headers (no options).
pub fn tcp_efficiency(mtu: u32) -> f64 {
    let mtu = mtu.max(68) as f64;
    (mtu - 40.0) / (mtu + ETHERNET_OVERHEAD)
}

/// Seconds to move `bytes` over `bps` at the given efficiency (0–1].
pub fn transfer_secs(bytes: f64, bps: f64, efficiency: f64) -> Option<f64> {
    let rate = bps * efficiency;
    (rate > 0.0 && bytes >= 0.0).then(|| bytes * 8.0 / rate)
}

/// Line rate in bit/s needed to move `bytes` within `secs`.
pub fn required_bps(bytes: f64, secs: f64, efficiency: f64) -> Option<f64> {
    (secs > 0.0 && efficiency > 0.0).then(|| bytes * 8.0 / secs / efficiency)
}

/// `3 d 4 h`, `2 h 13 min`, `4 min 05 s`, `12.4 s`, `35 ms`.
pub fn format_duration(secs: f64) -> String {
    if !secs.is_finite() {
        return "—".into();
    }
    if secs < 1.0 {
        return format!("{:.0} ms", secs * 1000.0);
    }
    if secs < 60.0 {
        return format!("{secs:.1} s");
    }
    let s = secs.round() as u64;
    let (d, h, m, s) = (s / 86_400, s / 3600 % 24, s / 60 % 60, s % 60);
    if d > 0 {
        format!("{d} d {h} h")
    } else if h > 0 {
        format!("{h} h {m} min")
    } else {
        format!("{m} min {s:02} s")
    }
}

/// `950 kbit/s`, `94.9 Mbit/s`, `1.25 Gbit/s`.
pub fn format_bps(bps: f64) -> String {
    if bps >= 1e9 {
        format!("{:.2} Gbit/s", bps / 1e9)
    } else if bps >= 1e6 {
        format!("{:.1} Mbit/s", bps / 1e6)
    } else {
        format!("{:.0} kbit/s", bps / 1e3)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn efficiency_and_time() {
        assert!((tcp_efficiency(1500) - 0.9493).abs() < 1e-3);
        assert!((tcp_efficiency(9000) - 0.9914).abs() < 1e-3);
        // 1 GB over 1 Gbit/s at 100 %: 8 s.
        assert_eq!(transfer_secs(1e9, 1e9, 1.0), Some(8.0));
        assert_eq!(transfer_secs(1e9, 0.0, 1.0), None);
        let need = required_bps(100.0 * SizeUnit::GB.bytes(), 3600.0, 1.0).unwrap();
        assert!((need - 222.2e6).abs() < 1e5);
    }

    #[test]
    fn formatting() {
        assert_eq!(format_duration(0.035), "35 ms");
        assert_eq!(format_duration(12.44), "12.4 s");
        assert_eq!(format_duration(245.0), "4 min 05 s");
        assert_eq!(format_duration(7980.0), "2 h 13 min");
        assert_eq!(format_duration(3.0 * 86_400.0 + 4.0 * 3600.0), "3 d 4 h");
        assert_eq!(format_bps(949.3e6), "949.3 Mbit/s");
    }
}
