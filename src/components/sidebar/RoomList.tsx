import { For, Show, createMemo, createSignal, createEffect } from "solid-js";
import type { RoomInfo } from "../../lib/types";
import {
  rooms,
  selectedRoomId,
  selectRoom,
  selectedSpaceId,
  spaces,
} from "../../stores/rooms";
import { loadMessages, loadMembers } from "../../stores/messages";
import { Avatar, nameColor } from "../chat/MessageBubble";
import { getMediaUrl } from "../../lib/tauri";

export default function RoomList() {
  const currentSpace = () => {
    if (!selectedSpaceId()) return null;
    return spaces.find((s) => s.room_id === selectedSpaceId()) || null;
  };

  const currentSpaceName = () => currentSpace()?.name || "All Rooms";

  // Resolve space banner image
  const [bannerSrc, setBannerSrc] = createSignal<string | null>(null);
  const bannerCache = new Map<string, string>();

  createEffect(() => {
    const space = currentSpace();
    if (!space?.avatar_url) {
      setBannerSrc(null);
      return;
    }
    const cached = bannerCache.get(space.avatar_url);
    if (cached) {
      setBannerSrc(cached);
      return;
    }
    getMediaUrl(space.avatar_url).then((uri) => {
      bannerCache.set(space.avatar_url!, uri);
      setBannerSrc(uri);
    }).catch(() => setBannerSrc(null));
  });

  const handleRoomClick = (roomId: string) => {
    selectRoom(roomId);
    Promise.all([
      loadMessages(roomId).catch(() => {}),
      loadMembers(roomId).catch(() => {}),
    ]);
  };

  // Group rooms into sections
  const sections = createMemo(() => {
    const dms: RoomInfo[] = [];
    const bySpace: Record<string, RoomInfo[]> = {};
    const ungrouped: RoomInfo[] = [];

    for (const room of rooms) {
      if (room.is_direct) {
        dms.push(room);
      } else if (room.space_name) {
        if (!bySpace[room.space_name]) bySpace[room.space_name] = [];
        bySpace[room.space_name].push(room);
      } else {
        ungrouped.push(room);
      }
    }

    const result: { label: string; rooms: RoomInfo[]; isDm: boolean; avatarMxc: string | null }[] = [];

    // Ungrouped channels first
    if (ungrouped.length > 0) {
      result.push({ label: "Channels", rooms: ungrouped, isDm: false, avatarMxc: null });
    }

    // Then space groups, sorted by name
    const spaceNames = Object.keys(bySpace).sort();
    for (const name of spaceNames) {
      const space = spaces.find((s) => s.name === name);
      result.push({ label: name, rooms: bySpace[name], isDm: false, avatarMxc: space?.avatar_url ?? null });
    }

    // DMs last
    if (dms.length > 0) {
      result.push({ label: "Direct Messages", rooms: dms, isDm: true, avatarMxc: null });
    }

    return result;
  });

  return (
    <div
      class="flex w-60 flex-shrink-0 flex-col"
      style={{ background: "var(--bg-secondary)" }}
    >
      {/* Header / Space Banner */}
      <Show
        when={bannerSrc()}
        fallback={
          <div
            class="flex h-12 flex-shrink-0 items-center px-4 font-semibold shadow-sm"
            style={{
              color: "var(--text-primary)",
              "border-bottom": "1px solid rgba(0,0,0,0.2)",
              "font-size": "0.9375rem",
            }}
          >
            {currentSpaceName()}
          </div>
        }
      >
        <div class="relative flex-shrink-0 overflow-hidden">
          <img
            src={bannerSrc()!}
            alt={currentSpaceName()}
            class="h-24 w-full object-cover"
          />
          <div
            class="absolute inset-0"
            style={{ background: "linear-gradient(transparent 30%, rgba(0,0,0,0.7))" }}
          />
          <span
            class="absolute bottom-2 left-3 font-bold text-white"
            style={{ "font-size": "1.05rem", "text-shadow": "0 1px 3px rgba(0,0,0,0.5)" }}
          >
            {currentSpaceName()}
          </span>
        </div>
      </Show>

      <div class="flex-1 overflow-y-auto px-2 pt-1 pb-2">
        <For each={sections()}>
          {(section) => (
            <>
              <CategoryHeader label={section.label} avatarMxc={section.avatarMxc} />
              <For each={section.rooms}>
                {(room) => (
                  <RoomEntry
                    room={room}
                    isSelected={selectedRoomId() === room.room_id}
                    hasUnread={room.unread_count > 0}
                    isDm={section.isDm}
                    onClick={() => handleRoomClick(room.room_id)}
                  />
                )}
              </For>
            </>
          )}
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

function CategoryHeader(props: { label: string; avatarMxc: string | null }) {
  return (
    <div
      class="mb-1 mt-4 flex items-center gap-1.5 px-1 first:mt-2"
      style={{ color: "var(--text-secondary)" }}
    >
      <Show when={props.avatarMxc}>
        <div class="flex-shrink-0">
          <Avatar
            mxcUrl={props.avatarMxc}
            name={props.label}
            color={nameColor(props.label)}
            size="h-4 w-4"
          />
        </div>
      </Show>
      <span class="text-xs font-bold uppercase tracking-wide truncate">
        {props.label}
      </span>
    </div>
  );
}

function RoomEntry(props: {
  room: RoomInfo;
  isSelected: boolean;
  hasUnread: boolean;
  isDm: boolean;
  onClick: () => void;
}) {
  const room = props.room;

  const displayName = () => {
    if (props.isDm) return room.name;
    return room.name.toLowerCase().replace(/\s+/g, "-");
  };

  return (
    <button
      onClick={props.onClick}
      class="group mb-px flex w-full items-center rounded px-2 py-[5px] text-left transition-colors"
      style={{
        background: props.isSelected ? "rgba(79, 84, 92, 0.6)" : "transparent",
        color: props.isSelected
          ? "var(--text-primary)"
          : props.hasUnread
            ? "var(--text-primary)"
            : "var(--text-muted)",
      }}
      onMouseOver={(e) => {
        if (!props.isSelected) {
          e.currentTarget.style.background = "rgba(79, 84, 92, 0.3)";
          e.currentTarget.style.color = "var(--text-secondary)";
        }
      }}
      onMouseOut={(e) => {
        if (!props.isSelected) {
          e.currentTarget.style.background = "transparent";
          e.currentTarget.style.color = props.hasUnread
            ? "var(--text-primary)"
            : "var(--text-muted)";
        }
      }}
    >
      {/* Icon */}
      <span class="mr-1.5 flex w-5 flex-shrink-0 items-center justify-center">
        {props.isDm ? (
          <RoomAvatar name={room.name} avatarMxc={room.avatar_url} />
        ) : room.avatar_url ? (
          <RoomAvatar name={room.name} avatarMxc={room.avatar_url} />
        ) : room.is_encrypted ? (
          <svg
            width="14"
            height="14"
            viewBox="0 0 24 24"
            fill="currentColor"
            class="opacity-40"
          >
            <path d="M18 8h-1V6c0-2.76-2.24-5-5-5S7 3.24 7 6v2H6c-1.1 0-2 .9-2 2v10c0 1.1.9 2 2 2h12c1.1 0 2-.9 2-2V10c0-1.1-.9-2-2-2zM12 17c-1.1 0-2-.9-2-2s.9-2 2-2 2 .9 2 2-.9 2-2 2zM9 8V6c0-1.66 1.34-3 3-3s3 1.34 3 3v2H9z" />
          </svg>
        ) : (
          <span class="text-[15px] font-light opacity-60">#</span>
        )}
      </span>

      {/* Name */}
      <span
        class="min-w-0 flex-1 truncate"
        style={{
          "font-size": "0.9375rem",
          "font-weight": props.hasUnread || props.isSelected ? "500" : "400",
        }}
      >
        {displayName()}
      </span>

      {/* Unread badge */}
      <Show when={room.unread_count > 0}>
        <span
          class="ml-auto flex h-4 min-w-[16px] flex-shrink-0 items-center justify-center rounded-full px-1 text-[10px] font-bold text-white"
          style={{ background: "var(--red-dnd)" }}
        >
          {room.unread_count}
        </span>
      </Show>
    </button>
  );
}

function RoomAvatar(props: { name: string; avatarMxc: string | null }) {
  return (
    <div class="flex h-5 w-5 items-center justify-center">
      <Avatar
        mxcUrl={props.avatarMxc}
        name={props.name}
        color={nameColor(props.name)}
        size="h-5 w-5"
      />
    </div>
  );
}
