//! IPv4/IPv6 subnet math for NetVis.
//!
//! All arithmetic is done on `u128` so the same code serves both families.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

pub use ipnet::{IpNet, Ipv4Net, Ipv6Net};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum IpError {
    #[error("“{0}” is not a valid address or prefix")]
    Invalid(String),
    #[error("{0} is not a contiguous subnet mask")]
    BadMask(Ipv4Addr),
    #[error("the requested subnets do not fit in {0}")]
    DoesNotFit(IpNet),
    #[error("cannot mix IPv4 and IPv6")]
    MixedFamilies,
    #[error("too many subnets ({0}); narrow the range")]
    TooMany(u128),
}

pub fn to_u128(ip: IpAddr) -> u128 {
    match ip {
        IpAddr::V4(a) => u32::from(a) as u128,
        IpAddr::V6(a) => u128::from(a),
    }
}

pub fn from_u128(v: u128, v6: bool) -> IpAddr {
    if v6 { IpAddr::V6(Ipv6Addr::from(v)) } else { IpAddr::V4(Ipv4Addr::from(v as u32)) }
}

fn width(net: &IpNet) -> u32 {
    match net {
        IpNet::V4(_) => 32,
        IpNet::V6(_) => 128,
    }
}

fn is_v6(net: &IpNet) -> bool {
    matches!(net, IpNet::V6(_))
}

/// Total number of addresses in the prefix (saturating for `::/0`).
pub fn total_addresses(net: &IpNet) -> u128 {
    let host_bits = width(net) - net.prefix_len() as u32;
    1u128.checked_shl(host_bits).unwrap_or(u128::MAX)
}

/// First and last assignable host address.
///
/// IPv4 excludes network and broadcast except for /31 (RFC 3021) and /32.
/// IPv6 excludes the subnet-router anycast address except for /127 and /128.
pub fn usable_range(net: &IpNet) -> (IpAddr, IpAddr) {
    let first = to_u128(net.network());
    let last = to_u128(net.broadcast());
    let host_bits = width(net) - net.prefix_len() as u32;
    let v6 = is_v6(net);
    match host_bits {
        0 | 1 => (from_u128(first, v6), from_u128(last, v6)),
        _ if v6 => (from_u128(first + 1, v6), from_u128(last, v6)),
        _ => (from_u128(first + 1, v6), from_u128(last - 1, v6)),
    }
}

pub fn usable_count(net: &IpNet) -> u128 {
    let (a, b) = usable_range(net);
    (to_u128(b) - to_u128(a)).saturating_add(1)
}

/// The `n`-th usable host (0-based).
pub fn nth_usable(net: &IpNet, n: u128) -> Option<IpAddr> {
    let (a, b) = usable_range(net);
    let v = to_u128(a).checked_add(n)?;
    (v <= to_u128(b)).then(|| from_u128(v, is_v6(net)))
}

pub fn overlaps(a: &IpNet, b: &IpNet) -> bool {
    a.contains(&b.network()) || b.contains(&a.network())
}

/// Is `ip` one of `net`'s assignable host addresses?
pub fn is_usable_host(net: &IpNet, ip: IpAddr) -> bool {
    let (a, b) = usable_range(net);
    let v = to_u128(ip);
    net.contains(&ip) && v >= to_u128(a) && v <= to_u128(b)
}

/// Count of addresses in the inclusive range `[a, b]` (0 if reversed or mixed).
pub fn range_len(a: IpAddr, b: IpAddr) -> u128 {
    if a.is_ipv4() != b.is_ipv4() {
        return 0;
    }
    let (a, b) = (to_u128(a), to_u128(b));
    if b < a { 0 } else { (b - a).saturating_add(1) }
}

pub fn in_range(ip: IpAddr, a: IpAddr, b: IpAddr) -> bool {
    ip.is_ipv4() == a.is_ipv4() && to_u128(ip) >= to_u128(a) && to_u128(ip) <= to_u128(b)
}

pub fn mask_to_prefix(mask: Ipv4Addr) -> Result<u8, IpError> {
    let m = u32::from(mask);
    let ones = m.leading_ones();
    if m.checked_shl(ones).unwrap_or(0) != 0 {
        return Err(IpError::BadMask(mask));
    }
    Ok(ones as u8)
}

pub fn prefix_to_mask(prefix: u8) -> Ipv4Addr {
    let p = prefix.min(32) as u32;
    Ipv4Addr::from(u32::MAX.checked_shl(32 - p).unwrap_or(0))
}

