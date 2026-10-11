# Panes and windows

fluxchat can split its main area into **panes**, tmux-style, and keep several layouts as **windows**.

## Panes

Each pane shows a room or a thread. One pane at a time is **focused** (outlined in the accent colour). It shows the full room view: header, panels and the main message box. The other panes show their messages live, each with its own message box.

- **Focus** a pane by clicking it; the cursor goes to its message box. Buttons in an unfocused pane work on the first click; acting on a message there (react, reply, edit, delete, pin) focuses that pane first, so the action applies to its room. Hovering never moves focus. Clicking a room in the room list (or picking one with **Ctrl/⌘+K**) puts it in the focused pane, unless the window has an empty pane: then it goes there.
- **Split**:
  - the split button in the room header;
  - the split buttons in a pane's title bar;
  - **Ctrl+B %** (side by side) or **Ctrl+B "** (stacked);
  - the palette (**Ctrl/⌘+K** → "Split pane").

  You stay in the room you were reading. The new pane is empty and lists rooms to put in it: click one, or **Search all rooms** to find any room. Picking a room there, or from the room list, fills that pane and focuses it. Rooms already open in a pane aren't listed.
- **Open a room or thread beside the current one**:
  - right-click it in the room list (or a thread anywhere) → **Open to the right**, **Open below** or **Open in new window**;
  - **middle-click** it, or **⌘-click** (Ctrl-click on Linux/Windows): opens to the right;
  - **drag** it from the room list or the threads panel onto a pane. Drop near an edge to split that side, or in the middle to show it in that pane. The area it will fill is shaded while you drag.
- **Threads** open in a pane beside their room; each thread you click gets a pane of its own (an open thread pane is never replaced). The full rules are in [Threads](threads.md#reading-and-replying).
- **Already open?** Clicking a room or thread that's already in a pane, in this window or another, takes you there instead of opening a second copy.
- **Resize** by dragging the gap between panes.
- **Rearrange** by dragging a pane's title bar onto another pane or an edge.
- **Zoom** a pane to fill the window and back: **Ctrl+B z**, or the title bar's zoom button. Zoom ends by itself when you move to another pane or open something beside it, so nothing opens out of sight. A zoomed pane stays zoomed after a restart.
- **Close** a pane: **Ctrl+B x**, or its ×. Closing a window's last pane closes the window.

When a window has more than one pane, each pane gets a title bar: drag handle, name, and split / zoom / close buttons. The focused pane's title bar is its room header (name, agent chips, search, threads, members…), so there's one bar, not two; in a narrow pane it keeps search, threads and members, and the agent chip shrinks to its icon and state (hover for the rest). In the room list, the focused room is highlighted and rooms in the other panes get a lighter highlight. You aren't sent notifications for any room on screen.

**Esc** in a thread pane, with nothing else to close, goes back to the thread's room if it's in another pane. The thread pane stays open.

A non-focused pane shows a room's latest 300 messages; focus it to scroll through the full history.

When you leave a room (or are removed from it), its panes and its threads' panes close in every window. If it was the only pane, the next room in the list takes its place.

Need more space for panes? Hide the room list (**Ctrl/⌘+[**) and the member list (**Ctrl/⌘+]**).

## Windows

Windows are separate pane layouts, like tmux windows or browser tabs.

- **New window**: **Ctrl+B c**. It starts empty, showing the welcome view; pick a room from the sidebar or **Ctrl/⌘+K**. Your unsent text in the window you left is kept.
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
