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

export async function selectSpace(spaceId: string | null) {
  setSelectedSpaceId(spaceId);
  if (spaceId) {
    await loadSpaceRooms(spaceId);
  } else {
    const r = await api.getRooms();
    setRooms(r);
  }
}

export async function selectRoom(roomId: string) {
  setSelectedRoomId(roomId);
}

// Listen for room list updates from sync
listen("matrix://room-list-update", () => {
  loadRooms();
});
