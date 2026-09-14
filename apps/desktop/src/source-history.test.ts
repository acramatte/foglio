// @vitest-environment jsdom
import { describe, it, expect, vi } from "vitest";
import { SourceHistory } from "./source-history";

function setup(value = "original") {
  const source = document.createElement("textarea");source.value = value;
  source.setSelectionRange(value.length, value.length);
  const changed = vi.fn();
  const history = new SourceHistory(source, changed);
  const input = (value: string, type = "insertText") => {
    source.dispatchEvent(new InputEvent("beforeinput", {inputType:type, cancelable:true}));
    source.value = value;source.setSelectionRange(value.length, value.length);
    source.dispatchEvent(new InputEvent("input", {inputType:type}));
  };
  return {source, history, changed, input};
}
describe("SourceHistory", () => {
  it("groups adjacent typing and restores selection on undo/redo", () => {
    const {source, history, changed, input} = setup("abc");
    source.setSelectionRange(0, 3, "backward");
    input("d");input("de");input("def");
    history.apply(false);
    expect(source.value).toBe("abc");
    expect([source.selectionStart, source.selectionEnd, source.selectionDirection]).toEqual([0,3,"backward"]);
    history.apply(true);expect(source.value).toBe("def");expect(source.selectionStart).toBe(3);
    expect(changed).toHaveBeenCalledTimes(2);
  });
  it("separates paste, deletion, and cursor moves from typing", () => {
    const {source, history, input} = setup("a");
    input("ab");input("ab pasted", "insertFromPaste");input("ab paste", "deleteContentBackward");
    history.apply(false);expect(source.value).toBe("ab pasted");
    history.apply(false);expect(source.value).toBe("ab");
    source.setSelectionRange(0,0);input("Xab");
    history.apply(false);expect(source.value).toBe("ab");expect(source.selectionStart).toBe(0);
    history.apply(false);expect(source.value).toBe("a");
  });
  it("ends a typing group when navigation returns to the same caret", () => {
    const {source, history, input} = setup("");
    input("a");
    source.dispatchEvent(new KeyboardEvent("keydown", {key:"ArrowLeft"}));source.setSelectionRange(0,0);
    source.dispatchEvent(new KeyboardEvent("keydown", {key:"ArrowRight"}));source.setSelectionRange(1,1);
    input("ab");history.apply(false);expect(source.value).toBe("a");
  });
  it("ends typing groups after a pause or focus change", () => {
    const {source, history, input} = setup("a");
    const time = vi.spyOn(performance, "now");
    try {
      time.mockReturnValue(0);input("ab");
      time.mockReturnValue(1500);input("abc");
      source.dispatchEvent(new Event("blur"));input("abcd");
      history.apply(false);expect(source.value).toBe("abc");
      history.apply(false);expect(source.value).toBe("ab");
      history.apply(false);expect(source.value).toBe("a");
    } finally { time.mockRestore(); }
  });
  it("discards redo on a new edit and clears both stacks for a new note", () => {
    const {source, history, input} = setup();
    input("edited");history.apply(false);input("branch");history.apply(true);
    expect(source.value).toBe("branch");
    source.value = "other note";history.reset();history.apply(false);history.apply(true);
    expect(source.value).toBe("other note");
  });
  it("routes beforeinput history actions without using document-wide history", () => {
    const {source, input, changed} = setup();input("edited");
    for (const [type, value] of [["historyUndo","original"],["historyRedo","edited"]]) {
      const event = new InputEvent("beforeinput", {inputType:type, cancelable:true});
      source.dispatchEvent(event);expect(event.defaultPrevented).toBe(true);
      expect(source.value).toBe(value);
    }
    expect(changed).toHaveBeenCalledTimes(2);
  });
  it("groups an IME composition and ignores undo until it completes", () => {
    const {source, history, input} = setup("");
    source.dispatchEvent(new CompositionEvent("compositionstart"));
    input("n", "insertCompositionText");input("に", "insertCompositionText");
    history.apply(false);expect(source.value).toBe("に");
    source.dispatchEvent(new CompositionEvent("compositionend"));
    history.apply(false);expect(source.value).toBe("");
    history.apply(true);expect(source.value).toBe("に");
  });
  it("does not undo while read-only or notify for empty history", () => {
    const {source, history, changed, input} = setup();
    history.apply(false);history.apply(true);expect(changed).not.toHaveBeenCalled();
    input("edited");source.readOnly = true;history.apply(false);
    expect(source.value).toBe("edited");expect(changed).not.toHaveBeenCalled();
    source.readOnly = false;history.apply(false);expect(source.value).toBe("original");
  });
});
