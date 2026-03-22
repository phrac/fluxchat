import { createSignal, Show, For, onMount } from "solid-js";
import { getProfile, setDisplayName, uploadAvatar, removeAvatar, uploadBanner, removeBanner, getMediaUrl } from "../../lib/tauri";
import { logout, userId } from "../../stores/auth";

interface Props {
  onClose: () => void;
}

type SettingsPage = "account" | "appearance" | "notifications" | "devices" | "about";

export default function SettingsView(props: Props) {
  const [page, setPage] = createSignal<SettingsPage>("account");

  return (
    <div class="fixed inset-0 z-50 flex" style={{ background: "var(--bg-tertiary)" }}>
      {/* Sidebar */}
      <div class="flex w-56 flex-shrink-0 flex-col overflow-y-auto py-6 pl-6 pr-2" style={{ background: "var(--bg-secondary)" }}>
        <NavSection label="User Settings">
          <NavItem path="M12 12c2.21 0 4-1.79 4-4s-1.79-4-4-4-4 1.79-4 4 1.79 4 4 4zm0 2c-2.67 0-8 1.34-8 4v2h16v-2c0-2.66-5.33-4-8-4z" label="My Account" active={page() === "account"} onClick={() => setPage("account")} />
          <NavItem path="M12 3c-4.97 0-9 4.03-9 9s4.03 9 9 9c.83 0 1.5-.67 1.5-1.5 0-.39-.15-.74-.39-1.01-.23-.26-.38-.61-.38-1 0-.83.67-1.5 1.5-1.5H16c2.76 0 5-2.24 5-5 0-4.42-4.03-8-9-8zm-5.5 9c-.83 0-1.5-.67-1.5-1.5S5.67 9 6.5 9 8 9.67 8 10.5 7.33 12 6.5 12zm3-4C8.67 8 8 7.33 8 6.5S8.67 5 9.5 5s1.5.67 1.5 1.5S10.33 8 9.5 8zm5 0c-.83 0-1.5-.67-1.5-1.5S13.67 5 14.5 5s1.5.67 1.5 1.5S15.33 8 14.5 8zm3 4c-.83 0-1.5-.67-1.5-1.5S16.67 9 17.5 9s1.5.67 1.5 1.5-.67 1.5-1.5 1.5z" label="Appearance" active={page() === "appearance"} onClick={() => setPage("appearance")} />
          <NavItem path="M12 22c1.1 0 2-.9 2-2h-4c0 1.1.9 2 2 2zm6-6v-5c0-3.07-1.63-5.64-4.5-6.32V4c0-.83-.67-1.5-1.5-1.5s-1.5.67-1.5 1.5v.68C7.64 5.36 6 7.92 6 11v5l-2 2v1h16v-1l-2-2zm-2 1H8v-6c0-2.48 1.51-4.5 4-4.5s4 2.02 4 4.5v6z" label="Notifications" active={page() === "notifications"} onClick={() => setPage("notifications")} />
          <NavItem path="M17 1H7c-1.1 0-2 .9-2 2v18c0 1.1.9 2 2 2h10c1.1 0 2-.9 2-2V3c0-1.1-.9-2-2-2zm0 18H7V5h10v14zm-4.2-5.78v1.75l3.2-2.99L12.8 9v1.7c-3.11.43-4.35 2.56-4.8 4.7 1.11-1.5 2.58-2.18 4.8-2.18z" label="Devices" active={page() === "devices"} onClick={() => setPage("devices")} />
        </NavSection>

        <NavSection label="App">
          <NavItem path="M11 7h2v2h-2zm0 4h2v6h-2zm1-9C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm0 18c-4.41 0-8-3.59-8-8s3.59-8 8-8 8 3.59 8 8-3.59 8-8 8z" label="About" active={page() === "about"} onClick={() => setPage("about")} />
        </NavSection>

        <div class="mt-2 h-px" style={{ background: "var(--border-subtle)" }} />

        <button
          onClick={async () => { await logout(); props.onClose(); }}
          class="mt-2 flex w-full items-center gap-2 rounded px-3 py-2 text-sm text-left transition-colors"
          style={{ color: "var(--red-dnd)" }}
          onMouseOver={(e) => (e.currentTarget.style.background = "rgba(242,63,67,0.1)")}
          onMouseOut={(e) => (e.currentTarget.style.background = "transparent")}
        >
          <svg width="16" height="16" viewBox="0 0 24 24" fill="currentColor"><path d="M10.09 15.59L11.5 17l5-5-5-5-1.41 1.41L12.67 11H3v2h9.67l-2.58 2.59zM19 3H5c-1.11 0-2 .9-2 2v4h2V5h14v14H5v-4H3v4c0 1.1.89 2 2 2h14c1.1 0 2-.9 2-2V5c0-1.1-.9-2-2-2z"/></svg>
          Log Out
        </button>

        <div class="mt-auto pt-4 text-[10px]" style={{ color: "var(--text-muted)" }}>
          fluxchat v0.1.0
        </div>
      </div>

      {/* Content */}
      <div class="flex-1 overflow-y-auto">
        {/* Close button */}
        <div class="sticky top-0 flex justify-end p-4">
          <button
            onClick={props.onClose}
            class="flex h-9 w-9 items-center justify-center rounded-full transition-colors"
            style={{ background: "rgba(255,255,255,0.06)", color: "var(--text-muted)" }}
            onMouseOver={(e) => { e.currentTarget.style.background = "rgba(255,255,255,0.12)"; e.currentTarget.style.color = "var(--text-primary)"; }}
            onMouseOut={(e) => { e.currentTarget.style.background = "rgba(255,255,255,0.06)"; e.currentTarget.style.color = "var(--text-muted)"; }}
          >
            <svg width="18" height="18" viewBox="0 0 24 24" fill="currentColor">
              <path d="M19 6.41L17.59 5 12 10.59 6.41 5 5 6.41 10.59 12 5 17.59 6.41 19 12 13.41 17.59 19 19 17.59 13.41 12z"/>
            </svg>
          </button>
        </div>

        <div class="mx-auto max-w-2xl px-10 pb-16 -mt-6">
          <Show when={page() === "account"}><AccountPage /></Show>
          <Show when={page() === "appearance"}><AppearancePage /></Show>
          <Show when={page() === "notifications"}><NotificationsPage /></Show>
          <Show when={page() === "devices"}><DevicesPage /></Show>
          <Show when={page() === "about"}><AboutPage /></Show>
        </div>
      </div>
    </div>
  );
}

