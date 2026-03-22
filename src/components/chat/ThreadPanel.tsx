import { Show, For, createSignal, createEffect } from "solid-js";
import { getThreadMessages, sendReply } from "../../lib/tauri";
import { selectedRoomId } from "../../stores/rooms";
import MessageBubble from "./MessageBubble";
import type { MessageInfo } from "../../lib/types";

interface Props {
  threadRootId: string;
  onClose: () => void;
}

export default function ThreadPanel(props: Props) {
  const [messages, setMessages] = createSignal<MessageInfo[]>([]);
  const [loading, setLoading] = createSignal(true);
  const [replyText, setReplyText] = createSignal("");
  const [sending, setSending] = createSignal(false);
  let inputRef!: HTMLTextAreaElement;
  let scrollRef!: HTMLDivElement;

  createEffect(() => {
    const roomId = selectedRoomId();
    if (!roomId || !props.threadRootId) return;

    setLoading(true);
    getThreadMessages(roomId, props.threadRootId)
      .then((msgs) => {
        setMessages(msgs);
        requestAnimationFrame(() => {
          if (scrollRef) scrollRef.scrollTop = scrollRef.scrollHeight;
        });
      })
      .catch(() => setMessages([]))
      .finally(() => setLoading(false));
  });

  const handleSend = async () => {
    const body = replyText().trim();
    const roomId = selectedRoomId();
    if (!body || !roomId || sending()) return;

    setSending(true);
    try {
      await sendReply(roomId, body, props.threadRootId);
      setReplyText("");
      if (inputRef) inputRef.style.height = "auto";

      // Refresh thread
      const msgs = await getThreadMessages(roomId, props.threadRootId);
      setMessages(msgs);
      requestAnimationFrame(() => {
        if (scrollRef) scrollRef.scrollTop = scrollRef.scrollHeight;
      });
    } catch (err) {
      console.error("Thread reply failed:", err);
    }
    setSending(false);
  };

  const handleKeyDown = (e: KeyboardEvent) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      handleSend();
    }
    if (e.key === "Escape") {
      props.onClose();
    }
  };

  return (
    <div
      class="flex w-80 flex-shrink-0 flex-col"
      style={{ background: "var(--bg-secondary)" }}
    >
      {/* Header */}
      <div
        class="flex h-12 flex-shrink-0 items-center justify-between px-4"
        style={{ "border-bottom": "1px solid rgba(0,0,0,0.2)" }}
      >
        <span class="text-sm font-semibold" style={{ color: "var(--text-primary)" }}>
          Thread
        </span>
        <button
          onClick={props.onClose}
          class="rounded p-1 transition-colors"
          style={{ color: "var(--text-muted)" }}
          onMouseOver={(e) => (e.currentTarget.style.background = "rgba(79, 84, 92, 0.3)")}
          onMouseOut={(e) => (e.currentTarget.style.background = "transparent")}
        >
          <svg width="16" height="16" viewBox="0 0 24 24" fill="currentColor">
            <path d="M19 6.41L17.59 5 12 10.59 6.41 5 5 6.41 10.59 12 5 17.59 6.41 19 12 13.41 17.59 19 19 17.59 13.41 12z"/>
          </svg>
        </button>
      </div>

      {/* Messages */}
      <div
        ref={scrollRef}
        class="flex-1 overflow-y-auto px-3 py-3"
      >
        <Show when={loading()}>
          <div class="flex items-center justify-center py-8" style={{ color: "var(--text-muted)" }}>
            <div
              class="mr-2 h-4 w-4 animate-spin rounded-full border-2 border-t-transparent"
              style={{ "border-color": "var(--accent)", "border-top-color": "transparent" }}
            />
            Loading thread...
          </div>
        </Show>

        <Show when={!loading()}>
          <For each={messages()}>
            {(msg, i) => {
              const prev = () => (i() > 0 ? messages()[i() - 1] : null);
              const showHeader = () => {
                const p = prev();
                if (!p) return true;
                if (p.sender !== msg.sender) return true;
                return msg.timestamp - p.timestamp > 300;
              };

              return (
                <div style={i() === 0 ? {
                  "border-bottom": "1px solid var(--border-subtle)",
                  "padding-bottom": "8px",
                  "margin-bottom": "8px",
                } : {}}>
                  <MessageBubble message={msg} showHeader={showHeader()} />
                </div>
              );
            }}
          </For>
        </Show>
      </div>

      {/* Reply input */}
      <div class="flex-shrink-0 px-3 pb-3">
        <div
          class="flex items-end rounded-lg"
          style={{ background: "var(--bg-primary)" }}
        >
          <textarea
            ref={inputRef}
            value={replyText()}
            onInput={(e) => {
              setReplyText(e.currentTarget.value);
              e.currentTarget.style.height = "auto";
              e.currentTarget.style.height = Math.min(e.currentTarget.scrollHeight, 120) + "px";
            }}
            onKeyDown={handleKeyDown}
            placeholder="Reply in thread..."
            rows={1}
            class="max-h-[120px] min-h-[36px] flex-1 resize-none bg-transparent px-3 py-2 text-sm outline-none"
            style={{ color: "var(--text-primary)" }}
          />
        </div>
      </div>
    </div>
  );
}
