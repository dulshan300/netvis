//! Well-known TCP/UDP ports and IP protocol numbers, for the port reference.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Proto {
    Tcp,
    Udp,
    TcpUdp,
    /// An IP protocol number rather than a port.
    Ip,
}

impl Proto {
    pub fn label(self) -> &'static str {
        match self {
            Proto::Tcp => "TCP",
            Proto::Udp => "UDP",
            Proto::TcpUdp => "TCP/UDP",
            Proto::Ip => "IP proto",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WellKnown {
    pub port: u16,
    /// Last port of a range; equal to `port` for single ports.
    pub end: u16,
    pub proto: Proto,
    pub name: &'static str,
    pub description: &'static str,
    /// Sends credentials or data in clear text, or is risky to expose.
    pub insecure: bool,
}

impl WellKnown {
    pub fn port_label(&self) -> String {
        if self.end > self.port { format!("{}–{}", self.port, self.end) } else { self.port.to_string() }
    }

    pub fn matches(&self, needle: &str) -> bool {
        let n = needle.trim().to_lowercase();
        if n.is_empty() {
            return true;
        }
        if let Ok(p) = n.parse::<u16>() {
            return (self.port..=self.end).contains(&p);
        }
        self.name.to_lowercase().contains(&n) || self.description.to_lowercase().contains(&n)
    }
}

macro_rules! p {
    ($port:expr, $proto:ident, $name:literal, $desc:literal) => {
        WellKnown { port: $port, end: $port, proto: Proto::$proto, name: $name, description: $desc, insecure: false }
    };
    ($port:expr, $proto:ident, $name:literal, $desc:literal, insecure) => {
        WellKnown { port: $port, end: $port, proto: Proto::$proto, name: $name, description: $desc, insecure: true }
    };
    ($port:expr, $end:expr, $proto:ident, $name:literal, $desc:literal) => {
        WellKnown { port: $port, end: $end, proto: Proto::$proto, name: $name, description: $desc, insecure: false }
    };
}

pub const PORTS: &[WellKnown] = &[
    p!(20, Tcp, "FTP data", "File transfer (active-mode data channel)", insecure),
    p!(21, Tcp, "FTP", "File transfer control", insecure),
    p!(22, Tcp, "SSH", "Secure shell, SCP and SFTP"),
    p!(23, Tcp, "Telnet", "Clear-text remote terminal", insecure),
    p!(25, Tcp, "SMTP", "Mail transfer between servers"),
    p!(49, Tcp, "TACACS+", "Device administration AAA"),
    p!(53, TcpUdp, "DNS", "Name resolution (TCP for zone transfers and large replies)"),
    p!(67, Udp, "DHCP server", "DHCP/BOOTP server; relays forward to this port"),
    p!(68, Udp, "DHCP client", "DHCP/BOOTP client"),
    p!(69, Udp, "TFTP", "Trivial file transfer (phone/AP provisioning, PXE)", insecure),
    p!(80, Tcp, "HTTP", "Web", insecure),
    p!(88, TcpUdp, "Kerberos", "Authentication (Active Directory)"),
    p!(110, Tcp, "POP3", "Mail retrieval", insecure),
    p!(123, Udp, "NTP", "Time synchronisation"),
    p!(135, Tcp, "MS RPC", "Microsoft RPC endpoint mapper"),
    p!(137, Udp, "NetBIOS name", "NetBIOS name service", insecure),
    p!(138, Udp, "NetBIOS datagram", "NetBIOS datagram service", insecure),
    p!(139, Tcp, "NetBIOS session", "Legacy SMB over NetBIOS", insecure),
    p!(143, Tcp, "IMAP", "Mail access", insecure),
    p!(161, Udp, "SNMP", "Monitoring polls (v1/v2c community strings are clear text)", insecure),
    p!(162, Udp, "SNMP trap", "Monitoring notifications", insecure),
    p!(179, Tcp, "BGP", "Border Gateway Protocol"),
    p!(389, TcpUdp, "LDAP", "Directory access", insecure),
    p!(443, TcpUdp, "HTTPS", "Web over TLS; UDP for HTTP/3 (QUIC)"),
    p!(445, Tcp, "SMB", "Windows file sharing"),
    p!(464, TcpUdp, "Kerberos kpasswd", "Kerberos password change"),
    p!(465, Tcp, "SMTPS", "Mail submission over implicit TLS"),
    p!(500, Udp, "IKE", "IPsec key exchange"),
    p!(514, Udp, "Syslog", "Log forwarding", insecure),
    p!(515, Tcp, "LPD", "Line printer daemon"),
    p!(520, Udp, "RIP", "Routing Information Protocol"),
    p!(521, Udp, "RIPng", "RIP for IPv6"),
    p!(546, Udp, "DHCPv6 client", "DHCPv6 client"),
    p!(547, Udp, "DHCPv6 server", "DHCPv6 server and relay"),
    p!(554, TcpUdp, "RTSP", "Streaming control (IP cameras, NVRs)"),
    p!(587, Tcp, "Submission", "Mail submission (STARTTLS)"),
    p!(623, Udp, "IPMI", "Out-of-band server management (RMCP)", insecure),
    p!(636, Tcp, "LDAPS", "LDAP over TLS"),
    p!(646, TcpUdp, "LDP", "MPLS label distribution"),
    p!(830, Tcp, "NETCONF", "Network configuration over SSH"),
    p!(853, Tcp, "DNS over TLS", "Encrypted DNS"),
    p!(873, Tcp, "rsync", "File synchronisation"),
    p!(902, Tcp, "VMware ESXi", "vSphere host management and console"),
    p!(989, Tcp, "FTPS data", "FTP over TLS (data)"),
    p!(990, Tcp, "FTPS", "FTP over TLS (control)"),
    p!(993, Tcp, "IMAPS", "IMAP over TLS"),
    p!(995, Tcp, "POP3S", "POP3 over TLS"),
    p!(1080, Tcp, "SOCKS", "Proxy"),
    p!(1194, TcpUdp, "OpenVPN", "OpenVPN tunnels"),
    p!(1433, Tcp, "MS SQL", "Microsoft SQL Server"),
    p!(1434, Udp, "MS SQL browser", "SQL Server instance discovery"),
    p!(1521, Tcp, "Oracle", "Oracle database listener"),
    p!(1701, Udp, "L2TP", "Layer 2 Tunnelling Protocol"),
    p!(1723, Tcp, "PPTP", "Legacy VPN (weak encryption)", insecure),
    p!(1812, Udp, "RADIUS", "Authentication (802.1X, VPN, Wi-Fi)"),
    p!(1813, Udp, "RADIUS accounting", "Accounting"),
    p!(1883, Tcp, "MQTT", "IoT messaging", insecure),
    p!(1900, Udp, "SSDP", "UPnP discovery"),
    p!(1985, Udp, "HSRP", "Hot Standby Router Protocol"),
    p!(2049, TcpUdp, "NFS", "Network file system"),
    p!(2055, Udp, "NetFlow", "Flow export (common default)"),
    p!(2083, Tcp, "RadSec", "RADIUS over TLS"),
    p!(3260, Tcp, "iSCSI", "Block storage"),
    p!(3268, Tcp, "Global Catalog", "Active Directory global catalog"),
    p!(3269, Tcp, "Global Catalog TLS", "Global catalog over TLS"),
    p!(3306, Tcp, "MySQL", "MySQL / MariaDB"),
    p!(3389, TcpUdp, "RDP", "Remote Desktop"),
    p!(3478, TcpUdp, "STUN/TURN", "NAT traversal for voice and video"),
    p!(3784, Udp, "BFD", "Bidirectional forwarding detection (single hop)"),
    p!(3799, Udp, "RADIUS CoA", "Change of authorisation / disconnect"),
    p!(4500, Udp, "IPsec NAT-T", "IPsec through NAT"),
    p!(4739, Udp, "IPFIX", "Flow export"),
    p!(4786, Tcp, "Smart Install", "Cisco Smart Install — disable if unused", insecure),
    p!(4789, Udp, "VXLAN", "VXLAN overlay"),
    p!(5060, TcpUdp, "SIP", "VoIP signalling", insecure),
    p!(5061, Tcp, "SIP-TLS", "VoIP signalling over TLS"),
    p!(5222, Tcp, "XMPP", "Chat client connections"),
    p!(5246, Udp, "CAPWAP control", "Wireless controller ↔ AP control"),
    p!(5247, Udp, "CAPWAP data", "Wireless controller ↔ AP data"),
    p!(5353, Udp, "mDNS", "Multicast DNS / Bonjour"),
    p!(5355, Udp, "LLMNR", "Link-local name resolution"),
    p!(5432, Tcp, "PostgreSQL", "PostgreSQL database"),
    p!(5671, Tcp, "AMQPS", "AMQP over TLS"),
    p!(5672, Tcp, "AMQP", "Message queueing"),
    p!(5900, Tcp, "VNC", "Remote framebuffer", insecure),
    p!(5985, Tcp, "WinRM", "Windows remote management (HTTP)"),
    p!(5986, Tcp, "WinRM HTTPS", "Windows remote management (HTTPS)"),
    p!(6081, Udp, "Geneve", "Geneve overlay"),
    p!(6379, Tcp, "Redis", "Redis key-value store"),
    p!(6443, Tcp, "Kubernetes API", "Kubernetes API server"),
    p!(6514, Tcp, "Syslog TLS", "Log forwarding over TLS"),
    p!(6653, Tcp, "OpenFlow", "SDN controller"),
    p!(8080, Tcp, "HTTP alt", "Alternate web / proxies", insecure),
    p!(8443, Tcp, "HTTPS alt", "Alternate HTTPS / management UIs"),
    p!(8883, Tcp, "MQTT TLS", "MQTT over TLS"),
    p!(9100, Tcp, "Raw print", "JetDirect / raw printing"),
    p!(9200, Tcp, "Elasticsearch", "Elasticsearch REST API"),
    p!(16384, 32767, Udp, "RTP", "Voice/video media (common range)"),
    p!(27017, Tcp, "MongoDB", "MongoDB database"),
    // IP protocol numbers.
    p!(1, Ip, "ICMP", "Ping, unreachables, PMTU discovery"),
    p!(2, Ip, "IGMP", "IPv4 multicast group membership"),
    p!(6, Ip, "TCP", "Transmission Control Protocol"),
    p!(17, Ip, "UDP", "User Datagram Protocol"),
    p!(47, Ip, "GRE", "Generic routing encapsulation tunnels"),
    p!(50, Ip, "ESP", "IPsec encrypted payload"),
    p!(51, Ip, "AH", "IPsec authentication header"),
    p!(58, Ip, "ICMPv6", "IPv6 control, neighbour discovery"),
    p!(88, Ip, "EIGRP", "Enhanced Interior Gateway Routing Protocol"),
    p!(89, Ip, "OSPF", "Open Shortest Path First"),
    p!(103, Ip, "PIM", "Multicast routing"),
    p!(112, Ip, "VRRP", "Virtual Router Redundancy Protocol"),
    p!(132, Ip, "SCTP", "Stream Control Transmission Protocol"),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search() {
        assert!(PORTS.iter().any(|p| p.matches("443") && p.name == "HTTPS"));
        assert!(PORTS.iter().any(|p| p.matches("20000") && p.name == "RTP"));
        assert!(PORTS.iter().filter(|p| p.matches("radius")).count() >= 3);
        let mut seen: Vec<_> = PORTS.iter().map(|p| (p.port, p.proto == Proto::Ip)).collect();
        seen.sort();
        let n = seen.len();
        seen.dedup();
        assert_eq!(seen.len(), n, "duplicate port entries");
    }
}
