import { createSignal, Show, onMount } from "solid-js";
import { getProfile, setDisplayName, uploadAvatar, removeAvatar, uploadBanner, removeBanner, getMediaUrl } from "../../lib/tauri";
import { logout, userId } from "../../stores/auth";

interface Props {
  onClose: () => void;
}

export default function ProfileModal(props: Props) {
  const [displayNameVal, setDisplayNameVal] = createSignal("");
  const [avatarSrc, setAvatarSrc] = createSignal<string | null>(null);
  const [avatarMxc, setAvatarMxc] = createSignal<string | null>(null);
  const [bannerSrc, setBannerSrc] = createSignal<string | null>(null);
  const [saving, setSaving] = createSignal(false);
  const [error, setError] = createSignal("");
  const [success, setSuccess] = createSignal("");
  let avatarInputRef!: HTMLInputElement;
  let bannerInputRef!: HTMLInputElement;

  onMount(async () => {
    try {
      const profile = await getProfile();
      setDisplayNameVal(profile.display_name || "");
      setAvatarMxc(profile.avatar_url);
      if (profile.avatar_url) {
        setAvatarSrc(await getMediaUrl(profile.avatar_url).catch(() => null));
      }
      if (profile.banner_url) {
        setBannerSrc(await getMediaUrl(profile.banner_url).catch(() => null));
      }
    } catch {}
  });

  const handleSave = async () => {
    setSaving(true);
    setError(""); setSuccess("");
    try {
      await setDisplayName(displayNameVal().trim());
      setSuccess("Profile updated");
      setTimeout(() => setSuccess(""), 2000);
    } catch (err: any) { setError(String(err)); }
    setSaving(false);
  };

  const handleAvatarSelect = async (e: Event) => {
    const file = (e.target as HTMLInputElement).files?.[0];
    if (!file) return;
    setSaving(true); setError("");
    try {
      const bytes = Array.from(new Uint8Array(await file.arrayBuffer()));
      const mxc = await uploadAvatar(file.type || "image/png", bytes);
      setAvatarMxc(mxc);
      setAvatarSrc(await getMediaUrl(mxc).catch(() => null));
      setSuccess("Avatar updated"); setTimeout(() => setSuccess(""), 2000);
    } catch (err: any) { setError(String(err)); }
    setSaving(false);
    (e.target as HTMLInputElement).value = "";
  };

  const handleBannerSelect = async (e: Event) => {
    const file = (e.target as HTMLInputElement).files?.[0];
    if (!file) return;
    setSaving(true); setError("");
    try {
      const bytes = Array.from(new Uint8Array(await file.arrayBuffer()));
      const mxc = await uploadBanner(file.type || "image/png", bytes);
      setBannerSrc(await getMediaUrl(mxc).catch(() => null));
      setSuccess("Banner updated"); setTimeout(() => setSuccess(""), 2000);
    } catch (err: any) { setError(String(err)); }
    setSaving(false);
    (e.target as HTMLInputElement).value = "";
  };

  const initials = () => (displayNameVal() || userId()).replace(/^@/, "")[0]?.toUpperCase() || "?";

  return (
    <div
      class="fixed inset-0 z-50 flex items-center justify-center"
      style={{ background: "rgba(0,0,0,0.6)" }}
      onClick={(e) => { if (e.target === e.currentTarget) props.onClose(); }}
    >
      <div class="w-[500px] rounded-xl overflow-hidden" style={{ background: "var(--bg-primary)" }}>

        {/* Banner */}
        <div
          class="relative h-36 cursor-pointer overflow-hidden"
          style={{ background: bannerSrc() ? undefined : "linear-gradient(135deg, #5865f2 0%, #8b5cf6 50%, #eb459e 100%)" }}
          onClick={() => bannerInputRef?.click()}
        >
          <Show when={bannerSrc()}>
            <img src={bannerSrc()!} class="h-full w-full object-cover" />
          </Show>
          <div
            class="absolute inset-0 flex items-center justify-center transition-opacity"
            style={{ background: "rgba(0,0,0,0.4)", opacity: "0" }}
            onMouseOver={(e) => (e.currentTarget.style.opacity = "1")}
            onMouseOut={(e) => (e.currentTarget.style.opacity = "0")}
          >
            <div class="flex flex-col items-center text-white">
              <svg width="28" height="28" viewBox="0 0 24 24" fill="currentColor">
                <path d="M21 19V5c0-1.1-.9-2-2-2H5c-1.1 0-2 .9-2 2v14c0 1.1.9 2 2 2h14c1.1 0 2-.9 2-2zM8.5 13.5l2.5 3.01L14.5 12l4.5 6H5l3.5-4.5z"/>
              </svg>
              <span class="mt-1 text-xs font-semibold uppercase tracking-wide">Change Banner</span>
            </div>
          </div>
          <input ref={bannerInputRef} type="file" accept="image/*" class="hidden" onChange={handleBannerSelect} />
        </div>

        {/* Profile card overlay */}
        <div class="relative mx-4 -mt-16 rounded-xl p-4" style={{ background: "var(--bg-tertiary)" }}>

          {/* Avatar row */}
          <div class="flex items-end gap-4 mb-4">
            {/* Avatar */}
            <div
              class="relative -mt-10 h-24 w-24 flex-shrink-0 cursor-pointer overflow-hidden rounded-full border-[5px]"
              style={{ "border-color": "var(--bg-tertiary)" }}
              onClick={(e) => { e.stopPropagation(); avatarInputRef?.click(); }}
            >
              <Show
                when={avatarSrc()}
                fallback={
                  <div class="flex h-full w-full items-center justify-center text-3xl font-bold text-white" style={{ background: "var(--accent)" }}>
                    {initials()}
                  </div>
                }
              >
                <img src={avatarSrc()!} class="h-full w-full object-cover" />
              </Show>
              <div
                class="absolute inset-0 flex flex-col items-center justify-center transition-opacity"
                style={{ background: "rgba(0,0,0,0.6)", opacity: "0" }}
                onMouseOver={(e) => (e.currentTarget.style.opacity = "1")}
                onMouseOut={(e) => (e.currentTarget.style.opacity = "0")}
              >
                <svg width="20" height="20" viewBox="0 0 24 24" fill="white">
                  <path d="M12 12c2.21 0 4-1.79 4-4s-1.79-4-4-4-4 1.79-4 4 1.79 4 4 4zm0 2c-2.67 0-8 1.34-8 4v2h16v-2c0-2.66-5.33-4-8-4z"/>
                </svg>
                <span class="text-[8px] font-bold uppercase text-white">Change</span>
              </div>
              <input ref={avatarInputRef} type="file" accept="image/*" class="hidden" onChange={handleAvatarSelect} />
            </div>

            {/* Name + ID */}
            <div class="min-w-0 flex-1 pb-1">
              <div class="text-lg font-bold" style={{ color: "var(--text-primary)" }}>
                {displayNameVal() || userId().replace(/^@/, "").split(":")[0]}
              </div>
              <div class="text-xs" style={{ color: "var(--text-muted)" }}>
                {userId()}
              </div>
            </div>
          </div>

          {/* Divider */}
          <div class="mb-3 h-px" style={{ background: "var(--border-subtle)" }} />

          {/* Messages */}
          <Show when={error()}>
            <div class="mb-3 rounded p-2 text-sm" style={{ background: "rgba(242,63,67,0.1)", color: "var(--red-dnd)" }}>{error()}</div>
          </Show>
          <Show when={success()}>
            <div class="mb-3 rounded p-2 text-sm" style={{ background: "rgba(35,165,89,0.1)", color: "var(--green-online)" }}>{success()}</div>
          </Show>

          {/* Display Name */}
          <label class="mb-1 block text-[11px] font-bold uppercase" style={{ color: "var(--text-secondary)" }}>
            Display Name
          </label>
          <input
            type="text"
            value={displayNameVal()}
            onInput={(e) => setDisplayNameVal(e.currentTarget.value)}
            class="mb-3 w-full rounded px-3 py-2 text-sm outline-none"
            style={{ background: "var(--bg-primary)", color: "var(--text-primary)" }}
          />

          {/* Remove links */}
          <div class="mb-3 flex gap-4">
            <Show when={avatarMxc()}>
              <button
                onClick={async () => { await removeAvatar(); setAvatarSrc(null); setAvatarMxc(null); }}
                class="text-[11px] hover:underline"
                style={{ color: "var(--text-muted)" }}
              >
                Remove Avatar
              </button>
            </Show>
            <Show when={bannerSrc()}>
              <button
                onClick={async () => { await removeBanner(); setBannerSrc(null); }}
                class="text-[11px] hover:underline"
                style={{ color: "var(--text-muted)" }}
              >
                Remove Banner
              </button>
            </Show>
          </div>

          {/* Actions */}
          <div class="flex items-center justify-between">
            <button
              onClick={async () => { await logout(); props.onClose(); }}
              class="rounded px-3 py-1.5 text-xs font-medium"
              style={{ background: "rgba(242,63,67,0.1)", color: "var(--red-dnd)" }}
            >
              Log Out
            </button>
            <div class="flex gap-2">
              <button
                onClick={props.onClose}
                class="rounded px-3 py-1.5 text-xs"
                style={{ color: "var(--text-secondary)" }}
              >
                Cancel
              </button>
              <button
                onClick={handleSave}
                disabled={saving()}
                class="rounded px-4 py-1.5 text-xs font-medium text-white"
                style={{ background: saving() ? "var(--accent-hover)" : "var(--accent)" }}
              >
                {saving() ? "Saving..." : "Save"}
              </button>
            </div>
          </div>
        </div>

        {/* Bottom padding */}
        <div class="h-4" />
      </div>
    </div>
  );
}
