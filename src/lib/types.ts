export interface RoomInfo {
  room_id: string;
  name: string;
  is_encrypted: boolean;
  is_space: boolean;
  unread_count: number;
  avatar_url: string | null;
  topic: string | null;
}

export interface MessageInfo {
  event_id: string;
  sender: string;
  sender_name: string;
  body: string;
  formatted_body: string | null;
  image_mxc: string | null;
  timestamp: number;
  avatar_url: string | null;
}

export interface MemberInfo {
  user_id: string;
  display_name: string;
  avatar_url: string | null;
}

export interface PaginatedMessages {
  messages: MessageInfo[];
  end_token: string | null;
}

export interface LoginResult {
  user_id: string;
  display_name: string | null;
}

export interface EmojiInfo {
  symbol: string;
  description: string;
}

export interface VerificationInfo {
  flow_id: string;
  other_user_id: string;
  is_self_verification: boolean;
  state: string;
  emojis: EmojiInfo[] | null;
}
