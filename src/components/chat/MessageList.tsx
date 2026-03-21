import { For, Show, createEffect, onMount } from "solid-js";
import { messagesByRoom } from "../../stores/messages";
import { loadOlderMessages } from "../../stores/messages";
import MessageBubble from "./MessageBubble";

interface Props {
  roomId: string;
}

export default function MessageList(props: Props) {
  let containerRef!: HTMLDivElement;
  let isAtBottom = true;
  let loadingOlder = false;

  const messages = () => messagesByRoom[props.roomId] || [];

  const scrollToBottom = () => {
    if (containerRef) {
      containerRef.scrollTop = containerRef.scrollHeight;
    }
  };

  // Auto-scroll to bottom when new messages arrive (if already at bottom)
  createEffect(() => {
    const _msgs = messages();
    if (isAtBottom) {
      requestAnimationFrame(scrollToBottom);
    }
  });

  // Scroll to bottom on mount / room change
  createEffect(() => {
    const _roomId = props.roomId;
    requestAnimationFrame(scrollToBottom);
  });

  const handleScroll = async () => {
    if (!containerRef) return;

    const { scrollTop, scrollHeight, clientHeight } = containerRef;

    // Track if user is at bottom (within 50px)
    isAtBottom = scrollHeight - scrollTop - clientHeight < 50;

    // Load older messages when scrolled to top
    if (scrollTop < 100 && !loadingOlder) {
      loadingOlder = true;
      const prevScrollHeight = containerRef.scrollHeight;

      const loaded = await loadOlderMessages(props.roomId);

      if (loaded) {
        // Maintain scroll position after prepending
        requestAnimationFrame(() => {
          const newScrollHeight = containerRef.scrollHeight;
          containerRef.scrollTop = newScrollHeight - prevScrollHeight;
        });
      }
      loadingOlder = false;
    }
  };

  return (
    <div
      ref={containerRef}
      class="flex-1 overflow-y-auto px-4 py-4"
      onScroll={handleScroll}
    >
      <Show
        when={messages().length > 0}
        fallback={
          <div
            class="flex h-full items-center justify-center"
            style={{ color: "var(--text-muted)" }}
          >
            No messages yet
          </div>
        }
      >
        <For each={messages()}>
          {(msg, i) => {
            const prev = () => (i() > 0 ? messages()[i() - 1] : null);
            const showHeader = () => {
              const p = prev();
              if (!p) return true;
              if (p.sender !== msg.sender) return true;
              // Group messages within 5 minutes
              return msg.timestamp - p.timestamp > 300;
            };

            return (
              <MessageBubble message={msg} showHeader={showHeader()} />
            );
          }}
        </For>
      </Show>
    </div>
  );
}
