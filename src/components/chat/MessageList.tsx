import { For, Show, createEffect, createSignal } from "solid-js";
import { messagesByRoom, loadMessages, loadOlderMessages, loadMembers } from "../../stores/messages";
import { sendReadReceipt } from "../../lib/tauri";
import MessageBubble from "./MessageBubble";

interface Props {
  roomId: string;
}

// Keep last N rooms in DOM, evict oldest
const MAX_CACHED_ROOMS = 5;
const roomOrder: string[] = [];

export default function MessageList(props: Props) {
  let scrollRefs: Record<string, HTMLDivElement> = {};
  let isAtBottom = true;
  let loadingOlder = false;

  // All rooms we've ever visited — we render a hidden div for each
  const [visitedRooms, setVisitedRooms] = createSignal<string[]>([]);

  createEffect(() => {
    const roomId = props.roomId;
    if (!roomId) return;

    // Move to front of recency list
    const idx = roomOrder.indexOf(roomId);
    if (idx !== -1) roomOrder.splice(idx, 1);
    roomOrder.unshift(roomId);

    // Evict oldest if over limit
    while (roomOrder.length > MAX_CACHED_ROOMS) {
      roomOrder.pop();
    }

    setVisitedRooms([...roomOrder]);

    // Load data if needed (loadMessages/loadMembers skip if already attempted)
    loadMessages(roomId).catch(() => {});
    loadMembers(roomId).catch(() => {});

    // Scroll to bottom of the newly active room
    requestAnimationFrame(() => {
      const el = scrollRefs[roomId];
      if (el) el.scrollTop = el.scrollHeight;
    });
  });

  const handleScroll = async (roomId: string, el: HTMLDivElement) => {
    const { scrollTop, scrollHeight, clientHeight } = el;
    isAtBottom = scrollHeight - scrollTop - clientHeight < 50;

    if (scrollTop < 100 && !loadingOlder) {
      loadingOlder = true;
      const prevScrollHeight = el.scrollHeight;
      const loaded = await loadOlderMessages(roomId);
      if (loaded) {
        requestAnimationFrame(() => {
          el.scrollTop = el.scrollHeight - prevScrollHeight;
        });
      }
      loadingOlder = false;
    }
  };

  return (
    <div class="relative flex-1 overflow-hidden">
      <For each={visitedRooms()}>
        {(roomId) => {
          const messages = () => messagesByRoom[roomId] || [];
          const isActive = () => props.roomId === roomId;

          // Auto-scroll to bottom when new messages arrive in this room
          createEffect(() => {
            const msgs = messages();
            const len = msgs.length;
            if (len > 0 && isActive()) {
              const el = scrollRefs[roomId];
              if (el) {
                const { scrollTop, scrollHeight, clientHeight } = el;
                const atBottom = scrollHeight - scrollTop - clientHeight < 100;
                if (atBottom) {
                  requestAnimationFrame(() => {
                    el.scrollTop = el.scrollHeight;
                  });
                  // Send read receipt for the latest message
                  const lastMsg = msgs[len - 1];
                  if (lastMsg && !lastMsg.event_id.startsWith("~")) {
                    sendReadReceipt(roomId, lastMsg.event_id).catch(() => {});
                  }
                }
              }
            }
          });

          return (
            <div
              ref={(el) => { scrollRefs[roomId] = el; }}
              class="absolute inset-0 overflow-y-auto px-4 py-4"
              style={{ display: isActive() ? "block" : "none" }}
              onScroll={() => {
                const el = scrollRefs[roomId];
                if (el && isActive()) handleScroll(roomId, el);
              }}
            >
              <Show when={messages().length === 0}>
                <div class="flex h-full items-center justify-center" style={{ color: "var(--text-muted)" }}>
                  No messages yet
                </div>
              </Show>

              <For each={messages()}>
                {(msg, i) => {
                  const prev = () => (i() > 0 ? messages()[i() - 1] : null);
                  const showHeader = () => {
                    const p = prev();
                    if (!p) return true;
                    if (p.sender !== msg.sender) return true;
                    return msg.timestamp - p.timestamp > 300;
                  };
                  return <MessageBubble message={msg} showHeader={showHeader()} />;
                }}
              </For>
            </div>
          );
        }}
      </For>
    </div>
  );
}
