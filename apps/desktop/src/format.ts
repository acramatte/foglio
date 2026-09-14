export type FormatAction =
  | "bold"
  | "italic"
  | "code"
  | "strike"
  | "link"
  | "bullet"
  | "ordered"
  | "quote"
  | "heading"
  | "fence";

export type FormatResult = { text: string; start: number; end: number };

type InlineSpec = { open: string; close: string; placeholder: string };
type LinePrefix = { prefix: string; pattern: RegExp };

const INLINE: Record<"bold" | "italic" | "code" | "strike", InlineSpec> = {
  bold: { open: "**", close: "**", placeholder: "bold" },
  italic: { open: "*", close: "*", placeholder: "italic" },
  code: { open: "`", close: "`", placeholder: "code" },
  strike: { open: "~~", close: "~~", placeholder: "text" },
};

const LINE: Record<"bullet" | "ordered" | "quote" | "heading", LinePrefix> = {
  bullet: { prefix: "- ", pattern: /^(?:[-*+]|\d+\.)\s+/ },
  ordered: { prefix: "1. ", pattern: /^(?:[-*+]|\d+\.)\s+/ },
  quote: { prefix: "> ", pattern: /^>\s?/ },
  heading: { prefix: "# ", pattern: /^#{1,6}\s+/ },
};

export function applyFormat(text: string, start: number, end: number, action: Exclude<FormatAction, "link">): FormatResult {
  const from = Math.max(0, Math.min(start, end, text.length));
  const to = Math.max(0, Math.min(Math.max(start, end), text.length));
  if (action === "fence") return wrapFence(text, from, to);
  if (action in LINE) return toggleLines(text, from, to, LINE[action as keyof typeof LINE]);
  return wrapInline(text, from, to, INLINE[action as keyof typeof INLINE]);
}

function wrapInline(text: string, start: number, end: number, spec: InlineSpec): FormatResult {
  const { open, close } = spec;
  if (start === end) {
    const insert = open + spec.placeholder + close;
    return {
      text: text.slice(0, start) + insert + text.slice(end),
      start: start + open.length,
      end: start + open.length + spec.placeholder.length,
    };
  }
  if (wrapped(text, start, end, spec, "inside")) {
    return {
      text: text.slice(0, start) + text.slice(start + open.length, end - close.length) + text.slice(end),
      start,
      end: end - open.length - close.length,
    };
  }
  if (wrapped(text, start, end, spec, "outside")) {
    return {
      text: text.slice(0, start - open.length) + text.slice(start, end) + text.slice(end + close.length),
      start: start - open.length,
      end: end - open.length,
    };
  }
  return {
    text: text.slice(0, start) + open + text.slice(start, end) + close + text.slice(end),
    start,
    end: end + open.length + close.length,
  };
}

function wrapped(text: string, start: number, end: number, spec: InlineSpec, where: "inside" | "outside"): boolean {
  const { open, close } = spec;
  const from = where === "inside" ? start : start - open.length;
  const to = where === "inside" ? end : end + close.length;
  if (from < 0 || to > text.length || to - from < open.length + close.length) return false;
  if (text.slice(from, from + open.length) !== open || text.slice(to - close.length, to) !== close) return false;
  // A lone "*" must not steal a "**" bold pair. Count the whole run containing `from`.
  if (open === "*" && asteriskRun(text, from) % 2 === 0) return false;
  return true;
}

function asteriskRun(text: string, index: number): number {
  if (text[index] !== "*") return 0;
  let start = index, end = index;
  while (start > 0 && text[start - 1] === "*") start--;
  while (end + 1 < text.length && text[end + 1] === "*") end++;
  return end - start + 1;
}

function wrapFence(text: string, start: number, end: number): FormatResult {
  const range = expandLines(text, start, end);
  const block = text.slice(range.start, range.end);
  const fenced = block.match(/^```[^\n]*\n([\s\S]*?)\n```$/);
  if (fenced) {
    return {
      text: text.slice(0, range.start) + fenced[1] + text.slice(range.end),
      start: range.start,
      end: range.start + fenced[1].length,
    };
  }
  const inner = block || "code";
  const replacement = "```\n" + inner + "\n```";
  return {
    text: text.slice(0, range.start) + replacement + text.slice(range.end),
    start: range.start + 4,
    end: range.start + 4 + inner.length,
  };
}

function toggleLines(text: string, start: number, end: number, spec: LinePrefix): FormatResult {
  const range = expandLines(text, start, end);
  const block = text.slice(range.start, range.end);
  const lines = block.split("\n");
  const nonempty = lines.filter(line => line.length > 0);
  const allPrefixed = nonempty.length > 0 && nonempty.every(line => spec.pattern.test(line));
  const next = lines.map(line => {
    if (!line.length) return line;
    return allPrefixed ? line.replace(spec.pattern, "") : spec.prefix + line;
  }).join("\n");
  return {
    text: text.slice(0, range.start) + next + text.slice(range.end),
    start: range.start,
    end: range.start + next.length,
  };
}

function expandLines(text: string, start: number, end: number): { start: number; end: number } {
  let from = start;
  let to = end;
  while (from > 0 && text[from - 1] !== "\n") from--;
  if (to > from && text[to - 1] === "\n") to--;
  while (to < text.length && text[to] !== "\n") to++;
  return { start: from, end: to };
}
