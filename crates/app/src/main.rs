mod actions;
mod app;
mod board;
mod brand;
mod images;
mod issue_panel;
mod mock;
mod onboarding;
mod rich;
mod settings;
mod sync;
mod theme;
mod workspace;

use eframe::egui;

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("mcp") {
        return mcp::run_stdio();
    }
    let opts = app::Options::parse(&args)?;

    let mut viewport = egui::ViewportBuilder::default()
        .with_title("Kanbanana")
        .with_app_id("kanbanana")
        .with_inner_size([1440.0, 900.0])
        .with_min_inner_size([1000.0, 640.0]);
    // X11 uses this icon. Wayland takes it from assets/kanbanana.desktop through the app id.
    match eframe::icon_data::from_png_bytes(brand::ICON_PNG) {
        Ok(icon) => viewport = viewport.with_icon(icon),
        Err(e) => eprintln!("kanbanana: window icon: {e}"),
    }
    let mut native = eframe::NativeOptions { viewport, ..Default::default() };
    if opts.screenshot.is_some() {
        // Without vsync the app renders on a hidden workspace too.
        native.glow_options.vsync = false;
    }
    eframe::run_native("Kanbanana", native, Box::new(move |cc| Ok(Box::new(app::App::new(cc, opts)))))
        .map_err(|e| anyhow::anyhow!("{e}"))
}
