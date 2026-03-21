import { For } from "solid-js";
import { spaces, selectedSpaceId, selectSpace } from "../../stores/rooms";

function SpaceIcon(props: {
  name: string;
  active: boolean;
  onClick: () => void;
}) {
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
        class="flex h-12 w-12 items-center justify-center text-sm font-medium transition-all"
        style={{
          "border-radius": props.active ? "16px" : "24px",
          background: props.active ? "var(--accent)" : "var(--bg-primary)",
          color: props.active ? "white" : "var(--text-secondary)",
        }}
        onMouseOver={(e) => {
          if (!props.active) {
            e.currentTarget.style.borderRadius = "16px";
            e.currentTarget.style.background = "var(--accent)";
            e.currentTarget.style.color = "white";
          }
        }}
        onMouseOut={(e) => {
          if (!props.active) {
            e.currentTarget.style.borderRadius = "24px";
            e.currentTarget.style.background = "var(--bg-primary)";
            e.currentTarget.style.color = "var(--text-secondary)";
          }
        }}
        title={props.name}
      >
        {initials()}
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
        active={selectedSpaceId() === null}
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
            active={selectedSpaceId() === space.room_id}
            onClick={() => selectSpace(space.room_id)}
          />
        )}
      </For>
    </div>
  );
}
