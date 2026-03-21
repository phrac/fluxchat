import { Show, createSignal, createEffect, untrack } from "solid-js";
import { marked } from "marked";
import type { MessageInfo } from "../../lib/types";
import { getMediaUrl } from "../../lib/tauri";
import { setReplyingTo, insertSystemMessage } from "../../stores/messages";
import { sendReaction, deleteMessage, pinMessage } from "../../lib/tauri";
import { selectedRoomId } from "../../stores/rooms";

// Configure marked: only standard markdown, no GFM extensions that cause false positives
marked.setOptions({
  breaks: true,
  gfm: false, // Disable GFM to avoid ~strikethrough~ on chat text
});

interface Props {
  message: MessageInfo;
  showHeader: boolean;
}

function nameColor(name: string): string {
  const colors = [
    "#e74c3c", "#e67e22", "#f1c40f", "#2ecc71", "#1abc9c",
    "#3498db", "#9b59b6", "#e91e63", "#e57e31", "#71a6d2",
    "#ed4245", "#faa61a", "#57f287", "#5865f2", "#eb459e",
  ];
  let hash = 0;
  for (let i = 0; i < name.length; i++) {
    hash = name.charCodeAt(i) + ((hash << 5) - hash);
  }
  return colors[Math.abs(hash) % colors.length];
}

function formatTime(timestamp: number): string {
  const date = new Date(timestamp * 1000);
  const now = new Date();
  const isToday = date.toDateString() === now.toDateString();

  const time = date.toLocaleTimeString([], {
    hour: "2-digit",
    minute: "2-digit",
  });

  if (isToday) return time;

  const yesterday = new Date(now);
  yesterday.setDate(yesterday.getDate() - 1);
  if (date.toDateString() === yesterday.toDateString()) {
    return `Yesterday ${time}`;
  }

  return `${date.toLocaleDateString([], { month: "short", day: "numeric" })} ${time}`;
}

function avatarInitials(name: string): string {
  const parts = name.replace(/^@/, "").split(/[\s:]/);
  return (parts[0]?.[0] || "?").toUpperCase();
}

function renderBodySync(body: string, formattedBody: string | null): string {
  if (formattedBody) {
    // Strip Matrix reply quotes — we render our own reply context
    return formattedBody.replace(/<mx-reply>[\s\S]*?<\/mx-reply>/gi, "").trim();
  }
  // Strip plain text reply fallback (> quoted lines at start)
  if (body.startsWith("> ")) {
    const lines = body.split("\n");
    let i = 0;
    while (i < lines.length && lines[i].startsWith("> ")) i++;
    // Skip the blank line after the quote
    if (i < lines.length && lines[i].trim() === "") i++;
    body = lines.slice(i).join("\n");
  }
  try {
    const html = marked.parse(body, { async: false }) as string;
    const trimmed = html.trim();
    if (trimmed.startsWith("<p>") && trimmed.endsWith("</p>") && trimmed.indexOf("<p>", 1) === -1) {
      return trimmed.slice(3, -4);
    }
    return trimmed;
  } catch {
    return body;
  }
}

// Cache resolved media URLs in memory
const mediaCache = new Map<string, string>();

async function resolveMedia(mxc: string): Promise<string | null> {
  const cached = mediaCache.get(mxc);
  if (cached) return cached;
  try {
    const dataUri = await getMediaUrl(mxc);
    mediaCache.set(mxc, dataUri);
    return dataUri;
  } catch {
    return null;
  }
}

