use dioxus::prelude::*;
use std::sync::Arc;
use fluxchat_core::AppState;

const CATEGORIES: &[(&str, &str, &[&str])] = &[
    ("Smileys", "😀", &[
        "😀", "😃", "😄", "😁", "😆", "😅", "🤣", "😂", "🙂", "😊",
        "😇", "🥰", "😍", "🤩", "😘", "😗", "😚", "😙", "🥲", "😋",
        "😛", "😜", "🤪", "😝", "🤑", "🤗", "🤭", "🤫", "🤔", "🫡",
        "🤐", "🤨", "😐", "😑", "😶", "🫥", "😏", "😒", "🙄", "😬",
        "😌", "😔", "😪", "🤤", "😴", "😷", "🤒", "🤕",
        "🤢", "🤮", "🥵", "🥶", "🥴", "😵", "🤯", "🤠", "🥳", "🥸",
        "😎", "🤓", "🧐", "😕", "😟", "🙁", "😮", "😯", "😲",
        "😳", "🥺", "🥹", "😦", "😧", "😨", "😰", "😥", "😢", "😭",
        "😱", "😖", "😣", "😞", "😓", "😩", "😫", "🥱", "😤", "😡",
        "😠", "🤬", "😈", "👿", "💀", "☠️", "💩", "🤡", "👹", "👺",
    ]),
    ("Gestures", "👍", &[
        "👍", "👎", "👊", "✊", "🤛", "🤜", "👏", "🙌", "🫶", "👐",
        "🤲", "🤝", "🙏", "✌️", "🤞", "🫰", "🤟", "🤘", "🤙", "👈",
        "👉", "👆", "👇", "☝️", "✋", "🤚", "🖐️", "🖖", "🫱", "🫲",
        "👋", "🤏", "💪", "🦾", "🖕", "✍️", "🤳", "💅",
    ]),
    ("Hearts", "❤️", &[
        "❤️", "🧡", "💛", "💚", "💙", "💜", "🖤", "🤍", "🤎", "💔",
        "❤️‍🔥", "❤️‍🩹", "💕", "💞", "💓", "💗", "💖", "💘", "💝", "💟",
    ]),
    ("Objects", "🎉", &[
        "🎉", "🎊", "🎈", "🎁", "🏆", "🥇", "🥈", "🥉", "⚽", "🏀",
        "🎮", "🎯", "🎨", "🎭", "🎬", "🎤", "🎧", "🎵", "🎶", "🎹",
        "🔥", "⭐", "🌟", "💫", "✨", "⚡", "💥", "💯", "💢", "💤",
        "🔔", "🔕", "📢", "📣", "💬", "💭", "♠️", "♥️", "♦️",
        "♣️", "🃏", "🎲", "🧩", "🔮",
    ]),
    ("Food", "🍕", &[
        "🍎", "🍊", "🍋", "🍌", "🍉", "🍇", "🍓", "🫐", "🍒", "🍑",
        "🥭", "🍍", "🥝", "🍅", "🥑", "🌶️", "🌽", "🥕", "🧅", "🥔",
        "🍕", "🍔", "🍟", "🌭", "🍿", "🧂", "🥤", "☕", "🍺", "🍻",
        "🥂", "🍷", "🍸", "🍹", "🧋", "🍩", "🍰", "🎂", "🍫", "🍬",
    ]),
    ("Nature", "🌿", &[
        "🐶", "🐱", "🐭", "🐹", "🐰", "🦊", "🐻", "🐼", "🐨", "🐯",
        "🦁", "🐮", "🐷", "🐸", "🐵", "🙈", "🙉", "🙊", "🐔", "🐧",
        "🦄", "🐝", "🐛", "🦋", "🐌", "🐞", "🐢", "🐍", "🦎", "🦖",
        "🌸", "🌺", "🌻", "🌹", "🌷", "🌱", "🌿", "🍀", "🍁", "🍂",
        "🌊", "🌈", "☀️", "🌙", "🌍", "🌎", "🌏",
    ]),
    ("Symbols", "💯", &[
        "✅", "❌", "❓", "❗", "‼️", "⁉️", "⚠️", "🚫", "♻️", "✳️",
        "❇️", "🔰", "⭕", "✔️", "☑️", "➡️", "⬅️", "⬆️", "⬇️", "↩️",
        "↪️", "🔄", "🔃", "🔙", "🔚", "🔛", "🔜", "🔝",
        "🔴", "🟠", "🟡", "🟢", "🔵", "🟣", "⚫", "⚪", "🟤",
    ]),
];

/// Load frequently used emojis from settings.
fn load_frequent(app_state: &Arc<AppState>) -> Vec<String> {
    app_state.db.get_setting("frequent_emoji")
        .and_then(|json| serde_json::from_str::<Vec<String>>(&json).ok())
        .unwrap_or_default()
}

/// Record an emoji usage.
fn record_usage(app_state: &Arc<AppState>, emoji: &str) {
    let mut freq = load_frequent(app_state);
    // Move to front if exists, or insert at front
    freq.retain(|e| e != emoji);
    freq.insert(0, emoji.to_string());
    freq.truncate(32); // keep top 32
    if let Ok(json) = serde_json::to_string(&freq) {
        let _ = app_state.db.set_setting("frequent_emoji", &json);
    }
}

