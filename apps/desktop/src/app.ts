import {
  type Api,
  type Browse,
  type DesktopState,
  type Note,
  errorText,
} from "./api";
import { classifyLink, renderMarkdown } from "./markdown";

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
    middle.append(this.search, this.count, this.list);
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
    reader.append(this.metadata, this.preview);
    workspace.append(sidebar, middle, reader);
    this.diagnostics.dataset.testid = "diagnostics";
    const footer = element("footer");
    this.status.title =
      "Monitors changes made by external editors and sync tools. Foglio does not save or edit files.";
    footer.append(
      element("span", "READ ONLY · Edit in your favorite text editor."),
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
      "Choose an existing notes folder above to begin. Nothing here edits your files.",
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
    if (this.selecting || !path.trim()) return;
    this.selecting = true;
    this.select.disabled = true;
    const epoch = ++this.epoch;
    this.listRequest++;
    this.noteRequest++;
    this.browse = null;
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
    if (this.polling || this.selecting || this.stopped) return;
    this.polling = true;
    const epoch = this.epoch;
    try {
      const state = await this.api.state();
      if (epoch === this.epoch && !this.selecting && !this.stopped)
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
    else this.error.hidden = true;
    if (!changed) return;
    const epoch = ++this.epoch;
    this.listRequest++;
    this.noteRequest++;
    this.browse = null;
    if (switched) {
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
    this.metadata.replaceChildren();
    if (!state.root) {
      this.list.replaceChildren();
      this.empty(
        "Open your library",
        "Choose an existing notes folder above to begin.",
      );
      return;
    }
    if (this.selected)
      this.empty(
        "Refreshing note…",
        "Checking the current file at its selected path.",
      );
    else
      this.empty(
        "Your notes, at a glance",
        "Select a note to read. Browse by folder or tag, or search across your library.",
      );
    const refreshNote = this.selected
      ? this.open(this.selected)
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
              : "No notes yet. Add Markdown files in your text editor, or check diagnostics.",
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
  async open(path: string): Promise<void> {
    if (!this.state?.root) return;
    const request = ++this.noteRequest,
      epoch = this.epoch,
      session = this.state.session;
    this.selected = path;
    this.note = null;
    this.empty("Loading note…", "Reading the current Markdown from disk.");
    for (const button of this.list.querySelectorAll<HTMLButtonElement>(
      "[data-note-path]",
    ))
      button.setAttribute("aria-pressed", String(button.dataset.notePath === path));
    try {
      const note = await this.api.open(session, path);
      if (
        !this.valid(epoch, note.session) ||
        request !== this.noteRequest ||
        note.path !== this.selected
      )
        return;
      this.note = note;
      this.metadata.replaceChildren(
        element("span", note.path),
        element("span", note.tags.map((t) => "#" + t).join(" ")),
        element("small", "Read only"),
      );
      this.preview.innerHTML = renderMarkdown(note.body);
      if (!note.body.trim())
        this.preview.append(element("p", "This note is empty."));
      this.preview.scrollTop = 0;
    } catch (error) {
      if (this.valid(epoch, session) && request === this.noteRequest)
        this.empty("Note unavailable", errorText(error));
    }
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
