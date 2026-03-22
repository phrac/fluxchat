use dioxus::prelude::*;
use std::sync::Arc;
use fluxchat_core::AppState;

use crate::state::UiState;
use crate::app::load_initial_data;

#[component]
pub fn Login() -> Element {
    let core = use_context::<Arc<AppState>>();
    let mut ui = use_context::<UiState>();

    let mut homeserver = use_signal(|| "matrix.org".to_string());
    let mut username = use_signal(|| String::new());
    let mut password = use_signal(|| String::new());
    let mut error = use_signal(|| String::new());
    let mut submitting = use_signal(|| false);

    let handle_login = move |_| {
        let core = core.clone();
        let mut ui = ui.clone();
        let hs = homeserver().clone();
        let user = username().clone();
        let pass = password().clone();
        spawn(async move {
            submitting.set(true);
            error.set(String::new());
            match fluxchat_core::auth::login(&core, &hs, &user, &pass).await {
                Ok((_client, result)) => {
                    ui.user_id.set(result.user_id);
                    ui.display_name.set(result.display_name);
                    ui.logged_in.set(true);
                    load_initial_data(&core, &mut ui).await;
                }
                Err(e) => error.set(e.to_string()),
            }
            submitting.set(false);
        });
    };

    rsx! {
        div {
            style: "display: flex; align-items: center; justify-content: center; height: 100vh; background: var(--bg-deep);",
            div {
                style: "width: 380px; background: var(--bg-dark); border-radius: 8px; padding: 32px; border: 1px solid var(--border);",

                h1 { style: "color: var(--text-bright); font-size: 22px; margin-bottom: 4px; text-align: center;",
                    "fluxchat"
                }
                p { style: "color: var(--text-muted); font-size: 13px; text-align: center; margin-bottom: 24px;",
                    "Sign in with your Matrix account"
                }

                if !error().is_empty() {
                    div { style: "background: rgba(239,68,68,0.1); color: var(--red); padding: 8px 12px; border-radius: 4px; font-size: 13px; margin-bottom: 16px;",
                        "{error}"
                    }
                }

                label { style: "display: block; color: var(--text-muted); font-size: 11px; font-weight: 700; text-transform: uppercase; margin-bottom: 4px;",
                    "Homeserver"
                }
                input {
                    style: "width: 100%; background: var(--bg-deep); color: var(--text-bright); border: none; border-radius: 4px; padding: 8px 12px; font-size: 14px; outline: none; margin-bottom: 12px; box-sizing: border-box;",
                    r#type: "text",
                    value: "{homeserver}",
                    oninput: move |e| homeserver.set(e.value()),
                }

                label { style: "display: block; color: var(--text-muted); font-size: 11px; font-weight: 700; text-transform: uppercase; margin-bottom: 4px;",
                    "Username"
                }
                input {
                    style: "width: 100%; background: var(--bg-deep); color: var(--text-bright); border: none; border-radius: 4px; padding: 8px 12px; font-size: 14px; outline: none; margin-bottom: 12px; box-sizing: border-box;",
                    r#type: "text",
                    value: "{username}",
                    oninput: move |e| username.set(e.value()),
                }

                label { style: "display: block; color: var(--text-muted); font-size: 11px; font-weight: 700; text-transform: uppercase; margin-bottom: 4px;",
                    "Password"
                }
                input {
                    style: "width: 100%; background: var(--bg-deep); color: var(--text-bright); border: none; border-radius: 4px; padding: 8px 12px; font-size: 14px; outline: none; margin-bottom: 20px; box-sizing: border-box;",
                    r#type: "password",
                    value: "{password}",
                    oninput: move |e| password.set(e.value()),
                }

                button {
                    style: "width: 100%; background: var(--accent); color: white; border: none; border-radius: 4px; padding: 10px; font-size: 14px; font-weight: 500; cursor: pointer;",
                    disabled: *submitting.read(),
                    onclick: handle_login,
                    if *submitting.read() { "Signing in..." } else { "Sign In" }
                }
            }
        }
    }
}
