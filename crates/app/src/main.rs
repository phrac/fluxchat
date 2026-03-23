mod app;
mod state;
mod components;

use dioxus::desktop::{Config, WindowBuilder};

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::new("warn,fluxchat=info,matrix_sdk_crypto=error")
        )
        .init();

    dioxus::LaunchBuilder::desktop()
        .with_cfg(
            Config::new()
                .with_window(
                    WindowBuilder::new()
                        .with_title("fluxchat")
                        .with_decorations(cfg!(target_os = "macos"))
                        .with_inner_size(dioxus::desktop::LogicalSize::new(1200, 800))
                )
                .with_menu(None)
                .with_disable_context_menu(true)
                .with_custom_head(r#"<style>*{-webkit-tap-highlight-color:transparent;}</style><script>
                    // Suppress all sounds/notifications in WebView
                    window.Audio = function(){return{play:()=>{},pause:()=>{},load:()=>{}}};
                    window.Notification = {permission:'denied',requestPermission:()=>Promise.resolve('denied')};
                    window.alert = ()=>{};
                </script>"#.to_string())
        )
        .launch(app::App);
}
