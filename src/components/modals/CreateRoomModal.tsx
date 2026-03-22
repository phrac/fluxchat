import { createSignal, Show } from "solid-js";
import { createRoom } from "../../lib/tauri";
import { loadRooms, selectedSpaceId } from "../../stores/rooms";

interface Props {
  onClose: () => void;
}

export default function CreateRoomModal(props: Props) {
  const [name, setName] = createSignal("");
  const [topic, setTopic] = createSignal("");
  const [isPublic, setIsPublic] = createSignal(false);
  const [isEncrypted, setIsEncrypted] = createSignal(true);
  const [invites, setInvites] = createSignal("");
  const [error, setError] = createSignal("");
  const [creating, setCreating] = createSignal(false);

  const handleCreate = async () => {
    if (!name().trim() || creating()) return;
    setError("");
    setCreating(true);
    try {
      const inviteIds = invites()
        .split(/[,\s]+/)
        .map((s) => s.trim())
        .filter((s) => s.startsWith("@"));

      await createRoom(
        name().trim(),
        topic().trim() || null,
        isPublic(),
        isEncrypted(),
        inviteIds,
        selectedSpaceId()
      );
      await loadRooms();
      props.onClose();
    } catch (err: any) {
      setError(String(err));
    }
    setCreating(false);
  };

  return (
    <div
      class="fixed inset-0 z-50 flex items-center justify-center"
      style={{ background: "rgba(0,0,0,0.6)" }}
      onClick={(e) => { if (e.target === e.currentTarget) props.onClose(); }}
    >
      <div class="w-[460px] rounded-lg p-6" style={{ background: "var(--bg-primary)" }}>
        <h2 class="mb-1 text-lg font-bold" style={{ color: "var(--text-primary)" }}>
          Create a Room
        </h2>
        <p class="mb-4 text-sm" style={{ color: "var(--text-muted)" }}>
          Create a new room to chat with others.
        </p>

        <Show when={error()}>
          <div class="mb-3 rounded p-2 text-sm" style={{ background: "rgba(242,63,67,0.1)", color: "var(--red-dnd)" }}>
            {error()}
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
          placeholder="my-awesome-room"
          class="mb-3 w-full rounded px-3 py-2 text-sm outline-none"
          style={{ background: "var(--bg-tertiary)", color: "var(--text-primary)" }}
          autofocus
        />

        {/* Topic */}
        <label class="mb-1 block text-xs font-bold uppercase" style={{ color: "var(--text-secondary)" }}>
          Topic (optional)
        </label>
        <input
          type="text"
          value={topic()}
          onInput={(e) => setTopic(e.currentTarget.value)}
          placeholder="What's this room about?"
          class="mb-3 w-full rounded px-3 py-2 text-sm outline-none"
          style={{ background: "var(--bg-tertiary)", color: "var(--text-primary)" }}
        />

        {/* Invite users */}
        <label class="mb-1 block text-xs font-bold uppercase" style={{ color: "var(--text-secondary)" }}>
          Invite Users (optional)
        </label>
        <input
          type="text"
          value={invites()}
          onInput={(e) => setInvites(e.currentTarget.value)}
          placeholder="@user1:server.com, @user2:server.com"
          class="mb-4 w-full rounded px-3 py-2 text-sm outline-none"
          style={{ background: "var(--bg-tertiary)", color: "var(--text-primary)" }}
        />

        {/* Toggles */}
        <div class="mb-4 flex gap-6">
          <label class="flex cursor-pointer items-center gap-2 text-sm" style={{ color: "var(--text-secondary)" }}>
            <input
              type="checkbox"
              checked={isEncrypted()}
              onChange={(e) => setIsEncrypted(e.currentTarget.checked)}
              class="accent-[var(--accent)]"
            />
            Encrypted
          </label>
          <label class="flex cursor-pointer items-center gap-2 text-sm" style={{ color: "var(--text-secondary)" }}>
            <input
              type="checkbox"
              checked={isPublic()}
              onChange={(e) => setIsPublic(e.currentTarget.checked)}
              class="accent-[var(--accent)]"
            />
            Public
          </label>
        </div>

        {/* Buttons */}
        <div class="flex justify-end gap-3">
          <button
            onClick={props.onClose}
            class="rounded px-4 py-2 text-sm"
            style={{ background: "var(--bg-tertiary)", color: "var(--text-secondary)" }}
          >
            Cancel
          </button>
          <button
            onClick={handleCreate}
            disabled={creating() || !name().trim()}
            class="rounded px-4 py-2 text-sm font-medium text-white"
            style={{ background: creating() ? "var(--accent-hover)" : "var(--accent)" }}
          >
            {creating() ? "Creating..." : "Create Room"}
          </button>
        </div>
      </div>
    </div>
  );
}
