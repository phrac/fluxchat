import { createSignal } from "solid-js";
import * as api from "../lib/tauri";

const [loggedIn, setLoggedIn] = createSignal(false);
const [userId, setUserId] = createSignal("");
const [displayName, setDisplayName] = createSignal<string | null>(null);
const [loading, setLoading] = createSignal(true);

export { loggedIn, userId, displayName, loading };

export async function tryRestoreSession() {
  setLoading(true);
  try {
    const result = await api.restoreSession();
    if (result) {
      setUserId(result.user_id);
      setDisplayName(result.display_name);
      setLoggedIn(true);
    }
  } catch {
    // No saved session or restore failed
  }
  setLoading(false);
}

export async function login(
  homeserver: string,
  username: string,
  password: string
) {
  const result = await api.login(homeserver, username, password);
  setUserId(result.user_id);
  setDisplayName(result.display_name);
  setLoggedIn(true);
}

export async function logout() {
  await api.logout();
  setLoggedIn(false);
  setUserId("");
  setDisplayName(null);
}
