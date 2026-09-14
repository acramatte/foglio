import { describe, expect, it } from "vitest";
import { applyFormat } from "./format";

describe("applyFormat", () => {
  it("wraps a selection in bold markers and unwraps the same range", () => {
    expect(applyFormat("say hello", 4, 9, "bold")).toEqual({
      text: "say **hello**",
      start: 4,
      end: 13,
    });
    expect(applyFormat("say **hello**", 4, 13, "bold")).toEqual({
      text: "say hello",
      start: 4,
      end: 9,
    });
  });

  it("unwraps bold when the markers sit just outside the selection", () => {
    expect(applyFormat("say **hello**", 6, 11, "bold")).toEqual({
      text: "say hello",
      start: 4,
      end: 9,
    });
  });

  it("inserts a selected bold placeholder when the caret is empty", () => {
    expect(applyFormat("say ", 4, 4, "bold")).toEqual({
      text: "say **bold**",
      start: 6,
      end: 10,
    });
  });

  it("wraps italic with single asterisks without stealing a bold pair", () => {
    expect(applyFormat("hello", 0, 5, "italic")).toEqual({
      text: "*hello*",
      start: 0,
      end: 7,
    });
    expect(applyFormat("**hello**", 0, 9, "italic")).toEqual({
      text: "***hello***",
      start: 0,
      end: 11,
    });
    expect(applyFormat("***hello***", 0, 11, "italic")).toEqual({
      text: "**hello**",
      start: 0,
      end: 9,
    });
    expect(applyFormat("**hello**", 2, 7, "italic")).toEqual({
      text: "***hello***",
      start: 2,
      end: 9,
    });
  });

  it("toggles inline code and strikethrough", () => {
    expect(applyFormat("x", 0, 1, "code")).toEqual({ text: "`x`", start: 0, end: 3 });
    expect(applyFormat("`x`", 0, 3, "code")).toEqual({ text: "x", start: 0, end: 1 });
    expect(applyFormat("x", 0, 1, "strike")).toEqual({ text: "~~x~~", start: 0, end: 5 });
    expect(applyFormat("~~x~~", 0, 5, "strike")).toEqual({ text: "x", start: 0, end: 1 });
  });

  it("prefixes selected lines as a bullet list and strips an existing list marker", () => {
    expect(applyFormat("one\ntwo", 0, 7, "bullet")).toEqual({
      text: "- one\n- two",
      start: 0,
      end: 11,
    });
    expect(applyFormat("- one\n- two", 0, 11, "bullet")).toEqual({
      text: "one\ntwo",
      start: 0,
      end: 7,
    });
  });

  it("expands to the current line when the caret is empty", () => {
    expect(applyFormat("alpha\nbravo\n", 8, 8, "quote")).toEqual({
      text: "alpha\n> bravo\n",
      start: 6,
      end: 13,
    });
  });

  it("toggles quotes, ordered lists, headings and fenced code", () => {
    expect(applyFormat("note", 0, 4, "quote")).toEqual({
      text: "> note",
      start: 0,
      end: 6,
    });
    expect(applyFormat("> note", 0, 6, "quote")).toEqual({
      text: "note",
      start: 0,
      end: 4,
    });
    expect(applyFormat("one\ntwo", 0, 7, "ordered")).toEqual({
      text: "1. one\n1. two",
      start: 0,
      end: 13,
    });
    expect(applyFormat("title", 0, 5, "heading")).toEqual({
      text: "# title",
      start: 0,
      end: 7,
    });
    expect(applyFormat("# title", 0, 7, "heading")).toEqual({
      text: "title",
      start: 0,
      end: 5,
    });
    expect(applyFormat("code", 0, 4, "fence")).toEqual({
      text: "```\ncode\n```",
      start: 4,
      end: 8,
    });
    expect(applyFormat("```\ncode\n```", 0, 12, "fence")).toEqual({
      text: "code",
      start: 0,
      end: 4,
    });
  });
});