/// Parse `10.0.0.5/24`, `10.0.0.5 255.255.255.0`, `2001:db8::1/64` or a bare
/// address (treated as a host prefix). Host bits are preserved.
pub fn parse_net(s: &str) -> Result<IpNet, IpError> {
    let s = s.trim();
    let bad = || IpError::Invalid(s.to_owned());
    if let Ok(n) = s.parse::<IpNet>() {
        return Ok(n);
    }
    let mut parts = s.split_whitespace();
    let (Some(addr), mask) = (parts.next(), parts.next()) else { return Err(bad()) };
    if parts.next().is_some() {
        return Err(bad());
    }
    let ip: IpAddr = addr.parse().map_err(|_| bad())?;
    let prefix = match (ip, mask) {
        (IpAddr::V4(_), Some(m)) => mask_to_prefix(m.parse().map_err(|_| bad())?)?,
        (IpAddr::V6(_), Some(_)) => return Err(bad()),
        (IpAddr::V4(_), None) => 32,
        (IpAddr::V6(_), None) => 128,
    };
    IpNet::new(ip, prefix).map_err(|_| bad())
}

/// Human-readable facts about a subnet, for calculators and reports.
#[derive(Debug, Clone, PartialEq)]
pub struct SubnetInfo {
    pub network: IpNet,
    pub netmask: IpAddr,
    pub wildcard: Option<Ipv4Addr>,
    pub broadcast: Option<IpAddr>,
    pub first_usable: IpAddr,
    pub last_usable: IpAddr,
    pub usable: u128,
    pub total: u128,
    pub scope: &'static str,
}

impl SubnetInfo {
    pub fn of(net: &IpNet) -> Self {
        let n = net.trunc();
        let (first, last) = usable_range(&n);
        let (wildcard, broadcast) = match n {
            IpNet::V4(v4) => (Some(v4.hostmask()), (v4.prefix_len() < 31).then(|| IpAddr::V4(v4.broadcast()))),
            IpNet::V6(_) => (None, None),
        };
        Self {
            network: n,
            netmask: n.netmask(),
            wildcard,
            broadcast,
            first_usable: first,
            last_usable: last,
            usable: usable_count(&n),
            total: total_addresses(&n),
            scope: scope(n.network()),
        }
    }
}

/// Address scope label (private, public, link-local…).
pub fn scope(ip: IpAddr) -> &'static str {
    match ip {
        IpAddr::V4(a) if a.is_private() => "Private (RFC 1918)",
        IpAddr::V4(a) if a.is_loopback() => "Loopback",
        IpAddr::V4(a) if a.is_link_local() => "Link-local",
        IpAddr::V4(a) if (u32::from(a) >> 22) == (u32::from(Ipv4Addr::new(100, 64, 0, 0)) >> 22) => {
            "Shared / CGNAT (RFC 6598)"
        }
        IpAddr::V4(a) if a.is_multicast() => "Multicast",
        IpAddr::V4(_) => "Public",
        IpAddr::V6(a) if a.is_loopback() => "Loopback",
        IpAddr::V6(a) if (a.segments()[0] & 0xfe00) == 0xfc00 => "Unique local (ULA)",
        IpAddr::V6(a) if (a.segments()[0] & 0xffc0) == 0xfe80 => "Link-local",
        IpAddr::V6(a) if a.is_multicast() => "Multicast",
        IpAddr::V6(a) if a.segments()[0] == 0x2001 && a.segments()[1] == 0x0db8 => "Documentation",
        IpAddr::V6(_) => "Global unicast",
    }
}

/// Smallest prefix that holds `hosts` usable addresses. IPv4 never goes
/// smaller than /30, so every subnet keeps a network and broadcast address.
pub fn prefix_for_hosts(hosts: u128, v6: bool) -> Option<u8> {
    let bits = if v6 { 128 } else { 32 };
    let max_prefix = if v6 { 128 } else { 30 };
    (0..=max_prefix).rev().find(|&p| {
        let host_bits = bits - p as u32;
        let total = 1u128.checked_shl(host_bits).unwrap_or(u128::MAX);
        let usable = if v6 { total.saturating_sub(1).max(1) } else { total.saturating_sub(2) };
        usable >= hosts.max(1)
    })
}

