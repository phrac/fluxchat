import { Show, For, createSignal, createEffect } from "solid-js";
import { selectedRoomId } from "../../stores/rooms";
import { messagesByRoom } from "../../stores/messages";
import { getPinnedMessages } from "../../lib/tauri";
import MessageBubble from "./MessageBubble";
import type { MessageInfo } from "../../lib/types";

interface Props {
  onClose: () => void;
}

function jumpToMessage(eventId: string, closeModal: () => void) {
  const roomId = selectedRoomId();
  if (!roomId) return;

  // Check if message is in current loaded messages
  const msgs = messagesByRoom[roomId] || [];
  const exists = msgs.some((m) => m.event_id === eventId);

  closeModal();

  if (exists) {
    // Scroll to the message element
    requestAnimationFrame(() => {
      const el = document.querySelector(`[data-event-id="${eventId}"]`);
      if (el) {
        el.scrollIntoView({ behavior: "smooth", block: "center" });
        // Flash highlight
        const htmlEl = el as HTMLElement;
        htmlEl.style.background = "rgba(88, 101, 242, 0.2)";
        setTimeout(() => {
          htmlEl.style.background = "";
        }, 2000);
      }
    });
  }
}

export default function PinnedSidebar(props: Props) {
  const [pins, setPins] = createSignal<MessageInfo[]>([]);
  const [loading, setLoading] = createSignal(false);

  createEffect(() => {
    const roomId = selectedRoomId();
    if (!roomId) return;
    setLoading(true);
    getPinnedMessages(roomId)
      .then(setPins)
      .catch(() => setPins([]))
      .finally(() => setLoading(false));
  });

  return (
    <div
      class="fixed inset-0 z-50 flex items-start justify-center pt-12"
      style={{ background: "rgba(0,0,0,0.5)" }}
      onClick={(e) => {
        if (e.target === e.currentTarget) props.onClose();
      }}
    >
      <div
        class="flex max-h-[80vh] w-[480px] flex-col rounded-lg shadow-2xl overflow-hidden"
        style={{ background: "var(--bg-primary)" }}
      >
        {/* Header */}
        <div
          class="flex flex-shrink-0 items-center gap-2 px-4 py-3"
          style={{ background: "var(--bg-secondary)" }}
        >
          <svg width="20" height="20" viewBox="0 0 24 24" fill="currentColor" style={{ color: "var(--text-muted)" }}>
            <path d="M16 12V4h1V2H7v2h1v8l-2 2v2h5.2v6h1.6v-6H18v-2l-2-2z"/>
          </svg>
          <span class="text-base font-bold" style={{ color: "var(--text-primary)" }}>
            Pinned Messages
          </span>
          <span class="ml-auto">
            <button
              onClick={props.onClose}
              class="rounded p-1 transition-colors"
              style={{ color: "var(--text-muted)" }}
              onMouseOver={(e) => (e.currentTarget.style.color = "var(--text-primary)")}
              onMouseOut={(e) => (e.currentTarget.style.color = "var(--text-muted)")}
            >
              <svg width="18" height="18" viewBox="0 0 24 24" fill="currentColor">
                <path d="M19 6.41L17.59 5 12 10.59 6.41 5 5 6.41 10.59 12 5 17.59 6.41 19 12 13.41 17.59 19 19 17.59 13.41 12z"/>
              </svg>
            </button>
          </span>
        </div>

        {/* Content */}
        <div class="flex-1 overflow-y-auto">
          <Show when={loading()}>
            <div class="flex items-center justify-center py-12" style={{ color: "var(--text-muted)" }}>
              <div
                class="mr-2 h-5 w-5 animate-spin rounded-full border-2 border-t-transparent"
                style={{ "border-color": "var(--accent)", "border-top-color": "transparent" }}
              />
              Loading...
            </div>
          </Show>

          <Show when={!loading() && pins().length === 0}>
            <div class="flex flex-col items-center justify-center py-16" style={{ color: "var(--text-muted)" }}>
              <svg
                width="48"
                height="48"
                viewBox="0 0 24 24"
                fill="currentColor"
                class="mb-4 opacity-20"
              >
                <path d="M16 12V4h1V2H7v2h1v8l-2 2v2h5.2v6h1.6v-6H18v-2l-2-2z"/>
              </svg>
              <p class="text-lg font-medium" style={{ color: "var(--text-secondary)" }}>
                No pinned messages
              </p>
              <p class="mt-1 text-sm">
                Pin important messages so they're easy to find.
              </p>
            </div>
          </Show>

          <Show when={!loading() && pins().length > 0}>
            <div class="p-2">
              <For each={pins()}>
                {(msg) => (
                  <div
                    class="mb-2 cursor-pointer rounded-lg p-1 transition-colors"
                    style={{ background: "var(--bg-secondary)" }}
                    onMouseOver={(e) => (e.currentTarget.style.background = "rgba(79, 84, 92, 0.5)")}
                    onMouseOut={(e) => (e.currentTarget.style.background = "var(--bg-secondary)")}
                    onClick={() => jumpToMessage(msg.event_id, props.onClose)}
                    title="Jump to message"
                  >
                    <MessageBubble message={msg} showHeader={true} />
                  </div>
                )}
              </For>
            </div>
          </Show>
        </div>
      </div>
    </div>
  );
}
