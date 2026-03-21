import { Show, onMount } from "solid-js";
import { loggedIn, loading, tryRestoreSession } from "./stores/auth";
import LoginView from "./components/login/LoginView";
import MainLayout from "./components/layout/MainLayout";
import VerificationModal from "./components/verification/VerificationModal";

export default function App() {
  onMount(() => {
    tryRestoreSession();
  });

  return (
    <Show
      when={!loading()}
      fallback={
        <div
          class="flex h-screen items-center justify-center"
          style={{ background: "var(--bg-tertiary)", color: "var(--text-muted)" }}
        >
          <div class="text-center">
            <div
              class="mx-auto mb-4 h-10 w-10 animate-spin rounded-full border-2 border-t-transparent"
              style={{ "border-color": "var(--accent)", "border-top-color": "transparent" }}
            />
            <p>Loading...</p>
          </div>
        </div>
      }
    >
      <Show when={loggedIn()} fallback={<LoginView />}>
        <MainLayout />
        <VerificationModal />
      </Show>
    </Show>
  );
}
