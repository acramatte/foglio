// @vitest-environment jsdom
import { describe, it, expect } from "vitest";
import { renderMarkdown, classifyLink } from "./markdown";

describe("safe Markdown", () => {
  it("renders GFM while preserving source and removing executable surfaces", () => {
    const source = '# Heading\n\n| A | B |\n|---|---|\n|1|2|\n\n- [x] done\n\n~~old~~ **bold** `code`\n\n```js\nalert(1)\n```\n\n<script>alert(1)</script>\n<svg onload="alert(1)"></svg>\n<iframe src="https://bad.test"></iframe>\n\n![remote](https://bad.test/x)\n![local][image]\n\n[image]: file:///etc/passwd\n\n[bad](javascript:alert(1))\n[good](https://example.org)';
    const host = document.createElement("div");
    host.innerHTML = renderMarkdown(source);
    expect(host.querySelector("h1")?.textContent).toBe("Heading");
    expect(host.querySelector("table")).not.toBeNull();
    expect(host.querySelector("del")?.textContent).toBe("old");
    expect(host.querySelector("pre code")?.textContent).toContain("alert(1)");
    expect(host.textContent).toContain("☑");
    expect(host.textContent).toContain("[Image: remote]");
    expect(host.textContent).toContain("[Image: local]");
    expect(host.querySelector("script,svg,iframe,img,style,form,input,object")).toBeNull();
    expect(host.querySelector("[onload],[onerror],[src],[srcset]")).toBeNull();
    expect(host.querySelector("a")?.getAttribute("data-target")).toBe("https://example.org");
    expect(host.querySelector("a")?.getAttribute("href")).toBe("#");
    expect(source).toContain("![remote](https://bad.test/x)");
  });
  it.each(["javascript:alert(1)", "data:text/html,<script/>", "file:///etc/passwd", "//host/x", "https://user:pass@host/x", "a\\b.md", "%00.md", "%252e%252e/x.md"])("rejects %s", value => {
    expect(classifyLink(value)).toBeNull();
  });
  it("delegates relative parent links to the contained backend resolver", () => {
    expect(classifyLink("../second.md")).toEqual({kind:"note", target:"../second.md"});
    expect(classifyLink("other.md#heading")).toEqual({kind:"note", target:"other.md#heading"});
  });
});
