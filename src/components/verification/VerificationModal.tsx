import { createSignal, For, Show, onCleanup, onMount } from "solid-js";
import { listen } from "@tauri-apps/api/event";
import * as api from "../../lib/tauri";
import type { VerificationInfo, EmojiInfo } from "../../lib/types";

type VerificationState =
  | { type: "idle" }
  | { type: "requested"; info: VerificationInfo }
  | { type: "accepting" }
  | { type: "emojis"; info: VerificationInfo; emojis: EmojiInfo[] }
  | { type: "confirming" }
  | { type: "done" }
  | { type: "cancelled" }
  | { type: "error"; message: string };

export default function VerificationModal() {
  const [state, setState] = createSignal<VerificationState>({ type: "idle" });
  const [currentInfo, setCurrentInfo] = createSignal<VerificationInfo | null>(null);

  onMount(() => {
    // Listen for incoming verification requests
    const unlistenRequest = listen<VerificationInfo>(
      "matrix://verification-request",
      (event) => {
        setCurrentInfo(event.payload);
        setState({ type: "requested", info: event.payload });
      }
    );

    // Listen for emojis being ready
    const unlistenEmojis = listen<VerificationInfo>(
      "matrix://verification-emojis",
      (event) => {
        setCurrentInfo(event.payload);
        setState({
          type: "emojis",
          info: event.payload,
          emojis: event.payload.emojis || [],
        });
      }
    );

    // Listen for cancellation
    const unlistenCancelled = listen<VerificationInfo>(
      "matrix://verification-cancelled",
      () => {
        setState({ type: "cancelled" });
        setTimeout(() => setState({ type: "idle" }), 3000);
      }
    );

    onCleanup(() => {
      unlistenRequest.then((f) => f());
      unlistenEmojis.then((f) => f());
      unlistenCancelled.then((f) => f());
    });
  });

  const handleAccept = async () => {
    const info = currentInfo();
    if (!info) return;
    setState({ type: "accepting" });
    try {
      await api.acceptVerification(info.other_user_id, info.flow_id);
    } catch (err: any) {
      setState({ type: "error", message: String(err) });
    }
  };

  const handleConfirm = async () => {
    const info = currentInfo();
    if (!info) return;
    setState({ type: "confirming" });
    try {
      await api.confirmVerification(info.other_user_id, info.flow_id);
      setState({ type: "done" });
      setTimeout(() => setState({ type: "idle" }), 3000);
    } catch (err: any) {
      setState({ type: "error", message: String(err) });
    }
  };

  const handleCancel = async () => {
    const info = currentInfo();
    if (info) {
      try {
        await api.cancelVerification(info.other_user_id, info.flow_id);
      } catch {}
    }
    setState({ type: "idle" });
  };

  const isVisible = () => state().type !== "idle";

  return (
    <Show when={isVisible()}>
      {/* Backdrop */}
      <div
        class="fixed inset-0 z-50 flex items-center justify-center"
        style={{ background: "rgba(0,0,0,0.6)" }}
      >
        <div
          class="w-[440px] rounded-lg p-6"
          style={{ background: "var(--bg-primary)" }}
        >
          {/* Requested state */}
          <Show when={state().type === "requested"}>
            <h2
              class="mb-2 text-lg font-bold"
              style={{ color: "var(--text-primary)" }}
            >
              Verification Request
            </h2>
            <p class="mb-4 text-sm" style={{ color: "var(--text-secondary)" }}>
              <span style={{ color: "var(--accent)" }}>
                {currentInfo()?.other_user_id}
              </span>{" "}
              wants to verify this device. Accept to compare emojis and confirm
              your identity.
            </p>
            <div class="flex gap-3 justify-end">
              <button
                onClick={handleCancel}
                class="rounded px-4 py-2 text-sm"
                style={{
                  background: "var(--bg-tertiary)",
                  color: "var(--text-secondary)",
                }}
              >
                Decline
              </button>
              <button
                onClick={handleAccept}
                class="rounded px-4 py-2 text-sm font-medium text-white"
                style={{ background: "var(--accent)" }}
              >
                Accept
              </button>
            </div>
          </Show>

          {/* Accepting state */}
          <Show when={state().type === "accepting"}>
            <h2
              class="mb-2 text-lg font-bold"
              style={{ color: "var(--text-primary)" }}
            >
              Starting Verification...
            </h2>
            <p class="text-sm" style={{ color: "var(--text-muted)" }}>
              Waiting for emojis...
            </p>
          </Show>

          {/* Emoji comparison state */}
          <Show when={state().type === "emojis"}>
            <h2
              class="mb-2 text-lg font-bold"
              style={{ color: "var(--text-primary)" }}
            >
              Verify with Emojis
            </h2>
            <p class="mb-4 text-sm" style={{ color: "var(--text-secondary)" }}>
              Confirm these emojis match on your other device:
            </p>

            <div
              class="mb-6 grid grid-cols-7 gap-2 rounded-lg p-4"
              style={{ background: "var(--bg-tertiary)" }}
            >
              <For
                each={
                  state().type === "emojis"
                    ? (state() as { emojis: EmojiInfo[] }).emojis
                    : []
                }
              >
                {(emoji) => (
                  <div class="flex flex-col items-center gap-1">
                    <span class="text-2xl">{emoji.symbol}</span>
                    <span
                      class="text-[10px] text-center leading-tight"
                      style={{ color: "var(--text-muted)" }}
                    >
                      {emoji.description}
                    </span>
                  </div>
                )}
              </For>
            </div>

            <div class="flex gap-3 justify-end">
              <button
                onClick={handleCancel}
                class="rounded px-4 py-2 text-sm"
                style={{
                  background: "var(--bg-tertiary)",
                  color: "var(--text-secondary)",
                }}
              >
                They Don't Match
              </button>
              <button
                onClick={handleConfirm}
                class="rounded px-4 py-2 text-sm font-medium text-white"
                style={{ background: "var(--green-online)" }}
              >
                They Match
              </button>
            </div>
          </Show>

          {/* Confirming */}
          <Show when={state().type === "confirming"}>
            <h2
              class="mb-2 text-lg font-bold"
              style={{ color: "var(--text-primary)" }}
            >
              Confirming...
            </h2>
          </Show>

          {/* Done */}
          <Show when={state().type === "done"}>
            <h2
              class="mb-2 text-lg font-bold"
              style={{ color: "var(--green-online)" }}
            >
              Verification Complete
            </h2>
            <p class="text-sm" style={{ color: "var(--text-secondary)" }}>
              This device is now verified.
            </p>
          </Show>

          {/* Cancelled */}
          <Show when={state().type === "cancelled"}>
            <h2
              class="mb-2 text-lg font-bold"
              style={{ color: "var(--red-dnd)" }}
            >
              Verification Cancelled
            </h2>
          </Show>

          {/* Error */}
          <Show when={state().type === "error"}>
            <h2
              class="mb-2 text-lg font-bold"
              style={{ color: "var(--red-dnd)" }}
            >
              Verification Failed
            </h2>
            <p class="mb-4 text-sm" style={{ color: "var(--text-secondary)" }}>
              {(state() as { message: string }).message}
            </p>
            <button
              onClick={() => setState({ type: "idle" })}
              class="rounded px-4 py-2 text-sm"
              style={{
                background: "var(--bg-tertiary)",
                color: "var(--text-secondary)",
              }}
            >
              Close
            </button>
          </Show>
        </div>
      </div>
    </Show>
  );
}