function NavSection(props: { label: string; children: any }) {
  return (
    <div class="mb-2">
      <div class="mb-1 px-3 text-[11px] font-bold uppercase tracking-wide" style={{ color: "var(--text-muted)" }}>
        {props.label}
      </div>
      {props.children}
    </div>
  );
}

function NavItem(props: { path: string; label: string; active: boolean; onClick: () => void }) {
  return (
    <button
      onClick={props.onClick}
      class="mb-0.5 flex w-full items-center gap-2.5 rounded px-3 py-1.5 text-sm text-left transition-colors"
      style={{
        background: props.active ? "rgba(79, 84, 92, 0.6)" : "transparent",
        color: props.active ? "var(--text-primary)" : "var(--text-secondary)",
      }}
      onMouseOver={(e) => { if (!props.active) e.currentTarget.style.background = "rgba(79, 84, 92, 0.3)"; }}
      onMouseOut={(e) => { if (!props.active) e.currentTarget.style.background = "transparent"; }}
    >
      <svg width="16" height="16" viewBox="0 0 24 24" fill="currentColor" class="flex-shrink-0 opacity-70">
        <path d={props.path} />
      </svg>
      {props.label}
    </button>
  );
}

function SectionTitle(props: { children: any }) {
  return <h2 class="mb-4 text-lg font-bold" style={{ color: "var(--text-primary)" }}>{props.children}</h2>;
}

function Divider() {
  return <div class="my-6 h-px" style={{ background: "var(--border-subtle)" }} />;
}

// ── Account Page ──

