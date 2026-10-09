#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("Kankan")
            .with_inner_size([1280.0, 820.0])
            .with_min_inner_size([900.0, 600.0])
            .with_fullsize_content_view(true)
            .with_title_shown(false)
            .with_titlebar_shown(false)
            .with_titlebar_buttons_shown(true),
        ..Default::default()
    };

    if let Err(error) = eframe::run_native(
        "Kankan",
        options,
        Box::new(|creation_context| {
            Ok(Box::new(kankan::app::KankanApp::new(creation_context)))
        }),
    ) {
        eprintln!("Failed to start Kankan: {error}");
    }
}
