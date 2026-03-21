import { createSignal, Show, onCleanup } from "solid-js";
import { sendMessage, replyingTo, setReplyingTo, insertSystemMessage } from "../../stores/messages";
import { rooms, selectedRoomId } from "../../stores/rooms";
import { sendTyping, uploadFile } from "../../lib/tauri";
import { nameColor } from "./MessageBubble";

interface Props {
  roomId: string;
}

export default function MessageInput(props: Props) {
  const [text, setText] = createSignal("");
  const [sending, setSending] = createSignal(false);
  const [uploading, setUploading] = createSignal(false);
  let textareaRef!: HTMLTextAreaElement;
  let fileInputRef!: HTMLInputElement;
  let typingTimeout: number | undefined;
  let isTyping = false;

  const currentRoom = () => rooms.find((r) => r.room_id === props.roomId);

  const stopTyping = () => {
    if (isTyping) {
      isTyping = false;
      sendTyping(props.roomId, false).catch(() => {});
    }
    if (typingTimeout) {
      clearTimeout(typingTimeout);
      typingTimeout = undefined;
    }
  };

  onCleanup(stopTyping);

  const handleSend = async () => {
    const body = text().trim();
    if (!body || sending()) return;

    stopTyping();
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
    textareaRef?.focus();
  };

  const handleKeyDown = (e: KeyboardEvent) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      handleSend();
    }
    if (e.key === "Escape" && replyingTo()) {
      setReplyingTo(null);
    }
  };

  const handleInput = (e: InputEvent) => {
    const target = e.currentTarget as HTMLTextAreaElement;
    setText(target.value);
    target.style.height = "auto";
    target.style.height = Math.min(target.scrollHeight, 200) + "px";

    if (target.value.length > 0 && !isTyping) {
      isTyping = true;
      sendTyping(props.roomId, true).catch(() => {});
    }

    if (typingTimeout) clearTimeout(typingTimeout);
    typingTimeout = window.setTimeout(() => {
      stopTyping();
    }, 5000);

    if (target.value.length === 0) {
      stopTyping();
    }
  };

  const handleFileSelect = async (e: Event) => {
    const input = e.target as HTMLInputElement;
    const files = input.files;
    if (!files || files.length === 0) return;

    setUploading(true);
    for (const file of Array.from(files)) {
      try {
        // Read file path — in Tauri webview, files have a path property
        // But web File API doesn't expose paths. We need to read the file
        // and use a temp approach. Actually Tauri's file input provides paths.
        const filePath = (file as any).path;
        if (filePath) {
          await uploadFile(props.roomId, filePath);
        } else {
          // Fallback: read as array buffer and write to temp file
          // For now just show the filename
          insertSystemMessage(props.roomId, `File upload for "${file.name}" - drag & drop not yet supported, use the file picker`);
        }
      } catch (err) {
        console.error("Upload failed:", err);
        insertSystemMessage(props.roomId, `Failed to upload ${file.name}`);
      }
    }
    setUploading(false);
    input.value = ""; // Reset for re-selection
  };

  const reply = () => replyingTo();
  const replyColor = () => nameColor(reply()?.sender || "");

  return (
    <div class="flex-shrink-0 px-4 pb-4">
      {/* Reply preview */}
      <Show when={reply()}>
        <div
          class="flex items-center gap-2 rounded-t-lg px-4 py-2"
          style={{ background: "var(--bg-tertiary)" }}
        >
          <div
            class="h-4 w-0.5 flex-shrink-0 rounded-full"
            style={{ background: replyColor() }}
          />
          <span class="text-xs font-medium" style={{ color: replyColor() }}>
            {reply()!.sender_name}
          </span>
          <span
            class="flex-1 truncate text-xs"
            style={{ color: "var(--text-muted)" }}
          >
            {reply()!.body}
          </span>
          <button
            onClick={() => setReplyingTo(null)}
            class="flex-shrink-0 rounded p-0.5"
            style={{ color: "var(--text-muted)" }}
          >
            <svg width="16" height="16" viewBox="0 0 24 24" fill="currentColor">
              <path d="M19 6.41L17.59 5 12 10.59 6.41 5 5 6.41 10.59 12 5 17.59 6.41 19 12 13.41 17.59 19 19 17.59 13.41 12z"/>
            </svg>
          </button>
        </div>
      </Show>

      {/* Upload progress */}
      <Show when={uploading()}>
        <div
          class="flex items-center gap-2 px-4 py-2"
          style={{ background: "var(--bg-tertiary)", color: "var(--text-muted)" }}
        >
          <div
            class="h-4 w-4 animate-spin rounded-full border-2 border-t-transparent"
            style={{ "border-color": "var(--accent)", "border-top-color": "transparent" }}
          />
          <span class="text-xs">Uploading...</span>
        </div>
      </Show>

      <div
        class="flex items-end"
        style={{
          background: "var(--bg-secondary)",
          "border-radius": reply() || uploading() ? "0 0 8px 8px" : "8px",
        }}
      >
        {/* File upload button */}
        <button
          onClick={() => fileInputRef?.click()}
          class="flex-shrink-0 rounded-l-lg p-3 transition-colors"
          style={{ color: "var(--text-muted)" }}
          onMouseOver={(e) => (e.currentTarget.style.color = "var(--text-primary)")}
          onMouseOut={(e) => (e.currentTarget.style.color = "var(--text-muted)")}
          title="Upload file"
          disabled={uploading()}
        >
          <svg width="20" height="20" viewBox="0 0 24 24" fill="currentColor">
            <path d="M19 13h-6v6h-2v-6H5v-2h6V5h2v6h6v2z"/>
          </svg>
        </button>

        <input
          ref={fileInputRef}
          type="file"
          multiple
          class="hidden"
          onChange={handleFileSelect}
        />

        <textarea
          ref={textareaRef}
          value={text()}
          onInput={handleInput}
          onKeyDown={handleKeyDown}
          placeholder={
            reply()
              ? `Reply to ${reply()!.sender_name}...`
              : `Message #${currentRoom()?.name || "room"}`
          }
          rows={1}
          class="max-h-[200px] min-h-[44px] flex-1 resize-none bg-transparent px-2 py-3 text-sm outline-none"
          style={{ color: "var(--text-primary)" }}
        />
      </div>
    </div>
  );
}
