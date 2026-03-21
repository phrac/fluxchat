import { invoke } from "@tauri-apps/api/core";
import type {
  RoomInfo,
  MemberInfo,
  PaginatedMessages,
  LoginResult,
} from "./types";

export async function login(
  homeserver: string,
  username: string,
  password: string
): Promise<LoginResult> {
  return invoke("login", { homeserver, username, password });
}

export async function logout(): Promise<void> {
  return invoke("logout");
}

export async function restoreSession(): Promise<LoginResult | null> {
  return invoke("restore_session");
}

export async function getRooms(): Promise<RoomInfo[]> {
  return invoke("get_rooms");
}

export async function getSpaces(): Promise<RoomInfo[]> {
  return invoke("get_spaces");
}

export async function getSpaceRooms(spaceId: string): Promise<RoomInfo[]> {
  return invoke("get_space_rooms", { spaceId });
}

export async function getRoomMessages(
  roomId: string,
  fromToken?: string
): Promise<PaginatedMessages> {
  return invoke("get_room_messages", { roomId, fromToken: fromToken ?? null });
}

export async function sendMessage(
  roomId: string,
  body: string
): Promise<string> {
  return invoke("send_message", { roomId, body });
}

export async function getRoomMembers(roomId: string): Promise<MemberInfo[]> {
  return invoke("get_room_members", { roomId });
}

export async function joinRoom(roomIdOrAlias: string): Promise<RoomInfo> {
  return invoke("join_room", { roomIdOrAlias });
}

export async function leaveRoom(roomId: string): Promise<void> {
  return invoke("leave_room", { roomId });
}

export async function inviteUser(
  roomId: string,
  userId: string
): Promise<void> {
  return invoke("invite_user", { roomId, userId });
}

export async function getMediaUrl(mxcUri: string): Promise<string> {
  return invoke("get_media_url", { mxcUri });
}

export async function sendReply(
  roomId: string,
  body: string,
  replyToEventId: string
): Promise<string> {
  return invoke("send_reply", { roomId, body, replyToEventId });
}

export async function sendReaction(
  roomId: string,
  eventId: string,
  emoji: string
): Promise<void> {
  return invoke("send_reaction", { roomId, eventId, emoji });
}

export async function deleteMessage(
  roomId: string,
  eventId: string
): Promise<void> {
  return invoke("delete_message", { roomId, eventId });
}

export async function getPinnedMessages(
  roomId: string
): Promise<import("./types").MessageInfo[]> {
  return invoke("get_pinned_messages", { roomId });
}

export async function pinMessage(
  roomId: string,
  eventId: string
): Promise<void> {
  return invoke("pin_message", { roomId, eventId });
}

export async function sendTyping(
  roomId: string,
  typing: boolean
): Promise<void> {
  return invoke("send_typing", { roomId, typing });
}

export async function acceptVerification(
  userId: string,
  flowId: string
): Promise<void> {
  return invoke("accept_verification", { userId, flowId });
}

export async function confirmVerification(
  userId: string,
  flowId: string
): Promise<void> {
  return invoke("confirm_verification", { userId, flowId });
}

export async function cancelVerification(
  userId: string,
  flowId: string
): Promise<void> {
  return invoke("cancel_verification", { userId, flowId });
}
