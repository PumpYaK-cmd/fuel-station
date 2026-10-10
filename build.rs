fn main() {
    println!("cargo:rerun-if-changed=assets/icon/fuel-station.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        winresource::WindowsResource::new()
            .set_icon("assets/icon/fuel-station.ico")
            .set("FileDescription", "Fuel Station")
            .set("ProductName", "Fuel Station")
            .compile()
            .expect("embed Windows resources");
    }
}
