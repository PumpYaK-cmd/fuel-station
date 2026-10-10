fn main() {
    println!("cargo:rerun-if-changed=assets/icon/fuel-station.ico");
    #[cfg(windows)]
    winresource::WindowsResource::new()
        .set_icon("assets/icon/fuel-station.ico")
        .set("FileDescription", "Fuel Station")
        .set("ProductName", "Fuel Station")
        .compile()
        .expect("embed Windows resources");
}
