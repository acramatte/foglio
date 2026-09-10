import {
  type Api,
  type Browse,
  type DesktopState,
  type Note,
  errorText,
  errorCode,
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
type DialogField = {
  label: string;
  value: string;
  options?: string[];
  placeholder?: string;
  required?: boolean;
  testid?: string;
};
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
  private readonly resolution = element("div", undefined, "conflict-tools");
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
  private readonly previewScroll = element("div", "", "preview-scroll");
  private readonly metadata = element("div", "", "metadata");
  private readonly diagnostics = element("details", undefined, "diagnostics");
  private readonly count = element("span", "", "count");
  private readonly onKeyDown = (event: KeyboardEvent): void => {
    if (event.defaultPrevented || event.isComposing || event.repeat || event.altKey) return;
    // Native modal focus/Enter/Escape behavior owns input while a choice is open.
    if (this.host.querySelector("dialog[open]")) return;
    if (!(event.ctrlKey || event.metaKey)) return;
    if (event.key.toLowerCase() === "f") {
      event.preventDefault();this.search.focus();this.search.select();return;
    }
    if (this.busy || this.selecting || event.shiftKey) return;
    if (event.key.toLowerCase() === "s") {
      event.preventDefault();
      if (!this.busy) void this.editor?.flush();
    }
    if (event.key.toLowerCase() === "e") {
      event.preventDefault();
      this.setMode(this.mode === "source" ? "preview" : "source");
    }
    if (event.key.toLowerCase() === "n") {
      event.preventDefault();
      void this.mutate("create");
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
    this.count.setAttribute("role", "status");
    this.navigation.setAttribute("aria-label", "Library filters");
    this.search.setAttribute("aria-keyshortcuts", "Control+f Meta+f");
    this.search.addEventListener("keydown", event => {
      if (event.key === "ArrowDown" && !event.ctrlKey && !event.metaKey && !event.altKey) {
        event.preventDefault();this.list.querySelector<HTMLButtonElement>(".note-card")?.focus();
      }
    });
    this.list.addEventListener("keydown", event => {
      if (event.ctrlKey || event.metaKey || event.altKey || event.isComposing) return;
      const cards=[...this.list.querySelectorAll<HTMLButtonElement>(".note-card")];
      const index=cards.indexOf(this.host.ownerDocument.activeElement as HTMLButtonElement);
      if (index < 0) return;
      if (event.key === "Escape") {event.preventDefault();this.search.focus();}
      else if (["ArrowDown","ArrowUp","Home","End"].includes(event.key)) {
        event.preventDefault();
        const next=event.key==="Home" ? 0 : event.key==="End" ? cards.length-1 : Math.max(0,Math.min(cards.length-1,index+(event.key==="ArrowDown" ? 1 : -1)));
        cards[next]?.focus();
      }
    });
    const create = element("button", "New note", "new-note");
    create.dataset.testid="new-note";
    create.setAttribute("aria-keyshortcuts", "Control+n Meta+n");
    create.addEventListener("click",()=>{void this.mutate("create");});
    const listScroll = element("div", "", "note-list-scroll");
    listScroll.append(this.list);
    middle.append(create, this.search, this.count, listScroll);
    const reader = element("section", undefined, "reader");
    reader.setAttribute("aria-label", "Note reader");
    this.preview.dataset.testid = "preview";
    this.previewScroll.tabIndex=0;
    this.previewScroll.setAttribute("role", "region");
    this.previewScroll.setAttribute("aria-label", "Note preview");
    this.preview.addEventListener("click", (event) => {
      void this.follow(event);
    });
    this.preview.addEventListener("auxclick", (event) =>
      event.preventDefault(),
    );
    this.source.dataset.testid="source";
    this.source.setAttribute("aria-label", "Markdown source (body only)");
    this.source.spellcheck=false;
    this.source.addEventListener("input",()=>{if (!this.busy) this.editor?.edit(this.source.value);});
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
    for (const [action,label] of [["reload","Reload / discard local"],["copy","Save local as new note"]] as const) {
      const button=element("button",label);button.dataset.testid="conflict-"+action;
      button.addEventListener("click",()=>{void this.resolveConflict(action);});this.resolution.append(button);
    }
    this.previewScroll.append(this.preview);
    reader.append(this.metadata, this.tools, this.saveStatus, this.saveError, this.retry, this.resolution, this.source, this.previewScroll);
    this.host.ownerDocument.addEventListener("keydown", this.onKeyDown);
    this.renderEditor();
    workspace.append(sidebar, middle, reader);
    this.diagnostics.dataset.testid = "diagnostics";
    const footer = element("footer");
    this.status.title =
      "Monitors changes made by external editors and sync tools. Save status is shown separately.";
    const help=element("button", "Keyboard shortcuts");
    help.dataset.testid="keyboard-help";help.type="button";
    help.addEventListener("click",()=>{
      if (this.busy || this.selecting || this.host.querySelector("dialog[open]")) return;
      void this.dialog("Keyboard shortcuts", "Ctrl+N: create a note. Ctrl+F: focus library search (literal words; current filters apply). Arrow Down from search: focus results. Up/Down or Home/End: navigate results; Enter or Space: open. Escape in results: return to search. Ctrl+E: switch source/preview. Ctrl+S: save. Tab/Shift+Tab: reach all controls, including Move / rename. Enter: apply a dialog; Escape: cancel. On macOS use Command instead of Ctrl.");
    });
    footer.append(
      help,
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
      this.renderEditor();
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
      if (this.pendingRefresh && this.selected)
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
    if (!this.list.contains(this.host.ownerDocument.activeElement)) this.list.replaceChildren(element("p", "Loading notes…", "empty-list"));
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
    const focused=this.host.ownerDocument.activeElement;
    const focusKey=nav.contains(focused) ? (focused as HTMLElement).dataset.filterKey : undefined;
    nav.replaceChildren();
    const button = (key: string, text: string, active: boolean, action: () => void) => {
      const b = element("button", text, active ? "filter active" : "filter");
      b.type = "button";
      b.dataset.filterKey=key;
      b.setAttribute("aria-pressed", String(active));
      b.addEventListener("click", action);
      return b;
    };
    nav.append(
      button("all", "All notes", !this.folder && !this.tag, () => {
        this.folder = null;
        this.tag = null;
        this.renderNavigation();
        void this.loadList();
      }),
      element("h3", "Folders"),
    );
    for (const path of this.browse!.folders) {
      const b = button("folder:"+path, path || "/", this.folder === path, () => {
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
        button("tag:"+tag, "# " + tag, this.tag === tag, () => {
          this.tag = this.tag === tag ? null : tag;
          this.renderNavigation();
          void this.loadList();
        }),
      );
    if (!tags.length) nav.append(element("p", "No tags yet", "hint"));
    if (focusKey !== undefined) ([...nav.querySelectorAll<HTMLButtonElement>("button")].find(b=>b.dataset.filterKey===focusKey) ?? nav.querySelector<HTMLButtonElement>("button"))?.focus();
  }
  private renderDiagnostics(): void {
    const browse = this.browse!;
    this.diagnostics.replaceChildren(
      element(
        "summary",
        `${browse.diagnostics.length} diagnostics${browse.incomplete ? " · Library results are incomplete" : " · Library scan complete"}`,
      ),
    );
    for (const diagnostic of browse.diagnostics) {
      const action = diagnostic.code === "metadata"
        ? "Check the frontmatter in an external editor; supported tags are a string list. Keep a backup before repairing metadata."
        : diagnostic.code === "io" || diagnostic.code === "permission"
          ? "Check that this path and its parent folders are readable by your user."
          : diagnostic.code === "busy"
            ? "Another library operation is active. Wait for it to finish, then retry."
            : diagnostic.code === "index"
              ? "Close other clients, run notes doctor for this library, then explicitly reindex disposable cache state if advised."
              : "Inspect the reported path. Unsupported files are not rewritten or adopted.";
      this.diagnostics.append(
        element("p", `${diagnostic.path} — ${diagnostic.code}: ${diagnostic.message}`),
        element("p", action, "hint"),
      );
    }
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
    const focused=this.host.ownerDocument.activeElement as HTMLElement | null;
    const focusedPath=this.list.contains(focused) ? focused?.dataset.notePath : undefined;
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
        b.disabled = this.busy || this.selecting;
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
      if (!rows.length && (query || this.tag || this.folder)) {
        const clear=element("button","Clear search and filters");clear.dataset.testid="clear-filters";
        clear.addEventListener("click",()=>{this.search.value="";this.tag=null;this.folder=null;this.renderNavigation();this.search.focus();void this.loadList();});
        this.list.append(clear);
      }
      // Do not steal focus if the user left results while the read was pending.
      if (focusedPath && this.host.ownerDocument.activeElement === this.host.ownerDocument.body) {
        const card=[...this.list.querySelectorAll<HTMLButtonElement>(".note-card")].find(b=>b.dataset.notePath===focusedPath);
        (card ?? this.search).focus();
      }
    } catch (error) {
      if (this.valid(epoch, session) && request === this.listRequest) {
        this.report(error);
        this.count.textContent = "Search unavailable";
        this.list.replaceChildren(
          element(
            "p",
            "Could not load results. Retry, or change the search.",
            "empty-list",
          ),
        );
        const retry=element("button","Retry results");retry.dataset.testid="retry-results";
        retry.addEventListener("click",()=>{this.search.focus();void this.loadList();});this.list.append(retry);
      }
    }
  }
  async open(path: string, refresh = false): Promise<void> {
    if (refresh) this.pendingRefresh = true;
    if (!this.state?.root || this.busy || this.selecting) return;
    if (!refresh && this.editor?.pending && !await this.protect()) return;
    await this.readNote(path, refresh);
  }
  // Only the mutation owner calls this while busy; public navigation cannot bypass it.
  private async readNote(path: string, refresh = false): Promise<void> {
    if (!this.state?.root) return;
    const refreshingEditor = this.editor;

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
      const note = await (refresh && refreshingEditor
        ? refreshingEditor.inspect(()=>this.api.open(session,path))
        : this.api.open(session,path));
      if (
        !this.valid(epoch, note.session) ||
        request !== this.noteRequest ||
        note.path !== this.selected ||
        (refresh && (this.editor !== refreshingEditor || this.busy))
      )
        return;
      this.pendingRefresh = false;
      if (refresh && this.editor?.pending) {this.editor.observe(note);return;}
      if (refresh && this.editor?.revision === note.revision) return;
      this.installNote(note, refresh);
    } catch (error) {
      if (this.valid(epoch, session) && request === this.noteRequest && (!refresh || (!this.busy && this.editor === refreshingEditor))) {
        if (refresh && this.editor?.pending) {this.pendingRefresh=false;this.editor.unavailable(error);return;}
        this.pendingRefresh = false;
        this.editor?.dispose();this.editor=null;this.note=null;this.renderEditor();
        this.empty("Note unavailable", errorText(error));
      }
    }
  }
  private installNote(note: Note, preservePosition = false): void {
    const start=this.source.selectionStart, end=this.source.selectionEnd;
    const sourceScroll=this.source.scrollTop, previewScroll=this.previewScroll.scrollTop;
    this.editor?.dispose();this.note=note;this.selected=note.path;
    this.editor=new Editor(note,this.api.save,()=>this.renderEditor(),()=>this.api.open(note.session,note.path));
    this.source.value=note.body;this.renderEditor();
    this.metadata.replaceChildren(
      element("span",note.path),element("span",note.tags.map(t=>"#"+t).join(" ")),
      element("small","Body editing preserves frontmatter. Tags are managed separately."),
    );
    if (preservePosition) this.source.setSelectionRange(start,end);
    this.source.scrollTop=preservePosition ? sourceScroll : 0;
    this.previewScroll.scrollTop=preservePosition ? previewScroll : 0;
  }
  private async diskSnapshot(editor: Editor): Promise<Note | null> {
    try {
      const note=await this.api.open(editor.session,editor.path);
      if (note.session!==editor.session || note.path!==editor.path) throw new Error("Invalid disk snapshot");
      return note;
    } catch(error) {if (errorCode(error)==="not_found") return null;throw error;}
  }
  private async resolveConflict(action: "reload" | "copy"): Promise<void> {
    const editor=this.editor, base=this.note;
    if (this.busy || this.selecting || !editor || !base || (editor.status!=="conflict" && editor.status!=="missing_on_disk")) return;
    this.busy=true;this.noteRequest++;this.renderEditor();this.error.hidden=true;
    try {
      const observed=await this.diskSnapshot(editor);
      if (this.stopped || this.editor!==editor) return;
      editor.observe(observed);
      const choice=action==="reload"
        ? await this.dialog("Discard local source?", observed
          ? `Discard your unsaved local source and reload the current disk version of ${editor.path}? This cannot be undone.`
          : `${editor.path} is missing. Discard your retained local source and clear the selection? No file will be recreated. This cannot be undone.`,
          undefined,"Discard local source")
        : await this.dialog("Save local as new note",`Keep ${editor.path} untouched. Save your local body and its original frontmatter at a different library-relative .md path. Existing files are never overwritten.`,
          [{label:"New note path",value:"",placeholder:"For example recovered/local-copy.md"}]);
      if (choice===null || this.stopped || this.editor!==editor) return;
      const current=await this.diskSnapshot(editor);
      if (this.stopped || this.editor!==editor) return;
      if (current?.revision!==observed?.revision) {
        editor.observe(current);
        this.report("The original path changed again while the choice was open. Nothing was discarded or copied. Review the current state and choose again.");return;
      }
      if (action==="reload") {
        if (current) this.installNote(current,true);
        else {
          editor.dispose();this.editor=null;this.note=null;this.selected=null;
          this.empty("Local source discarded","The original path is still missing. Select another note to continue.");
        }
      } else {
        const destination=choice[0]!;
        if (destination.toLowerCase()===editor.path.toLowerCase()) throw new Error("Choose a different path; the original path must not be recreated or overwritten.");
        const result=await this.api.copy(editor.session,editor.path,current?.revision ?? null,destination,base.source,editor.body);
        if (result.session!==editor.session || result.path!==destination || !result.file_committed || !result.revision) throw new Error("Invalid copy acknowledgement; local source retained.");
        // Never lose the only retained buffer if the just-created copy is replaced,
        // removed or unreadable before its follow-up read. A commit is not retried.
        this.report(`Local copy committed at ${destination}. `+result.warnings.join("\n"));
        let copied: Note;
        try {copied=await this.api.open(editor.session,destination);} catch(error) {
          this.report(`Copy committed at ${destination}; it could not be reopened. Local buffer retained. ${errorText(error)}`);return;
        }
        if (this.stopped || this.editor!==editor) return;
        if (copied.session!==editor.session || copied.path!==destination || copied.revision!==result.revision) {
          this.report(`Copy committed at ${destination}, but that path changed again. Your original local buffer is still retained.`);return;
        }
        this.installNote(copied);this.setMode("source");
      }
      this.pendingRefresh=false;
      if (this.state) this.state={...this.state,generation:-1};
    } catch(error) {
      if (errorCode(error)==="conflict" || errorCode(error)==="not_found") editor.unavailable(error);
      this.report(error);
    } finally {this.busy=false;this.renderEditor();void this.poll();}
  }
  private setMode(mode: "source" | "preview"): void {
    this.mode=mode;this.renderEditor();
    if (this.editor) (mode === "source" ? this.source : this.previewScroll).focus();
  }
  private renderEditor(): void {
    for (const b of this.host.querySelectorAll<HTMLButtonElement>(".note-card, [data-testid=new-note]")) b.disabled=this.busy || this.selecting;
    const editor=this.editor;
    this.tools.hidden=!editor;this.source.hidden=!editor || this.mode!=="source";
    this.preview.hidden=!!editor && this.mode!=="preview";
    this.previewScroll.hidden=this.preview.hidden;
    this.saveStatus.hidden=!editor;this.saveError.hidden=!editor?.message && !editor?.warning;
    this.retry.hidden=editor?.status!=="save_error";
    this.source.readOnly=this.busy;
    this.retry.disabled=this.busy;
    this.resolution.hidden=editor?.status!=="conflict" && editor?.status!=="missing_on_disk";
    for (const b of this.resolution.querySelectorAll("button")) b.disabled=this.busy;
    for (const b of this.tools.querySelectorAll<HTMLButtonElement>("button")) {
      b.disabled=this.busy || (b.dataset.testid==="untag-note" && !this.note?.tags.length);
      if (b.dataset.testid?.endsWith("-mode")) b.setAttribute("aria-pressed", String(b.dataset.testid===this.mode+"-mode"));
    }
    if (!editor) {this.saveError.textContent="";return;}
    const labels={clean:"Saved",dirty:"Unsaved changes",saving:"Saving…",save_error:"Save failed · buffer retained",conflict:"Conflict · autosave paused",missing_on_disk:"Missing on disk · autosave paused"};
    this.saveStatus.textContent=labels[editor.status];this.saveStatus.dataset.state=editor.status;
    this.saveError.textContent=[editor.message,editor.warning].filter(Boolean).join("\n");
    this.preview.innerHTML=renderMarkdown(editor.body);
    if (!editor.body.trim()) this.preview.append(element("p","This note is empty. Switch to Source to start writing."));
  }
  get hasUnsavedChanges(): boolean {return !!this.editor?.pending || this.busy;}
  private async protect(): Promise<boolean> {
    this.busy=true;this.renderEditor();
    try {
      if (await this.editor?.flush() !== false) return true;
      await this.dialog("Unsaved changes", "Your buffer is retained. Navigation was cancelled. Use the conflict choices or resolve the save error before leaving.");
      return false;
    } finally {this.busy=false;this.renderEditor();}
  }
  async requestClose(): Promise<void> {
    if (this.busy || this.selecting) return;
    if (!await this.protect()) return;
    this.busy=true;this.renderEditor();
    try {await this.api.close();} catch(error) {this.report(error);this.busy=false;this.renderEditor();}
  }
  private dialog(title: string, detail: string, fields?: DialogField[], destructive: boolean | string = false): Promise<string[] | null> {
    return new Promise(resolve=>{
      const dialog=element("dialog");dialog.dataset.testid="operation-dialog";
      const form=element("form");form.method="dialog";
      const heading=element("h2",title);heading.id="dialog-heading";dialog.setAttribute("aria-labelledby",heading.id);
      const description=element("p",detail);description.id="dialog-detail";dialog.setAttribute("aria-describedby",description.id);
      const origin=this.host.ownerDocument.activeElement as HTMLElement | null;
      form.append(heading,description);
      const controls=(fields ?? []).map((field,index)=>{
        const control=field.options ? element("select") : element("input");
        const label=element("label",field.label);control.id=`operation-value-${index}`;control.dataset.testid=field.testid ?? "operation-value";
        control.required=field.required ?? true;control.value=field.value;label.htmlFor=control.id;
        if (field.placeholder && control instanceof HTMLInputElement) control.placeholder=field.placeholder;
        if (control instanceof HTMLSelectElement) for (const value of field.options!) {
          const option=element("option");option.value=value;option.textContent=value;control.append(option);
        }
        form.append(label,control);return control;
      });
      const cancel=element("button",controls.length || destructive ? "Cancel" : "Stay here");cancel.type="button";cancel.dataset.testid="dialog-cancel";
      const finish=(value:string[]|null)=>{dialog.close();dialog.remove();if (origin?.isConnected) origin.focus();resolve(value);};
      cancel.addEventListener("click",()=>finish(null));form.append(cancel);
      if (controls.length || destructive) {const submit=element("button",typeof destructive === "string" ? destructive : destructive ? "Permanently delete" : "Apply");submit.type="submit";submit.dataset.testid="dialog-submit";form.append(submit);}
      form.addEventListener("submit",event=>{event.preventDefault();finish(controls.map(control=>control.value));});
      dialog.addEventListener("cancel",event=>{event.preventDefault();finish(null);});
      dialog.append(form);this.host.append(dialog);dialog.showModal();
      const first=controls[0];if (first) {first.focus();if (first instanceof HTMLInputElement) first.select();} else cancel.focus();
    });
  }
  private async mutate(action: "create" | "move" | "tag" | "untag" | "delete"): Promise<void> {
    if (this.busy || this.selecting || !this.state?.root || (action!=="create" && !this.editor) || (action==="untag" && !this.note?.tags.length)) return;
    const origin=this.host.ownerDocument.activeElement as HTMLElement | null;
    this.busy=true;this.renderEditor();this.error.hidden=true;
    try {
      if (await this.editor?.flush() === false) {
        await this.dialog("Unsaved changes", "Operation cancelled. Your source is retained; resolve the save error first.");return;
      }
      const editor=this.editor, session=this.state.session;
      const values=action==="delete"
        ? await this.dialog("Permanently delete note?",`Delete ${editor!.path} from disk? There is no undo.`,undefined,true)
        : await this.dialog(action==="create"?"New note":action==="move"?"Move / rename note":action==="tag"?"Add tag":"Remove tag",
          action==="create" ? "The title determines the Markdown filename. Choose a folder inside your library, or leave it blank for the library root. Existing files will never be overwritten."
            : action==="move" ? "Use a complete library-relative .md path. Existing files will never be overwritten."
            : action==="untag" ? "Choose a tag currently attached to this note. Other frontmatter and the body stay unchanged."
            : "Tags are case-sensitive. Other frontmatter and the body stay unchanged.",
          action==="create"
            ? [{label:"Note title",value:""},{label:"Folder (optional)",value:"",required:false,testid:"operation-folder",placeholder:"For example blog or blog/engineering"}]
            : action==="untag"
              ? [{label:"Tag",value:this.note!.tags[0]!,options:this.note!.tags}]
              : [{label:action==="move" ? "Note path" : "Tag",value:action==="move"?editor!.path:""}]);
      if (values===null) return;
      const value=values[0] ?? "";
      const result=action==="create" ? await this.api.create(session,value,values[1] || null,"",[])
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
    } catch(error) {this.report(error);} finally {
      this.busy=false;this.renderEditor();
      if (origin?.isConnected && (this.host.ownerDocument.activeElement===this.host.ownerDocument.body || this.host.ownerDocument.activeElement===origin)) origin.focus();
    }
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
