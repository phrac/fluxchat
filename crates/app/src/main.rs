mod app;
mod state;
mod components;

use dioxus::prelude::*;
use dioxus::desktop::{Config, WindowBuilder};

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::new("warn,fluxchat=info")
        )
        .init();

    dioxus::LaunchBuilder::desktop()
        .with_cfg(
            Config::new()
                .with_window(
                    WindowBuilder::new()
                        .with_title("fluxchat")
                        .with_decorations(false)
                        .with_inner_size(dioxus::desktop::LogicalSize::new(1200, 800))
                )
                .with_menu(None)
        )
        .launch(app::App);
}
