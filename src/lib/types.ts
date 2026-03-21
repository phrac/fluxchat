export interface RoomInfo {
  room_id: string;
  name: string;
  is_encrypted: boolean;
  is_space: boolean;
  is_direct: boolean;
  unread_count: number;
  avatar_url: string | null;
  topic: string | null;
  space_name: string | null;
}

export interface MessageInfo {
  event_id: string;
  sender: string;
  sender_name: string;
  body: string;
  formatted_body: string | null;
  image_mxc: string | null;
  reactions: ReactionInfo[];
  reply_to_id: string | null;
  reply_to_sender: string | null;
  reply_to_body: string | null;
  timestamp: number;
  avatar_url: string | null;
}

export interface MemberInfo {
  user_id: string;
  display_name: string;
  avatar_url: string | null;
  power_level: number;
  role: string;
  presence: string;
  server_name: string;
}

export interface ReactionInfo {
  emoji: string;
  count: number;
  users: string[];
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
