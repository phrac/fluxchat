import { For, Show } from "solid-js";
import { membersByRoom } from "../../stores/messages";
import { selectedRoomId } from "../../stores/rooms";
import { Avatar, nameColor } from "../chat/MessageBubble";

export default function MemberList() {
  const members = () => membersByRoom[selectedRoomId()!] || [];

  return (
    <div
      class="flex w-60 flex-shrink-0 flex-col"
      style={{ background: "var(--bg-secondary)" }}
    >
      <div
        class="flex h-12 flex-shrink-0 items-center px-4"
        style={{ "border-bottom": "1px solid rgba(0,0,0,0.2)" }}
      >
        <span
          class="text-xs font-semibold uppercase"
          style={{ color: "var(--text-muted)" }}
        >
          Members — {members().length}
        </span>
      </div>

      <div class="flex-1 overflow-y-auto px-2 py-2">
        <For each={members()}>
          {(member) => (
            <div
              class="mb-0.5 flex items-center rounded px-2 py-1.5 transition-colors"
              onMouseOver={(e) =>
                (e.currentTarget.style.background = "rgba(79, 84, 92, 0.3)")
              }
              onMouseOut={(e) =>
                (e.currentTarget.style.background = "transparent")
              }
            >
              <div class="mr-2">
                <Avatar
                  mxcUrl={member.avatar_url}
                  name={member.display_name}
                  color={nameColor(member.user_id)}
                  size="h-8 w-8"
                />
              </div>
              <span
                class="min-w-0 flex-1 truncate text-sm"
                style={{ color: "var(--text-secondary)" }}
              >
                {member.display_name}
              </span>
            </div>
          )}
        </For>

        <Show when={members().length === 0}>
          <p
            class="px-2 py-4 text-center text-sm"
            style={{ color: "var(--text-muted)" }}
          >
            Loading members...
          </p>
        </Show>
      </div>
    </div>
  );
}
