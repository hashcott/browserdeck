/** Whitespace separates arguments; a double-quoted run is one argument. Inside quotes, backslashes before a quote
 *  follow the Windows rule (2n -> n backslashes and the quote closes; 2n+1 -> n backslashes and a literal quote);
 *  other backslashes are literal. An unterminated quote runs to the end. */
export function parseArgs(text: string): string[] {
  const out: string[] = [];
  let cur = "";
  let has = false;
  let quoted = false;
  for (let i = 0; i < text.length; i++) {
    const c = text[i];
    if (quoted && c === "\\") {
      let n = 1;
      while (text[i + n] === "\\") n++;
      if (text[i + n] === '"') {
        cur += "\\".repeat(n >> 1);
        if (n & 1) cur += '"'; else quoted = false;
        i += n;
      } else { cur += "\\".repeat(n); i += n - 1; }
    } else if (quoted) {
      if (c === '"') quoted = false; else cur += c;
    } else if (c === '"') { quoted = true; has = true; }
    else if (/\s/.test(c)) { if (has) out.push(cur); cur = ""; has = false; }
    else { cur += c; has = true; }
  }
  if (has) out.push(cur);
  return out;
}

const quote = (a: string) => {
  let s = '"';
  let slashes = 0;
  for (const c of a) {
    if (c === "\\") { slashes++; continue; }
    s += c === '"' ? "\\".repeat(slashes * 2 + 1) + c : "\\".repeat(slashes) + c;
    slashes = 0;
  }
  return s + "\\".repeat(slashes * 2) + '"';
};

/** Quotes any argument that is empty or contains whitespace or a quote. */
export const formatArgs = (args: string[]): string => args.map((a) => (a === "" || /[\s"]/.test(a) ? quote(a) : a)).join(" ");