/// Variable-length subnetting: carve `parent` into blocks big enough for each
/// host count. Largest blocks are placed first to avoid fragmentation; the
/// result is returned in the same order as `hosts`.
pub fn vlsm(parent: &IpNet, hosts: &[u128]) -> Result<Vec<IpNet>, IpError> {
    let parent = parent.trunc();
    let v6 = is_v6(&parent);
    let bits = width(&parent);
    let mut prefixes = Vec::with_capacity(hosts.len());
    for &h in hosts {
        let p = prefix_for_hosts(h, v6).ok_or(IpError::DoesNotFit(parent))?;
        if p < parent.prefix_len() {
            return Err(IpError::DoesNotFit(parent));
        }
        prefixes.push(p);
    }
    let mut order: Vec<usize> = (0..hosts.len()).collect();
    order.sort_by_key(|&i| prefixes[i]);

    let start = to_u128(parent.network());
    let end = to_u128(parent.broadcast());
    let mut cursor = start;
    let mut out = vec![parent; hosts.len()];
    for i in order {
        let block = 1u128 << (bits - prefixes[i] as u32);
        let aligned = cursor.div_ceil(block) * block;
        let last = aligned.checked_add(block - 1).ok_or(IpError::DoesNotFit(parent))?;
        if last > end {
            return Err(IpError::DoesNotFit(parent));
        }
        out[i] = IpNet::new(from_u128(aligned, v6), prefixes[i]).expect("valid prefix");
        cursor = last + 1;
    }
    Ok(out)
}

/// Split `net` into equal subnets of `new_prefix` (capped at 4096 results).
pub fn split(net: &IpNet, new_prefix: u8) -> Result<Vec<IpNet>, IpError> {
    let net = net.trunc();
    if new_prefix < net.prefix_len() || new_prefix as u32 > width(&net) {
        return Err(IpError::Invalid(format!("/{new_prefix}")));
    }
    let count = 1u128.checked_shl((new_prefix - net.prefix_len()) as u32).unwrap_or(u128::MAX);
    if count > 4096 {
        return Err(IpError::TooMany(count));
    }
    Ok(net.subnets(new_prefix).expect("checked above").collect())
}

/// Minimal set of prefixes covering exactly the same addresses.
pub fn summarize(nets: &[IpNet]) -> Vec<IpNet> {
    IpNet::aggregate(&nets.to_vec())
}

/// Smallest single prefix covering all of `nets`.
pub fn supernet(nets: &[IpNet]) -> Result<Option<IpNet>, IpError> {
    let Some(first) = nets.first() else { return Ok(None) };
    let v6 = is_v6(first);
    if nets.iter().any(|n| is_v6(n) != v6) {
        return Err(IpError::MixedFamilies);
    }
    let lo = nets.iter().map(|n| to_u128(n.network())).min().expect("non-empty");
    let hi = nets.iter().map(|n| to_u128(n.broadcast())).max().expect("non-empty");
    let bits = width(first);
    let shift = 128 - bits;
    let common = ((lo << shift) ^ (hi << shift)).leading_zeros().min(bits);
    Ok(Some(IpNet::new(from_u128(lo, v6), common as u8).expect("valid").trunc()))
}