#[component]
pub fn EmojiPicker(on_select: EventHandler<String>) -> Element {
    let app_state = use_context::<Arc<AppState>>();
    let mut search = use_signal(|| String::new());
    let mut active_cat = use_signal(|| 0usize); // 0 = frequent
    let frequent = load_frequent(&app_state);

    let query = search().to_lowercase();
    let cat_idx = *active_cat.read();

    rsx! {
        div { style: "width: 352px; background: var(--bg-dark); border: 1px solid var(--border); border-radius: 8px; overflow: hidden; display: flex; flex-direction: column; height: 340px;",
            onclick: move |e| { e.stop_propagation(); },
            // Search bar
            div { style: "padding: 8px 8px 4px; flex-shrink: 0;",
                input {
                    style: "width: 100%; background: var(--bg-deep); border: 1px solid var(--border); border-radius: 4px; padding: 6px 8px; color: var(--text-bright); font-family: var(--font-mono); font-size: 12px; outline: none;",
                    placeholder: "search emoji...",
                    value: "{search}",
                    oninput: move |e| search.set(e.value()),
                }
            }
            // Category tabs — horizontal strip with emoji icons
            div { style: "display: flex; padding: 2px 6px; gap: 1px; border-bottom: 1px solid var(--border); flex-shrink: 0;",
                // Frequent tab
                { let freq_op = if cat_idx == 0 && query.is_empty() { "1" } else { "0.5" };
                rsx! {
                span {
                    style: "padding: 4px 6px; cursor: pointer; font-size: 14px; border-radius: 4px; opacity: {freq_op};",
                    class: "cmd-item",
                    onclick: move |_| { active_cat.set(0); search.set(String::new()); },
                    "🕐"
                } } }
                for (i, (_, icon, _)) in CATEGORIES.iter().enumerate() {
                    { let op = if (i + 1) == cat_idx && query.is_empty() { "1" } else { "0.5" };
                    rsx! {
                        span {
                            style: "padding: 4px 6px; cursor: pointer; font-size: 14px; border-radius: 4px; opacity: {op};",
                            class: "cmd-item",
                            onclick: move |_| { active_cat.set(i + 1); search.set(String::new()); },
                            "{icon}"
                        }
                    } }
                }
            }
            // Emoji grid
            div { style: "flex: 1; overflow-y: auto; padding: 6px;",
                if !query.is_empty() {
                    // Search results — show all matching
                    div { style: "display: flex; flex-wrap: wrap; gap: 1px;",
                        for emoji in CATEGORIES.iter().flat_map(|(_, _, e)| e.iter()) {
                            span {
                                style: "font-size: 22px; padding: 3px; cursor: pointer; border-radius: 4px; line-height: 1;",
                                class: "cmd-item",
                                onclick: {
                                    let e = emoji.to_string();
                                    let app_state = app_state.clone();
                                    move |_| { record_usage(&app_state, &e); on_select.call(e.clone()); }
                                },
                                "{emoji}"
                            }
                        }
                    }
                } else if cat_idx == 0 {
                    // Frequently used
                    if frequent.is_empty() {
                        div { style: "color: var(--text-dim); font-size: 12px; font-family: var(--font-mono); padding: 12px; text-align: center;",
                            "no frequently used emoji yet"
                        }
                    } else {
                        div { style: "display: flex; flex-wrap: wrap; gap: 1px;",
                            for emoji in frequent.iter() {
                                span {
                                    style: "font-size: 22px; padding: 3px; cursor: pointer; border-radius: 4px; line-height: 1;",
                                    class: "cmd-item",
                                    onclick: {
                                        let e = emoji.clone();
                                        let app_state = app_state.clone();
                                        move |_| { record_usage(&app_state, &e); on_select.call(e.clone()); }
                                    },
                                    "{emoji}"
                                }
                            }
                        }
                    }
                } else {
                    // Category grid
                    { let (cat_name, _, emojis) = CATEGORIES[(cat_idx - 1) % CATEGORIES.len()];
                    rsx! {
                        div { style: "font-size: 10px; font-weight: 700; text-transform: uppercase; color: var(--text-dim); letter-spacing: 0.5px; padding: 4px 2px 6px; font-family: var(--font-mono);",
                            "{cat_name}"
                        }
                        div { style: "display: flex; flex-wrap: wrap; gap: 1px;",
                            for emoji in emojis.iter() {
                                span {
                                    style: "font-size: 22px; padding: 3px; cursor: pointer; border-radius: 4px; line-height: 1;",
                                    class: "cmd-item",
                                    onclick: {
                                        let e = emoji.to_string();
                                        let app_state = app_state.clone();
                                        move |_| { record_usage(&app_state, &e); on_select.call(e.clone()); }
                                    },
                                    "{emoji}"
                                }
                            }
                        }
                    } }
                }
            }
        }
    }
}
