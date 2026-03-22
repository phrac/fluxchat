import { Show, For, createSignal, createEffect } from "solid-js";
import { selectedSpaceId, spaces, rooms } from "../../stores/rooms";
import { getMediaUrl, inviteUser } from "../../lib/tauri";
import { Avatar, nameColor } from "./MessageBubble";
import { setShowCreateRoom } from "../layout/MainLayout";

export default function SpaceHome() {
  const [avatarSrc, setAvatarSrc] = createSignal<string | null>(null);
  const [showInvite, setShowInvite] = createSignal(false);
  const [inviteId, setInviteId] = createSignal("");
  const [inviteMsg, setInviteMsg] = createSignal("");

  const space = () => spaces.find((s) => s.room_id === selectedSpaceId());
  const spaceRooms = () => rooms.filter((r) => !r.is_direct);

  createEffect(() => {
    const s = space();
    if (s?.avatar_url) {
      getMediaUrl(s.avatar_url).then(setAvatarSrc).catch(() => setAvatarSrc(null));
    } else {
      setAvatarSrc(null);
    }
  });

  return (
    <div class="flex flex-1 flex-col overflow-y-auto" style={{ background: "var(--bg-primary)" }}>
      {/* Banner */}
      <div
        class="relative h-48 flex-shrink-0"
        style={{ background: "linear-gradient(135deg, #2d3436 0%, #636e72 100%)" }}
      >
        {/* Space avatar centered */}
        <Show when={avatarSrc()}>
          <div class="absolute inset-0 flex items-center justify-center">
            <img
              src={avatarSrc()!}
              class="h-32 w-32 rounded-full border-4 object-cover shadow-2xl"
              style={{ "border-color": "rgba(0,0,0,0.3)" }}
            />
          </div>
        </Show>
      </div>

      {/* Space info */}
      <div class="px-6 py-4">
        <p class="text-sm" style={{ color: "var(--text-muted)" }}>Welcome to</p>
        <h1 class="text-3xl font-bold" style={{ color: "var(--text-primary)" }}>
          {space()?.name || "Space"}
        </h1>
        <Show when={space()?.topic}>
          <p class="mt-1 text-sm" style={{ color: "var(--text-secondary)" }}>
            {space()!.topic}
          </p>
        </Show>

        <div class="mt-2 flex items-center gap-4">
          <div class="flex items-center gap-2">
            <svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor" style={{ color: "var(--text-muted)" }}>
              <path d="M18 8h-1V6c0-2.76-2.24-5-5-5S7 3.24 7 6v2H6c-1.1 0-2 .9-2 2v10c0 1.1.9 2 2 2h12c1.1 0 2-.9 2-2V10c0-1.1-.9-2-2-2zM12 17c-1.1 0-2-.9-2-2s.9-2 2-2 2 .9 2 2-.9 2-2 2zM9 8V6c0-1.66 1.34-3 3-3s3 1.34 3 3v2H9z"/>
            </svg>
            <span class="text-sm" style={{ color: "var(--text-secondary)" }}>
              {space()?.is_encrypted ? "Private space" : "Space"}
            </span>
          </div>

          <button
            onClick={() => setShowInvite(!showInvite())}
            class="flex items-center gap-1.5 rounded px-3 py-1.5 text-sm font-medium transition-colors"
            style={{ background: "var(--accent)", color: "white" }}
            onMouseOver={(e) => (e.currentTarget.style.background = "var(--accent-hover)")}
            onMouseOut={(e) => (e.currentTarget.style.background = "var(--accent)")}
          >
            <svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor">
              <path d="M15 12c2.21 0 4-1.79 4-4s-1.79-4-4-4-4 1.79-4 4 1.79 4 4 4zm-9-2V7H4v3H1v2h3v3h2v-3h3v-2H6zm9 4c-2.67 0-8 1.34-8 4v2h16v-2c0-2.66-5.33-4-8-4z"/>
            </svg>
            Invite People
          </button>
        </div>

        <Show when={showInvite()}>
          <div class="mt-3 flex items-center gap-2">
            <input
              type="text"
              value={inviteId()}
              onInput={(e) => setInviteId(e.currentTarget.value)}
              onKeyDown={async (e) => {
                if (e.key === "Enter" && inviteId().trim()) {
                  try {
                    await inviteUser(selectedSpaceId()!, inviteId().trim());
                    setInviteMsg("Invited!"); setInviteId("");
                    setTimeout(() => setInviteMsg(""), 2000);
                  } catch (err: any) { setInviteMsg(String(err)); }
                }
              }}
              placeholder="@user:server.com"
              class="flex-1 rounded px-3 py-1.5 text-sm outline-none"
              style={{ background: "var(--bg-tertiary)", color: "var(--text-primary)" }}
            />
            <button
              onClick={async () => {
                if (!inviteId().trim()) return;
                try {
                  await inviteUser(selectedSpaceId()!, inviteId().trim());
                  setInviteMsg("Invited!"); setInviteId("");
                  setTimeout(() => setInviteMsg(""), 2000);
                } catch (err: any) { setInviteMsg(String(err)); }
              }}
              class="rounded px-3 py-1.5 text-sm font-medium text-white"
              style={{ background: "var(--accent)" }}
            >
              Send
            </button>
          </div>
          <Show when={inviteMsg()}>
            <p class="mt-1 text-xs" style={{ color: inviteMsg().startsWith("Invited") ? "var(--green-online)" : "var(--red-dnd)" }}>
              {inviteMsg()}
            </p>
          </Show>
        </Show>
      </div>

      {/* Room list */}
      <div class="px-4 pb-6">
        <div class="rounded-xl overflow-hidden" style={{ background: "var(--bg-secondary)" }}>
          <For each={spaceRooms()}>
            {(room) => (
              <div
                class="flex cursor-pointer items-center gap-3 px-4 py-3 transition-colors"
                style={{ "border-bottom": "1px solid rgba(0,0,0,0.1)" }}
                onMouseOver={(e) => (e.currentTarget.style.background = "rgba(79,84,92,0.2)")}
                onMouseOut={(e) => (e.currentTarget.style.background = "transparent")}
                onClick={() => {
                  import("../../stores/rooms").then((mod) => {
                    mod.setSelectedRoomId(room.room_id);
                  });
                  import("../sidebar/RoomList").then((mod) => {
                    mod.setClickedRoomId(room.room_id);
                  });
                }}
              >
                <Avatar
                  mxcUrl={room.avatar_url}
                  name={room.name}
                  color={nameColor(room.name)}
                  size="h-10 w-10"
                />
                <div class="min-w-0 flex-1">
                  <div class="font-medium" style={{ color: "var(--text-primary)" }}>
                    {room.name}
                  </div>
                  <Show when={room.topic}>
                    <div class="truncate text-xs" style={{ color: "var(--text-muted)" }}>
                      {room.topic}
                    </div>
                  </Show>
                </div>
                <Show when={room.is_encrypted}>
                  <svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor" class="opacity-30">
                    <path d="M18 8h-1V6c0-2.76-2.24-5-5-5S7 3.24 7 6v2H6c-1.1 0-2 .9-2 2v10c0 1.1.9 2 2 2h12c1.1 0 2-.9 2-2V10c0-1.1-.9-2-2-2zM12 17c-1.1 0-2-.9-2-2s.9-2 2-2 2 .9 2 2-.9 2-2 2zM9 8V6c0-1.66 1.34-3 3-3s3 1.34 3 3v2H9z"/>
                  </svg>
                </Show>
              </div>
            )}
          </For>

          {/* Add room button */}
          <div
            class="flex cursor-pointer items-center justify-end px-4 py-3"
            onClick={() => setShowCreateRoom(true)}
          >
            <button
              class="flex h-8 w-8 items-center justify-center rounded-full transition-colors"
              style={{ background: "rgba(255,255,255,0.06)", color: "var(--green-online)" }}
              onMouseOver={(e) => { e.currentTarget.style.background = "var(--green-online)"; e.currentTarget.style.color = "white"; }}
              onMouseOut={(e) => { e.currentTarget.style.background = "rgba(255,255,255,0.06)"; e.currentTarget.style.color = "var(--green-online)"; }}
              title="Add room"
            >
              <svg width="16" height="16" viewBox="0 0 24 24" fill="currentColor">
                <path d="M19 13h-6v6h-2v-6H5v-2h6V5h2v6h6v2z"/>
              </svg>
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
