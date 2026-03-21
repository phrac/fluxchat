import { createSignal } from "solid-js";
import { createStore } from "solid-js/store";
import { listen } from "@tauri-apps/api/event";
import * as api from "../lib/tauri";
import type { MessageInfo, MemberInfo } from "../lib/types";
import { userId } from "./auth";

const [messagesByRoom, setMessagesByRoom] = createStore<
  Record<string, MessageInfo[]>
>({});
const [membersByRoom, setMembersByRoom] = createStore<
  Record<string, MemberInfo[]>
>({});
const [endTokens, setEndTokens] = createStore<
  Record<string, string | null>
>({});
const [typingByRoom, setTypingByRoom] = createStore<
  Record<string, string[]>
>({});
const [replyingTo, setReplyingTo] = createSignal<MessageInfo | null>(null);

export { messagesByRoom, membersByRoom, typingByRoom, replyingTo, setReplyingTo, insertSystemMessage };

export async function loadMessages(roomId: string) {
  const result = await api.getRoomMessages(roomId);
  const messages = enrichReplies(result.messages.reverse());
  setMessagesByRoom(roomId, messages);
  setEndTokens(roomId, result.end_token);
}

export async function loadOlderMessages(roomId: string): Promise<boolean> {
  const token = endTokens[roomId];
  if (!token) return false;

  const result = await api.getRoomMessages(roomId, token);
  if (result.messages.length === 0) return false;

  const older = enrichReplies(result.messages.reverse());
  setMessagesByRoom(roomId, (prev) => [...older, ...(prev || [])]);
  setEndTokens(roomId, result.end_token);
  return true;
}

export async function sendMessage(roomId: string, body: string) {
  const reply = replyingTo();

  // Optimistic local insert — show immediately
  const tempId = `~local-${Date.now()}`;
  const optimistic: MessageInfo = {
    event_id: tempId,
    sender: userId(),
    sender_name: userId().replace(/^@/, "").split(":")[0],
    body,
    formatted_body: null,
    image_mxc: null,
    reply_to_id: reply?.event_id ?? null,
    reply_to_sender: reply?.sender_name ?? null,
    reply_to_body: reply?.body ?? null,
    timestamp: Math.floor(Date.now() / 1000),
    avatar_url: null,
  };
  setMessagesByRoom(roomId, (prev) => [...(prev || []), optimistic]);

  if (reply) {
    setReplyingTo(null);
    await api.sendReply(roomId, body, reply.event_id);
  } else {
    await api.sendMessage(roomId, body);
  }
}

export async function loadMembers(roomId: string) {
  const members = await api.getRoomMembers(roomId);
  setMembersByRoom(roomId, members);
}

function insertSystemMessage(roomId: string, text: string) {
  const msg: MessageInfo = {
    event_id: `~system-${Date.now()}`,
    sender: "",
    sender_name: "",
    body: text,
    formatted_body: null,
    image_mxc: null,
    reactions: [],
    reply_to_id: null,
    reply_to_sender: null,
    reply_to_body: null,
    timestamp: Math.floor(Date.now() / 1000),
    avatar_url: null,
  };
  setMessagesByRoom(roomId, (prev) => [...(prev || []), msg]);
}

/// Enrich messages with reply context from the same batch
function enrichReplies(messages: MessageInfo[]): MessageInfo[] {
  const byId = new Map<string, MessageInfo>();
  for (const m of messages) {
    byId.set(m.event_id, m);
  }

  return messages.map((m) => {
    if (m.reply_to_id && !m.reply_to_body) {
      const parent = byId.get(m.reply_to_id);
      if (parent) {
        return {
          ...m,
          reply_to_sender: parent.sender_name,
          reply_to_body: parent.body,
        };
      }
    }
    return m;
  });
}

// Real-time message updates from sync
listen<{ room_id: string; message: MessageInfo }>(
  "matrix://sync-room-message",
  (event) => {
    const { room_id, message } = event.payload;

    // Enrich reply from existing messages
    if (message.reply_to_id && !message.reply_to_body) {
      const existing = messagesByRoom[room_id] || [];
      const parent = existing.find((m) => m.event_id === message.reply_to_id);
      if (parent) {
        message.reply_to_sender = parent.sender_name;
        message.reply_to_body = parent.body;
      }
    }

    setMessagesByRoom(room_id, (prev) => {
      const existing = prev || [];
      // Remove optimistic message if this is the real version
      const filtered = existing.filter(
        (m) => !(m.event_id.startsWith("~local-") && m.sender === message.sender && m.body === message.body)
      );
      // Don't add if already present (dedup by event_id)
      if (filtered.some((m) => m.event_id === message.event_id)) return filtered;
      return [...filtered, message];
    });
  }
);

// Reaction updates
listen<{ room_id: string; event_id: string; emoji: string; sender: string }>(
  "matrix://reaction",
  (event) => {
    const { room_id, event_id, emoji, sender } = event.payload;
    setMessagesByRoom(room_id, (prev) =>
      (prev || []).map((m) => {
        if (m.event_id !== event_id) return m;
        const reactions = [...m.reactions];
        const existing = reactions.find((r) => r.emoji === emoji);
        if (existing) {
          if (!existing.users.includes(sender)) {
            existing.count++;
            existing.users.push(sender);
          }
        } else {
          reactions.push({ emoji, count: 1, users: [sender] });
        }
        return { ...m, reactions };
      })
    );
  }
);

// Typing indicator updates
listen<{ room_id: string; user_ids: string[] }>(
  "matrix://typing-indicator",
  (event) => {
    setTypingByRoom(event.payload.room_id, event.payload.user_ids);
  }
);
