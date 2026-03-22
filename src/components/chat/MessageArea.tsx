import { Show, createMemo, createSignal } from "solid-js";
import { selectedRoomId, rooms } from "../../stores/rooms";
import { activeThreadId, setActiveThreadId } from "../layout/MainLayout";
import { typingByRoom, messagesByRoom } from "../../stores/messages";
import { inviteUser } from "../../lib/tauri";
import MessageList from "./MessageList";
import MessageInput from "./MessageInput";

interface Props {
  showMembers: boolean;
  onToggleMembers: () => void;
  showPins: boolean;
  onTogglePins: () => void;
}

export default function MessageArea(props: Props) {
  const currentRoom = () => rooms.find((r) => r.room_id === selectedRoomId());
  const [showInvite, setShowInvite] = createSignal(false);
  const [showSearch, setShowSearch] = createSignal(false);
  const [inviteId, setInviteId] = createSignal("");
  const [inviteError, setInviteError] = createSignal("");
  const [searchQuery, setSearchQuery] = createSignal("");

  const typingUsers = createMemo(() => {
    const roomId = selectedRoomId();
    if (!roomId) return [];
    return typingByRoom[roomId] || [];
  });

  const typingText = createMemo(() => {
    const users = typingUsers();
    if (users.length === 0) return null;
    const names = users.map((u) => u.replace(/^@/, "").split(":")[0]);
    if (names.length === 1) return `${names[0]} is typing...`;
    if (names.length === 2) return `${names[0]} and ${names[1]} are typing...`;
    return `${names[0]} and ${names.length - 1} others are typing...`;
  });

  // Simple message search (client-side for now) — only compute when search is active
  const searchResults = createMemo(() => {
    if (!showSearch()) return [];
    const q = searchQuery().toLowerCase().trim();
    if (!q) return [];
    const roomId = selectedRoomId();
    if (!roomId) return [];
    const msgs = messagesByRoom[roomId] || [];
    return msgs.filter((m) => m.body.toLowerCase().includes(q)).slice(-20);
  });

  const handleInvite = async () => {
    const id = inviteId().trim();
    const roomId = selectedRoomId();
    if (!id || !roomId) return;
    setInviteError("");
    try {
      await inviteUser(roomId, id);
      setInviteId("");
      setShowInvite(false);
    } catch (err: any) {
      setInviteError(String(err));
    }
  };

  function HeaderButton(btnProps: {
    active?: boolean;
    title: string;
    onClick: () => void;
    children: any;
  }) {
    return (
      <button
        onClick={btnProps.onClick}
        class="rounded p-1.5 transition-colors"
        style={{
          color: btnProps.active ? "var(--text-primary)" : "var(--text-muted)",
        }}
        onMouseOver={(e) =>
          (e.currentTarget.style.background = "rgba(79, 84, 92, 0.3)")
        }
        onMouseOut={(e) =>
          (e.currentTarget.style.background = "transparent")
        }
        title={btnProps.title}
      >
        {btnProps.children}
      </button>
    );
  }

  return (
    <div
      class="flex min-w-0 flex-1 flex-col"
      style={{ background: "var(--bg-primary)" }}
    >
      {/* Header bar */}
      <div
        class="flex h-12 flex-shrink-0 items-center justify-between px-4 shadow-sm"
        style={{ "border-bottom": "1px solid rgba(0,0,0,0.2)" }}
      >
        <div class="flex items-center min-w-0">
          <Show when={currentRoom()}>
            <span style={{ color: "var(--text-muted)" }} class="mr-2 text-lg">
              #
            </span>
            <span
              class="font-semibold"
              style={{ color: "var(--text-primary)" }}
            >
              {currentRoom()!.name}
            </span>
            <Show when={currentRoom()!.topic}>
              <div
                class="mx-3 h-5 w-px flex-shrink-0"
                style={{ background: "var(--border-subtle)" }}
              />
              <span
                class="truncate text-sm"
                style={{ color: "var(--text-muted)" }}
              >
                {currentRoom()!.topic}
              </span>
            </Show>
          </Show>
        </div>

        <Show when={selectedRoomId()}>
          <div class="flex items-center gap-0.5">
            {/* Invite */}
            <HeaderButton
              title="Invite"
              active={showInvite()}
              onClick={() => { setShowInvite(!showInvite()); setShowSearch(false); setShowPins(false); }}
            >
              <svg width="20" height="20" viewBox="0 0 24 24" fill="currentColor">
                <path d="M15 12c2.21 0 4-1.79 4-4s-1.79-4-4-4-4 1.79-4 4 1.79 4 4 4zm-9-2V7H4v3H1v2h3v3h2v-3h3v-2H6zm9 4c-2.67 0-8 1.34-8 4v2h16v-2c0-2.66-5.33-4-8-4z"/>
              </svg>
            </HeaderButton>

            {/* Threads */}
            <HeaderButton
              title="Threads"
              active={!!activeThreadId()}
              onClick={() => {
                if (activeThreadId()) {
                  setActiveThreadId(null);
                } else {
                  // Open thread on last message
                  const msgs = messagesByRoom[selectedRoomId()!] || [];
                  const lastReal = [...msgs].reverse().find(m => !m.event_id.startsWith("~"));
                  if (lastReal) setActiveThreadId(lastReal.event_id);
                }
                setShowSearch(false); setShowInvite(false);
              }}
            >
              <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                <path d="M12 2a4 4 0 0 0-4 4c0 1.5.8 2.8 2 3.5v1L8 12l2 1.5v1c-1.2.7-2 2-2 3.5a4 4 0 0 0 8 0c0-1.5-.8-2.8-2-3.5v-1l2-1.5-2-1.5v-1c1.2-.7 2-2 2-3.5a4 4 0 0 0-4-4z"/>
              </svg>
            </HeaderButton>

            {/* Pins */}
            <HeaderButton
              title="Pinned Messages"
              active={props.showPins}
              onClick={() => { props.onTogglePins(); setShowSearch(false); setShowInvite(false); }}
            >
              <svg width="20" height="20" viewBox="0 0 24 24" fill="currentColor">
                <path d="M16 12V4h1V2H7v2h1v8l-2 2v2h5.2v6h1.6v-6H18v-2l-2-2z"/>
              </svg>
            </HeaderButton>

            {/* Search */}
            <HeaderButton
              title="Search"
              active={showSearch()}
              onClick={() => { setShowSearch(!showSearch()); setShowInvite(false); setShowPins(false); }}
            >
              <svg width="20" height="20" viewBox="0 0 24 24" fill="currentColor">
                <path d="M15.5 14h-.79l-.28-.27C15.41 12.59 16 11.11 16 9.5 16 5.91 13.09 3 9.5 3S3 5.91 3 9.5 5.91 16 9.5 16c1.61 0 3.09-.59 4.23-1.57l.27.28v.79l5 4.99L20.49 19l-4.99-5zm-6 0C7.01 14 5 11.99 5 9.5S7.01 5 9.5 5 14 7.01 14 9.5 11.99 14 9.5 14z"/>
              </svg>
            </HeaderButton>

            {/* Members toggle */}
            <HeaderButton
              title="Members"
              active={props.showMembers}
              onClick={props.onToggleMembers}
            >
              <svg width="20" height="20" viewBox="0 0 24 24" fill="currentColor">
                <path d="M14 8.00598C14 10.211 12.206 12.006 10 12.006C7.795 12.006 6 10.211 6 8.00598C6 5.80098 7.794 4.00598 10 4.00598C12.206 4.00598 14 5.80098 14 8.00598ZM2 19.006C2 15.473 5.29 13.006 10 13.006C14.711 13.006 18 15.473 18 19.006V20.006H2V19.006ZM20 20.006H19V18.006C19 16.345 18.22 14.845 16.826 13.675C19.305 14.117 22 15.573 22 18.006V20.006H20ZM14.218 12.006C14.684 11.373 15 10.588 15 9.68498C15 9.18098 14.897 8.70498 14.719 8.27098C15.449 7.65198 16.392 7.26898 17.428 7.26898C19.318 7.26898 20.858 8.80898 20.858 10.699C20.858 12.589 19.318 14.129 17.428 14.129C16.238 14.129 15.19 13.449 14.218 12.006Z" />
              </svg>
            </HeaderButton>
          </div>
        </Show>
      </div>

      {/* Invite modal */}
      <Show when={showInvite()}>
        <div
          class="fixed inset-0 z-50 flex items-center justify-center"
          style={{ background: "rgba(0,0,0,0.6)" }}
          onClick={(e) => { if (e.target === e.currentTarget) setShowInvite(false); }}
        >
          <div
            class="w-[420px] rounded-lg p-6"
            style={{ background: "var(--bg-primary)" }}
          >
            <h2
              class="mb-1 text-lg font-bold"
              style={{ color: "var(--text-primary)" }}
            >
              Invite to {currentRoom()?.name}
            </h2>
            <p class="mb-4 text-sm" style={{ color: "var(--text-muted)" }}>
              Enter the Matrix user ID of the person you want to invite.
            </p>

            <Show when={inviteError()}>
              <div
                class="mb-3 rounded p-2 text-sm"
                style={{ background: "rgba(242, 63, 67, 0.1)", color: "var(--red-dnd)" }}
              >
                {inviteError()}
              </div>
            </Show>

            <input
              type="text"
              value={inviteId()}
              onInput={(e) => setInviteId(e.currentTarget.value)}
              onKeyDown={(e) => e.key === "Enter" && handleInvite()}
              placeholder="@user:server.com"
              class="mb-4 w-full rounded px-3 py-2 text-sm outline-none"
              style={{ background: "var(--bg-tertiary)", color: "var(--text-primary)" }}
              autofocus
            />

            <div class="flex justify-end gap-3">
              <button
                onClick={() => { setShowInvite(false); setInviteError(""); }}
                class="rounded px-4 py-2 text-sm"
                style={{ background: "var(--bg-tertiary)", color: "var(--text-secondary)" }}
              >
                Cancel
              </button>
              <button
                onClick={handleInvite}
                class="rounded px-4 py-2 text-sm font-medium text-white"
                style={{ background: "var(--accent)" }}
              >
                Send Invite
              </button>
            </div>
          </div>
        </div>
      </Show>

      {/* Search panel */}
      <Show when={showSearch()}>
        <div
          class="flex-shrink-0 px-4 py-2"
          style={{ background: "var(--bg-secondary)", "border-bottom": "1px solid rgba(0,0,0,0.2)" }}
        >
          <input
            type="text"
            value={searchQuery()}
            onInput={(e) => setSearchQuery(e.currentTarget.value)}
            placeholder="Search messages..."
            class="w-full rounded px-3 py-1.5 text-sm outline-none"
            style={{ background: "var(--bg-tertiary)", color: "var(--text-primary)" }}
          />
          <Show when={searchQuery().trim() && searchResults().length > 0}>
            <div class="mt-2 max-h-48 overflow-y-auto rounded" style={{ background: "var(--bg-tertiary)" }}>
              {searchResults().map((m) => (
                <div class="px-3 py-1.5 text-sm" style={{ "border-bottom": "1px solid rgba(0,0,0,0.1)" }}>
                  <span class="font-medium" style={{ color: "var(--accent)" }}>{m.sender_name}</span>
                  <span class="ml-2" style={{ color: "var(--text-normal)" }}>{m.body.slice(0, 100)}</span>
                </div>
              ))}
            </div>
          </Show>
          <Show when={searchQuery().trim() && searchResults().length === 0}>
            <p class="mt-2 text-xs" style={{ color: "var(--text-muted)" }}>No results found</p>
          </Show>
        </div>
      </Show>


      {/* Content — always mounted to avoid DOM thrashing on room switch */}
      <div
        class="flex flex-1 flex-col overflow-hidden"
        style={{ display: selectedRoomId() ? "flex" : "none" }}
      >
        <MessageList roomId={selectedRoomId() || ""} />

        <Show when={typingText()}>
          <div
            class="flex-shrink-0 px-4 pb-1"
            style={{ color: "var(--text-muted)" }}
          >
            <span class="text-xs font-medium">
              <span class="inline-flex gap-0.5 mr-1">
                <span class="animate-bounce" style={{ "animation-delay": "0ms" }}>.</span>
                <span class="animate-bounce" style={{ "animation-delay": "150ms" }}>.</span>
                <span class="animate-bounce" style={{ "animation-delay": "300ms" }}>.</span>
              </span>
              {typingText()}
            </span>
          </div>
        </Show>

        <MessageInput roomId={selectedRoomId() || ""} />
      </div>

      <div
        class="flex flex-1 items-center justify-center"
        style={{
          color: "var(--text-muted)",
          display: selectedRoomId() ? "none" : "flex",
        }}
      >
        <div class="text-center">
          <p class="mb-2 text-4xl">💬</p>
          <p class="text-lg">Select a room to start chatting</p>
        </div>
      </div>
    </div>
  );
}
