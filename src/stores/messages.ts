import { createStore } from "solid-js/store";
import { listen } from "@tauri-apps/api/event";
import * as api from "../lib/tauri";
import type { MessageInfo, MemberInfo } from "../lib/types";

const [messagesByRoom, setMessagesByRoom] = createStore<
  Record<string, MessageInfo[]>
>({});
const [membersByRoom, setMembersByRoom] = createStore<
  Record<string, MemberInfo[]>
>({});
const [endTokens, setEndTokens] = createStore<
  Record<string, string | null>
>({});

export { messagesByRoom, membersByRoom };

export async function loadMessages(roomId: string) {
  const result = await api.getRoomMessages(roomId);
  // Messages come in reverse order (backward pagination), so reverse them
  setMessagesByRoom(roomId, result.messages.reverse());
  setEndTokens(roomId, result.end_token);
}

export async function loadOlderMessages(roomId: string): Promise<boolean> {
  const token = endTokens[roomId];
  if (!token) return false; // no more history

  const result = await api.getRoomMessages(roomId, token);
  if (result.messages.length === 0) return false;

  setMessagesByRoom(roomId, (prev) => [
    ...result.messages.reverse(),
    ...(prev || []),
  ]);
  setEndTokens(roomId, result.end_token);
  return true;
}

export async function sendMessage(roomId: string, body: string) {
  await api.sendMessage(roomId, body);
}

export async function loadMembers(roomId: string) {
  const members = await api.getRoomMembers(roomId);
  setMembersByRoom(roomId, members);
}

// Real-time message updates from sync
listen<{ room_id: string; message: MessageInfo }>(
  "matrix://sync-room-message",
  (event) => {
    const { room_id, message } = event.payload;
    setMessagesByRoom(room_id, (prev) => [...(prev || []), message]);
  }
);
