import { type Api, type Note, errorText, errorCode } from "./api";
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
  private inspection: Promise<Note> | null = null;
  private verificationPending = false;
  constructor(note: Note, private readonly save: Api["save"], private readonly changed: () => void, private readonly read?: () => Promise<Note>) {
    this.session=note.session;this.path=note.path;this.body=note.body;this.savedBody=note.body;this.revision=note.revision;
    this.newline=note.body.match(/\r\n|\r|\n/)?.[0] ?? "\n";
  }
  get pending(): boolean { return this.status !== "clean"; }
  // Pause queued saves until a fresh read after the current acknowledgement.
  // A watcher self-event then compares against the committed revision, not its base.
  inspect(read: () => Promise<Note>): Promise<Note> {
    if (this.inspection) return this.inspection;
    clearTimeout(this.timer);
    this.inspection=(async()=>{
      // Publish the inspection before even a synchronously throwing reader runs.
      await Promise.resolve();
      if (this.flight) await this.flight;
      try {
        const note=await read();
        if (note.session!==this.session || note.path!==this.path) throw new Error("Invalid disk snapshot; buffer retained.");
        this.observe(note);
        return note;
      } catch(error) {
        if (this.pending && !this.disposed) this.unavailable(error);
        throw error;
      } finally {
        this.inspection=null;this.schedule();
      }
    })();
    return this.inspection;
  }
  // Observations must be fetched after an in-flight acknowledgement, not before it.
  observe(note: Note | null): void {
    if (this.disposed || this.status === "saving") return;
    if (note && note.revision === this.revision && this.status !== "missing_on_disk") return;
    if (note && this.status === "clean") return; // App installs the new clean snapshot.
    clearTimeout(this.timer);
    this.status=note ? "conflict" : "missing_on_disk";
    this.message=note ? "The file changed outside Foglio. Your local source is retained. Choose reload/discard or save a copy."
      : "The selected path is missing. It will not be recreated automatically. Save a copy or explicitly discard your local source.";
    this.changed();
  }
  unavailable(error: unknown): void {
    clearTimeout(this.timer);
    this.status=errorCode(error)==="not_found" ? "missing_on_disk" : errorCode(error)==="conflict" ? "conflict" : "save_error";
    this.message=errorText(error);this.changed();
  }
  edit(value: string): void {
    this.body=normalized(value)===normalized(this.savedBody) ? this.savedBody : preserveNewlines(this.body,value,this.newline);this.generation++;
    if (this.status === "clean" || this.status === "dirty") this.status=this.body===this.savedBody ? "clean" : "dirty";
    this.schedule();this.changed();
  }
  private schedule(): void {
    clearTimeout(this.timer);
    if (!this.disposed && !this.inspection && this.status === "dirty") this.timer=setTimeout(()=>{void this.dispatch();},400);
  }
  private async dispatch(): Promise<void> {
    if (this.flight) return this.flight;
    if (this.disposed || this.inspection || this.status !== "dirty") return;
    clearTimeout(this.timer);
    const body=this.body, revision=this.revision, generation=this.generation;
    this.status="saving";this.message="";this.changed();
    this.flight=(async()=>{
      try {
        const result=await this.save(this.session,this.path,revision,body);
        if (result.session!==this.session || result.path!==this.path || !result.file_committed || !result.revision) throw new Error("Invalid save acknowledgement; buffer retained.");
        this.revision=result.revision;this.savedBody=body;this.warning=result.warnings.join("\n");
        // An external writer can replace a committed file before its delayed IPC
        // acknowledgement arrives. Keep the buffer and navigation lock until readback.
        if (this.read) {
          this.verificationPending=true;
          const current=await this.read();
          if (current.session!==this.session || current.path!==this.path) throw new Error("Invalid save verification; buffer retained.");
          if (current.revision!==result.revision) throw {code:"conflict",message:"The file changed before save verification. Your local source is retained."};
          this.verificationPending=false;
        }
        this.status=generation===this.generation || this.body===body ? "clean" : "dirty";
      } catch(error) {
        const kind=errorCode(error);
        this.status=kind==="conflict" ? "conflict" : kind==="not_found" ? "missing_on_disk" : "save_error";
        this.message=errorText(error);
      }
    })();
    await this.flight;this.flight=null;this.schedule();this.changed();
  }
  async flush(): Promise<boolean> {
    clearTimeout(this.timer);
    while (!this.disposed && (this.inspection || this.flight || this.status === "dirty")) {
      if (this.inspection) await this.inspection.catch(()=>{});
      else if (this.flight) await this.flight; else await this.dispatch();
      // The dispatch continuation owns acknowledgement handling and timer cleanup.
      clearTimeout(this.timer);
    }
    return this.status === "clean";
  }
  async retry(): Promise<boolean> {
    if (this.status !== "save_error") return false;
    if (this.verificationPending && this.read) {
      // The write already committed: retry verification, never replay that mutation.
      try {
        const current=await this.inspect(this.read);
        if (current.revision!==this.revision || this.status!=="save_error") return false;
        this.verificationPending=false;this.message="";
        this.status=this.body===this.savedBody ? "clean" : "dirty";this.changed();
      } catch {return false;}
    } else this.status="dirty";
    return this.flush();
  }
  dispose(): void { this.disposed=true;clearTimeout(this.timer); }
}
