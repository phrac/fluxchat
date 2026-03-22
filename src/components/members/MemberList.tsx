import { For, Show, createMemo, createSignal, createEffect } from "solid-js";
import type { MemberInfo } from "../../lib/types";
import { membersByRoom } from "../../stores/messages";
import { selectedRoomId } from "../../stores/rooms";
import { userId } from "../../stores/auth";
import { Avatar, nameColor } from "../chat/MessageBubble";
import { kickUser, banUser, setUserPowerLevel, getMediaUrl } from "../../lib/tauri";

const presenceColors: Record<string, string> = {
  online: "var(--green-online)",
  idle: "var(--yellow-idle)",
  offline: "#80848e",
};

const roleIcons: Record<string, string> = {
  Admin: "M12 1L3 5v6c0 5.55 3.84 10.74 9 12 5.16-1.26 9-6.45 9-12V5l-9-4z",
  Moderator: "M12 1L3 5v6c0 5.55 3.84 10.74 9 12 5.16-1.26 9-6.45 9-12V5l-9-4z",
};

export default function MemberList() {
  const members = () => membersByRoom[selectedRoomId()!] || [];
  const [contextMenu, setContextMenu] = createSignal<{ member: MemberInfo; x: number; y: number } | null>(null);
  const [profilePopup, setProfilePopup] = createSignal<MemberInfo | null>(null);
  const [profileAvatar, setProfileAvatar] = createSignal<string | null>(null);

  // Close context menu on click outside or Escape
  createEffect(() => {
    if (contextMenu()) {
      const close = () => setContextMenu(null);
      const handleKey = (e: KeyboardEvent) => { if (e.key === "Escape") close(); };
      setTimeout(() => {
        document.addEventListener("click", close, { once: true });
        document.addEventListener("keydown", handleKey);
      }, 0);
      return () => document.removeEventListener("keydown", handleKey);
    }
  });

  // Close profile popup on Escape only — click handling is done by the member entries
  createEffect(() => {
    if (profilePopup()) {
      const handleKey = (e: KeyboardEvent) => { if (e.key === "Escape") setProfilePopup(null); };
      document.addEventListener("keydown", handleKey);
      return () => document.removeEventListener("keydown", handleKey);
    }
  });

  // Resolve avatar for profile popup
  createEffect(() => {
    const member = profilePopup();
    if (member?.avatar_url) {
      getMediaUrl(member.avatar_url).then(setProfileAvatar).catch(() => setProfileAvatar(null));
    } else {
      setProfileAvatar(null);
    }
  });

  const myPowerLevel = () => {
    const me = members().find((m) => m.user_id === userId());
    return me?.power_level ?? 0;
  };

  const grouped = createMemo(() => {
    const groups: { role: string; members: MemberInfo[] }[] = [];
    const byRole: Record<string, MemberInfo[]> = {};
    for (const m of members()) {
      if (!byRole[m.role]) byRole[m.role] = [];
      byRole[m.role].push(m);
    }
    for (const role of ["Admin", "Moderator", "Member"]) {
      if (byRole[role]?.length) groups.push({ role, members: byRole[role] });
    }
    return groups;
  });

  const handleContextMenu = (e: MouseEvent, member: MemberInfo) => {
    e.preventDefault();
    if (member.user_id === userId()) return; // Can't manage yourself
    setContextMenu({ member, x: e.clientX, y: e.clientY });
    setProfilePopup(null);
  };

  const handleClick = (e: MouseEvent, member: MemberInfo) => {
    e.stopPropagation();
    // Toggle if clicking same member, otherwise switch
    if (profilePopup()?.user_id === member.user_id) {
      setProfilePopup(null);
    } else {
      setProfilePopup(member);
    }
    setContextMenu(null);
  };

  const doAction = async (action: string, member: MemberInfo) => {
    const roomId = selectedRoomId();
    if (!roomId) return;
    setContextMenu(null);
    try {
      switch (action) {
        case "kick": await kickUser(roomId, member.user_id); break;
        case "ban": await banUser(roomId, member.user_id); break;
        case "admin": await setUserPowerLevel(roomId, member.user_id, 100); break;
        case "mod": await setUserPowerLevel(roomId, member.user_id, 50); break;
        case "member": await setUserPowerLevel(roomId, member.user_id, 0); break;
      }
    } catch (err) {
      console.error("Action failed:", err);
    }
  };

  return (
    <div class="flex w-60 flex-shrink-0 flex-col" style={{ background: "var(--bg-secondary)" }}>
      <div class="flex h-12 flex-shrink-0 items-center px-4" style={{ "border-bottom": "1px solid rgba(0,0,0,0.2)" }}>
        <span class="text-xs font-semibold uppercase" style={{ color: "var(--text-secondary)" }}>
          Members — {members().length}
        </span>
      </div>

      <div class="flex-1 overflow-y-auto px-2 py-1">
        <For each={grouped()}>
          {(group) => (
            <>
              <div class="mb-1 mt-3 flex items-center gap-1.5 px-2 first:mt-2" style={{ color: "var(--text-secondary)" }}>
                <Show when={roleIcons[group.role]}>
                  <svg width="12" height="12" viewBox="0 0 24 24"
                    fill={group.role === "Admin" ? "var(--yellow-idle)" : "var(--text-muted)"}>
                    <path d={roleIcons[group.role]} />
                  </svg>
                </Show>
                <span class="text-[11px] font-bold uppercase tracking-wide">
                  {group.role} — {group.members.length}
                </span>
              </div>
              <For each={group.members}>
                {(member) => (
                  <MemberEntry
                    member={member}
                    onClick={(e) => handleClick(e, member)}
                    onContextMenu={(e) => handleContextMenu(e, member)}
                  />
                )}
              </For>
            </>
          )}
        </For>

        <Show when={members().length === 0}>
          <p class="px-2 py-4 text-center text-sm" style={{ color: "var(--text-muted)" }}>
            Loading members...
          </p>
        </Show>
      </div>

      {/* Context menu */}
      <Show when={contextMenu()}>
        <div
          class="fixed z-50 w-48 rounded-md py-1 shadow-xl"
          style={{
            background: "var(--bg-tertiary)",
            border: "1px solid rgba(0,0,0,0.3)",
            left: `${Math.min(contextMenu()!.x, window.innerWidth - 200)}px`,
            top: `${Math.min(contextMenu()!.y, window.innerHeight - 200)}px`,
          }}
          onClick={(e) => e.stopPropagation()}
        >
          <div class="px-3 py-1.5 text-xs font-bold" style={{ color: "var(--text-muted)" }}>
            {contextMenu()!.member.display_name}
          </div>
          <div class="mx-2 my-1 h-px" style={{ background: "var(--border-subtle)" }} />

          <Show when={myPowerLevel() >= 100}>
            <CtxItem label="Make Admin" onClick={() => doAction("admin", contextMenu()!.member)} />
          </Show>
          <Show when={myPowerLevel() >= 50}>
            <CtxItem label="Make Moderator" onClick={() => doAction("mod", contextMenu()!.member)} />
            <CtxItem label="Set as Member" onClick={() => doAction("member", contextMenu()!.member)} />
          </Show>
          <Show when={myPowerLevel() >= 50}>
            <div class="mx-2 my-1 h-px" style={{ background: "var(--border-subtle)" }} />
            <CtxItem label="Kick" onClick={() => doAction("kick", contextMenu()!.member)} danger />
          </Show>
          <Show when={myPowerLevel() >= 50}>
            <CtxItem label="Ban" onClick={() => doAction("ban", contextMenu()!.member)} danger />
          </Show>
        </div>
      </Show>

      {/* Profile popup */}
      <Show when={profilePopup()}>
        <div
          class="fixed z-50 w-72 overflow-hidden rounded-xl shadow-2xl"
          style={{
            background: "var(--bg-tertiary)",
            right: "260px",
            top: "100px",
          }}
          onClick={(e) => e.stopPropagation()}
        >
          {/* Banner */}
          <div class="h-16" style={{ background: `linear-gradient(135deg, ${nameColor(profilePopup()!.user_id)}, #8b5cf6)` }} />

          {/* Avatar */}
          <div class="px-4 -mt-8">
            <div class="h-16 w-16 overflow-hidden rounded-full border-4" style={{ "border-color": "var(--bg-tertiary)" }}>
              <Show when={profileAvatar()} fallback={
                <div class="flex h-full w-full items-center justify-center text-xl font-bold text-white"
                  style={{ background: nameColor(profilePopup()!.user_id) }}>
                  {profilePopup()!.display_name[0]?.toUpperCase() || "?"}
                </div>
              }>
                <img src={profileAvatar()!} class="h-full w-full object-cover" />
              </Show>
            </div>
          </div>

          {/* Info */}
          <div class="px-4 pt-2 pb-4">
            <div class="text-base font-bold" style={{ color: nameColor(profilePopup()!.user_id) }}>
              {profilePopup()!.display_name}
            </div>
            <div class="text-xs" style={{ color: "var(--text-muted)" }}>
              {profilePopup()!.user_id}
            </div>

            <div class="mt-3 rounded-lg p-3" style={{ background: "var(--bg-primary)" }}>
              <div class="mb-1 text-[11px] font-bold uppercase" style={{ color: "var(--text-muted)" }}>Role</div>
              <div class="text-sm" style={{ color: "var(--text-primary)" }}>{profilePopup()!.role}</div>

              <div class="mt-2 mb-1 text-[11px] font-bold uppercase" style={{ color: "var(--text-muted)" }}>Server</div>
              <div class="text-sm" style={{ color: "var(--text-primary)" }}>{profilePopup()!.server_name}</div>

              <div class="mt-2 mb-1 text-[11px] font-bold uppercase" style={{ color: "var(--text-muted)" }}>Status</div>
              <div class="flex items-center gap-1.5 text-sm" style={{ color: "var(--text-primary)" }}>
                <div class="h-2.5 w-2.5 rounded-full" style={{ background: presenceColors[profilePopup()!.presence] || presenceColors.offline }} />
                {profilePopup()!.presence === "online" ? "Online" : profilePopup()!.presence === "idle" ? "Idle" : "Offline"}
              </div>
            </div>
          </div>
        </div>
      </Show>
    </div>
  );
}

