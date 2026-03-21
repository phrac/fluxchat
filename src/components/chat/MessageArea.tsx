import { Show } from "solid-js";
import { selectedRoomId, rooms } from "../../stores/rooms";
import MessageList from "./MessageList";
import MessageInput from "./MessageInput";

interface Props {
  showMembers: boolean;
  onToggleMembers: () => void;
}

export default function MessageArea(props: Props) {
  const currentRoom = () => rooms.find((r) => r.room_id === selectedRoomId());

  return (
    <div
      class="flex min-w-0 flex-1 flex-col"
      style={{ background: "var(--bg-primary)" }}
    >
      {/* Header bar */}
      <div
        class="flex h-12 flex-shrink-0 items-center justify-between px-4 shadow-sm"
        style={{ "border-bottom": "1px solid rgba(0,0,0,0.2)" }}
      >
        <div class="flex items-center">
          <Show when={currentRoom()}>
            <span style={{ color: "var(--text-muted)" }} class="mr-2 text-lg">
              #
            </span>
            <span
              class="font-semibold"
              style={{ color: "var(--text-primary)" }}
            >
              {currentRoom()!.name}
            </span>
            <Show when={currentRoom()!.topic}>
              <span
                class="ml-3 truncate text-sm"
                style={{ color: "var(--text-muted)" }}
              >
                {currentRoom()!.topic}
              </span>
            </Show>
          </Show>
        </div>

        <Show when={selectedRoomId()}>
          <button
            onClick={props.onToggleMembers}
            class="rounded p-1.5 transition-colors"
            style={{
              color: props.showMembers
                ? "var(--text-primary)"
                : "var(--text-muted)",
            }}
            onMouseOver={(e) =>
              (e.currentTarget.style.background = "rgba(79, 84, 92, 0.3)")
            }
            onMouseOut={(e) =>
              (e.currentTarget.style.background = "transparent")
            }
            title="Toggle member list"
          >
            <svg
              width="20"
              height="20"
              viewBox="0 0 24 24"
              fill="currentColor"
            >
              <path d="M14 8.00598C14 10.211 12.206 12.006 10 12.006C7.795 12.006 6 10.211 6 8.00598C6 5.80098 7.794 4.00598 10 4.00598C12.206 4.00598 14 5.80098 14 8.00598ZM2 19.006C2 15.473 5.29 13.006 10 13.006C14.711 13.006 18 15.473 18 19.006V20.006H2V19.006ZM20 20.006H19V18.006C19 16.345 18.22 14.845 16.826 13.675C19.305 14.117 22 15.573 22 18.006V20.006H20ZM14.218 12.006C14.684 11.373 15 10.588 15 9.68498C15 9.18098 14.897 8.70498 14.719 8.27098C15.449 7.65198 16.392 7.26898 17.428 7.26898C19.318 7.26898 20.858 8.80898 20.858 10.699C20.858 12.589 19.318 14.129 17.428 14.129C16.238 14.129 15.19 13.449 14.218 12.006Z" />
            </svg>
          </button>
        </Show>
      </div>

      {/* Content */}
      <Show
        when={selectedRoomId()}
        fallback={
          <div
            class="flex flex-1 items-center justify-center"
            style={{ color: "var(--text-muted)" }}
          >
            <div class="text-center">
              <p class="mb-2 text-4xl">💬</p>
              <p class="text-lg">Select a room to start chatting</p>
            </div>
          </div>
        }
      >
        <MessageList roomId={selectedRoomId()!} />
        <MessageInput roomId={selectedRoomId()!} />
      </Show>
    </div>
  );
}
