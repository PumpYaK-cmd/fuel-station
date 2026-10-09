mod swatch;
pub mod theme;

pub fn run() {
    let result = iced::application(|| (), swatch::update, swatch::view)
        .title("Fuel Station")
        .theme(theme::theme())
        .font(include_bytes!("../../assets/fonts/JetBrainsMonoNerdFont-Regular.ttf").as_slice())
        .default_font(theme::FONT)
        .run();
    if let Err(e) = result {
        eprintln!("fuel-station: {e}");
    }
}
