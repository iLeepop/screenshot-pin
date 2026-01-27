fn main() {
    if std::env::var("CARGO_CFG_WINDOWS").is_ok() {
        let mut res = winres::WindowsResource::new();
        res.set_icon("asset/logo.ico");
        if let Err(_) = res.compile() {
            eprintln!("build windows app with ico err!");
        }
    }
}