import { createSignal, onMount, Show } from "solid-js";
import SpaceSidebar from "../sidebar/SpaceSidebar";
import RoomList from "../sidebar/RoomList";
import MessageArea from "../chat/MessageArea";
import MemberList from "../members/MemberList";
import PinnedSidebar from "../chat/PinnedSidebar";
import ThreadPanel from "../chat/ThreadPanel";
import { loadRooms, selectedRoomId } from "../../stores/rooms";

// Export thread state so MessageBubble can open threads
const [activeThreadId, setActiveThreadId] = createSignal<string | null>(null);
export { activeThreadId, setActiveThreadId };

export default function MainLayout() {
  const [showMembers, setShowMembers] = createSignal(true);
  const [showPins, setShowPins] = createSignal(false);

  onMount(() => {
    loadRooms();
  });

  return (
    <div class="flex h-screen">
      <SpaceSidebar />
      <RoomList />
      <MessageArea
        showMembers={showMembers()}
        onToggleMembers={() => setShowMembers((p) => !p)}
        showPins={showPins()}
        onTogglePins={() => setShowPins((p) => !p)}
      />

      {/* Thread panel */}
      <Show when={activeThreadId()}>
        <ThreadPanel
          threadRootId={activeThreadId()!}
          onClose={() => setActiveThreadId(null)}
        />
      </Show>

      <Show when={showMembers() && selectedRoomId()}>
        <MemberList />
      </Show>

      {/* Pinned messages modal overlay */}
      <Show when={showPins()}>
        <PinnedSidebar onClose={() => setShowPins(false)} />
      </Show>
    </div>
  );
}
