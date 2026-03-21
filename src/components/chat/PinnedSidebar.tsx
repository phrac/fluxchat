import { Show, For, createSignal, createEffect } from "solid-js";
import { selectedRoomId } from "../../stores/rooms";
import { getPinnedMessages } from "../../lib/tauri";
import MessageBubble from "./MessageBubble";
import type { MessageInfo } from "../../lib/types";

interface Props {
  onClose: () => void;
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
      class="flex w-80 flex-shrink-0 flex-col"
      style={{ background: "var(--bg-secondary)" }}
    >
      <div
        class="flex h-12 flex-shrink-0 items-center justify-between px-4"
        style={{ "border-bottom": "1px solid rgba(0,0,0,0.2)" }}
      >
        <span
          class="text-sm font-semibold"
          style={{ color: "var(--text-primary)" }}
        >
          Pinned Messages
        </span>
        <button
          onClick={props.onClose}
          class="rounded p-1"
          style={{ color: "var(--text-muted)" }}
          onMouseOver={(e) =>
            (e.currentTarget.style.background = "rgba(79, 84, 92, 0.3)")
          }
          onMouseOut={(e) =>
            (e.currentTarget.style.background = "transparent")
          }
        >
          <svg width="16" height="16" viewBox="0 0 24 24" fill="currentColor">
            <path d="M19 6.41L17.59 5 12 10.59 6.41 5 5 6.41 10.59 12 5 17.59 6.41 19 12 13.41 17.59 19 19 17.59 13.41 12z"/>
          </svg>
        </button>
      </div>

      <div class="flex-1 overflow-y-auto">
        <Show when={loading()}>
          <p class="px-4 py-4 text-center text-sm" style={{ color: "var(--text-muted)" }}>
            Loading...
          </p>
        </Show>

        <Show when={!loading() && pins().length === 0}>
          <div
            class="flex flex-col items-center justify-center px-4 py-12"
            style={{ color: "var(--text-muted)" }}
          >
            <svg
              width="40"
              height="40"
              viewBox="0 0 24 24"
              fill="currentColor"
              class="mb-3 opacity-30"
            >
              <path d="M16 12V4h1V2H7v2h1v8l-2 2v2h5.2v6h1.6v-6H18v-2l-2-2z"/>
            </svg>
            <p class="text-sm">No pinned messages</p>
          </div>
        </Show>

        <Show when={!loading() && pins().length > 0}>
          <For each={pins()}>
            {(msg, i) => {
              const prev = () => (i() > 0 ? pins()[i() - 1] : null);
              const showHeader = () => {
                const p = prev();
                if (!p) return true;
                if (p.sender !== msg.sender) return true;
                return msg.timestamp - p.timestamp > 300;
              };

              return (
                <div style={{ "border-bottom": "1px solid rgba(0,0,0,0.1)" }}>
                  <MessageBubble message={msg} showHeader={showHeader()} />
                </div>
              );
            }}
          </For>
        </Show>
      </div>
    </div>
  );
}