function MemberEntry(props: { member: MemberInfo; onClick: (e: MouseEvent) => void; onContextMenu: (e: MouseEvent) => void }) {
  const m = props.member;
  const presenceColor = () => presenceColors[m.presence] || presenceColors.offline;

  return (
    <div
      class="mb-px flex cursor-pointer items-center rounded px-2 py-1.5 transition-colors"
      onClick={props.onClick}
      onContextMenu={props.onContextMenu}
      onMouseOver={(e) => (e.currentTarget.style.background = "rgba(79, 84, 92, 0.3)")}
      onMouseOut={(e) => (e.currentTarget.style.background = "transparent")}
    >
      <div class="relative mr-2.5 flex-shrink-0">
        <Avatar mxcUrl={m.avatar_url} name={m.display_name} color={nameColor(m.user_id)} size="h-8 w-8" />
        <div class="absolute -bottom-0.5 -right-0.5 h-3 w-3 rounded-full border-2"
          style={{ background: presenceColor(), "border-color": "var(--bg-secondary)" }} />
      </div>
      <div class="min-w-0 flex-1">
        <div class="truncate text-sm font-medium"
          style={{ color: m.presence === "offline" ? "var(--text-muted)" : "var(--text-primary)" }}>
          {m.display_name}
        </div>
        <div class="truncate text-[11px]" style={{ color: "var(--text-muted)" }}>{m.server_name}</div>
      </div>
    </div>
  );
}

function CtxItem(props: { label: string; onClick: () => void; danger?: boolean }) {
  return (
    <button
      onClick={props.onClick}
      class="flex w-full items-center px-3 py-1.5 text-sm transition-colors"
      style={{ color: props.danger ? "var(--red-dnd)" : "var(--text-normal)" }}
      onMouseOver={(e) => (e.currentTarget.style.background = props.danger ? "rgba(242,63,67,0.1)" : "rgba(79,84,92,0.3)")}
      onMouseOut={(e) => (e.currentTarget.style.background = "transparent")}
    >
      {props.label}
    </button>
  );
}
