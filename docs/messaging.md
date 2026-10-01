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

The toolbar under the message box inserts these for you. To send text exactly as typed, turn off *Markdown* in Preferences, or use `/plain`. `/html` sends raw HTML.

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
- **Retry**: a message that failed to send shows *failed to send [retry]*.

## Reactions

Hover a message and click the smiley to pick a reaction, or click an existing reaction pill to add or remove yours. Hover a reaction to see who reacted.

## Files, images and captions

- Attach with the **+** button, drag and drop a file, paste an image (**Ctrl/⌘+V**), or `/attach`.
- The file appears as a card above the message box. Anything you type is sent **with** the file as its caption: one message, not two. Press **Enter** (or **Send**) to send. With an empty box, just the file is sent.
- Images get a thumbnail. Click an image to view it large, where you can **Copy** or **Save** it. Click the image again or press **Esc** to close.
- Files show as download cards. Videos play inline.
- Files over your server's upload limit are rejected with a message before uploading.

## GIFs

`/gif <search>` opens a GIF picker above the message box; click a GIF to send it. A Giphy API key is required: set it once with `/giphykey <key>` (or the `GIPHY_API_KEY` environment variable).

## Polls

`/poll Question | Option 1 | Option 2 | …` creates a poll. Click an option to vote; results show as bars. `/pollend` ends your latest poll.

## Location

`/location <lat> <lon> [description]` shares a location card with a link to OpenStreetMap.

## Voice messages

Click the microphone in the message box to record, click again to stop, preview, then send. Voice messages play inline with a progress bar.

## Links

Web and email links open in your browser or mail app. Links to rooms, messages and people (matrix.to) open inside fluxchat. Other kinds of links (local files, app links) aren't opened, for safety.

## Forwarding and pins

- **Forward**: hover a message → forward arrow → pick a room.
- **Pin**: hover → pin. The room header's pin button lists pinned messages (red when there are any).

## Search

Click the search button (or **Ctrl/⌘+F**), type and press **Enter**. Choose a result with **↑ ↓** and **Enter**; the timeline scrolls to it and highlights it.

## Read receipts and typing

Small avatars under a message show who has read up to there; hover for names. "X is typing…" appears above the message box. Both can be hidden, and you can stop sending your own, in [Preferences](settings.md#preferences).
