import { createSignal } from "solid-js";
import { login } from "../../stores/auth";

export default function LoginView() {
  const [homeserver, setHomeserver] = createSignal("matrix.org");
  const [username, setUsername] = createSignal("");
  const [password, setPassword] = createSignal("");
  const [error, setError] = createSignal("");
  const [submitting, setSubmitting] = createSignal(false);

  const handleSubmit = async (e: Event) => {
    e.preventDefault();
    setError("");
    setSubmitting(true);
    try {
      await login(homeserver(), username(), password());
    } catch (err: any) {
      setError(typeof err === "string" ? err : err.message || "Login failed");
    }
    setSubmitting(false);
  };

  return (
    <div
      class="flex h-screen items-center justify-center"
      style={{ background: "var(--bg-tertiary)" }}
    >
      <form
        onSubmit={handleSubmit}
        class="w-[400px] rounded-lg p-8"
        style={{ background: "var(--bg-primary)" }}
      >
        <h1
          class="mb-2 text-center text-2xl font-bold"
          style={{ color: "var(--text-primary)" }}
        >
          Welcome to fluxchat
        </h1>
        <p
          class="mb-6 text-center text-sm"
          style={{ color: "var(--text-secondary)" }}
        >
          Sign in with your Matrix account
        </p>

        {error() && (
          <div
            class="mb-4 rounded p-3 text-sm"
            style={{ background: "rgba(242, 63, 67, 0.1)", color: "var(--red-dnd)" }}
          >
            {error()}
          </div>
        )}

        <div class="mb-4">
          <label
            class="mb-2 block text-xs font-bold uppercase"
            style={{ color: "var(--text-secondary)" }}
          >
            Homeserver
          </label>
          <input
            type="text"
            value={homeserver()}
            onInput={(e) => setHomeserver(e.currentTarget.value)}
            class="w-full rounded px-3 py-2 text-sm outline-none"
            style={{
              background: "var(--bg-tertiary)",
              color: "var(--text-primary)",
              border: "none",
            }}
            placeholder="matrix.org"
          />
        </div>

        <div class="mb-4">
          <label
            class="mb-2 block text-xs font-bold uppercase"
            style={{ color: "var(--text-secondary)" }}
          >
            Username
          </label>
          <input
            type="text"
            value={username()}
            onInput={(e) => setUsername(e.currentTarget.value)}
            class="w-full rounded px-3 py-2 text-sm outline-none"
            style={{
              background: "var(--bg-tertiary)",
              color: "var(--text-primary)",
              border: "none",
            }}
            placeholder="@user:matrix.org"
          />
        </div>

        <div class="mb-6">
          <label
            class="mb-2 block text-xs font-bold uppercase"
            style={{ color: "var(--text-secondary)" }}
          >
            Password
          </label>
          <input
            type="password"
            value={password()}
            onInput={(e) => setPassword(e.currentTarget.value)}
            class="w-full rounded px-3 py-2 text-sm outline-none"
            style={{
              background: "var(--bg-tertiary)",
              color: "var(--text-primary)",
              border: "none",
            }}
          />
        </div>

        <button
          type="submit"
          disabled={submitting()}
          class="w-full rounded py-2.5 text-sm font-medium text-white transition-colors"
          style={{
            background: submitting() ? "var(--accent-hover)" : "var(--accent)",
            cursor: submitting() ? "not-allowed" : "pointer",
          }}
          onMouseOver={(e) => {
            if (!submitting())
              e.currentTarget.style.background = "var(--accent-hover)";
          }}
          onMouseOut={(e) => {
            if (!submitting())
              e.currentTarget.style.background = "var(--accent)";
          }}
        >
          {submitting() ? "Signing in..." : "Sign In"}
        </button>
      </form>
    </div>
  );
}
