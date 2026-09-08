import { type Api, type Note, errorText } from "./api";
export type SaveState = "clean" | "dirty" | "saving" | "save_error" | "conflict" | "missing_on_disk";
const normalized = (text: string) => text.replace(/\r\n?/g, "\n");
// A textarea normalizes line endings. Retain the raw unchanged prefix/suffix,
// using the document's first newline for the edited range, never the preview DOM.
export function preserveNewlines(original: string, input: string, preferredNewline?: string): string {
  const old = normalized(original), next = normalized(input);
  if (old === next) return original;
  let start = 0, end = 0;
  while (start < old.length && start < next.length && old[start] === next[start]) start++;
  while (end < old.length - start && end < next.length - start && old[old.length-1-end] === next[next.length-1-end]) end++;
  const rawOffset = (offset: number) => {
    let raw = 0;
    for (let n = 0; n < offset; n++, raw++) if (original[raw] === "\r" && original[raw+1] === "\n") raw++;
    return raw;
  };
  const newline = preferredNewline ?? original.match(/\r\n|\r|\n/)?.[0] ?? "\n";
  return original.slice(0, rawOffset(start)) + next.slice(start, next.length-end).replace(/\n/g, newline) + original.slice(rawOffset(old.length-end));
}
function code(error: unknown): string {
  if (typeof error === "string") { try { return code(JSON.parse(error)); } catch { return ""; } }
  return error && typeof error === "object" && "code" in error ? String(error.code) : "";
}
export class Editor {
  readonly session: number;
  readonly path: string;
  body: string;
  revision: string;
  status: SaveState = "clean";
  message = "";
  warning = "";
  private savedBody: string;
  private readonly newline: string;
  private generation = 0;
  private timer?: ReturnType<typeof setTimeout>;
  private flight: Promise<void> | null = null;
  private disposed = false;
  constructor(note: Note, private readonly save: Api["save"], private readonly changed: () => void) {
    this.session=note.session;this.path=note.path;this.body=note.body;this.savedBody=note.body;this.revision=note.revision;
    this.newline=note.body.match(/\r\n|\r|\n/)?.[0] ?? "\n";
  }
  get pending(): boolean { return this.status !== "clean"; }
  edit(value: string): void {
    this.body=normalized(value)===normalized(this.savedBody) ? this.savedBody : preserveNewlines(this.body,value,this.newline);this.generation++;
    if (this.status === "clean" || this.status === "dirty") this.status=this.body===this.savedBody ? "clean" : "dirty";
    this.schedule();this.changed();
  }
  private schedule(): void {
    clearTimeout(this.timer);
    if (!this.disposed && this.status === "dirty") this.timer=setTimeout(()=>{void this.dispatch();},400);
  }
  private async dispatch(): Promise<void> {
    if (this.flight) return this.flight;
    if (this.disposed || this.status !== "dirty") return;
    clearTimeout(this.timer);
    const body=this.body, revision=this.revision, generation=this.generation;
    this.status="saving";this.message="";this.changed();
    this.flight=(async()=>{
      try {
        const result=await this.save(this.session,this.path,revision,body);
        if (result.session!==this.session || result.path!==this.path || !result.file_committed || !result.revision) throw new Error("Invalid save acknowledgement; buffer retained.");
        this.revision=result.revision;this.savedBody=body;this.warning=result.warnings.join("\n");
        this.status=generation===this.generation || this.body===body ? "clean" : "dirty";
      } catch(error) {
        const kind=code(error);
        this.status=kind==="conflict" ? "conflict" : kind==="not_found" ? "missing_on_disk" : "save_error";
        this.message=errorText(error);
      }
    })();
    await this.flight;this.flight=null;this.schedule();this.changed();
  }
  async flush(): Promise<boolean> {
    clearTimeout(this.timer);
    while (!this.disposed && (this.flight || this.status === "dirty")) {
      if (this.flight) await this.flight; else await this.dispatch();
      // The dispatch continuation owns acknowledgement handling and timer cleanup.
      clearTimeout(this.timer);
    }
    return this.status === "clean";
  }
  async retry(): Promise<boolean> {
    if (this.status !== "save_error") return false;
    this.status="dirty";return this.flush();
  }
  dispose(): void { this.disposed=true;clearTimeout(this.timer); }
}
