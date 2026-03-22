use dioxus::prelude::*;
use crate::state::UiState;

#[component]
pub fn SpaceSidebar() -> Element {
    let ui = use_context::<UiState>();
    let spaces = ui.spaces.read().clone();
    let selected = ui.selected_space_id.read().clone();

    rsx! {
        div { style: "width: 56px; flex-shrink: 0; background: var(--bg-deep); display: flex; flex-direction: column; align-items: center; padding: 8px 0; overflow-y: auto; border-right: 1px solid var(--border);",
            // Home button
            div {
                style: "width: 40px; height: 40px; border-radius: {if selected.is_none() { \"12px\" } else { \"20px\" }}; background: {if selected.is_none() { \"var(--accent)\" } else { \"var(--bg-mid)\" }}; display: flex; align-items: center; justify-content: center; cursor: pointer; margin-bottom: 8px; transition: border-radius 0.2s; color: {if selected.is_none() { \"white\" } else { \"var(--text-muted)\" }};",
                onclick: move |_| {
                    ui.selected_space_id.set(None);
                    ui.selected_room_id.set(None);
                },
                "⌂"
            }

            // Separator
            div { style: "width: 24px; height: 2px; background: var(--border); margin-bottom: 8px; border-radius: 1px;" }

            // Space icons
            for space in spaces.iter() {
                { let space_id = space.room_id.clone();
                  let is_active = selected.as_ref() == Some(&space.room_id);
                  let initials: String = space.name.split_whitespace().take(2).map(|w| w.chars().next().unwrap_or('?')).collect::<String>().to_uppercase();
                  rsx! {
                    div {
                        style: "width: 40px; height: 40px; border-radius: {if is_active { \"12px\" } else { \"20px\" }}; background: var(--bg-mid); display: flex; align-items: center; justify-content: center; cursor: pointer; margin-bottom: 4px; font-size: 12px; font-weight: 600; color: var(--text-muted); transition: border-radius 0.2s;",
                        onclick: move |_| {
                            ui.selected_space_id.set(Some(space_id.clone()));
                            ui.selected_room_id.set(None);
                        },
                        "{initials}"
                    }
                  }
                }
            }
        }
    }
}
