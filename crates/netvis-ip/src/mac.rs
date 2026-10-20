//! 48-bit MAC addresses: parsing, the usual notations and the flag bits.

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Mac(pub [u8; 6]);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MacStyle {
    /// `aa:bb:cc:dd:ee:ff`
    Colon,
    /// `aa-bb-cc-dd-ee-ff` (Windows)
    Hyphen,
    /// `aabb.ccdd.eeff` (Cisco)
    Dotted,
    /// `aabbccddeeff`
    Bare,
}

impl MacStyle {
    pub const ALL: [MacStyle; 4] = [MacStyle::Colon, MacStyle::Hyphen, MacStyle::Dotted, MacStyle::Bare];

    pub fn label(self) -> &'static str {
        match self {
            MacStyle::Colon => "Colon",
            MacStyle::Hyphen => "Hyphen (Windows)",
            MacStyle::Dotted => "Dotted (Cisco)",
            MacStyle::Bare => "Bare hex",
        }
    }
}

/// Keep only hex digits, dropping the usual separators. `None` if anything
/// else is present.
fn hex_digits(s: &str) -> Option<String> {
    let mut out = String::with_capacity(12);
    for c in s.trim().chars() {
        match c {
            ':' | '-' | '.' | ' ' => {}
            c if c.is_ascii_hexdigit() => out.push(c.to_ascii_lowercase()),
            _ => return None,
        }
    }
    Some(out)
}

impl Mac {
    /// Accepts colon, hyphen, Cisco-dotted or bare hex notation.
    pub fn parse(s: &str) -> Option<Mac> {
        let hex = hex_digits(s)?;
        if hex.len() != 12 {
            return None;
        }
        let mut out = [0u8; 6];
        for (i, b) in out.iter_mut().enumerate() {
            *b = u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).ok()?;
        }
        Some(Mac(out))
    }

    /// The 24-bit organisationally unique identifier.
    pub fn oui(&self) -> u32 {
        u32::from_be_bytes([0, self.0[0], self.0[1], self.0[2]])
    }

    /// Group (multicast) address: I/G bit set.
    pub fn is_multicast(&self) -> bool {
        self.0[0] & 0x01 != 0
    }

    pub fn is_broadcast(&self) -> bool {
        self.0 == [0xff; 6]
    }

    /// Locally administered (U/L bit set): randomised or software-assigned,
    /// so the OUI does not identify a vendor.
    pub fn is_local(&self) -> bool {
        self.0[0] & 0x02 != 0
    }

    pub fn format(&self, style: MacStyle, upper: bool) -> String {
        let b = self.0;
        let s = match style {
            MacStyle::Colon => format!("{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}", b[0], b[1], b[2], b[3], b[4], b[5]),
            MacStyle::Hyphen => format!("{:02x}-{:02x}-{:02x}-{:02x}-{:02x}-{:02x}", b[0], b[1], b[2], b[3], b[4], b[5]),
            MacStyle::Dotted => format!("{:02x}{:02x}.{:02x}{:02x}.{:02x}{:02x}", b[0], b[1], b[2], b[3], b[4], b[5]),
            MacStyle::Bare => format!("{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}", b[0], b[1], b[2], b[3], b[4], b[5]),
        };
        if upper { s.to_uppercase() } else { s }
    }

    /// Modified EUI-64 interface identifier (RFC 4291): `ff:fe` in the
    /// middle and the U/L bit inverted.
    pub fn eui64(&self) -> [u8; 8] {
        let b = self.0;
        [b[0] ^ 0x02, b[1], b[2], 0xff, 0xfe, b[3], b[4], b[5]]
    }
}

impl fmt::Display for Mac {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.format(MacStyle::Colon, false))
    }
}

/// The OUI from a full MAC or from just its first 6+ hex digits
/// (`00:00:0c`, `00000C`, `0000.0c`).
pub fn parse_oui(s: &str) -> Option<u32> {
    let hex = hex_digits(s)?;
    if hex.len() < 6 || hex.len() > 12 {
        return None;
    }
    u32::from_str_radix(&hex[..6], 16).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notations() {
        let m = Mac::parse("AA:bb:cc:00:11:22").unwrap();
        assert_eq!(Mac::parse("aabb.cc00.1122"), Some(m));
        assert_eq!(Mac::parse("aa-bb-cc-00-11-22"), Some(m));
        assert_eq!(Mac::parse("aabbcc00112"), None);
        assert_eq!(Mac::parse("zz:bb:cc:00:11:22"), None);
        assert_eq!(m.format(MacStyle::Dotted, false), "aabb.cc00.1122");
        assert_eq!(m.format(MacStyle::Hyphen, true), "AA-BB-CC-00-11-22");
        assert_eq!(m.oui(), 0xaabbcc);
        assert_eq!(parse_oui("00:00:0C"), Some(0x00000c));
        assert_eq!(parse_oui("00:00"), None);
    }

    #[test]
    fn flags_and_eui64() {
        let m = Mac::parse("00:1b:63:84:45:e6").unwrap();
        assert!(!m.is_local() && !m.is_multicast());
        assert_eq!(m.eui64(), [0x02, 0x1b, 0x63, 0xff, 0xfe, 0x84, 0x45, 0xe6]);
        assert!(Mac::parse("01:00:5e:00:00:01").unwrap().is_multicast());
        assert!(Mac::parse("da:a1:19:00:00:01").unwrap().is_local());
        assert!(Mac::parse("ff:ff:ff:ff:ff:ff").unwrap().is_broadcast());
    }
}
