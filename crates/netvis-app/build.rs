//! Windows builds: embed the app icon and version details in the exe.

fn main() {
    println!("cargo:rerun-if-changed=../../assets/netvis.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let mut res = winresource::WindowsResource::new();
    res.set_icon("../../assets/netvis.ico")
        .set("ProductName", "NetVis")
        .set("FileDescription", "NetVis network planner")
        .set("CompanyName", "WebPotato")
        .set("LegalCopyright", "© WebPotato");
    if let Err(e) = res.compile() {
        // A missing resource compiler shouldn't stop the build; the exe just
        // has no icon.
        println!("cargo:warning=could not embed the Windows icon: {e}");
    }
}