function AccountPage() {
  const [displayNameVal, setDisplayNameVal] = createSignal("");
  const [avatarSrc, setAvatarSrc] = createSignal<string | null>(null);
  const [bannerSrc, setBannerSrc] = createSignal<string | null>(null);
  const [avatarMxc, setAvatarMxc] = createSignal<string | null>(null);
  const [saving, setSaving] = createSignal(false);
  const [msg, setMsg] = createSignal("");
  let avatarInput!: HTMLInputElement;
  let bannerInput!: HTMLInputElement;

  onMount(async () => {
    try {
      const p = await getProfile();
      setDisplayNameVal(p.display_name || "");
      setAvatarMxc(p.avatar_url);
      if (p.avatar_url) setAvatarSrc(await getMediaUrl(p.avatar_url).catch(() => null));
      if (p.banner_url) setBannerSrc(await getMediaUrl(p.banner_url).catch(() => null));
    } catch {}
  });

  const save = async (fn: () => Promise<void>, label: string) => {
    setSaving(true); setMsg("");
    try { await fn(); setMsg(label + " updated"); setTimeout(() => setMsg(""), 2000); }
    catch (e: any) { setMsg("Error: " + String(e)); }
    setSaving(false);
  };

  return (
    <>
      <SectionTitle>My Account</SectionTitle>

      {/* Profile card */}
      <div class="overflow-hidden rounded-xl" style={{ background: "var(--bg-secondary)" }}>
        {/* Banner */}
        <div
          class="relative h-32 cursor-pointer overflow-hidden"
          style={{ background: bannerSrc() ? undefined : "linear-gradient(135deg, #5865f2, #8b5cf6, #eb459e)" }}
          onClick={() => bannerInput?.click()}
        >
          <Show when={bannerSrc()}><img src={bannerSrc()!} class="h-full w-full object-cover" /></Show>
          <div
            class="absolute inset-0 flex items-center justify-center transition-opacity"
            style={{ background: "rgba(0,0,0,0.4)", opacity: "0" }}
            onMouseOver={(e) => (e.currentTarget.style.opacity = "1")}
            onMouseOut={(e) => (e.currentTarget.style.opacity = "0")}
          >
            <span class="text-xs font-semibold uppercase text-white">Change Banner</span>
          </div>
          <input ref={bannerInput} type="file" accept="image/*" class="hidden" onChange={async (e) => {
            const f = (e.target as HTMLInputElement).files?.[0]; if (!f) return;
            await save(async () => {
              const mxc = await uploadBanner(f.type || "image/png", Array.from(new Uint8Array(await f.arrayBuffer())));
              setBannerSrc(await getMediaUrl(mxc).catch(() => null));
            }, "Banner");
            (e.target as HTMLInputElement).value = "";
          }} />
        </div>

        {/* Avatar + name row */}
        <div class="flex items-end gap-4 px-4 -mt-10 pb-4">
          <div
            class="relative h-20 w-20 flex-shrink-0 cursor-pointer overflow-hidden rounded-full border-4"
            style={{ "border-color": "var(--bg-secondary)" }}
            onClick={() => avatarInput?.click()}
          >
            <Show when={avatarSrc()} fallback={
              <div class="flex h-full w-full items-center justify-center text-2xl font-bold text-white" style={{ background: "var(--accent)" }}>
                {(displayNameVal() || "?")[0].toUpperCase()}
              </div>
            }>
              <img src={avatarSrc()!} class="h-full w-full object-cover" />
            </Show>
            <div
              class="absolute inset-0 flex items-center justify-center transition-opacity"
              style={{ background: "rgba(0,0,0,0.6)", opacity: "0" }}
              onMouseOver={(e) => (e.currentTarget.style.opacity = "1")}
              onMouseOut={(e) => (e.currentTarget.style.opacity = "0")}
            >
              <span class="text-[8px] font-bold uppercase text-white">Change</span>
            </div>
            <input ref={avatarInput} type="file" accept="image/*" class="hidden" onChange={async (e) => {
              const f = (e.target as HTMLInputElement).files?.[0]; if (!f) return;
              await save(async () => {
                const mxc = await uploadAvatar(f.type || "image/png", Array.from(new Uint8Array(await f.arrayBuffer())));
                setAvatarMxc(mxc); setAvatarSrc(await getMediaUrl(mxc).catch(() => null));
              }, "Avatar");
              (e.target as HTMLInputElement).value = "";
            }} />
          </div>
          <div class="flex-1 pb-1">
            <span class="text-lg font-bold" style={{ color: "var(--text-primary)" }}>
              {displayNameVal() || userId().replace(/^@/, "").split(":")[0]}
            </span>
          </div>
        </div>
      </div>

      <Show when={msg()}>
        <div class="mt-3 rounded p-2 text-sm" style={{ background: "rgba(35,165,89,0.1)", color: "var(--green-online)" }}>
          {msg()}
        </div>
      </Show>

      <Divider />

      {/* Editable fields */}
      <SettingsField label="Display Name" value={displayNameVal()} onSave={async (v) => {
        setDisplayNameVal(v);
        await save(async () => { await setDisplayName(v); }, "Display name");
      }} />

      <SettingsField label="User ID" value={userId()} readonly />

      <Divider />

      {/* Remove links */}
      <div class="flex gap-4">
        <Show when={avatarMxc()}>
          <button onClick={async () => { await removeAvatar(); setAvatarSrc(null); setAvatarMxc(null); }}
            class="text-xs hover:underline" style={{ color: "var(--red-dnd)" }}>Remove Avatar</button>
        </Show>
        <Show when={bannerSrc()}>
          <button onClick={async () => { await removeBanner(); setBannerSrc(null); }}
            class="text-xs hover:underline" style={{ color: "var(--red-dnd)" }}>Remove Banner</button>
        </Show>
      </div>
    </>
  );
}

