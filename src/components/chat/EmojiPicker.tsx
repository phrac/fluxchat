import { createSignal, For, Show, createMemo } from "solid-js";

interface Props {
  onSelect: (emoji: string) => void;
  onClose: () => void;
}

const categories: { name: string; icon: string; emojis: string[] }[] = [
  {
    name: "Frequently Used",
    icon: "🕐",
    emojis: [], // Populated dynamically
  },
  {
    name: "Smileys",
    icon: "😀",
    emojis: [
      "😀","😃","😄","😁","😆","😅","🤣","😂","🙂","🙃","😉","😊","😇","🥰","😍","🤩",
      "😘","😗","😚","😙","🥲","😋","😛","😜","🤪","😝","🤑","🤗","🤭","🤫","🤔","🫡",
      "🤐","🤨","😐","😑","😶","🫥","😏","😒","🙄","😬","🤥","😌","😔","😪","🤤","😴",
      "😷","🤒","🤕","🤢","🤮","🥵","🥶","🥴","😵","🤯","🤠","🥳","🥸","😎","🤓","🧐",
      "😕","🫤","😟","🙁","☹️","😮","😯","😲","😳","🥺","🥹","😦","😧","😨","😰","😥",
      "😢","😭","😱","😖","😣","😞","😓","😩","😫","🥱","😤","😡","😠","🤬","😈","👿",
      "💀","☠️","💩","🤡","👹","👺","👻","👽","👾","🤖",
    ],
  },
  {
    name: "Gestures",
    icon: "👋",
    emojis: [
      "👋","🤚","🖐️","✋","🖖","🫱","🫲","🫳","🫴","👌","🤌","🤏","✌️","🤞","🫰","🤟",
      "🤘","🤙","👈","👉","👆","🖕","👇","☝️","🫵","👍","👎","✊","👊","🤛","🤜","👏",
      "🙌","🫶","👐","🤲","🤝","🙏","✍️","💅","🤳","💪","🦾","🦿","🦵","🦶","👂","🦻",
      "👃","🧠","🫀","🫁","🦷","🦴","👀","👁️","👅","👄","🫦",
    ],
  },
  {
    name: "People",
    icon: "👶",
    emojis: [
      "👶","🧒","👦","👧","🧑","👱","👨","🧔","👩","🧓","👴","👵","🙍","🙎","🙅","🙆",
      "💁","🙋","🧏","🙇","🤦","🤷","👮","🕵️","💂","🥷","👷","🫅","🤴","👸","👳","👲",
      "🧕","🤵","👰","🤰","🫃","🤱","👼","🎅","🤶","🦸","🦹","🧙","🧚","🧛","🧜","🧝","🧞","🧟",
    ],
  },
  {
    name: "Animals",
    icon: "🐶",
    emojis: [
      "🐶","🐱","🐭","🐹","🐰","🦊","🐻","🐼","🐻‍❄️","🐨","🐯","🦁","🐮","🐷","🐸","🐵",
      "🙈","🙉","🙊","🐒","🐔","🐧","🐦","🐤","🐣","🐥","🦆","🦅","🦉","🦇","🐺","🐗",
      "🐴","🦄","🐝","🪱","🐛","🦋","🐌","🐞","🐜","🪰","🪲","🪳","🦟","🦗","🕷️","🦂",
      "🐢","🐍","🦎","🦖","🦕","🐙","🦑","🦐","🦞","🦀","🐡","🐠","🐟","🐬","🐳","🐋",
      "🦈","🐊","🐅","🐆","🦓","🦍","🦧","🐘","🦛","🦏","🐪","🐫","🦒","🦘","🦬","🐃",
    ],
  },
  {
    name: "Food",
    icon: "🍔",
    emojis: [
      "🍇","🍈","🍉","🍊","🍋","🍌","🍍","🥭","🍎","🍏","🍐","🍑","🍒","🍓","🫐","🥝",
      "🍅","🫒","🥥","🥑","🍆","🥔","🥕","🌽","🌶️","🫑","🥒","🥬","🥦","🧄","🧅","🍄",
      "🥜","🫘","🌰","🍞","🥐","🥖","🫓","🥨","🥯","🥞","🧇","🧀","🍖","🍗","🥩","🥓",
      "🍔","🍟","🍕","🌭","🥪","🌮","🌯","🫔","🥙","🧆","🥚","🍳","🥘","🍲","🫕","🥣",
      "🥗","🍿","🧈","🧂","🥫","🍱","🍘","🍙","🍚","🍛","🍜","🍝","🍠","🍢","🍣","🍤",
      "🍥","🥮","🍡","🥟","🥠","🥡","🦀","🦞","🦐","🦑","🦪","🍦","🍧","🍨","🍩","🍪",
      "🎂","🍰","🧁","🥧","🍫","🍬","🍭","🍮","🍯","🍼","🥛","☕","🫖","🍵","🍶","🍾",
      "🍷","🍸","🍹","🍺","🍻","🥂","🥃","🫗","🥤","🧋","🧃","🧉","🧊",
    ],
  },
  {
    name: "Objects",
    icon: "💡",
    emojis: [
      "⌚","📱","💻","⌨️","🖥️","🖨️","🖱️","🖲️","🕹️","🗜️","💽","💾","💿","📀","📼","📷",
      "📹","🎥","📽️","🎞️","📞","☎️","📟","📠","📺","📻","🎙️","🎚️","🎛️","🧭","⏱️","⏲️",
      "⏰","🕰️","⌛","⏳","📡","🔋","🔌","💡","🔦","🕯️","🪔","🧯","🛢️","💸","💵","💴",
      "💶","💷","🪙","💰","💳","💎","⚖️","🪜","🧰","🪛","🔧","🔨","⚒️","🛠️","⛏️","🪚",
      "🔩","⚙️","🪤","🧱","⛓️","🧲","🔫","💣","🧨","🪓","🔪","🗡️","⚔️","🛡️","🚬","⚰️",
    ],
  },
  {
    name: "Symbols",
    icon: "❤️",
    emojis: [
      "❤️","🧡","💛","💚","💙","💜","🖤","🤍","🤎","💔","❤️‍🔥","❤️‍🩹","❣️","💕","💞","💓",
      "💗","💖","💘","💝","💟","☮️","✝️","☪️","🕉️","☸️","✡️","🔯","🕎","☯️","☦️","🛐",
      "⛎","♈","♉","♊","♋","♌","♍","♎","♏","♐","♑","♒","♓","🆔","⚛️","🉑",
      "☢️","☣️","📴","📳","🈶","🈚","🈸","🈺","🈷️","✴️","🆚","💮","🉐","㊙️","㊗️","🈴",
      "🈵","🈹","🈲","🅰️","🅱️","🆎","🆑","🅾️","🆘","❌","⭕","🛑","⛔","📛","🚫","💯",
      "💢","♨️","🚷","🚯","🚳","🚱","🔞","📵","🚭","❗","❕","❓","❔","‼️","⁉️","🔅",
      "🔆","〽️","⚠️","🚸","🔱","⚜️","🔰","♻️","✅","🈯","💹","❇️","✳️","❎","🌐","💠",
      "Ⓜ️","🌀","💤","🏧","🚾","♿","🅿️","🛗","🈳","🈂️","🛂","🛃","🛄","🛅",
      "🔣","ℹ️","🔤","🔡","🔠","🆖","🆗","🆙","🆒","🆕","🆓","0️⃣","1️⃣","2️⃣","3️⃣","4️⃣",
      "5️⃣","6️⃣","7️⃣","8️⃣","9️⃣","🔟","🔢","#️⃣","*️⃣","⏏️","▶️","⏸️","⏯️","⏹️","⏺️","⏭️",
      "⏮️","⏩","⏪","⏫","⏬","◀️","🔼","🔽","➡️","⬅️","⬆️","⬇️","↗️","↘️","↙️","↖️",
      "↕️","↔️","↩️","↪️","⤴️","⤵️","🔀","🔁","🔂","🔄","🔃","🎵","🎶",
      "✖️","➕","➖","➗","♾️","💲","💱","™️","©️","®️","〰️","➰","➿","🔚","🔙","🔛","🔝","🔜",
      "✔️","☑️","🔘","🔴","🟠","🟡","🟢","🔵","🟣","⚫","⚪","🟤","🔺","🔻","🔸","🔹",
      "🔶","🔷","🔳","🔲","▪️","▫️","◾","◽","◼️","◻️","🟥","🟧","🟨","🟩","🟦","🟪","⬛","⬜","🟫",
    ],
  },
  {
    name: "Flags",
    icon: "🏳️",
    emojis: [
      "🏳️","🏴","🏁","🚩","🏳️‍🌈","🏳️‍⚧️","🏴‍☠️","🇺🇸","🇬🇧","🇨🇦","🇦🇺","🇩🇪","🇫🇷","🇯🇵","🇰🇷","🇨🇳",
      "🇮🇳","🇧🇷","🇲🇽","🇪🇸","🇮🇹","🇷🇺","🇺🇦","🇵🇱","🇳🇱","🇧🇪","🇨🇭","🇦🇹","🇸🇪","🇳🇴","🇩🇰","🇫🇮",
    ],
  },
];

