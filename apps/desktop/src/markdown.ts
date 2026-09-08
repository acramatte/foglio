import { Marked } from "marked";
import DOMPurify from "dompurify";
const escape = (s: string) =>
  s.replace(
    /[&<>"']/g,
    (c) =>
      ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[
        c
      ]!,
  );
export type Link = { kind: "external" | "note"; target: string };
export function classifyLink(value: string): Link | null {
  if (!value || /[\\\u0000-\u0020\u007f]/.test(value)) return null;
  if (/^(https?:|mailto:)/i.test(value)) {
    try {
      const url = new URL(value);
      if (
        ["https:", "http:"].includes(url.protocol) &&
        (!url.hostname || url.username || url.password)
      )
        return null;
      return { kind: "external", target: value };
    } catch {
      return null;
    }
  }
  if (
    /^[a-z][a-z0-9+.-]*:/i.test(value) ||
    value.startsWith("/") ||
    value.startsWith("#")
  )
    return null;
  let decoded: string;
  try {
    decoded = decodeURIComponent(value);
  } catch {
    return null;
  }
  if (
    /[\\\\\u0000-\u001f\u007f?%]/.test(decoded) ||
    decoded.startsWith("/") ||
    decoded.includes(":") ||
    !decoded.split("#", 1)[0]!.endsWith(".md")
  )
    return null;
  return { kind: "note", target: value };
}
const marked = new Marked({
  gfm: true,
  breaks: false,
  renderer: {
    html({ text }) {
      return escape(text);
    },
    image({ text }) {
      return `<span>[Image: ${escape(text || "image omitted")}]</span>`;
    },
    link({ href, tokens }) {
      const label = this.parser.parseInline(tokens);
      return classifyLink(href)
        ? `<a data-target="${escape(href)}" href="#">${label}</a>`
        : `<span>${label} <span>(unsupported link)</span></span>`;
    },
    checkbox({ checked }) {
      return checked ? "☑ " : "☐ ";
    },
  },
});
export function renderMarkdown(source: string): string {
  return DOMPurify.sanitize(marked.parse(source, { async: false }), {
    ALLOWED_TAGS: [
      "p",
      "br",
      "hr",
      "h1",
      "h2",
      "h3",
      "h4",
      "h5",
      "h6",
      "blockquote",
      "ul",
      "ol",
      "li",
      "pre",
      "code",
      "em",
      "strong",
      "del",
      "table",
      "thead",
      "tbody",
      "tr",
      "th",
      "td",
      "a",
      "span",
    ],
    ALLOWED_ATTR: ["href", "data-target", "start", "align"],
    ALLOW_DATA_ATTR: false,
    ALLOW_ARIA_ATTR: false,
  });
}
