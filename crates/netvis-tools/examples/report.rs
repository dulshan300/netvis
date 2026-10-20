//! Print the project-aware tool reports for a `.netvis` file:
//! `cargo run -p netvis-tools --example report -- file.netvis`.

use netvis_tools::{bom, fibre, oversub};

fn main() {
    let path = std::env::args().nth(1).expect("usage: report <file.netvis>");
    let p = netvis_core::io::load(std::path::Path::new(&path)).expect("readable project");

    println!("Fibre links (2 connector pairs, 1 dB margin):");
    for r in fibre::project_links(&p, 2, 1.0) {
        let loss = r.budget.map(|b| format!("{:.2} dB", b.total_db())).unwrap_or_else(|| "—".into());
        let limit = r.limit_db.map(|l| format!("{l:.2} dB")).unwrap_or_else(|| "—".into());
        println!("  {:40} {:8} {:>7} m  {:14} loss {loss:>9} / allowed {limit}", r.label, r.cable.label(), r.length_m.unwrap_or(0.0), r.optic.map(|o| o.name).unwrap_or("—"));
    }

    println!("\nOversubscription:");
    for s in oversub::switch_report(&p) {
        let f = |r: Option<f64>| r.map(|r| format!("{r:.1}:1")).unwrap_or_else(|| "—".into());
        println!("  {:12} edge {:>3}  uplinks {}  all {:>7}  cabled {:>7}  guideline ≤{}:1", s.device.name, s.edge_ports, s.uplink_ports, f(s.ratio()), f(s.connected_ratio()), s.guideline());
    }

    println!("\nBill of materials (CSV):\n{}", bom::to_csv(&bom::build(&p, bom::BomOptions::default())));
}