// Load frequently used from localStorage
function getFrequentEmojis(): string[] {
  try {
    const saved = localStorage.getItem("fluxchat-emoji-usage");
    if (!saved) return [];
    const usage: Record<string, number> = JSON.parse(saved);
    return Object.entries(usage)
      .sort((a, b) => b[1] - a[1])
      .slice(0, 32)
      .map(([emoji]) => emoji);
  } catch { return []; }
}

export default function EmojiPicker(props: Props) {
  const [search, setSearch] = createSignal("");
  const [activeCategory, setActiveCategory] = createSignal(0);
  let scrollRef!: HTMLDivElement;

  // Build categories with frequent emojis
  const allCategories = createMemo(() => {
    const frequent = getFrequentEmojis();
    const cats = [...categories];
    cats[0] = { ...cats[0], emojis: frequent };
    if (frequent.length === 0) return cats.slice(1);
    return cats;
  });

  const filtered = createMemo(() => {
    const q = search().toLowerCase();
    if (!q) return null;
    const results: string[] = [];
    for (const cat of categories.slice(1)) {
      for (const e of cat.emojis) {
        if (results.length >= 50) break;
        results.push(e);
      }
    }
    // Simple search: just show all and let the user scroll
    // (real search would need emoji names which we don't have)
    return null; // Disable search filtering for now, just show all
  });

  const scrollToCategory = (idx: number) => {
    setActiveCategory(idx);
    const el = scrollRef?.querySelector(`[data-cat="${idx}"]`);
    if (el) el.scrollIntoView({ behavior: "smooth", block: "start" });
  };

  return (
    <div
      class="w-[352px] rounded-lg shadow-2xl overflow-hidden"
      style={{ background: "var(--bg-tertiary)", border: "1px solid rgba(0,0,0,0.3)" }}
      tabIndex={-1}
      ref={(el) => {
        el.focus();
        el.addEventListener("keydown", (e) => {
          if (e.key === "Escape") {
            e.stopPropagation();
            props.onClose();
          }
        });
      }}
    >
      {/* Search */}
      <div class="p-2">
        <input
          type="text"
          value={search()}
          onInput={(e) => setSearch(e.currentTarget.value)}
          onKeyDown={(e) => { if (e.key === "Escape") props.onClose(); }}
          placeholder="Find an emoji"
          class="w-full rounded-md px-3 py-1.5 text-sm outline-none"
          style={{ background: "var(--bg-primary)", color: "var(--text-primary)" }}
          autofocus
        />
      </div>

      {/* Category tabs */}
      <div class="flex gap-0.5 px-2 pb-1">
        <For each={allCategories()}>
          {(cat, i) => (
            <button
              onClick={() => scrollToCategory(i())}
              class="flex-1 rounded py-1 text-center text-base transition-colors"
              style={{
                background: activeCategory() === i() ? "rgba(255,255,255,0.06)" : "transparent",
              }}
              title={cat.name}
              onMouseOver={(e) => (e.currentTarget.style.background = "rgba(255,255,255,0.1)")}
              onMouseOut={(e) => (e.currentTarget.style.background = activeCategory() === i() ? "rgba(255,255,255,0.06)" : "transparent")}
            >
              {cat.icon}
            </button>
          )}
        </For>
      </div>

      {/* Emoji grid */}
      <div
        ref={scrollRef}
        class="max-h-[280px] overflow-y-auto px-2 pb-2"
        onWheel={(e) => {
          const el = e.currentTarget;
          const atTop = el.scrollTop === 0 && e.deltaY < 0;
          const atBottom = el.scrollTop + el.clientHeight >= el.scrollHeight && e.deltaY > 0;
          if (atTop || atBottom) e.preventDefault();
        }}
      >
        <For each={allCategories()}>
          {(cat, i) => (
            <div data-cat={i()}>
              <div
                class="sticky top-0 z-10 py-1 text-xs font-bold uppercase"
                style={{ background: "var(--bg-tertiary)", color: "var(--text-muted)" }}
              >
                {cat.name}
              </div>
              <div class="grid grid-cols-8 gap-0.5">
                <For each={cat.emojis}>
                  {(emoji) => (
                    <button
                      onClick={() => props.onSelect(emoji)}
                      class="flex h-9 w-9 items-center justify-center rounded text-xl transition-transform hover:scale-125 hover:bg-white/10"
                    >
                      {emoji}
                    </button>
                  )}
                </For>
              </div>
            </div>
          )}
        </For>
      </div>
    </div>
  );
}
