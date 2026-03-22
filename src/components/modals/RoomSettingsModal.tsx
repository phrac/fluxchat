import { createSignal, Show } from "solid-js";
import { setRoomName, setRoomTopic, leaveRoom } from "../../lib/tauri";
import { selectedRoomId, rooms, loadRooms } from "../../stores/rooms";

interface Props {
  onClose: () => void;
}

export default function RoomSettingsModal(props: Props) {
  const room = () => rooms.find((r) => r.room_id === selectedRoomId());
  const [name, setName] = createSignal(room()?.name || "");
  const [topic, setTopic] = createSignal(room()?.topic || "");
  const [saving, setSaving] = createSignal(false);
  const [error, setError] = createSignal("");
  const [success, setSuccess] = createSignal("");

  const handleSave = async () => {
    const roomId = selectedRoomId();
    if (!roomId || saving()) return;
    setError("");
    setSuccess("");
    setSaving(true);
    try {
      const currentRoom = room();
      if (name().trim() && name() !== currentRoom?.name) {
        await setRoomName(roomId, name().trim());
      }
      if (topic() !== (currentRoom?.topic || "")) {
        await setRoomTopic(roomId, topic().trim());
      }
      await loadRooms();
      setSuccess("Settings saved");
      setTimeout(() => setSuccess(""), 2000);
    } catch (err: any) {
      setError(String(err));
    }
    setSaving(false);
  };

  const handleLeave = async () => {
    const roomId = selectedRoomId();
    if (!roomId) return;
    if (!confirm("Are you sure you want to leave this room?")) return;
    try {
      await leaveRoom(roomId);
      await loadRooms();
      props.onClose();
    } catch (err: any) {
      setError(String(err));
    }
  };

  return (
    <div
      class="fixed inset-0 z-50 flex items-center justify-center"
      style={{ background: "rgba(0,0,0,0.6)" }}
      onClick={(e) => { if (e.target === e.currentTarget) props.onClose(); }}
    >
      <div class="w-[460px] rounded-lg p-6" style={{ background: "var(--bg-primary)" }}>
        <h2 class="mb-1 text-lg font-bold" style={{ color: "var(--text-primary)" }}>
          Room Settings
        </h2>
        <p class="mb-4 text-xs" style={{ color: "var(--text-muted)" }}>
          {room()?.room_id}
        </p>

        <Show when={error()}>
          <div class="mb-3 rounded p-2 text-sm" style={{ background: "rgba(242,63,67,0.1)", color: "var(--red-dnd)" }}>
            {error()}
          </div>
        </Show>
        <Show when={success()}>
          <div class="mb-3 rounded p-2 text-sm" style={{ background: "rgba(35,165,89,0.1)", color: "var(--green-online)" }}>
            {success()}
          </div>
        </Show>

        {/* Room name */}
        <label class="mb-1 block text-xs font-bold uppercase" style={{ color: "var(--text-secondary)" }}>
          Room Name
        </label>
        <input
          type="text"
          value={name()}
          onInput={(e) => setName(e.currentTarget.value)}
          class="mb-3 w-full rounded px-3 py-2 text-sm outline-none"
          style={{ background: "var(--bg-tertiary)", color: "var(--text-primary)" }}
        />

        {/* Topic */}
        <label class="mb-1 block text-xs font-bold uppercase" style={{ color: "var(--text-secondary)" }}>
          Topic
        </label>
        <textarea
          value={topic()}
          onInput={(e) => setTopic(e.currentTarget.value)}
          rows={3}
          class="mb-3 w-full resize-none rounded px-3 py-2 text-sm outline-none"
          style={{ background: "var(--bg-tertiary)", color: "var(--text-primary)" }}
        />

        {/* Info */}
        <div class="mb-4 flex gap-4 text-xs" style={{ color: "var(--text-muted)" }}>
          <span>{room()?.is_encrypted ? "🔒 Encrypted" : "🔓 Not encrypted"}</span>
          <span>{room()?.is_direct ? "👤 Direct Message" : "💬 Group Room"}</span>
        </div>

        {/* Buttons */}
        <div class="flex items-center justify-between">
          <button
            onClick={handleLeave}
            class="rounded px-4 py-2 text-sm font-medium"
            style={{ background: "rgba(242,63,67,0.1)", color: "var(--red-dnd)" }}
          >
            Leave Room
          </button>
          <div class="flex gap-3">
            <button
              onClick={props.onClose}
              class="rounded px-4 py-2 text-sm"
              style={{ background: "var(--bg-tertiary)", color: "var(--text-secondary)" }}
            >
              Cancel
            </button>
            <button
              onClick={handleSave}
              disabled={saving()}
              class="rounded px-4 py-2 text-sm font-medium text-white"
              style={{ background: saving() ? "var(--accent-hover)" : "var(--accent)" }}
            >
              {saving() ? "Saving..." : "Save Changes"}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
