import { createSignal } from "solid-js";
import { createStore } from "solid-js/store";
import { listen } from "@tauri-apps/api/event";
import * as api from "../lib/tauri";
import type { RoomInfo } from "../lib/types";

const [rooms, setRooms] = createStore<RoomInfo[]>([]);
const [spaces, setSpaces] = createStore<RoomInfo[]>([]);
const [selectedSpaceId, setSelectedSpaceId] = createSignal<string | null>(null);
const [selectedRoomId, setSelectedRoomId] = createSignal<string | null>(null);

export {
  rooms,
  spaces,
  selectedSpaceId,
  selectedRoomId,
  setSelectedSpaceId,
  setSelectedRoomId,
};

export async function loadRooms() {
  const [r, s] = await Promise.all([api.getRooms(), api.getSpaces()]);
  setRooms(r);
  setSpaces(s);
}

export async function loadSpaceRooms(spaceId: string) {
  const r = await api.getSpaceRooms(spaceId);
  setRooms(r);
}

export function selectSpace(spaceId: string | null) {
  setSelectedSpaceId(spaceId);
  setSelectedRoomId(null); // Clear room selection to show space home
  // Fire and forget — don't block UI
  if (spaceId) {
    loadSpaceRooms(spaceId).catch(() => {});
  } else {
    api.getRooms().then(setRooms).catch(() => {});
  }
}

export function selectRoom(roomId: string) {
  // Instant UI update
  setSelectedRoomId(roomId);

  // Update space highlight without blocking
  const room = rooms.find((r) => r.room_id === roomId);
  if (room?.space_name) {
    const space = spaces.find((s) => s.name === room.space_name);
    if (space && selectedSpaceId() !== space.room_id) {
      setSelectedSpaceId(space.room_id);
      // Load space rooms in background — don't await
      loadSpaceRooms(space.room_id).catch(() => {});
    }
  }
}

// Listen for room list updates from sync
listen("matrix://room-list-update", () => {
  // Refresh spaces list always
  api.getSpaces().then(setSpaces).catch(() => {});

  // Only refresh room list if we're on the home view
  // If viewing a space, the space rooms stay as-is until user switches
  if (!selectedSpaceId()) {
    api.getRooms().then(setRooms).catch(() => {});
  }
});
