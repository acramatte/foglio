type Selection = {
  value: string;
  body: string;
  start: number;
  end: number;
  direction: "forward" | "backward" | "none";
};
type Change = { before: Selection; after: Selection };
const snapshot = (source: HTMLTextAreaElement, body: string): Selection => ({
  body, value: source.value, start: source.selectionStart, end: source.selectionEnd,
  direction: source.selectionDirection,
});

// WebKit's document-wide undo history can reach another input. Keep source
// history local to the installed note; saves and preview toggles do not reset it.
export class SourceHistory {
  private undoStack: Change[] = [];
  private redoStack: Change[] = [];
  private before: Selection;
  private inputType = "";
  private lastType = "";
  private lastTime = 0;
  private composing = false;
  private composition: Selection | null = null;

  constructor(
    private readonly source: HTMLTextAreaElement,
    private readonly changed: (body: string) => void,
    private readonly rawBody: () => string = () => source.value,
  ) {
    this.before = snapshot(source, this.rawBody());
    source.addEventListener("beforeinput", event => {
      if (event.inputType === "historyUndo" || event.inputType === "historyRedo") {
        event.preventDefault();
        if (!source.readOnly) this.apply(event.inputType === "historyRedo");
        return;
      }
      this.before = snapshot(source, this.rawBody());
      this.inputType = event.inputType;
    });
    source.addEventListener("input", () => {
      if (this.composing) return;
      this.record(this.before, snapshot(source, this.rawBody()), this.inputType);
    });
    source.addEventListener("compositionstart", () => {
      this.composing = true;
      this.composition = snapshot(source, this.rawBody());
      this.breakGroup();
    });
    source.addEventListener("compositionend", () => {
      this.composing = false;
      if (this.composition) this.record(this.composition, snapshot(source, this.rawBody()), "composition");
      this.composition = null;
      this.breakGroup();
    });
    source.addEventListener("blur", () => this.breakGroup());
    source.addEventListener("pointerdown", () => this.breakGroup());
    source.addEventListener("keydown", event => {
      if (["ArrowLeft", "ArrowRight", "ArrowUp", "ArrowDown", "Home", "End", "PageUp", "PageDown"].includes(event.key)
        || ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "a")) this.breakGroup();
    });
  }

  reset(): void {
    this.undoStack = [];this.redoStack = [];
    this.before = snapshot(this.source, this.rawBody());
    this.composition = null;this.composing = false;
    this.breakGroup();
  }

  private breakGroup(): void { this.lastType = ""; }

  private record(before: Selection, after: Selection, type: string): void {
    if (before.value === after.value) return;
    const previous = this.undoStack.at(-1), now = performance.now();
    const groupable = ["insertText", "deleteContentBackward", "deleteContentForward"].includes(type);
    if (previous && groupable && type === this.lastType && now - this.lastTime < 1000
      && before.start === before.end && previous.after.start === before.start
      && previous.after.end === before.end && previous.after.value === before.value) {
      previous.after = after;
    } else {
      this.undoStack.push({before, after});
      // Bound session memory; each entry is a complete source snapshot.
      if (this.undoStack.length > 100) this.undoStack.shift();
    }
    this.redoStack = [];this.lastType = type;this.lastTime = now;
    this.before = after;
  }

  apply(redo: boolean): void {
    if (this.source.readOnly || this.composing) return;
    const change = (redo ? this.redoStack : this.undoStack).pop();
    if (!change) return;
    (redo ? this.undoStack : this.redoStack).push(change);
    const target = redo ? change.after : change.before;
    this.source.value = target.value;
    this.source.setSelectionRange(target.start, target.end, target.direction);
    this.changed(target.body);
    this.before = snapshot(this.source, this.rawBody());this.breakGroup();
  }

  // Programmatic replacements (format bar, link wrap) skip the input event
  // path; record them as one history step after the editor has committed.
  replace(value: string, start: number, end: number, commit: () => void): void {
    if (this.source.readOnly || this.composing) return;
    const before = snapshot(this.source, this.rawBody());
    this.source.value = value;
    this.source.setSelectionRange(start, end);
    commit();
    this.record(before, snapshot(this.source, this.rawBody()), "insertReplacementText");
  }
}
