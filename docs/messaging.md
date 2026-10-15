# Messaging

## Writing

Type in the box at the bottom of the room and press **Enter** to send. **Shift+Enter** adds a new line. Prefer **Ctrl/⌘+Enter** to send? Turn on *Send with Ctrl+Enter* in [Preferences](settings.md#preferences).

Unsent text is kept per room, so switching rooms doesn't lose a half-written message. **Ctrl+U** clears the box.

## Formatting (markdown)

| You type | You get |
|----------|---------|
| `**bold**` | **bold** |
| `*italic*` | *italic* |
| `~~strike~~` | ~~strike~~ |
| `` `code` `` | `code` |
| ```` ```rust ```` … ```` ``` ```` | a highlighted code block (100+ languages) |
| `# Heading` … `###### Heading` | headings |
| `- item` | bulleted list |
| `$x^2$` | inline maths (converted to Unicode) |

Code blocks longer than five lines are shown collapsed, each with its own **Show more** / **Show less** (Preferences → *Expand code blocks* flips the default). Every code block has a **copy** button at its top right, which copies the whole block, collapsed or not. Diffs are highlighted, tagged ```` ```diff ```` or not.

While there's text in the message box, buttons for bold, italic, strikethrough, code, code block and link appear in it (in place of the microphone and location buttons) and insert these for you. To send text exactly as typed, turn off *Markdown* in Preferences, or use `/plain`. `/html` sends raw HTML.

To send a message that starts with `/` (not a command), start it with `//`. For example, `//shrug` sends `/shrug`.

## Mentions

Type `@` and part of a name to get suggestions; **Tab** or click to insert. Mentions become clickable pills. `@room` notifies everyone, if you're allowed to.

## Emoji

- **Shortcodes**: type `:` and two or more letters (`:smi`) for suggestions; **Tab**/**Enter** inserts.
- **Picker**: the smiley button opens the emoji picker. It has a search box, ten category tabs (Frequently used, Smileys, People, Animals, Food, Travel, Activities, Objects, Symbols, Flags) and remembers the ones you use most.
- **Emoticons**: with *Replace emoticons* on, `:)` `<3` and similar become emoji when you send.
- Messages made only of emoji are shown large (turn off with *Big emoji*).

## Replies, edits, deletions

- **Reply**: hover a message and click the reply arrow. The quote is shown above your message; click a quote to jump to the original.
- **Edit**: hover your message and click the pencil, or press **↑** in an empty message box to edit your last message. Edited messages say *(edited)*.
- **Delete**: hover your message and click the trash can. With *Deleted messages* on in Preferences, a "Message deleted" placeholder remains.
- **Sending and retry**: a message on its way shows *sending… [cancel]*, and one that failed shows *failed to send [retry] [delete]*. **cancel** and **delete** take it back before it reaches anyone.
  - After a network error, fluxchat retries by itself, less often the longer it keeps failing.
  - After any other error, the failed message is set aside, so the messages after it still go out.
  - Messages still unsent when you quit go out the next time you start fluxchat.

## Reactions

Hover a message and click the smiley to pick a reaction, or click an existing reaction pill to add or remove yours. Hover a reaction to see who reacted.

## Files, images and captions

- Attach with the **+** button, drag and drop a file, paste (**Ctrl/⌘+V**) an image or a file you copied in your file manager, or `/attach`. Dragging files in doesn't work on Linux under Wayland yet; copy and paste them instead (see [Troubleshooting](troubleshooting.md#cant-drop-files-in-on-wayland)).
- The file appears as a card above the message box. Anything you type is sent **with** the file as its caption: one message, not two. Press **Enter** (or **Send**) to send. With an empty box, just the file is sent.
- Images get a thumbnail. Click an image to view it large, where you can **Copy** or **Save** it. Click the image again or press **Esc** to close. GIFs animate there even with *Autoplay GIFs* off. Very large animations (over 128 MB once decoded) show as a still image.
- Files show as download cards; PDFs show their first page on the card. Videos play inline.
- Files over your server's upload limit are rejected with a message before uploading.

## Gallery

The grid button in the room header (or **Ctrl/⌘+Shift+G**, `/gallery`, or *Gallery* in the command palette) shows every image and video in the room, or in the thread you're viewing, as thumbnails grouped by month, newest first. The **Files** tab (`/files`) lists files and audio with their size, sender and date, each with **Download** and **Jump to message**.

- It opens with what's already loaded, then fetches older history as you scroll, back to the start of the room. In a room's gallery, images posted in its threads are included too.
- In an encrypted room the server can't tell which messages have attachments, so fluxchat reads through the history itself, which takes longer. It stops after about 2,000 messages at a time; click **Load older** to keep going.
- Click an image to view it large, with **←** / **→** (or the arrow buttons) to step through the gallery, and **Jump to message** to see it in the conversation. **Esc** or clicking the image goes back to the gallery.
- Click a video to go to its message, where it plays.
- Media hidden by your *Show media* setting stays covered: click once to show it, again to open it.
- Thumbnails only download as they come into view. GIFs animate in the grid when *Autoplay GIFs* is on, and always when opened.
- **Jump to message** loads older history until it reaches the message. A thread reply opens its thread.

## GIFs

`/gif <search>` opens a GIF picker above the message box; click a GIF to send it. A Giphy API key is required: set it once with `/giphykey <key>` (or the `GIPHY_API_KEY` environment variable).

## Polls

`/poll Question | Option 1 | Option 2 | …` creates a poll. Click an option to vote; results show as bars. `/pollend` ends your latest poll.

## Location

`/location <lat> <lon> [description]` shares a location card with an embedded map preview (OpenStreetMap tiles), the coordinates and a link to OpenStreetMap. Map previews are fetched once and cached; without a network the card still shows coordinates and the link.

## Voice messages

**Hold** the microphone in the message box to talk: the message is sent when you let go. Or **tap** it to record, tap again to stop, listen, then send. In a thread view it goes into the thread. Voice messages play inline with a progress bar.

## Links

Addresses in messages are clickable even when the sender didn't link them (`https://…`, `www.…`, or `example.com/path`), formatted messages from bots and agents included. Web and email links open in your browser or mail app. Links to rooms, messages and people (matrix.to) open inside fluxchat. Other kinds of links (local files, app links) aren't opened, for safety.

## Copying text

Drag across a message, or across several, and press Ctrl/⌘+C. You get the text as shown, without markdown markers. On Linux the selection is also put in the primary selection (middle-click paste).

## Forwarding and pins

- **Forward**: hover a message → forward arrow → pick a room.
- **Pin**: hover → pin. The room header's pin button opens the Pinned sidebar (the button is red when the room has pins). Click a pin to jump to it, or its pin button to unpin.

## Search

Click the search button (or **Ctrl/⌘+F**), type and press **Enter**. Choose a result with **↑ ↓** and **Enter**; the timeline scrolls to it and highlights it.

## Read receipts and typing

Small avatars under a message show who has read up to there; hover for names. "ann is typing…" appears above the message box, by display name, in a line that's always there so the timeline doesn't move when someone starts or stops. Both can be hidden, and you can stop sending your own, in [Preferences](settings.md#preferences).
