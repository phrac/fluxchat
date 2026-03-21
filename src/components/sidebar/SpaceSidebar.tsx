import { For, Show, createSignal, createEffect } from "solid-js";
import { spaces, selectedSpaceId, selectSpace } from "../../stores/rooms";
import { getMediaUrl } from "../../lib/tauri";

const mediaCache = new Map<string, string>();

function SpaceIcon(props: {
  name: string;
  avatarMxc: string | null;
  active: boolean;
  isHome?: boolean;
  onClick: () => void;
}) {
  const [avatarSrc, setAvatarSrc] = createSignal<string | null>(null);

  createEffect(() => {
    const mxc = props.avatarMxc;
    if (!mxc) { setAvatarSrc(null); return; }
    const cached = mediaCache.get(mxc);
    if (cached) { setAvatarSrc(cached); return; }
    getMediaUrl(mxc).then((uri) => {
      mediaCache.set(mxc, uri);
      setAvatarSrc(uri);
    }).catch(() => setAvatarSrc(null));
  });

  const initials = () => {
    const words = props.name.split(/\s+/);
    if (words.length >= 2) {
      return (words[0][0] + words[1][0]).toUpperCase();
    }
    return props.name.slice(0, 2).toUpperCase();
  };

  return (
    <div class="group relative mb-2 flex justify-center">
      {/* Active pill indicator */}
      <div
        class="absolute left-0 top-1/2 w-1 -translate-y-1/2 rounded-r transition-all"
        style={{
          height: props.active ? "40px" : "0px",
          background: "var(--text-primary)",
        }}
        classList={{
          "group-hover:!h-[20px]": !props.active,
        }}
      />
      <button
        onClick={props.onClick}
        class="flex h-12 w-12 items-center justify-center overflow-hidden text-sm font-medium transition-all"
        style={{
          "border-radius": props.active ? "16px" : "24px",
          background: avatarSrc() ? "transparent" : props.active ? "var(--accent)" : "var(--bg-primary)",
          color: props.active ? "white" : "var(--text-secondary)",
        }}
        onMouseOver={(e) => {
          if (!props.active) {
            e.currentTarget.style.borderRadius = "16px";
            if (!avatarSrc()) {
              e.currentTarget.style.background = "var(--accent)";
              e.currentTarget.style.color = "white";
            }
          }
        }}
        onMouseOut={(e) => {
          if (!props.active) {
            e.currentTarget.style.borderRadius = "24px";
            if (!avatarSrc()) {
              e.currentTarget.style.background = "var(--bg-primary)";
              e.currentTarget.style.color = "var(--text-secondary)";
            }
          }
        }}
        title={props.name}
      >
        <Show when={props.isHome}>
          <svg width="24" height="24" viewBox="0 0 24 24" fill="currentColor">
            <path d="M10 20v-6h4v6h5v-8h3L12 3 2 12h3v8z"/>
          </svg>
        </Show>
        <Show when={!props.isHome && avatarSrc()}>
          <img
            src={avatarSrc()!}
            alt={props.name}
            class="h-full w-full object-cover"
          />
        </Show>
        <Show when={!props.isHome && !avatarSrc()}>
          {initials()}
        </Show>
      </button>
    </div>
  );
}

export default function SpaceSidebar() {
  return (
    <div
      class="flex w-[72px] flex-shrink-0 flex-col items-center overflow-y-auto py-3"
      style={{ background: "var(--bg-tertiary)" }}
    >
      {/* Home button */}
      <SpaceIcon
        name="Home"
        avatarMxc={null}
        active={selectedSpaceId() === null}
        isHome={true}
        onClick={() => selectSpace(null)}
      />

      {/* Separator */}
      <div
        class="mx-auto mb-2 h-[2px] w-8 rounded"
        style={{ background: "var(--border-subtle)" }}
      />

      {/* Space icons */}
      <For each={spaces}>
        {(space) => (
          <SpaceIcon
            name={space.name}
            avatarMxc={space.avatar_url}
            active={selectedSpaceId() === space.room_id}
            onClick={() => selectSpace(space.room_id)}
          />
        )}
      </For>
    </div>
  );
}
