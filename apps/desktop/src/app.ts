import {
  type Api,
  type Browse,
  type DesktopState,
  type Note,
  errorText,
} from "./api";
import { classifyLink, renderMarkdown } from "./markdown";
import { Editor } from "./editor";

function element<K extends keyof HTMLElementTagNameMap>(
  tag: K,
  text?: string,
  className?: string,
): HTMLElementTagNameMap[K] {
  const el = document.createElement(tag);
  if (text !== undefined) el.textContent = text;
  if (className) el.className = className;
  return el;
}
export class App {
  private editor: Editor | null = null;
  private busy = false;
  private pendingRefresh = false;
  private mode: "source" | "preview" = "preview";
  private readonly source = element("textarea", undefined, "source");
  private readonly saveStatus = element("span", "", "save-status");
  private readonly saveError = element("div", "", "save-error");
  private readonly tools = element("div", undefined, "editor-tools");
  private readonly retry = element("button", "Retry save");
  private state: DesktopState | null = null;
  private browse: Browse | null = null;
  private note: Note | null = null;
  private selected: string | null = null;
  private tag: string | null = null;
  private folder: string | null = null;
  private epoch = 0;
  private listRequest = 0;
  private noteRequest = 0;
  private selecting = false;
  private polling = false;
  private stopped = false;
  private timer?: ReturnType<typeof setInterval>;
  private readonly status = element("span", "Connecting…", "status");
  private readonly error = element("div", "", "error");
  private readonly root = element("input");
  private readonly select = element("button", "Open library");
  private readonly search = element("input");
  private readonly navigation = element("nav");
  private readonly list = element("div", "", "note-list");
  private readonly preview = element("article", "", "preview");
  private readonly metadata = element("div", "", "metadata");
  private readonly diagnostics = element("details", undefined, "diagnostics");
  private readonly count = element("span", "", "count");
  private readonly onKeyDown = (event: KeyboardEvent): void => {
    if (!(event.ctrlKey || event.metaKey)) return;
    if (event.key.toLowerCase() === "s") {
      event.preventDefault();
      void this.editor?.flush();
    }
    if (event.key.toLowerCase() === "e") {
      event.preventDefault();
      this.setMode(this.mode === "source" ? "preview" : "source");
    }
  };
  constructor(
    private readonly host: HTMLElement,
    private readonly api: Api,
  ) {
    host.innerHTML = "";
    const header = element("header");
    header.append(
      element("strong", "foglio", "wordmark"),
      element("span", "A quiet place for your notes", "strapline"),
    );
    const form = element("form", undefined, "library-form");
    const label = element("label", "Library folder");
    this.root.id = "root-path";
    this.root.dataset.testid = "root-path";
    label.htmlFor = this.root.id;
    this.root.placeholder = "/path/to/your/notes";
    this.root.required = true;
    this.root.autocomplete = "off";
    this.select.type = "submit";
    this.select.dataset.testid = "select-library";
    form.append(
      label,
      this.root,
      this.select,
      element(
        "span",
        "Existing folders only · Your Markdown stays yours",
        "hint",
      ),
    );
    form.addEventListener("submit", (event) => {
      event.preventDefault();
      void this.choose(this.root.value);
    });
    this.error.setAttribute("role", "alert");
    this.error.hidden = true;
    const workspace = element("main", undefined, "workspace");
    const sidebar = element("aside", undefined, "sidebar");
    sidebar.append(element("h2", "Library"), this.navigation);
    const middle = element("section", undefined, "notes");
    middle.setAttribute("aria-label", "Notes");
    this.search.type = "search";
    this.search.placeholder = "Search notes…";
    this.search.setAttribute("aria-label", "Search notes");
    this.search.dataset.testid = "search-input";
    this.search.addEventListener("input", () => {
      void this.loadList();
    });
    this.list.dataset.testid = "note-list";
    this.list.setAttribute("aria-live", "polite");
    const create = element("button", "New note", "new-note");
    create.dataset.testid="new-note";
    create.addEventListener("click",()=>{void this.mutate("create");});
    middle.append(create, this.search, this.count, this.list);
    const reader = element("section", undefined, "reader");
    reader.setAttribute("aria-label", "Note reader");
    this.preview.dataset.testid = "preview";
    this.preview.setAttribute("aria-live", "polite");
    this.preview.addEventListener("click", (event) => {
      void this.follow(event);
    });
    this.preview.addEventListener("auxclick", (event) =>
      event.preventDefault(),
    );
    this.source.dataset.testid="source";
    this.source.setAttribute("aria-label", "Markdown source (body only)");
    this.source.spellcheck=false;
    this.source.addEventListener("input",()=>{this.editor?.edit(this.source.value);});
    for (const mode of ["source", "preview"] as const) {
      const button=element("button",mode === "source" ? "Source" : "Preview");
      button.dataset.testid=mode+"-mode";
      button.addEventListener("click",()=>this.setMode(mode));
      this.tools.append(button);
    }
    for (const [action,label] of [["move","Move / rename"],["tag","Add tag"],["untag","Remove tag"],["delete","Delete note"]] as const) {
      const button=element("button",label);button.dataset.testid=action+"-note";
      button.addEventListener("click",()=>{void this.mutate(action);});this.tools.append(button);
    }
    this.saveStatus.dataset.testid="save-status";
    this.saveStatus.setAttribute("role","status");
    this.saveError.dataset.testid="save-error";
    this.saveError.setAttribute("role","alert");
    this.retry.addEventListener("click",()=>{void this.editor?.retry();});
    reader.append(this.metadata, this.tools, this.saveStatus, this.saveError, this.retry, this.source, this.preview);
    this.host.ownerDocument.addEventListener("keydown", this.onKeyDown);
    this.renderEditor();
    workspace.append(sidebar, middle, reader);
    this.diagnostics.dataset.testid = "diagnostics";
    const footer = element("footer");
    this.status.title =
      "Monitors changes made by external editors and sync tools. Save status is shown separately.";
    footer.append(
      element("span", "Markdown source · Ctrl+E source/preview · Ctrl+S save"),
      this.status,
    );
    host.append(
      header,
      form,
      this.error,
      workspace,
      this.diagnostics,
      footer,
    );
    this.empty(
      "Open your library",
      "Choose an existing notes folder above to begin.",
    );
  }
  async start(): Promise<void> {
    await this.poll();
    if (!this.stopped)
      this.timer = setInterval(() => {
        void this.poll();
      }, 750);
  }
  stop(): void {
    this.stopped = true;
    this.host.ownerDocument.removeEventListener("keydown", this.onKeyDown);
    this.editor?.dispose();
    clearInterval(this.timer);
    this.epoch++;
    this.listRequest++;
    this.noteRequest++;
  }
  private report(error: unknown): void {
    this.error.textContent = errorText(error);
    this.error.hidden = false;
  }
  private empty(title: string, detail: string): void {
    this.preview.replaceChildren(element("h1", title), element("p", detail));
    this.metadata.replaceChildren();
  }
  async choose(path: string): Promise<void> {
    if (this.selecting || this.busy || !path.trim()) return;
    if (this.editor?.pending && !await this.protect()) return;
    this.selecting = true;
    this.select.disabled = true;
    const epoch = ++this.epoch;
    this.listRequest++;
    this.noteRequest++;
    this.browse = null;
    this.editor?.dispose();this.editor=null;this.renderEditor();
    this.note = null;
    this.selected = null;
    this.list.replaceChildren();
    this.error.hidden = true;
    this.status.textContent = "Opening library…";
    this.empty("Opening library…", "Reading your existing Markdown files.");
    try {
      const state = await this.api.select(path);
      if (!this.stopped && epoch === this.epoch) await this.apply(state, true);
    } catch (error) {
      if (epoch === this.epoch) {
        this.report(error);
        this.state = null;
        this.empty(
          "Could not open library",
          "Check the path and permissions, then try again.",
        );
      }
    } finally {
      this.selecting = false;
      this.select.disabled = false;
    }
  }
  async poll(): Promise<void> {
    if (this.polling || this.selecting || this.busy || this.stopped) return;
    this.polling = true;
    const epoch = this.epoch;
    try {
      const state = await this.api.state();
      if (epoch === this.epoch && !this.selecting && !this.busy && !this.stopped)
        await this.apply(state);
    } catch (error) {
      if (epoch === this.epoch) {
        this.report(error);
        this.status.textContent = "Connection unavailable";
      }
    } finally {
      this.polling = false;
    }
  }
  private async apply(state: DesktopState, force = false): Promise<void> {
    const changed =
      force ||
      state.session !== this.state?.session ||
      state.generation !== this.state?.generation;
    const switched = force || state.session !== this.state?.session;
    this.state = state;
    this.status.textContent = state.root
      ? state.watcher_active
        ? "Monitoring external changes"
        : "Watcher inactive"
      : "No library selected";
    if (state.error) this.report(state.error);
    else if (state.root && !state.watcher_active)
      this.report(
        "External monitoring is inactive. Changes may not appear until the library is reopened.",
      );
    // Monitoring refresh must not erase an unrelated operation failure.
    if (!changed) {
      if (this.pendingRefresh && this.selected && !this.editor?.pending)
        await this.open(this.selected, true);
      return;
    }
    const epoch = ++this.epoch;
    this.listRequest++;
    this.noteRequest++;
    this.browse = null;
    if (switched) {
      this.editor?.dispose();this.editor=null;this.renderEditor();
      this.selected = null;
      this.note = null;
      this.tag = null;
      this.folder = null;
      this.search.value = "";
      this.root.value = state.root ?? "";
      this.navigation.replaceChildren();
      this.diagnostics.replaceChildren();
    }
    this.list.replaceChildren(element("p", "Loading notes…", "empty-list"));
    if (!this.editor) this.metadata.replaceChildren();
    if (!state.root) {
      this.list.replaceChildren();
      this.empty(
        "Open your library",
        "Choose an existing notes folder above to begin.",
      );
      return;
    }
    if (this.selected && !this.editor)
      this.empty(
        "Refreshing note…",
        "Checking the current file at its selected path.",
      );
    else if (!this.selected)
      this.empty(
        "Your notes, at a glance",
        "Select a note to read. Browse by folder or tag, or search across your library.",
      );
    const refreshNote = this.selected
      ? this.open(this.selected, true)
      : Promise.resolve();
    try {
      const browse = await this.api.browse(state.session);
      if (!this.valid(epoch, browse.session)) return;
      if (browse.generation !== state.generation) {
        this.state = { ...state, generation: -1 };
        return;
      }
      this.browse = browse;
      this.renderNavigation();
      this.renderDiagnostics();
      await this.loadList();
    } catch (error) {
      if (this.valid(epoch, state.session)) {
        this.report(error);
        this.list.replaceChildren(
          element(
            "p",
            "Notes unavailable. Waiting for the next refresh.",
            "empty-list",
          ),
        );
        this.state = { ...state, generation: -1 };
      }
    }
    await refreshNote;
  }
  private valid(epoch: number, session: number): boolean {
    return (
      !this.stopped && epoch === this.epoch && session === this.state?.session
    );
  }
  private renderNavigation(): void {
    if (!this.browse) return;
    const nav = this.navigation;
    nav.replaceChildren();
    const button = (text: string, active: boolean, action: () => void) => {
      const b = element("button", text, active ? "filter active" : "filter");
      b.type = "button";
      b.setAttribute("aria-pressed", String(active));
      b.addEventListener("click", action);
      return b;
    };
    nav.append(
      button("All notes", !this.folder && !this.tag, () => {
        this.folder = null;
        this.tag = null;
        this.renderNavigation();
        void this.loadList();
      }),
      element("h3", "Folders"),
    );
    for (const path of this.browse!.folders) {
      const b = button(path || "/", this.folder === path, () => {
        this.folder = this.folder === path ? null : path;
        this.renderNavigation();
        void this.loadList();
      });
      b.style.paddingLeft = `${12 + Math.min(path.split("/").length - 1, 5) * 12}px`;
      nav.append(b);
    }
    nav.append(element("h3", "Tags"));
    const tags = [...new Set(this.browse!.notes.flatMap((n) => n.tags))].sort();
    for (const tag of tags)
      nav.append(
        button("# " + tag, this.tag === tag, () => {
          this.tag = this.tag === tag ? null : tag;
          this.renderNavigation();
          void this.loadList();
        }),
      );
    if (!tags.length) nav.append(element("p", "No tags yet", "hint"));
  }
  private renderDiagnostics(): void {
    const browse = this.browse!;
    this.diagnostics.replaceChildren(
      element(
        "summary",
        `${browse.diagnostics.length} diagnostics${browse.incomplete ? " · Library results are incomplete" : " · Library scan complete"}`,
      ),
    );
    for (const diagnostic of browse.diagnostics)
      this.diagnostics.append(
        element(
          "p",
          `${diagnostic.path} — ${diagnostic.code}: ${diagnostic.message}`,
        ),
      );
    if (browse.incomplete)
      this.diagnostics.append(
        element(
          "p",
          "Some files could not be included. Resolve the reported problems outside Foglio.",
        ),
      );
  }
  async loadList(): Promise<void> {
    const browse = this.browse;
    if (!browse || !this.state) return;
    const request = ++this.listRequest,
      epoch = this.epoch,
      session = this.state.session;
    const query = this.search.value.trim();
    this.count.textContent = "Loading…";
    this.list.replaceChildren(element("p", "Loading notes…", "empty-list"));
    try {
      let rows: {
        path: string;
        title: string;
        snippet?: string;
      }[];
      let incomplete = browse.incomplete;
      if (query) {
        const result = await this.api.search(
          session,
          query,
          this.tag,
          this.folder,
        );
        if (!this.valid(epoch, result.session) || request !== this.listRequest)
          return;
        rows = result.hits;
        incomplete ||= result.incomplete;
      } else
        rows = browse.notes.filter(
          (n) =>
            (!this.tag || n.tags.includes(this.tag)) &&
            (this.folder === null ||
              this.folder === "" ||
              n.path.startsWith(this.folder + "/")),
        );
      if (!this.valid(epoch, session) || request !== this.listRequest) return;
      this.count.textContent = `${rows.length} ${rows.length === 1 ? "note" : "notes"}${incomplete ? " · incomplete" : ""}`;
      this.list.replaceChildren();
      for (const row of rows) {
        const b = element("button", undefined, "note-card");
        b.type = "button";
        b.dataset.notePath = row.path;
        b.setAttribute("aria-pressed", String(row.path === this.selected));
        b.append(
          element("strong", row.title || row.path),
          element("span", row.path, "note-path"),
        );
        if (row.snippet) b.append(element("span", row.snippet, "snippet"));
        b.addEventListener("click", () => {
          void this.open(row.path);
        });
        this.list.append(b);
      }
      if (!rows.length)
        this.list.append(
          element(
            "p",
            query || this.tag || this.folder
              ? "No matching notes. Try another search or clear your filters."
              : "No notes yet. Create a note above, or check diagnostics.",
            "empty-list",
          ),
        );
    } catch (error) {
      if (this.valid(epoch, session) && request === this.listRequest) {
        this.report(error);
        this.count.textContent = "Search unavailable";
        this.list.replaceChildren(
          element(
            "p",
            "Could not load results. Change the search to retry.",
            "empty-list",
          ),
        );
      }
    }
  }
  async open(path: string, refresh = false): Promise<void> {
    if (refresh) this.pendingRefresh = true;
    if (!this.state?.root || this.busy || this.selecting) return;
    if (refresh && this.editor?.pending) return;
    if (!refresh && this.editor?.pending && !await this.protect()) return;
    await this.readNote(path, refresh);
  }
  // Only the mutation owner calls this while busy; public navigation cannot bypass it.
  private async readNote(path: string, refresh = false): Promise<void> {
    if (!this.state?.root) return;
    const refreshRevision = this.editor?.revision;
    const request = ++this.noteRequest,
      epoch = this.epoch,
      session = this.state.session;
    this.selected = path;
    if (!refresh) {
      this.editor?.dispose();this.editor=null;this.note=null;this.renderEditor();
      this.empty("Loading note…", "Reading the current Markdown from disk.");
    }
    for (const button of this.list.querySelectorAll<HTMLButtonElement>(
      "[data-note-path]",
    ))
      button.setAttribute("aria-pressed", String(button.dataset.notePath === path));
    try {
      const note = await this.api.open(session, path);
      if (
        !this.valid(epoch, note.session) ||
        request !== this.noteRequest ||
        note.path !== this.selected ||
        (refresh && (!!this.editor?.pending || refreshRevision !== this.editor?.revision || this.busy))
      )
        return;
      this.pendingRefresh = false;
      if (refresh && this.editor?.revision === note.revision) return;
      this.editor?.dispose();
      this.note = note;
      this.editor = new Editor(note, this.api.save, ()=>this.renderEditor());
      this.source.value=note.body;
      this.renderEditor();
      this.metadata.replaceChildren(
        element("span", note.path),
        element("span", note.tags.map((t) => "#" + t).join(" ")),
        element("small", "Body editing preserves frontmatter. Tags are managed separately."),
      );
      this.preview.innerHTML = renderMarkdown(note.body);
      if (!note.body.trim())
        this.preview.append(element("p", "This note is empty."));
      this.preview.scrollTop = 0;
    } catch (error) {
      if (this.valid(epoch, session) && request === this.noteRequest && !this.editor?.pending && (!refresh || (!this.busy && refreshRevision === this.editor?.revision))) {
        this.pendingRefresh = false;
        this.editor?.dispose();this.editor=null;this.note=null;this.renderEditor();
        this.empty("Note unavailable", errorText(error));
      }
    }
  }
  private setMode(mode: "source" | "preview"): void {
    this.mode=mode;this.renderEditor();
    if (mode === "source" && this.editor) this.source.focus();
  }
  private renderEditor(): void {
    const editor=this.editor;
    this.tools.hidden=!editor;this.source.hidden=!editor || this.mode!=="source";
    this.preview.hidden=!!editor && this.mode!=="preview";
    this.saveStatus.hidden=!editor;this.saveError.hidden=!editor?.message && !editor?.warning;
    this.retry.hidden=editor?.status!=="save_error";
    this.source.readOnly=this.busy;
    for (const b of this.tools.querySelectorAll<HTMLButtonElement>("button")) {
      b.disabled=this.busy || (b.dataset.testid==="untag-note" && !this.note?.tags.length);
      if (b.dataset.testid?.endsWith("-mode")) b.setAttribute("aria-pressed", String(b.dataset.testid===this.mode+"-mode"));
    }
    if (!editor) {this.saveError.textContent="";return;}
    const labels={clean:"Saved",dirty:"Unsaved changes",saving:"Saving…",save_error:"Save failed · buffer retained",conflict:"Conflict · autosave paused",missing_on_disk:"Missing on disk · autosave paused"};
    this.saveStatus.textContent=labels[editor.status];this.saveStatus.dataset.state=editor.status;
    this.saveError.textContent=[editor.message,editor.warning].filter(Boolean).join("\n");
    this.preview.innerHTML=renderMarkdown(editor.body);
  }
  get hasUnsavedChanges(): boolean {return !!this.editor?.pending || this.busy;}
  private async protect(): Promise<boolean> {
    this.busy=true;this.renderEditor();
    try {
      if (await this.editor?.flush() !== false) return true;
      await this.dialog("Unsaved changes", "Your buffer is retained. Navigation was cancelled. Resolve the save error or copy your source before leaving.");
      return false;
    } finally {this.busy=false;this.renderEditor();}
  }
  async requestClose(): Promise<void> {
    if (this.busy || this.selecting) return;
    if (!await this.protect()) return;
    this.busy=true;this.renderEditor();
    try {await this.api.close();} catch(error) {this.report(error);this.busy=false;this.renderEditor();}
  }
  private dialog(title: string, detail: string, field?: {label: string; value: string; options?: string[]}, destructive = false): Promise<string | null> {
    return new Promise(resolve=>{
      const dialog=element("dialog");dialog.dataset.testid="operation-dialog";
      const form=element("form");form.method="dialog";
      const heading=element("h2",title);heading.id="dialog-heading";dialog.setAttribute("aria-labelledby",heading.id);
      form.append(heading,element("p",detail));
      const control=field?.options ? element("select") : element("input");
      if (field) {
        const label=element("label",field.label);control.id="operation-value";control.dataset.testid="operation-value";control.required=true;label.htmlFor=control.id;
        if (control instanceof HTMLSelectElement) for (const value of field.options!) {
          const option=element("option");option.value=value;option.textContent=value;control.append(option);
        }
        control.value=field.value;form.append(label,control);
      }
      const cancel=element("button",field || destructive ? "Cancel" : "Stay here");cancel.type="button";cancel.dataset.testid="dialog-cancel";
      const finish=(value:string|null)=>{dialog.close();dialog.remove();resolve(value);};
      cancel.addEventListener("click",()=>finish(null));form.append(cancel);
      if (field || destructive) {const submit=element("button",destructive ? "Permanently delete" : "Apply");submit.type="submit";submit.dataset.testid="dialog-submit";form.append(submit);}
      form.addEventListener("submit",event=>{event.preventDefault();finish(field?control.value:"");});
      dialog.addEventListener("cancel",event=>{event.preventDefault();finish(null);});
      dialog.append(form);this.host.append(dialog);dialog.showModal();
      if (field) {control.focus();if (control instanceof HTMLInputElement) control.select();} else cancel.focus();
    });
  }
  private async mutate(action: "create" | "move" | "tag" | "untag" | "delete"): Promise<void> {
    if (this.busy || this.selecting || !this.state?.root || (action!=="create" && !this.editor) || (action==="untag" && !this.note?.tags.length)) return;
    this.busy=true;this.renderEditor();this.error.hidden=true;
    try {
      if (await this.editor?.flush() === false) {
        await this.dialog("Unsaved changes", "Operation cancelled. Your source is retained; resolve the save error first.");return;
      }
      const editor=this.editor, session=this.state.session;
      const value=action==="delete"
        ? await this.dialog("Permanently delete note?",`Delete ${editor!.path} from disk? There is no undo.`,undefined,true)
        : await this.dialog(action==="create"?"New note":action==="move"?"Move / rename note":action==="tag"?"Add tag":"Remove tag",
          action==="create" ? "Spaces in the title become hyphens in the Markdown filename. Existing files will never be overwritten."
            : action==="move" ? "Use a complete library-relative .md path. Existing files will never be overwritten."
            : action==="untag" ? "Choose a tag currently attached to this note. Other frontmatter and the body stay unchanged."
            : "Tags are case-sensitive. Other frontmatter and the body stay unchanged.",
          action==="untag"
            ? {label:"Tag",value:this.note!.tags[0]!,options:this.note!.tags}
            : {label:action==="create" ? "Note title" : action==="move" ? "Note path" : "Tag",value:action==="move"?editor!.path:""});
      if (value===null) return;
      const result=action==="create" ? await this.api.create(session,value,"",[])
        : action==="move" ? await this.api.move(session,editor!.path,editor!.revision,value)
        : action==="delete" ? await this.api.delete(session,editor!.path,editor!.revision)
        : await this.api.tag(session,editor!.path,editor!.revision,value,action==="tag");
      if (result.session!==session || !result.file_committed) throw new Error("Invalid mutation acknowledgement");
      this.editor?.dispose();this.editor=null;this.note=null;this.selected=null;this.noteRequest++;
      this.renderEditor();
      if (action==="delete") this.empty("Note deleted", "The selected file was permanently removed.");
      else {await this.readNote(result.path);if (action==="create") this.setMode("source");}
      if (result.warnings.length) this.report("File committed. "+result.warnings.join("\n"));
      if (this.state) this.state={...this.state,generation:-1};
    } catch(error) {this.report(error);} finally {this.busy=false;this.renderEditor();}
    void this.poll();
  }
  private async follow(event: MouseEvent): Promise<void> {
    const anchor = (event.target as Element).closest<HTMLAnchorElement>("a");
    if (!anchor) return;
    event.preventDefault();
    const link = classifyLink(anchor.dataset.target ?? "");
    if (!link) return;
    const epoch = this.epoch,
      request = this.noteRequest,
      note = this.note;
    if (!note) return;
    try {
      if (link.kind === "external") await this.api.external(link.target);
      else {
        const resolved = await this.api.resolve(
          note.session,
          note.path,
          link.target,
        );
        if (this.valid(epoch, resolved.session) && request === this.noteRequest)
          await this.open(resolved.path);
      }
    } catch (error) {
      if (this.valid(epoch, note.session) && request === this.noteRequest)
        this.report(error);
    }
  }
}
