import { For, Show, createMemo } from "solid-js";
import type { MemberInfo } from "../../lib/types";
import { membersByRoom } from "../../stores/messages";
import { selectedRoomId } from "../../stores/rooms";
import { Avatar, nameColor } from "../chat/MessageBubble";

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

  // Group by role
  const grouped = createMemo(() => {
    const groups: { role: string; members: MemberInfo[] }[] = [];
    const byRole: Record<string, MemberInfo[]> = {};

    for (const m of members()) {
      if (!byRole[m.role]) byRole[m.role] = [];
      byRole[m.role].push(m);
    }

    // Order: Admin, Moderator, Member
    for (const role of ["Admin", "Moderator", "Member"]) {
      if (byRole[role]?.length) {
        groups.push({ role, members: byRole[role] });
      }
    }

    return groups;
  });

  return (
    <div
      class="flex w-60 flex-shrink-0 flex-col"
      style={{ background: "var(--bg-secondary)" }}
    >
      <div
        class="flex h-12 flex-shrink-0 items-center px-4"
        style={{ "border-bottom": "1px solid rgba(0,0,0,0.2)" }}
      >
        <span
          class="text-xs font-semibold uppercase"
          style={{ color: "var(--text-secondary)" }}
        >
          Members — {members().length}
        </span>
      </div>

      <div class="flex-1 overflow-y-auto px-2 py-1">
        <For each={grouped()}>
          {(group) => (
            <>
              {/* Role header */}
              <div
                class="mb-1 mt-3 flex items-center gap-1.5 px-2 first:mt-2"
                style={{ color: "var(--text-secondary)" }}
              >
                <Show when={roleIcons[group.role]}>
                  <svg
                    width="12"
                    height="12"
                    viewBox="0 0 24 24"
                    fill={group.role === "Admin" ? "var(--yellow-idle)" : "var(--text-muted)"}
                  >
                    <path d={roleIcons[group.role]} />
                  </svg>
                </Show>
                <span class="text-[11px] font-bold uppercase tracking-wide">
                  {group.role} — {group.members.length}
                </span>
              </div>

              <For each={group.members}>
                {(member) => <MemberEntry member={member} />}
              </For>
            </>
          )}
        </For>

        <Show when={members().length === 0}>
          <p
            class="px-2 py-4 text-center text-sm"
            style={{ color: "var(--text-muted)" }}
          >
            Loading members...
          </p>
        </Show>
      </div>
    </div>
  );
}

function MemberEntry(props: { member: MemberInfo }) {
  const m = props.member;
  const presenceColor = () => presenceColors[m.presence] || presenceColors.offline;

  return (
    <div
      class="mb-px flex items-center rounded px-2 py-1.5 transition-colors"
      onMouseOver={(e) =>
        (e.currentTarget.style.background = "rgba(79, 84, 92, 0.3)")
      }
      onMouseOut={(e) =>
        (e.currentTarget.style.background = "transparent")
      }
    >
      {/* Avatar with presence dot */}
      <div class="relative mr-2.5 flex-shrink-0">
        <Avatar
          mxcUrl={m.avatar_url}
          name={m.display_name}
          color={nameColor(m.user_id)}
          size="h-8 w-8"
        />
        {/* Presence indicator */}
        <div
          class="absolute -bottom-0.5 -right-0.5 h-3 w-3 rounded-full border-2"
          style={{
            background: presenceColor(),
            "border-color": "var(--bg-secondary)",
          }}
        />
      </div>

      {/* Name + server */}
      <div class="min-w-0 flex-1">
        <div
          class="truncate text-sm font-medium"
          style={{ color: m.presence === "offline" ? "var(--text-muted)" : "var(--text-primary)" }}
        >
          {m.display_name}
        </div>
        <div
          class="truncate text-[11px]"
          style={{ color: "var(--text-muted)" }}
        >
          {m.server_name}
        </div>
      </div>
    </div>
  );
}
