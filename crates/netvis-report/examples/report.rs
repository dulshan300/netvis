//! Render the design report for a project:
//! `cargo run -p netvis-report --example report -- in.netvis out.pdf`.

fn main() {
    let mut args = std::env::args().skip(1);
    let (Some(input), Some(output)) = (args.next(), args.next()) else {
        eprintln!("usage: report <file.netvis> <out.pdf>");
        std::process::exit(2);
    };
    let p = netvis_core::io::load(std::path::Path::new(&input)).expect("readable project");
    let pdf = netvis_report::render_pdf(&p, &netvis_report::ReportOptions::default()).expect("report renders");
    std::fs::write(&output, &pdf).expect("writable output");
    println!("wrote {output} ({} KB)", pdf.len() / 1024);
}
