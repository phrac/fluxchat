mod app;
mod state;
mod components;

use dioxus::prelude::*;

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::new("warn,fluxchat=info")
        )
        .init();

    dioxus::launch(app::App);
}
