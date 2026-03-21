import { Show, createSignal, createEffect } from "solid-js";
import { marked } from "marked";
import type { MessageInfo } from "../../lib/types";
import { getMediaUrl } from "../../lib/tauri";
import { setReplyingTo } from "../../stores/messages";

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

function MessageContent(props: { message: MessageInfo }) {
  const [html, setHtml] = createSignal(
    renderBodySync(props.message.body, props.message.formatted_body)
  );

  // Resolve mxc:// URLs in formatted HTML (custom emotes, inline images)
  createEffect(() => {
    const raw = renderBodySync(props.message.body, props.message.formatted_body);
    if (raw.includes("mxc://")) {
      resolveHtmlMedia(raw).then(setHtml);
    } else {
      setHtml(raw);
    }
  });

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
  const color = () => nameColor(props.message.sender);

  const handleReply = (e: MouseEvent) => {
    e.stopPropagation();
    setReplyingTo(props.message);
  };

  const isPending = () => props.message.event_id.startsWith("~local-");

  return (
    <div
      class="group relative py-0.5 transition-colors"
      classList={{ "mt-4": props.showHeader }}
      style={{
        "padding-left": props.showHeader ? undefined : "56px",
        opacity: isPending() ? "0.5" : "1",
        transition: "opacity 0.3s",
      }}
      onMouseOver={(e) =>
        (e.currentTarget.style.background = "rgba(0,0,0,0.1)")
      }
      onMouseOut={(e) => (e.currentTarget.style.background = "transparent")}
    >
      {/* Reply action button */}
      <button
        class="absolute right-2 top-0 hidden rounded px-1.5 py-0.5 group-hover:flex items-center"
        style={{ background: "var(--bg-tertiary)", color: "var(--text-muted)" }}
        onClick={handleReply}
        title="Reply"
      >
        <svg width="16" height="16" viewBox="0 0 24 24" fill="currentColor">
          <path d="M10 9V5l-7 7 7 7v-4.1c5 0 8.5 1.6 11 5.1-1-5-4-10-11-11z"/>
        </svg>
      </button>

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
          class="pointer-events-none absolute left-1 top-1 hidden text-[10px] group-hover:inline"
          style={{ color: "var(--text-muted)" }}
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

export { Avatar, nameColor };
