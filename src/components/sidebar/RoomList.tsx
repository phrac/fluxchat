import { For, Show } from "solid-js";
import {
  rooms,
  selectedRoomId,
  selectRoom,
  selectedSpaceId,
  spaces,
} from "../../stores/rooms";
import { loadMessages, loadMembers } from "../../stores/messages";

export default function RoomList() {
  const currentSpaceName = () => {
    if (!selectedSpaceId()) return "All Rooms";
    const space = spaces.find((s) => s.room_id === selectedSpaceId());
    return space?.name || "Space";
  };

  const handleRoomClick = (roomId: string) => {
    selectRoom(roomId);
    // Load messages and members in parallel
    Promise.all([
      loadMessages(roomId).catch(() => {}),
      loadMembers(roomId).catch(() => {}),
    ]);
  };

  return (
    <div
      class="flex w-60 flex-shrink-0 flex-col"
      style={{ background: "var(--bg-secondary)" }}
    >
      {/* Header */}
      <div
        class="flex h-12 flex-shrink-0 items-center px-4 font-semibold shadow-sm"
        style={{
          color: "var(--text-primary)",
          "border-bottom": "1px solid rgba(0,0,0,0.2)",
        }}
      >
        {currentSpaceName()}
      </div>

      {/* Room list */}
      <div class="flex-1 overflow-y-auto px-2 py-2">
        <For each={rooms}>
          {(room) => {
            const isSelected = () => selectedRoomId() === room.room_id;
            return (
              <button
                onClick={() => handleRoomClick(room.room_id)}
                class="mb-0.5 flex w-full items-center rounded px-2 py-1.5 text-left text-sm transition-colors"
                style={{
                  background: isSelected()
                    ? "var(--bg-primary)"
                    : "transparent",
                  color: isSelected()
                    ? "var(--text-primary)"
                    : "var(--text-muted)",
                }}
                onMouseOver={(e) => {
                  if (!isSelected())
                    e.currentTarget.style.background =
                      "rgba(79, 84, 92, 0.3)";
                }}
                onMouseOut={(e) => {
                  if (!isSelected())
                    e.currentTarget.style.background = "transparent";
                }}
              >
                <span
                  class="mr-1.5 text-lg"
                  style={{ color: "var(--text-muted)" }}
                >
                  #
                </span>
                <span class="min-w-0 flex-1 truncate">{room.name}</span>
                <Show when={room.unread_count > 0}>
                  <span
                    class="ml-1 flex h-4 min-w-[16px] items-center justify-center rounded-full px-1 text-xs font-bold text-white"
                    style={{ background: "var(--red-dnd)" }}
                  >
                    {room.unread_count}
                  </span>
                </Show>
                <Show when={room.is_encrypted}>
                  <span class="ml-1 text-xs" title="Encrypted">
                    🔒
                  </span>
                </Show>
              </button>
            );
          }}
        </For>

        <Show when={rooms.length === 0}>
          <p
            class="px-2 py-4 text-center text-sm"
            style={{ color: "var(--text-muted)" }}
          >
            No rooms yet
          </p>
        </Show>
      </div>
    </div>
  );
}
