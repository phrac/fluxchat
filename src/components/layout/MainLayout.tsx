import { createSignal, onMount } from "solid-js";
import SpaceSidebar from "../sidebar/SpaceSidebar";
import RoomList from "../sidebar/RoomList";
import MessageArea from "../chat/MessageArea";
import MemberList from "../members/MemberList";
import { loadRooms, selectedRoomId } from "../../stores/rooms";

export default function MainLayout() {
  const [showMembers, setShowMembers] = createSignal(true);

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
      />
      {showMembers() && selectedRoomId() && <MemberList />}
    </div>
  );
}
