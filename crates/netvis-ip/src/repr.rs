//! Alternative notations for a single address: expanded IPv6, integer, hex,
//! binary, reverse-DNS names and EUI-64 addresses.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use ipnet::Ipv6Net;

use crate::mac::Mac;

/// All eight IPv6 groups with leading zeros, e.g. `2001:0db8:0000:…`.
pub fn ipv6_expanded(a: Ipv6Addr) -> String {
    a.segments().iter().map(|s| format!("{s:04x}")).collect::<Vec<_>>().join(":")
}

/// RFC 5952 canonical (compressed, lower-case) form.
pub fn ipv6_compressed(a: Ipv6Addr) -> String {
    a.to_string()
}

/// `in-addr.arpa` / `ip6.arpa` name for PTR records.
pub fn reverse_pointer(ip: IpAddr) -> String {
    match ip {
        IpAddr::V4(a) => {
            let o = a.octets();
            format!("{}.{}.{}.{}.in-addr.arpa", o[3], o[2], o[1], o[0])
        }
        IpAddr::V6(a) => {
            let nibbles: Vec<String> = a
                .octets()
                .iter()
                .rev()
                .flat_map(|b| [b & 0x0f, b >> 4])
                .map(|n| format!("{n:x}"))
                .collect();
            format!("{}.ip6.arpa", nibbles.join("."))
        }
    }
}

/// Dotted binary for IPv4 (`11000000.10101000.…`), colon-grouped for IPv6.
pub fn binary(ip: IpAddr) -> String {
    match ip {
        IpAddr::V4(a) => a.octets().iter().map(|o| format!("{o:08b}")).collect::<Vec<_>>().join("."),
        IpAddr::V6(a) => a.segments().iter().map(|s| format!("{s:016b}")).collect::<Vec<_>>().join(":"),
    }
}

pub fn hex(ip: IpAddr) -> String {
    match ip {
        IpAddr::V4(a) => format!("0x{:08X}", u32::from(a)),
        IpAddr::V6(a) => format!("0x{:032X}", u128::from(a)),
    }
}

/// `::ffff:a.b.c.d`
pub fn ipv4_mapped(a: Ipv4Addr) -> Ipv6Addr {
    a.to_ipv6_mapped()
}

/// SLAAC address from a /64 prefix and a MAC (modified EUI-64).
pub fn eui64_address(prefix: Ipv6Net, mac: Mac) -> Option<Ipv6Addr> {
    if prefix.prefix_len() > 64 {
        return None;
    }
    let net = u128::from(prefix.trunc().network()) & !((1u128 << 64) - 1);
    let iid = u64::from_be_bytes(mac.eui64());
    Some(Ipv6Addr::from(net | iid as u128))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ipv6_forms() {
        let a: Ipv6Addr = "2001:db8::1".parse().unwrap();
        assert_eq!(ipv6_expanded(a), "2001:0db8:0000:0000:0000:0000:0000:0001");
        assert_eq!(ipv6_compressed("2001:0DB8:0:0:1:0:0:1".parse().unwrap()), "2001:db8::1:0:0:1");
        assert_eq!(
            reverse_pointer(IpAddr::V6(a)),
            "1.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.8.b.d.0.1.0.0.2.ip6.arpa"
        );
    }

    #[test]
    fn ipv4_forms() {
        let a = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 10));
        assert_eq!(reverse_pointer(a), "10.1.168.192.in-addr.arpa");
        assert_eq!(binary(a), "11000000.10101000.00000001.00001010");
        assert_eq!(hex(a), "0xC0A8010A");
        assert_eq!(ipv4_mapped(Ipv4Addr::new(10, 0, 0, 1)).to_string(), "::ffff:10.0.0.1");
    }

    #[test]
    fn slaac() {
        let mac = Mac::parse("00:1b:63:84:45:e6").unwrap();
        let p: Ipv6Net = "2001:db8:1:2::/64".parse().unwrap();
        assert_eq!(eui64_address(p, mac).unwrap().to_string(), "2001:db8:1:2:21b:63ff:fe84:45e6");
        assert_eq!(eui64_address("2001:db8::/80".parse().unwrap(), mac), None);
    }
}
