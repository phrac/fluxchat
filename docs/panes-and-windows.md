# Panes and windows

fluxchat can split its main area into **panes**, tmux-style, and keep several layouts as **windows**.

## Panes

Each pane shows a room or a thread. One pane at a time is **focused** (outlined in the accent colour). It shows the full room view: header, panels and the main message box. The other panes show their messages live, each with its own message box.

- **Focus** a pane by clicking it; the cursor goes to its message box. Buttons in an unfocused pane work on the first click. The room list then fills the focused pane: clicking a room puts it there.
- **Split**:
  - the split button in the room header;
  - the split buttons in a pane's title bar;
  - **Ctrl+B %** (side by side) or **Ctrl+B "** (stacked);
  - the palette (**Ctrl/⌘+K** → "Split pane").

  A new, empty pane opens the room picker so you can choose what goes in it.
- **Open a room beside the current one**: right-click it in the room list → **Open in split view**.
- **Threads** open in a pane beside their room automatically.
- **Resize** by dragging the gap between panes.
- **Rearrange** by dragging a pane's title bar onto another pane or an edge.
- **Zoom** a pane to fill the window and back: **Ctrl+B z**, or the title bar's zoom button.
- **Close** a pane: **Ctrl+B x**, or its ×. Closing a window's last pane closes the window.

When a window has more than one pane, each pane gets a title bar: drag handle, name, and split / zoom / close buttons.

A non-focused pane shows a room's latest 300 messages; focus it to scroll through the full history.

## Windows

Windows are separate pane layouts, like tmux windows or browser tabs.

- **New window**: **Ctrl+B c**. It starts empty and opens the room picker.
- **Switch**: **Ctrl+B n** / **p** (next / previous), **Ctrl+B 0–9**, or click it in the status bar (`0:general* 1:dev`).
- **Close the current window**: **Ctrl+B &**.

## Keys (tmux defaults)

Press **Ctrl+B**, release, then the key. While waiting for the key, `[prefix]` shows in the status bar; **Esc** cancels.

| Key | Action |
|-----|--------|
| `%` | Split side by side |
| `"` | Split stacked |
| `x` | Close pane |
| `z` | Zoom / restore pane |
| `o` | Next pane |
| ← → ↑ ↓ or `h` `l` `k` `j` | Move focus |
| `c` | New window |
| `n` / `p` | Next / previous window |
| `0`–`9` | Go to window |
| `&` | Close window |
| `?` | Show these keys |

The key after Ctrl+B is never typed into the message box.

## Saved layout

All windows, their panes, sizes and focused panes are saved and restored when fluxchat starts. A pane whose room you've since left comes back empty.
