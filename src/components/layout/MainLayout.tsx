import { createSignal, onMount, Show } from "solid-js";
import SpaceSidebar from "../sidebar/SpaceSidebar";
import RoomList, { UserPanel } from "../sidebar/RoomList";
import MessageArea from "../chat/MessageArea";
import MemberList from "../members/MemberList";
import PinnedSidebar from "../chat/PinnedSidebar";
import ThreadPanel from "../chat/ThreadPanel";
import CreateRoomModal from "../modals/CreateRoomModal";
import RoomSettingsModal from "../modals/RoomSettingsModal";
import SettingsView from "../modals/SettingsView";
import { loadRooms, selectedRoomId } from "../../stores/rooms";

// Export shared state
const [activeThreadId, setActiveThreadId] = createSignal<string | null>(null);
const [showCreateRoom, setShowCreateRoom] = createSignal(false);
const [showRoomSettings, setShowRoomSettings] = createSignal(false);
const [showProfile, setShowProfile] = createSignal(false);
export { activeThreadId, setActiveThreadId, showCreateRoom, setShowCreateRoom, showRoomSettings, setShowRoomSettings, showProfile, setShowProfile };

export default function MainLayout() {
  const [showMembers, setShowMembers] = createSignal(true);
  const [showPins, setShowPins] = createSignal(false);

  onMount(() => {
    loadRooms();
  });

  return (
    <div class="flex h-screen">
      {/* Left sidebar: spaces + rooms + user panel */}
      <div class="flex flex-shrink-0 flex-col" style={{ background: "var(--bg-secondary)" }}>
        <div class="flex flex-1 min-h-0">
          <SpaceSidebar />
          <RoomList />
        </div>
        <UserPanel />
      </div>
      <MessageArea
        showMembers={showMembers()}
        onToggleMembers={() => setShowMembers((p) => !p)}
        showPins={showPins()}
        onTogglePins={() => setShowPins((p) => !p)}
      />

      <Show when={activeThreadId()}>
        <ThreadPanel
          threadRootId={activeThreadId()!}
          onClose={() => setActiveThreadId(null)}
        />
      </Show>

      <Show when={showMembers() && selectedRoomId()}>
        <MemberList />
      </Show>

      <Show when={showPins()}>
        <PinnedSidebar onClose={() => setShowPins(false)} />
      </Show>

      <Show when={showCreateRoom()}>
        <CreateRoomModal onClose={() => setShowCreateRoom(false)} />
      </Show>

      <Show when={showRoomSettings()}>
        <RoomSettingsModal onClose={() => setShowRoomSettings(false)} />
      </Show>

      <Show when={showProfile()}>
        <SettingsView onClose={() => setShowProfile(false)} />
      </Show>
    </div>
  );
}
