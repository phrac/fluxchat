import { createSignal } from "solid-js";
import { sendMessage } from "../../stores/messages";
import { rooms, selectedRoomId } from "../../stores/rooms";

interface Props {
  roomId: string;
}

export default function MessageInput(props: Props) {
  const [text, setText] = createSignal("");
  const [sending, setSending] = createSignal(false);
  let textareaRef!: HTMLTextAreaElement;

  const currentRoom = () => rooms.find((r) => r.room_id === props.roomId);

  const handleSend = async () => {
    const body = text().trim();
    if (!body || sending()) return;

    setSending(true);
    try {
      await sendMessage(props.roomId, body);
      setText("");
      if (textareaRef) {
        textareaRef.style.height = "auto";
      }
    } catch (err) {
      console.error("Failed to send message:", err);
    }
    setSending(false);
  };

  const handleKeyDown = (e: KeyboardEvent) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      handleSend();
    }
  };

  const handleInput = (e: InputEvent) => {
    const target = e.currentTarget as HTMLTextAreaElement;
    setText(target.value);
    // Auto-resize
    target.style.height = "auto";
    target.style.height = Math.min(target.scrollHeight, 200) + "px";
  };

  return (
    <div class="flex-shrink-0 px-4 pb-4">
      <div
        class="flex items-end rounded-lg"
        style={{ background: "var(--bg-secondary)" }}
      >
        <textarea
          ref={textareaRef}
          value={text()}
          onInput={handleInput}
          onKeyDown={handleKeyDown}
          placeholder={`Message #${currentRoom()?.name || "room"}`}
          rows={1}
          class="max-h-[200px] min-h-[44px] flex-1 resize-none bg-transparent px-4 py-3 text-sm outline-none"
          style={{
            color: "var(--text-primary)",
          }}
        />
      </div>
    </div>
  );
}
