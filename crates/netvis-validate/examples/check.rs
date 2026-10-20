//! Validate a project from the command line.
//!
//! cargo run -p netvis-validate --example check -- project.netvis [--fix]
//!
//! With `--fix`, applies every available fix for errors and warnings (in
//! memory) and reports what remains. Exits non-zero if errors remain.

use std::process::ExitCode;

fn print(issues: &[netvis_validate::Issue], p: &netvis_core::Project) {
    for i in issues {
        let fix = i.fix.as_ref().map(|f| format!("  [fix: {}]", f.label(p))).unwrap_or_default();
        println!("{:<7} {:<22} {}{fix}", i.severity.label(), i.code, i.message);
    }
}

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let Some(path) = args.next() else {
        eprintln!("usage: check <file.netvis> [--fix]");
        return ExitCode::from(2);
    };
    let fix = args.any(|a| a == "--fix");
    let mut p = match netvis_core::io::load(path.as_ref()) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("{path}: {e}");
            return ExitCode::from(2);
        }
    };
    let issues = netvis_validate::validate(&p);
    print(&issues, &p);
    if fix {
        let codes: Vec<&str> = issues.iter().filter(|i| i.severity != netvis_validate::Severity::Info).map(|i| i.code).collect();
        let (edit, n) = netvis_validate::fix_all(&p, Some(&codes));
        edit.apply(&mut p);
        println!("\n--- applied {n} fix(es); remaining:");
        print(&netvis_validate::validate(&p), &p);
    }
    let errors = netvis_validate::validate(&p).iter().filter(|i| i.severity == netvis_validate::Severity::Error).count();
    if errors > 0 { ExitCode::FAILURE } else { ExitCode::SUCCESS }
}
