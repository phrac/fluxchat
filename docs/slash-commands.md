# Slash commands

Type `/` in the message box for suggestions; **Tab** cycles through them. `/help [command]` lists commands or explains one. To send text that starts with `/`, start it with `//`.

Commands marked *room* need a room open. Where a command takes `<user>`, you can use a display name (spaces are fine: `/kick Bob Smith spamming` kicks "Bob Smith" with the reason "spamming") or a full `@user:server` ID.

## Rooms

| Command | Does |
|---------|------|
| `/join <room>` | Join a room by address or ID |
| `/create [name]` | Create a room |
| `/leave`, `/part` | Leave the current room *(room)* |
| `/knock <room> [reason]` | Ask to join a room |
| `/topic [new topic]` | Show or set the topic *(room)* |
| `/roomname <name>` | Set the room name *(room)* |
| `/invite <user>` | Invite someone *(room)* |
| `/directory [query]` | Browse public rooms |
| `/upgraderoom <version>` | Upgrade the room *(room)* |
| `/rotatekeys` | Start a new encryption session *(room)* |
| `/mute`, `/unmute` | Mute / unmute room notifications *(room)* |

## People

| Command | Does |
|---------|------|
| `/msg <user> <message>` | Send a direct message |
| `/query <user>` | Open a DM |
| `/whois <user>` | Show someone's info |
| `/users [query]` | Search the user directory |
| `/ignore <user>`, `/unignore <user>` | Hide / show someone's messages |
| `/nick <name>` | Change your display name |
| `/status [message]` | Set your status message |
| `/away [message]` | Mark yourself away |

## Moderation *(room)*

| Command | Does |
|---------|------|
| `/kick <user> [reason]` | Remove someone |
| `/ban <user> [reason]` | Ban someone |
| `/unban <user>` | Lift a ban |
| `/op <user> [level]` | Set someone's power level, 0–100 (default 50, moderator) |
| `/deop <user>` | Reset someone to member |
| `/report <event_id> [reason]` | Report a message |

## Messages *(room)*

| Command | Does |
|---------|------|
| `/me <action>` | Send an emote ("* you waves") |
| `/spoiler <text>` | Send a spoiler |
| `/plain <text>` | Send without markdown |
| `/html <html>` | Send raw HTML |
| `/attach` | Attach a file |
| `/forward` | Forward a message |
| `/poll <question> \| <opt1> \| <opt2>…` | Create a poll |
| `/pollend` | End your latest poll |
| `/location <lat> <lon> [description]` | Share a location |
| `/gif <query>` | Search and send a GIF |
| `/giphykey <key>` | Save your Giphy API key |
| `/shrug`, `/tableflip`, `/unflip`, `/lenny` `[text]` | Append ¯\\_(ツ)_/¯, (╯°□°)╯︵ ┻━┻, ┬─┬ノ( º _ ºノ), ( ͡° ͜ʖ ͡°) |
| `/slap <user>`, `/troutslap <user>` | Slap someone with a trout |

## View *(mostly room)*

| Command | Does |
|---------|------|
| `/threads` | Threads sidebar |
| `/members` | Member list |
| `/search` | Search |
| `/pins` | Pinned messages sidebar |
| `/gallery` | [Gallery](messaging.md#gallery) of images and videos |
| `/files` | Gallery, Files tab |
| `/notices` | Server notices panel |
| `/clear` | Clear the visible scrollback (it comes back on the next update) |
| `/refresh` | Rebuild the room's timeline |
| `/timestamp [short\|full\|off]` | Timestamp format |
| `/compact` | Toggle compact layout |
| `/joinpart` | Toggle join/leave messages |
| `/zoom [50-200]` | Interface zoom |
| `/highlight <word>`, `/unhighlight <word>` | Local highlight words |

## App

| Command | Does |
|---------|------|
| `/help [command]` | Help |
| `/settings`, `/account`, `/sessions`, `/security` | Open Settings (at a tab) |
| `/devtools` | Room state inspector and raw event sender |
| `/quit` | Log out |