// Resolve all mxc:// URLs in HTML content (for inline custom emotes etc.)
async function resolveHtmlMedia(html: string): Promise<string> {
  const mxcPattern = /mxc:\/\/[^\s"'<>]+/g;
  const mxcUrls = [...new Set(html.match(mxcPattern) || [])];
  if (mxcUrls.length === 0) return html;

  let result = html;
  await Promise.all(
    mxcUrls.map(async (mxc) => {
      const resolved = await resolveMedia(mxc);
      if (resolved) {
        result = result.replaceAll(mxc, resolved);
      }
    })
  );
  return result;
}

function ImageMessage(props: { mxc: string; alt: string }) {
  const [src, setSrc] = createSignal<string | null>(null);
  const [expanded, setExpanded] = createSignal(false);

  createEffect(() => {
    resolveMedia(props.mxc).then(setSrc);
  });

  return (
    <Show when={src()} fallback={<span style={{ color: "var(--text-muted)" }}>Loading image...</span>}>
      <img
        src={src()!}
        alt={props.alt}
        class="mt-1 rounded cursor-pointer object-contain transition-all"
        style={{
          "max-width": expanded() ? "500px" : "200px",
          "max-height": expanded() ? "400px" : "150px",
        }}
        onClick={() => setExpanded(!expanded())}
      />
    </Show>
  );
}

function Avatar(props: { mxcUrl: string | null; name: string; color: string; size?: string }) {
  const sz = props.size || "h-10 w-10";
  const textSz = props.size === "h-8 w-8" ? "text-xs" : "text-sm";
  const [resolved, setResolved] = createSignal<string | null>(null);

  createEffect(() => {
    const mxc = props.mxcUrl;
    if (!mxc) {
      setResolved(null);
      return;
    }
    resolveMedia(mxc).then((uri) => setResolved(uri));
  });

  return (
    <Show
      when={resolved()}
      fallback={
        <div
          class={`flex ${sz} flex-shrink-0 items-center justify-center rounded-full ${textSz} font-medium text-white`}
          style={{ background: props.color }}
        >
          {avatarInitials(props.name)}
        </div>
      }
    >
      <img
        src={resolved()!}
        alt={props.name}
        class={`${sz} flex-shrink-0 rounded-full object-cover`}
      />
    </Show>
  );
}

// Cache rendered HTML to avoid re-parsing on room switch
const htmlCache = new Map<string, string>();

function MessageContent(props: { message: MessageInfo }) {
  const cached = htmlCache.get(props.message.event_id);
  const [html, setHtml] = createSignal(
    cached || renderBodySync(props.message.body, props.message.formatted_body)
  );

  // Resolve mxc:// URLs in formatted HTML (custom emotes, inline images)
  if (!cached) {
    const raw = renderBodySync(props.message.body, props.message.formatted_body);
    if (raw.includes("mxc://")) {
      resolveHtmlMedia(raw).then((resolved) => {
        htmlCache.set(props.message.event_id, resolved);
        setHtml(resolved);
      });
    } else {
      htmlCache.set(props.message.event_id, raw);
    }
  }

  return (
    <>
      <div
        ref={(el) => {
          el.addEventListener("click", (e) => {
            const target = e.target as HTMLElement;
            if (target.tagName === "IMG" && !target.hasAttribute("data-mx-emoticon")) {
              target.classList.toggle("expanded");
            }
          });
        }}
        class="msg-content leading-[1.375rem]"
        style={{ color: "var(--text-normal)", "font-size": "1rem" }}
        innerHTML={html()}
      />
      <Show when={props.message.image_mxc}>
        <ImageMessage mxc={props.message.image_mxc!} alt={props.message.body} />
      </Show>
      <Show when={props.message.reactions.length > 0}>
        <div class="mt-1 flex flex-wrap gap-1">
          {props.message.reactions.map((r) => (
            <span
              class="inline-flex items-center gap-1 rounded-full px-2 py-0.5 text-xs cursor-pointer hover:brightness-125"
              style={{
                background: "rgba(88, 101, 242, 0.15)",
                border: "1px solid rgba(88, 101, 242, 0.3)",
                color: "var(--text-normal)",
              }}
              title={r.users.join(", ")}
            >
              <span>{r.emoji}</span>
              <span style={{ color: "var(--text-muted)" }}>{r.count}</span>
            </span>
          ))}
        </div>
      </Show>
    </>
  );
}

function ReplyContext(props: { message: MessageInfo }) {
  const replyColor = () => nameColor(props.message.reply_to_sender || "");
  return (
    <Show when={props.message.reply_to_id}>
      <div
        class="mb-0.5 flex items-center gap-1.5 text-xs"
        style={{ "padding-left": "52px" }}
      >
        <div
          class="h-3 w-0.5 rounded-full"
          style={{ background: replyColor() }}
        />
        <span style={{ color: replyColor() }} class="font-medium">
          {props.message.reply_to_sender || "Unknown"}
        </span>
        <span
          class="truncate"
          style={{ color: "var(--text-muted)", "max-width": "400px" }}
        >
          {props.message.reply_to_body || "..."}
        </span>
      </div>
    </Show>
  );
}

export default function MessageBubble(props: Props) {
  // System messages (pins, etc.) render as centered muted text
  if (props.message.event_id.startsWith("~system-")) {
    return (
      <div class="flex items-center justify-center gap-2 py-1 px-4">
        <div class="h-px flex-1" style={{ background: "var(--border-subtle)" }} />
        <span class="text-xs flex-shrink-0" style={{ color: "var(--text-muted)" }}>
          {props.message.body}
        </span>
        <div class="h-px flex-1" style={{ background: "var(--border-subtle)" }} />
      </div>
    );
  }

  const color = () => nameColor(props.message.sender);

  const quickEmojis = ["😂", "🔥", "❤️", "👍", "😮", "😢"];
  const isPending = () => props.message.event_id.startsWith("~local-");
  const [showMenu, setShowMenu] = createSignal(false);

  const handleReply = () => setReplyingTo(props.message);

  const handleReact = (emoji: string) => {
    const roomId = untrack(selectedRoomId);
    if (roomId && !isPending()) {
      sendReaction(roomId, props.message.event_id, emoji).catch(() => {});
    }
  };

  const handleDelete = () => {
    const roomId = untrack(selectedRoomId);
    if (roomId && !isPending()) {
      deleteMessage(roomId, props.message.event_id).catch(() => {});
    }
  };

  const handlePin = async () => {
    const roomId = untrack(selectedRoomId);
    if (roomId && !isPending()) {
      try {
        await pinMessage(roomId, props.message.event_id);
        insertSystemMessage(
          roomId,
          `📌 pinned a message: "${props.message.body.slice(0, 80)}${props.message.body.length > 80 ? "..." : ""}"`
        );
      } catch {}
    }
    setShowMenu(false);
  };

  const handleCopy = () => {
    navigator.clipboard.writeText(props.message.body).catch(() => {});
    setShowMenu(false);
  };

  const handleShowSource = () => {
    const source = JSON.stringify(props.message, null, 2);
    navigator.clipboard.writeText(source).catch(() => {});
    setShowMenu(false);
  };

  return (
    <div
      class="group relative py-0.5 transition-colors"
      classList={{ "mt-4": props.showHeader }}
      data-event-id={props.message.event_id}
      style={{
        "padding-left": props.showHeader ? undefined : "56px",
        opacity: isPending() ? "0.5" : "1",
        transition: "opacity 0.3s, background 0.5s",
      }}
      onMouseOver={(e) => {
        e.currentTarget.style.background = "rgba(0,0,0,0.1)";
        const bar = e.currentTarget.querySelector("[data-action-bar]") as HTMLElement;
        if (bar) bar.style.display = "flex";
        const time = e.currentTarget.querySelector("[data-hover-time]") as HTMLElement;
        if (time) time.style.display = "inline";
      }}
      onMouseOut={(e) => {
        if (showMenu()) return; // Keep visible while menu is open
        e.currentTarget.style.background = "transparent";
        const bar = e.currentTarget.querySelector("[data-action-bar]") as HTMLElement;
        if (bar) bar.style.display = "none";
        const time = e.currentTarget.querySelector("[data-hover-time]") as HTMLElement;
        if (time) time.style.display = "none";
        setShowMenu(false);
      }}
    >
      {/* Action bar on hover */}
      <Show when={!isPending()}>
        <div
          class="absolute right-2 -top-3 items-center rounded shadow-lg z-10"
          style={{
            background: "var(--bg-tertiary)",
            border: "1px solid rgba(0,0,0,0.2)",
            display: "none",
          }}
          data-action-bar
        >
          {quickEmojis.map((emoji) => (
            <button
              class="px-1.5 py-1 text-base transition-transform hover:scale-125 hover:bg-white/5 first:rounded-l"
              onClick={() => handleReact(emoji)}
              title={emoji}
            >
              {emoji}
            </button>
          ))}
          <div class="mx-0.5 h-5 w-px" style={{ background: "var(--border-subtle)" }} />
          <button
            class="flex items-center px-1.5 py-1.5 hover:bg-white/5"
            style={{ color: "var(--text-muted)" }}
            onClick={handleReply}
            title="Reply"
          >
            <svg width="16" height="16" viewBox="0 0 24 24" fill="currentColor">
              <path d="M10 9V5l-7 7 7 7v-4.1c5 0 8.5 1.6 11 5.1-1-5-4-10-11-11z"/>
            </svg>
          </button>
          {/* More options */}
          <div class="relative">
            <button
              class="flex items-center px-1.5 py-1.5 hover:bg-white/5 rounded-r"
              style={{ color: "var(--text-muted)" }}
              onClick={() => setShowMenu(!showMenu())}
              title="More"
            >
              <svg width="16" height="16" viewBox="0 0 24 24" fill="currentColor">
                <path d="M12 8c1.1 0 2-.9 2-2s-.9-2-2-2-2 .9-2 2 .9 2 2 2zm0 2c-1.1 0-2 .9-2 2s.9 2 2 2 2-.9 2-2-.9-2-2-2zm0 6c-1.1 0-2 .9-2 2s.9 2 2 2 2-.9 2-2-.9-2-2-2z"/>
              </svg>
            </button>

            <Show when={showMenu()}>
              <div
                class="absolute right-0 top-full mt-1 w-48 rounded-md py-1 shadow-xl z-20"
                style={{ background: "var(--bg-tertiary)", border: "1px solid rgba(0,0,0,0.3)" }}
              >
                <MenuButton
                  label="Pin Message"
                  icon="M16 12V4h1V2H7v2h1v8l-2 2v2h5.2v6h1.6v-6H18v-2l-2-2z"
                  onClick={handlePin}
                />
                <MenuButton
                  label="Copy Text"
                  icon="M16 1H4c-1.1 0-2 .9-2 2v14h2V3h12V1zm3 4H8c-1.1 0-2 .9-2 2v14c0 1.1.9 2 2 2h11c1.1 0 2-.9 2-2V7c0-1.1-.9-2-2-2zm0 16H8V7h11v14z"
                  onClick={handleCopy}
                />
                <MenuButton
                  label="Show Source"
                  icon="M9.4 16.6L4.8 12l4.6-4.6L8 6l-6 6 6 6 1.4-1.4zm5.2 0l4.6-4.6-4.6-4.6L16 6l6 6-6 6-1.4-1.4z"
                  onClick={handleShowSource}
                />
                <div class="mx-2 my-1 h-px" style={{ background: "var(--border-subtle)" }} />
                <MenuButton
                  label="Delete Message"
                  icon="M6 19c0 1.1.9 2 2 2h8c1.1 0 2-.9 2-2V7H6v12zM19 4h-3.5l-1-1h-5l-1 1H5v2h14V4z"
                  onClick={handleDelete}
                  danger
                />
              </div>
            </Show>
          </div>
        </div>
      </Show>

      <ReplyContext message={props.message} />

      <Show when={props.showHeader}>
        <div class="flex items-start gap-3">
          <Avatar
            mxcUrl={props.message.avatar_url}
            name={props.message.sender_name}
            color={color()}
          />

          <div class="min-w-0 flex-1">
            <div class="flex items-baseline gap-2">
              <span class="font-medium" style={{ color: color() }}>
                {props.message.sender_name}
              </span>
              <span class="text-xs" style={{ color: "var(--text-muted)" }}>
                {formatTime(props.message.timestamp)}
              </span>
            </div>

            <MessageContent message={props.message} />
          </div>
        </div>
      </Show>

      <Show when={!props.showHeader}>
        <span
          class="pointer-events-none absolute left-1 top-1 text-[10px]"
          style={{ color: "var(--text-muted)", display: "none" }}
          data-hover-time
        >
          {new Date(props.message.timestamp * 1000).toLocaleTimeString([], {
            hour: "2-digit",
            minute: "2-digit",
          })}
        </span>
        <MessageContent message={props.message} />
      </Show>
    </div>
  );
}

function MenuButton(props: {
  label: string;
  icon: string;
  onClick: () => void;
  danger?: boolean;
}) {
  return (
    <button
      onClick={props.onClick}
      class="flex w-full items-center justify-between px-3 py-1.5 text-sm transition-colors"
      style={{ color: props.danger ? "var(--red-dnd)" : "var(--text-normal)" }}
      onMouseOver={(e) =>
        (e.currentTarget.style.background = props.danger
          ? "rgba(242, 63, 67, 0.1)"
          : "rgba(79, 84, 92, 0.3)")
      }
      onMouseOut={(e) => (e.currentTarget.style.background = "transparent")}
    >
      <span>{props.label}</span>
      <svg width="16" height="16" viewBox="0 0 24 24" fill="currentColor" class="opacity-60">
        <path d={props.icon} />
      </svg>
    </button>
  );
}

export { Avatar, nameColor };
