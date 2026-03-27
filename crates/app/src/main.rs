mod app;
mod state;
mod components;
mod media_server;

use dioxus::desktop::{Config, WindowBuilder};

fn load_icon() -> Option<dioxus::desktop::tao::window::Icon> {
    let bytes = include_bytes!("../../../assets/icons/icon_256x256.png");
    let img = image::load_from_memory(bytes).ok()?.into_rgba8();
    let (w, h) = img.dimensions();
    dioxus::desktop::tao::window::Icon::from_rgba(img.into_raw(), w, h).ok()
}

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::new("warn,fluxchat=info,matrix_sdk_crypto=error")
        )
        .init();

    media_server::start();

    let mut window_builder = WindowBuilder::new()
        .with_title("fluxchat")
        .with_decorations(cfg!(target_os = "macos"))
        .with_inner_size(dioxus::desktop::LogicalSize::new(1200, 800));

    if let Some(icon) = load_icon() {
        window_builder = window_builder.with_window_icon(Some(icon));
    }

    dioxus::LaunchBuilder::desktop()
        .with_cfg(
            Config::new()
                .with_window(window_builder)
                .with_disable_context_menu(true)
        )
        .launch(app::App);
}
