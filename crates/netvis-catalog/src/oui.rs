//! MAC vendor lookup from the bundled IEEE MA-L (OUI) registry.
//!
//! `data/oui.tsv` is generated from <https://standards-oui.ieee.org/oui/oui.csv>
//! as sorted `HEX6<TAB>Organisation` lines. Replace the file to update it.

use std::sync::OnceLock;

const RAW: &str = include_str!("../data/oui.tsv");

fn table() -> &'static [(u32, &'static str)] {
    static TABLE: OnceLock<Vec<(u32, &'static str)>> = OnceLock::new();
    TABLE.get_or_init(|| {
        let mut v: Vec<(u32, &'static str)> = RAW
            .lines()
            .filter(|l| !l.starts_with('#'))
            .filter_map(|l| {
                let (hex, name) = l.split_once('\t')?;
                Some((u32::from_str_radix(hex, 16).ok()?, name))
            })
            .collect();
        v.sort_unstable_by_key(|e| e.0);
        v
    })
}

/// Number of registered OUIs bundled.
pub fn count() -> usize {
    table().len()
}

/// Registered organisation for a 24-bit OUI.
pub fn vendor(oui: u32) -> Option<&'static str> {
    let t = table();
    t.binary_search_by_key(&oui, |e| e.0).ok().map(|i| t[i].1)
}

/// OUIs whose organisation name contains `needle` (case-insensitive).
pub fn search(needle: &str, limit: usize) -> Vec<(u32, &'static str)> {
    let n = needle.trim().to_lowercase();
    if n.is_empty() {
        return Vec::new();
    }
    table().iter().filter(|(_, name)| name.to_lowercase().contains(&n)).take(limit).copied().collect()
}

/// Total number of OUIs matching `needle`, for "showing N of M".
pub fn search_count(needle: &str) -> usize {
    let n = needle.trim().to_lowercase();
    if n.is_empty() {
        return 0;
    }
    table().iter().filter(|(_, name)| name.to_lowercase().contains(&n)).count()
}

/// `00:00:0C` style.
pub fn format_oui(oui: u32) -> String {
    format!("{:02X}:{:02X}:{:02X}", (oui >> 16) & 0xff, (oui >> 8) & 0xff, oui & 0xff)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookup() {
        assert!(count() > 30_000);
        assert_eq!(vendor(0x00000c), Some("Cisco Systems, Inc"));
        assert_eq!(vendor(0xfffffe), None);
        assert!(search("cisco", 5).len() == 5);
        assert!(search_count("cisco") > 100);
        assert_eq!(format_oui(0x00000c), "00:00:0C");
    }
}