/// First usable host in `net` for which `taken` is false. Scans at most
/// one million addresses so huge IPv6 prefixes cannot stall the UI.
pub fn next_free(net: &IpNet, taken: impl Fn(IpAddr) -> bool) -> Option<IpAddr> {
    let (a, b) = usable_range(net);
    let v6 = is_v6(net);
    let (a, b) = (to_u128(a), to_u128(b));
    let limit = b.min(a.saturating_add(1_000_000));
    (a..=limit).map(|v| from_u128(v, v6)).find(|ip| !taken(*ip))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn n(s: &str) -> IpNet {
        s.parse().unwrap()
    }
    fn ip(s: &str) -> IpAddr {
        s.parse().unwrap()
    }

    #[test]
    fn usable_ranges() {
        assert_eq!(usable_range(&n("10.0.0.0/24")), (ip("10.0.0.1"), ip("10.0.0.254")));
        assert_eq!(usable_count(&n("10.0.0.0/24")), 254);
        assert_eq!(usable_count(&n("10.0.0.0/30")), 2);
        assert_eq!(usable_count(&n("10.0.0.0/31")), 2);
        assert_eq!(usable_count(&n("10.0.0.7/32")), 1);
        assert_eq!(usable_range(&n("2001:db8::/126")), (ip("2001:db8::1"), ip("2001:db8::3")));
        assert_eq!(usable_count(&n("2001:db8::/64")), (1u128 << 64) - 1);
        assert_eq!(total_addresses(&n("::/0")), u128::MAX);
    }

    #[test]
    fn host_checks() {
        let net = n("192.168.1.0/24");
        assert!(is_usable_host(&net, ip("192.168.1.1")));
        assert!(!is_usable_host(&net, ip("192.168.1.0")));
        assert!(!is_usable_host(&net, ip("192.168.1.255")));
        assert!(!is_usable_host(&net, ip("192.168.2.1")));
        assert_eq!(nth_usable(&net, 9), Some(ip("192.168.1.10")));
        assert_eq!(nth_usable(&net, 254), None);
    }

    #[test]
    fn parsing() {
        assert_eq!(parse_net("10.1.2.3/16").unwrap(), n("10.1.2.3/16"));
        assert_eq!(parse_net(" 10.1.2.3 255.255.0.0 ").unwrap(), n("10.1.2.3/16"));
        assert_eq!(parse_net("10.1.2.3").unwrap(), n("10.1.2.3/32"));
        assert_eq!(parse_net("fe80::1").unwrap(), n("fe80::1/128"));
        assert!(matches!(parse_net("10.0.0.0 255.0.255.0"), Err(IpError::BadMask(_))));
        assert!(parse_net("10.0.0.0/33").is_err());
        assert!(parse_net("hello").is_err());
        assert_eq!(prefix_to_mask(22), Ipv4Addr::new(255, 255, 252, 0));
        assert_eq!(mask_to_prefix(Ipv4Addr::new(255, 255, 255, 192)).unwrap(), 26);
    }

    #[test]
    fn subnet_info() {
        let i = SubnetInfo::of(&n("172.16.5.77/20"));
        assert_eq!(i.network, n("172.16.0.0/20"));
        assert_eq!(i.broadcast, Some(ip("172.16.15.255")));
        assert_eq!(i.wildcard, Some(Ipv4Addr::new(0, 0, 15, 255)));
        assert_eq!(i.usable, 4094);
        assert_eq!(i.scope, "Private (RFC 1918)");
        assert_eq!(scope(ip("100.64.1.1")), "Shared / CGNAT (RFC 6598)");
        assert_eq!(scope(ip("fd00::1")), "Unique local (ULA)");
    }

    #[test]
    fn overlap() {
        assert!(overlaps(&n("10.0.0.0/8"), &n("10.20.0.0/16")));
        assert!(overlaps(&n("10.20.0.0/16"), &n("10.0.0.0/8")));
        assert!(!overlaps(&n("10.0.0.0/24"), &n("10.0.1.0/24")));
    }

    #[test]
    fn host_prefixes() {
        assert_eq!(prefix_for_hosts(2, false), Some(30));
        assert_eq!(prefix_for_hosts(1, false), Some(30));
        assert_eq!(prefix_for_hosts(254, false), Some(24));
        assert_eq!(prefix_for_hosts(255, false), Some(23));
        assert_eq!(prefix_for_hosts(50, false), Some(26));
        assert_eq!(prefix_for_hosts(1 << 40, false), None);
        assert_eq!(prefix_for_hosts(1000, true), Some(118));
    }

    #[test]
    fn vlsm_allocation() {
        // Classic textbook case.
        let got = vlsm(&n("192.168.10.0/24"), &[50, 100, 2, 20]).unwrap();
        assert_eq!(got, vec![n("192.168.10.128/26"), n("192.168.10.0/25"), n("192.168.10.224/30"), n("192.168.10.192/27")]);
        assert!(matches!(vlsm(&n("192.168.10.0/24"), &[200, 100]), Err(IpError::DoesNotFit(_))));
        // Parent given with host bits still works.
        assert_eq!(vlsm(&n("10.0.0.99/24"), &[10]).unwrap(), vec![n("10.0.0.0/28")]);
        let v6 = vlsm(&n("2001:db8::/48"), &[1u128 << 64, 1u128 << 64]).unwrap();
        assert_eq!(v6, vec![n("2001:db8::/63"), n("2001:db8:0:2::/63")]);
    }

    #[test]
    fn split_and_summarize() {
        let s = split(&n("10.0.0.0/22"), 24).unwrap();
        assert_eq!(s.len(), 4);
        assert_eq!(s[3], n("10.0.3.0/24"));
        assert!(matches!(split(&n("10.0.0.0/8"), 24), Err(IpError::TooMany(65536))));
        assert_eq!(summarize(&s), vec![n("10.0.0.0/22")]);
        assert_eq!(supernet(&[n("10.0.1.0/24"), n("10.0.6.0/24")]).unwrap(), Some(n("10.0.0.0/21")));
        assert_eq!(supernet(&[n("10.0.0.0/24")]).unwrap(), Some(n("10.0.0.0/24")));
        assert!(supernet(&[n("10.0.0.0/24"), n("::/0")]).is_err());
    }

    #[test]
    fn next_free_skips_taken() {
        let net = n("10.0.0.0/29");
        let taken = [ip("10.0.0.1"), ip("10.0.0.2")];
        assert_eq!(next_free(&net, |a| taken.contains(&a)), Some(ip("10.0.0.3")));
        assert_eq!(next_free(&net, |_| true), None);
    }
}
