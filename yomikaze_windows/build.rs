// embeds the icon and version info into the exe
fn main() {
    println!("cargo:rerun-if-changed=assets/icon.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut resource = winresource::WindowsResource::new();
        resource
            .set_icon("assets/icon.ico")
            .set("ProductName", "Yomikaze")
            .set("FileDescription", "Yomikaze - manga reader (hotfix)")
            // the file properties show the hotfix too, the numeric version stays the same
            .set("ProductVersion", &format!("{} hotfix", std::env::var("CARGO_PKG_VERSION").unwrap_or_default()))
            .set("CompanyName", "Kairo")
            .set("LegalCopyright", "Kairo");
        if let Err(error) = resource.compile() {
            println!("cargo:warning=Couldn't embed the icon: {error}");
        }
    }
}
