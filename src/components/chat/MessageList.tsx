import { For, Show, createEffect, createSignal } from "solid-js";
import { messagesByRoom, loadMessages, loadOlderMessages, loadMembers } from "../../stores/messages";
import { sendReadReceipt } from "../../lib/tauri";
import MessageBubble from "./MessageBubble";

interface Props {
  roomId: string;
}

const MAX_CACHED_ROOMS = 5;
const roomOrder: string[] = [];

// Track scroll-to-bottom state per room
const roomAtBottom: Record<string, boolean> = {};
// Track last seen message count per room to detect new messages
const roomLastCount: Record<string, number> = {};

export default function MessageList(props: Props) {
  let scrollRefs: Record<string, HTMLDivElement> = {};
  let loadingOlder = false;

  const [visitedRooms, setVisitedRooms] = createSignal<string[]>([]);

  createEffect(() => {
    const roomId = props.roomId;
    if (!roomId) return;

    const idx = roomOrder.indexOf(roomId);
    if (idx !== -1) roomOrder.splice(idx, 1);
    roomOrder.unshift(roomId);

    while (roomOrder.length > MAX_CACHED_ROOMS) {
      const evicted = roomOrder.pop()!;
      delete roomAtBottom[evicted];
      delete roomLastCount[evicted];
    }

    setVisitedRooms([...roomOrder]);

    // Default new rooms to "at bottom"
    if (!(roomId in roomAtBottom)) {
      roomAtBottom[roomId] = true;
    }

    loadMessages(roomId).catch(() => {});
    loadMembers(roomId).catch(() => {});

    // Scroll to bottom when switching to a room
    requestAnimationFrame(() => {
      const el = scrollRefs[roomId];
      if (el) {
        el.scrollTop = el.scrollHeight;
        roomAtBottom[roomId] = true;
      }
    });
  });

  const handleScroll = async (roomId: string, el: HTMLDivElement) => {
    const { scrollTop, scrollHeight, clientHeight } = el;
    // Consider "at bottom" if within 80px of the bottom
    roomAtBottom[roomId] = scrollHeight - scrollTop - clientHeight < 80;

    // Load older messages when scrolled near top
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

          // Watch for new messages and auto-scroll if at bottom
          createEffect(() => {
            const msgs = messages();
            const len = msgs.length;
            const prevLen = roomLastCount[roomId] || 0;
            roomLastCount[roomId] = len;

            // Only act on NEW messages (not initial load which is handled above)
            if (len > prevLen && prevLen > 0 && isActive()) {
              if (roomAtBottom[roomId]) {
                requestAnimationFrame(() => {
                  const el = scrollRefs[roomId];
                  if (el) el.scrollTop = el.scrollHeight;
                });
              }

              // Send read receipt for latest message if at bottom
              if (roomAtBottom[roomId]) {
                const lastMsg = msgs[len - 1];
                if (lastMsg && !lastMsg.event_id.startsWith("~")) {
                  sendReadReceipt(roomId, lastMsg.event_id).catch(() => {});
                }
              }
            }

            // Initial load — scroll to bottom
            if (prevLen === 0 && len > 0) {
              requestAnimationFrame(() => {
                const el = scrollRefs[roomId];
                if (el) {
                  el.scrollTop = el.scrollHeight;
                  roomAtBottom[roomId] = true;
                }
              });
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
