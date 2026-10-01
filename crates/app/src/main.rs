// matrix-sdk's async call chains nest deeply; rustc's default query depth
// (128) overflows computing some task futures' layouts in release builds.
#![recursion_limit = "256"]

#![allow(mismatched_lifetime_syntaxes)]

mod commands;
mod hover_bar;
mod html;
mod icons;
mod isolated;
mod measured;
mod panes;
mod prefs;
mod message;
mod primary_clipboard;
mod selectable_text;
mod state;
mod theme;
mod ui;
mod update;
mod update_call;
mod update_gallery;
mod update_room_admin;
mod update_settings;
mod update_spaces;
mod view;
mod view_gallery;
mod view_input;
mod view_members;
mod view_messages;
mod view_modals;
mod view_profile;
mod view_room_admin;
mod view_space;
mod view_thread_pane;

use std::path::PathBuf;
use std::sync::Arc;

use fluxchat_core::{AppDb, AppState};
use iced::window;
use iced::Size;

use state::FluxChat;

/// Initialize the core AppState with SQLite database.
fn init_core() -> Arc<AppState> {
    let data_dir = dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("fluxchat");
    std::fs::create_dir_all(&data_dir).expect("Failed to create data dir");
    // The folder holds the access token and encryption keys: owner only.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Err(e) = std::fs::set_permissions(&data_dir, std::fs::Permissions::from_mode(0o700)) {
            tracing::warn!("Couldn't restrict permissions on {}: {e}", data_dir.display());
        }
    }

    let db_path = data_dir.join("fluxchat.db");
    let db = AppDb::open(&db_path).expect("Failed to open database");

    Arc::new(AppState::new(db))
}

fn main() -> iced::Result {
    // RUST_LOG overrides the default filter (e.g. RUST_LOG=matrix_sdk_crypto=debug
    // to diagnose decryption failures).
    // matrix_sdk_crypto::backups warns on every received key while this
    // session isn't connected to the server-side key backup — once is plenty.
    let default_filter = "warn,fluxchat=info,matrix_sdk_crypto=warn,matrix_sdk_crypto::backups=error,html5ever=error";
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(default_filter)),
        )
        .init();

    // Set macOS notification app identity to avoid "Where is use_default?" dialog
    #[cfg(target_os = "macos")]
    { let _ = notify_rust::set_application("com.apple.Terminal"); }

    let core = init_core();
    let (win_w, win_h) = state::saved_window_size(&core.db)
        .unwrap_or((state::DEFAULT_WINDOW_WIDTH, state::DEFAULT_WINDOW_HEIGHT));

    let core2 = core.clone();
    iced::application(
        move || FluxChat::new_with_init(core2.clone()),
        FluxChat::update,
        FluxChat::view,
    )
    .executor::<BigStackExecutor>()
    .title(FluxChat::title)
    .subscription(FluxChat::subscription)
    .theme(FluxChat::theme)
    .font(include_bytes!("../fonts/JetBrainsMonoNerdFont-Regular.ttf").as_slice())
    .font(include_bytes!("../fonts/JetBrainsMonoNerdFont-Bold.ttf").as_slice())
    .default_font(iced::Font {
        family: iced::font::Family::Name("JetBrainsMono Nerd Font"),
        ..iced::Font::MONOSPACE
    })
    .window(window::Settings {
        size: Size::new(win_w, win_h),
        min_size: Some(Size::new(800.0, 400.0)),
        decorations: cfg!(target_os = "macos"),
        icon: load_icon(),
        // Closing first leaves any call and saves the layout (CloseRequested).
        exit_on_close_request: false,
        ..Default::default()
    })
    .run()
}

fn load_icon() -> Option<window::icon::Icon> {
    let bytes = include_bytes!("../../../assets/icons/icon_256x256.png");
    window::icon::from_file_data(bytes, None).ok()
}

/// Tokio runtime for iced tasks with 16 MB worker stacks. matrix-sdk's
/// crypto/encryption futures are very large and have overflowed tokio's
/// default 2 MB worker stacks ("thread 'tokio-rt-worker' has overflowed its
/// stack"). Stack memory is reserved virtually, so the cost is negligible.
struct BigStackExecutor(tokio::runtime::Runtime);

impl iced::Executor for BigStackExecutor {
    fn new() -> Result<Self, std::io::Error> {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .thread_name("fluxchat-rt")
            .thread_stack_size(16 * 1024 * 1024)
            .build()
            .map(Self)
    }

    fn spawn(&self, future: impl std::future::Future<Output = ()> + Send + 'static) {
        let _handle = self.0.spawn(future);
    }

    fn enter<R>(&self, f: impl FnOnce() -> R) -> R {
        let _guard = self.0.enter();
        f()
    }

    fn block_on<T>(&self, future: impl std::future::Future<Output = T>) -> T {
        self.0.block_on(future)
    }
}
