use dioxus::prelude::*;
use std::sync::Arc;
use fluxchat_core::AppState;
use crate::state::UiState;

#[component]
pub fn SpaceSidebar() -> Element {
    let mut ui = use_context::<UiState>();
    let core = use_context::<Arc<AppState>>();
    let spaces = ui.spaces.read().clone();
    let selected = ui.selected_space_id.read().clone();

    let home_style = format!(
        "width: 40px; height: 40px; border-radius: {}; background: {}; display: flex; align-items: center; justify-content: center; cursor: pointer; margin-bottom: 8px; color: {};",
        if selected.is_none() { "12px" } else { "20px" },
        if selected.is_none() { "var(--accent)" } else { "var(--bg-mid)" },
        if selected.is_none() { "white" } else { "var(--text-muted)" },
    );

    rsx! {
        div { style: "width: 56px; flex-shrink: 0; background: var(--bg-deep); display: flex; flex-direction: column; align-items: center; padding: 8px 0; overflow-y: auto; border-right: 1px solid var(--border);",
            div {
                style: "{home_style}",
                onclick: move |_| {
                    ui.selected_space_id.set(None);
                    ui.selected_room_id.set(None);
                },
                "⌂"
            }

            div { style: "width: 24px; height: 2px; background: var(--border); margin-bottom: 8px; border-radius: 1px;" }

            for space in spaces.iter() {
                SpaceIcon {
                    space_id: space.room_id.clone(),
                    name: space.name.clone(),
                    avatar_mxc: space.avatar_url.clone(),
                    is_active: selected.as_ref() == Some(&space.room_id),
                }
            }
        }
    }
}

#[component]
fn SpaceIcon(space_id: String, name: String, avatar_mxc: Option<String>, is_active: bool) -> Element {
    let mut ui = use_context::<UiState>();
    let core = use_context::<Arc<AppState>>();

    let radius = if is_active { "12px" } else { "20px" };

    // Resolve avatar
    let avatar_src = use_resource({
        let mxc = avatar_mxc.clone();
        let core = core.clone();
        move || {
            let mxc = mxc.clone();
            let core = core.clone();
            async move {
                if let Some(mxc) = mxc {
                    let client = core.get_client().await.ok()?;
                    fluxchat_core::media::get_media_url(&client, &core.db, &core.media_cache, &mxc).await.ok()
                } else {
                    None
                }
            }
        }
    });

    let has_avatar = avatar_src.read().as_ref().and_then(|v| v.as_ref()).is_some();
    let initials: String = name.split_whitespace().take(2).map(|w| w.chars().next().unwrap_or('?')).collect::<String>().to_uppercase();

    let sid = space_id.clone();

    rsx! {
        div {
            style: "width: 40px; height: 40px; border-radius: {radius}; background: var(--bg-mid); display: flex; align-items: center; justify-content: center; cursor: pointer; margin-bottom: 4px; font-size: 12px; font-weight: 600; color: var(--text-muted); overflow: hidden;",
            title: "{name}",
            onclick: move |_| {
                ui.selected_space_id.set(Some(sid.clone()));
                ui.selected_room_id.set(None);
            },
            if has_avatar {
                img {
                    src: "{avatar_src.read().as_ref().and_then(|v| v.as_ref()).unwrap()}",
                    style: "width: 100%; height: 100%; object-fit: cover;",
                }
            } else {
                "{initials}"
            }
        }
    }
}