function SettingsField(props: { label: string; value: string; readonly?: boolean; onSave?: (v: string) => void }) {
  const [editing, setEditing] = createSignal(false);
  const [val, setVal] = createSignal(props.value);

  return (
    <div class="mb-4 flex items-center justify-between rounded-lg px-4 py-3" style={{ background: "var(--bg-secondary)" }}>
      <div>
        <div class="text-[11px] font-bold uppercase" style={{ color: "var(--text-muted)" }}>{props.label}</div>
        <Show when={!editing()} fallback={
          <input
            type="text" value={val()} onInput={(e) => setVal(e.currentTarget.value)}
            onKeyDown={(e) => { if (e.key === "Enter") { props.onSave?.(val()); setEditing(false); } if (e.key === "Escape") setEditing(false); }}
            class="mt-1 w-64 rounded px-2 py-1 text-sm outline-none"
            style={{ background: "var(--bg-tertiary)", color: "var(--text-primary)" }}
            autofocus
          />
        }>
          <div class="text-sm" style={{ color: "var(--text-primary)" }}>{props.value}</div>
        </Show>
      </div>
      <Show when={!props.readonly}>
        <Show when={!editing()} fallback={
          <div class="flex gap-2">
            <button onClick={() => setEditing(false)} class="rounded px-3 py-1 text-xs" style={{ color: "var(--text-muted)" }}>Cancel</button>
            <button onClick={() => { props.onSave?.(val()); setEditing(false); }} class="rounded px-3 py-1 text-xs text-white" style={{ background: "var(--accent)" }}>Save</button>
          </div>
        }>
          <button onClick={() => setEditing(true)} class="rounded px-3 py-1 text-xs" style={{ background: "rgba(255,255,255,0.06)", color: "var(--text-secondary)" }}>Edit</button>
        </Show>
      </Show>
    </div>
  );
}

// ── Placeholder pages ──

function AppearancePage() {
  return (
    <>
      <SectionTitle>Appearance</SectionTitle>
      <p class="text-sm" style={{ color: "var(--text-muted)" }}>Theme customization coming soon.</p>
    </>
  );
}

function NotificationsPage() {
  return (
    <>
      <SectionTitle>Notifications</SectionTitle>
      <p class="text-sm" style={{ color: "var(--text-muted)" }}>Notification settings coming soon.</p>
    </>
  );
}

function DevicesPage() {
  return (
    <>
      <SectionTitle>Devices</SectionTitle>
      <p class="text-sm" style={{ color: "var(--text-muted)" }}>Device management coming soon.</p>
    </>
  );
}

function AboutPage() {
  return (
    <>
      <SectionTitle>About fluxchat</SectionTitle>
      <div class="rounded-xl p-4" style={{ background: "var(--bg-secondary)" }}>
        <p class="mb-2 text-sm" style={{ color: "var(--text-primary)" }}>
          <strong>fluxchat</strong> v0.1.0
        </p>
        <p class="mb-2 text-sm" style={{ color: "var(--text-secondary)" }}>
          A cross-platform Matrix chat client with a Discord-style UI.
        </p>
        <p class="text-sm" style={{ color: "var(--text-muted)" }}>
          Built with Tauri v2, Rust, SolidJS, and FluxDB.
        </p>
        <p class="mt-2 text-sm" style={{ color: "var(--text-muted)" }}>
          Vibecoded with Claude Code.
        </p>
      </div>
    </>
  );
}
