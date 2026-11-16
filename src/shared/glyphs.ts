import type { GlyphSet } from "./types";

const NAME = /^[a-z0-9-]{1,64}$/;

/** Same-origin URL of a bundled key icon: the desktop app serves public/glyphs itself, the phone gets them from the PC.
 *  Undefined for anything but the two sets and a plain name, since the URL goes into a CSS `url()`. */
export const glyphUrl = (set: string, name: string): string | undefined =>
  (set === "ph" || set === "si") && NAME.test(name) ? `/glyphs/${set}/${name}.svg` : undefined;

/** A logo's brand colour ("76B900") as a key colour ("#76b900"); null for an icon, or anything that is not a colour. */
export const brandColor = (hex: string | undefined): string | null =>
  hex && /^[0-9a-f]{6}$/i.test(hex) ? `#${hex.toLowerCase()}` : null;

export interface GlyphEntry {
  set: GlyphSet;
  name: string;
  title: string;
  /** Lower-case words the search matches. */
  words: string;
  /** Brand colour, "rrggbb" (logos only). */
  hex?: string;
}

/** Shown before anything is typed: what a stream deck uses most. */
const STARTERS: Record<GlyphSet, string[]> = {
  ph: ["play-pause", "skip-back", "skip-forward", "speaker-high", "speaker-low", "speaker-x", "microphone", "microphone-slash",
    "video-camera", "video-camera-slash", "record", "broadcast", "monitor-play", "screencast", "camera", "chat-circle", "keyboard",
    "rocket-launch", "game-controller", "music-notes", "headphones", "lightning", "star", "heart", "timer", "clock", "coffee", "moon",
    "sun", "lock", "power", "folder", "globe", "bell", "terminal-window", "browser", "house", "gear"],
  si: ["obsstudio", "twitch", "youtube", "kick", "spotify", "discord", "steam", "googlechrome", "firefoxbrowser", "streamlabs", "tiktok",
    "instagram", "x", "facebook", "reddit", "netflix", "applemusic", "soundcloud", "telegram", "whatsapp", "zoom", "vlcmediaplayer",
    "epicgames", "battledotnet", "riotgames", "ubisoft", "playstation", "nvidia", "amd"],
};

/** Best matches first: the exact name, then names starting with the query, then names containing it, then other words. */
export function searchGlyphs(all: GlyphEntry[], set: GlyphSet, query: string): GlyphEntry[] {
  const pool = all.filter((e) => e.set === set);
  const q = query.trim().toLowerCase();
  if (!q) return STARTERS[set].flatMap((n) => pool.filter((e) => e.name === n));
  const terms = q.split(/\s+/);
  const rank = (e: GlyphEntry) => {
    const name = e.name, title = e.title.toLowerCase();
    if (name === q || title === q) return 0;
    if (name.startsWith(q) || title.startsWith(q)) return 1;
    if (name.includes(q) || title.includes(q)) return 2;
    return 3;
  };
  return pool
    .filter((e) => terms.every((t) => e.words.includes(t)))
    .map((e) => [rank(e), e] as const)
    .sort((a, b) => a[0] - b[0] || a[1].name.length - b[1].name.length)
    .map(([, e]) => e);
}

let loading: Promise<GlyphEntry[]> | null = null;

/** The picker's lists (written by scripts/glyphs.mjs), fetched once; a failed fetch is tried again next time. */
export function loadGlyphs(): Promise<GlyphEntry[]> {
  loading ??= Promise.all([
    fetch("/glyphs/ph.json").then((r) => r.json() as Promise<[string, string][]>),
    fetch("/glyphs/si.json").then((r) => r.json() as Promise<[string, string, string][]>),
  ]).then(([ph, si]) => [
    ...ph.map(([name, words]): GlyphEntry => ({ set: "ph", name, title: name.replace(/-/g, " "), words })),
    ...si.map(([name, title, hex]): GlyphEntry => ({ set: "si", name, title, words: `${name} ${title.toLowerCase()}`, hex })),
  ]);
  loading.catch(() => { loading = null; });
  return loading;
}
