import { createSignal, Show, For, onCleanup, createEffect } from "solid-js";
import { sendMessage, replyingTo, setReplyingTo, editingMessage, setEditingMessage, insertSystemMessage } from "../../stores/messages";
import { rooms, selectedRoomId } from "../../stores/rooms";
import { sendTyping, uploadFile, editMessage } from "../../lib/tauri";
import { nameColor } from "./MessageBubble";

interface Props {
  roomId: string;
}

interface StagedFile {
  file: File;
  preview: string | null;
  name: string;
}

export default function MessageInput(props: Props) {
  const [text, setText] = createSignal("");
  const [sending, setSending] = createSignal(false);
  const [stagedFiles, setStagedFiles] = createSignal<StagedFile[]>([]);
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

  // Populate input when editing a message
  createEffect(() => {
    const editing = editingMessage();
    if (editing) {
      setText(editing.body);
      setReplyingTo(null);
      setStagedFiles([]);
      requestAnimationFrame(() => {
        if (textareaRef) {
          textareaRef.focus();
          textareaRef.style.height = "auto";
          textareaRef.style.height = Math.min(textareaRef.scrollHeight, 200) + "px";
        }
      });
    }
  });

  const handleSend = async () => {
    const body = text().trim();
    const files = stagedFiles();
    const editing = editingMessage();

    if (!body && files.length === 0) return;
    if (sending()) return;

    stopTyping();
    setSending(true);

    try {
      // Upload staged files first
      for (const staged of files) {
        const buffer = await staged.file.arrayBuffer();
        const bytes = Array.from(new Uint8Array(buffer));
        const mimeType = staged.file.type || "application/octet-stream";
        await uploadFile(props.roomId, staged.file.name, mimeType, bytes);
      }
      setStagedFiles([]);

      // Send text message (or edit)
      if (body) {
        if (editing) {
          await editMessage(props.roomId, editing.event_id, body);
          setEditingMessage(null);
        } else {
          await sendMessage(props.roomId, body);
        }
      }

      setText("");
      if (textareaRef) textareaRef.style.height = "auto";
    } catch (err) {
      console.error("Failed to send:", err);
    }

    setSending(false);
    textareaRef?.focus();
  };

  const handleKeyDown = (e: KeyboardEvent) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      handleSend();
    }
    if (e.key === "Escape") {
      if (editingMessage()) {
        setEditingMessage(null);
        setText("");
      } else if (replyingTo()) {
        setReplyingTo(null);
      } else if (stagedFiles().length > 0) {
        setStagedFiles([]);
      }
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
    typingTimeout = window.setTimeout(() => stopTyping(), 5000);

    if (target.value.length === 0) stopTyping();
  };

  const handleFileSelect = (e: Event) => {
    const input = e.target as HTMLInputElement;
    const files = input.files;
    if (!files || files.length === 0) return;

    const newStaged: StagedFile[] = [];
    for (const file of Array.from(files)) {
      let preview: string | null = null;
      if (file.type.startsWith("image/")) {
        preview = URL.createObjectURL(file);
      }
      newStaged.push({ file, preview, name: file.name });
    }
    setStagedFiles((prev) => [...prev, ...newStaged]);
    input.value = "";
    textareaRef?.focus();
  };

  const removeStaged = (index: number) => {
    setStagedFiles((prev) => {
      const copy = [...prev];
      const removed = copy.splice(index, 1);
      if (removed[0]?.preview) URL.revokeObjectURL(removed[0].preview);
      return copy;
    });
  };

  const [showPlusMenu, setShowPlusMenu] = createSignal(false);

  const reply = () => replyingTo();
  const replyColor = () => nameColor(reply()?.sender || "");
  const hasTopBar = () => editingMessage() || (reply() && !editingMessage()) || stagedFiles().length > 0;

  return (
    <div class="flex-shrink-0 px-4 pb-4">
      {/* Edit preview */}
      <Show when={editingMessage()}>
        <div
          class="flex items-center gap-2 rounded-t-lg px-4 py-2"
          style={{ background: "var(--bg-tertiary)" }}
        >
          <svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor" style={{ color: "var(--accent)" }}>
            <path d="M3 17.25V21h3.75L17.81 9.94l-3.75-3.75L3 17.25zM20.71 7.04c.39-.39.39-1.02 0-1.41l-2.34-2.34c-.39-.39-1.02-.39-1.41 0l-1.83 1.83 3.75 3.75 1.83-1.83z"/>
          </svg>
          <span class="text-xs font-medium" style={{ color: "var(--accent)" }}>Editing message</span>
          <span class="flex-1 truncate text-xs" style={{ color: "var(--text-muted)" }}>
            {editingMessage()!.body}
          </span>
          <button
            onClick={() => { setEditingMessage(null); setText(""); }}
            class="flex-shrink-0 rounded p-0.5"
            style={{ color: "var(--text-muted)" }}
          >
            <svg width="16" height="16" viewBox="0 0 24 24" fill="currentColor">
              <path d="M19 6.41L17.59 5 12 10.59 6.41 5 5 6.41 10.59 12 5 17.59 6.41 19 12 13.41 17.59 19 19 17.59 13.41 12z"/>
            </svg>
          </button>
        </div>
      </Show>

      {/* Reply preview */}
      <Show when={reply() && !editingMessage()}>
        <div
          class="flex items-center gap-2 rounded-t-lg px-4 py-2"
          style={{ background: "var(--bg-tertiary)" }}
        >
          <div class="h-4 w-0.5 flex-shrink-0 rounded-full" style={{ background: replyColor() }} />
          <span class="text-xs font-medium" style={{ color: replyColor() }}>{reply()!.sender_name}</span>
          <span class="flex-1 truncate text-xs" style={{ color: "var(--text-muted)" }}>{reply()!.body}</span>
          <button onClick={() => setReplyingTo(null)} class="flex-shrink-0 rounded p-0.5" style={{ color: "var(--text-muted)" }}>
            <svg width="16" height="16" viewBox="0 0 24 24" fill="currentColor">
              <path d="M19 6.41L17.59 5 12 10.59 6.41 5 5 6.41 10.59 12 5 17.59 6.41 19 12 13.41 17.59 19 19 17.59 13.41 12z"/>
            </svg>
          </button>
        </div>
      </Show>

      {/* Staged files preview */}
      <Show when={stagedFiles().length > 0}>
        <div
          class="flex gap-2 overflow-x-auto px-4 py-3"
          style={{
            background: "var(--bg-tertiary)",
            "border-radius": !editingMessage() && !reply() ? "8px 8px 0 0" : "0",
          }}
        >
          <For each={stagedFiles()}>
            {(staged, i) => (
              <div
                class="relative flex-shrink-0 rounded-lg overflow-hidden"
                style={{ background: "var(--bg-primary)", border: "1px solid var(--border-subtle)" }}
              >
                {/* Remove button */}
                <button
                  onClick={() => removeStaged(i())}
                  class="absolute right-1 top-1 z-10 flex h-5 w-5 items-center justify-center rounded-full"
                  style={{ background: "var(--red-dnd)" }}
                >
                  <svg width="10" height="10" viewBox="0 0 24 24" fill="white">
                    <path d="M19 6.41L17.59 5 12 10.59 6.41 5 5 6.41 10.59 12 5 17.59 6.41 19 12 13.41 17.59 19 19 17.59 13.41 12z"/>
                  </svg>
                </button>

                {/* Preview */}
                <Show
                  when={staged.preview}
                  fallback={
                    <div class="flex h-28 w-36 flex-col items-center justify-center p-2">
                      <svg width="32" height="32" viewBox="0 0 24 24" fill="currentColor" style={{ color: "var(--text-muted)" }}>
                        <path d="M14 2H6c-1.1 0-1.99.9-1.99 2L4 20c0 1.1.89 2 1.99 2H18c1.1 0 2-.9 2-2V8l-6-6zm4 18H6V4h7v5h5v11z"/>
                      </svg>
                      <span class="mt-1 max-w-[128px] truncate text-[10px]" style={{ color: "var(--text-muted)" }}>
                        {staged.name}
                      </span>
                    </div>
                  }
                >
                  <div class="h-28 w-36">
                    <img src={staged.preview!} alt={staged.name} class="h-full w-full object-cover" />
                  </div>
                  <div class="px-2 py-1">
                    <span class="block max-w-[128px] truncate text-[10px]" style={{ color: "var(--text-muted)" }}>
                      {staged.name}
                    </span>
                  </div>
                </Show>
              </div>
            )}
          </For>
        </div>
      </Show>

      {/* Sending indicator */}
      <Show when={sending()}>
        <div
          class="flex items-center gap-2 px-4 py-2"
          style={{ background: "var(--bg-tertiary)", color: "var(--text-muted)" }}
        >
          <div
            class="h-4 w-4 animate-spin rounded-full border-2 border-t-transparent"
            style={{ "border-color": "var(--accent)", "border-top-color": "transparent" }}
          />
          <span class="text-xs">Sending...</span>
        </div>
      </Show>

      {/* Input bar */}
      <div
        class="flex items-end"
        style={{
          background: "var(--bg-secondary)",
          "border-radius": hasTopBar() || sending() ? "0 0 8px 8px" : "8px",
        }}
      >
        {/* Plus menu */}
        <div class="relative flex-shrink-0">
          <button
            onClick={() => setShowPlusMenu(!showPlusMenu())}
            class="rounded-l-lg p-3 transition-colors"
            style={{ color: showPlusMenu() ? "var(--text-primary)" : "var(--text-muted)" }}
            onMouseOver={(e) => (e.currentTarget.style.color = "var(--text-primary)")}
            onMouseOut={(e) => {
              if (!showPlusMenu()) e.currentTarget.style.color = "var(--text-muted)";
            }}
          >
            <svg
              width="20"
              height="20"
              viewBox="0 0 24 24"
              fill="currentColor"
              style={{
                transition: "transform 0.2s",
                transform: showPlusMenu() ? "rotate(45deg)" : "none",
              }}
            >
              <path d="M19 13h-6v6h-2v-6H5v-2h6V5h2v6h6v2z"/>
            </svg>
          </button>

          <Show when={showPlusMenu()}>
            <div
              class="absolute bottom-full left-0 mb-2 w-52 rounded-lg py-1.5 shadow-xl"
              style={{ background: "var(--bg-tertiary)", border: "1px solid rgba(0,0,0,0.3)" }}
            >
              <PlusMenuItem
                label="Upload a File"
                icon="M14 2H6c-1.1 0-2 .9-2 2v16c0 1.1.9 2 2 2h12c1.1 0 2-.9 2-2V8l-6-6zm4 18H6V4h7v5h5v11z"
                onClick={() => { setShowPlusMenu(false); fileInputRef?.click(); }}
              />
              <PlusMenuItem
                label="Create Thread"
                icon="M20 2H4c-1.1 0-2 .9-2 2v18l4-4h14c1.1 0 2-.9 2-2V4c0-1.1-.9-2-2-2zm0 14H6l-2 2V4h16v12z"
                onClick={() => { setShowPlusMenu(false); insertSystemMessage(props.roomId, "Threads are not yet supported"); }}
              />
              <PlusMenuItem
                label="Create Poll"
                icon="M19 3H5c-1.1 0-2 .9-2 2v14c0 1.1.9 2 2 2h14c1.1 0 2-.9 2-2V5c0-1.1-.9-2-2-2zM9 17H7v-7h2v7zm4 0h-2V7h2v10zm4 0h-2v-4h2v4z"
                onClick={() => { setShowPlusMenu(false); insertSystemMessage(props.roomId, "Polls are not yet supported"); }}
              />
            </div>
          </Show>
        </div>

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
            editingMessage()
              ? "Edit your message..."
              : reply()
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

function PlusMenuItem(props: { label: string; icon: string; onClick: () => void }) {
  return (
    <button
      onClick={props.onClick}
      class="flex w-full items-center gap-3 px-3 py-2 text-sm transition-colors"
      style={{ color: "var(--text-normal)" }}
      onMouseOver={(e) => (e.currentTarget.style.background = "rgba(79, 84, 92, 0.3)")}
      onMouseOut={(e) => (e.currentTarget.style.background = "transparent")}
    >
      <svg width="18" height="18" viewBox="0 0 24 24" fill="currentColor" style={{ color: "var(--text-muted)" }}>
        <path d={props.icon} />
      </svg>
      {props.label}
    </button>
  );
}
