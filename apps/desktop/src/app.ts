import {
  type Api,
  type Browse,
  type DesktopState,
  type Note,
  type UpdateInfo,
  errorText,
  errorCode,
} from "./api";
import { classifyLink, renderMarkdown, wrapMarkdownLink } from "./markdown";
import { Editor } from "./editor";
import { SourceHistory } from "./source-history";
import { applyFormat, type FormatAction } from "./format";
import { appearanceLabel, resolveAppearance, type AppearancePreference } from "./appearance";

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
type Shortcut = {
  keys: ReadonlyArray<ReadonlyArray<string>>;
  description: string;
};
// "Mod" is the platform's primary accelerator: Command on macOS, Ctrl elsewhere.
const MODIFIER = "Mod";
export function primaryModifier(): string {
  const nav = navigator as Navigator & { userAgentData?: { platform?: string } };
  return /mac/i.test(nav.userAgentData?.platform || nav.platform || nav.userAgent) ? "⌘" : "Ctrl";
}
type FormatControl = { action: FormatAction; label: string; title: string; shortcut?: string };
const FORMAT_CONTROLS: ReadonlyArray<FormatControl> = [
  { action: "bold", label: "B", title: "Bold", shortcut: "B" },
  { action: "italic", label: "I", title: "Italic", shortcut: "I" },
  { action: "strike", label: "S", title: "Strikethrough" },
  { action: "code", label: "</>", title: "Inline code" },
  { action: "link", label: "Link", title: "Link", shortcut: "K" },
  { action: "heading", label: "H", title: "Heading" },
  { action: "quote", label: "Quote", title: "Quote" },
  { action: "bullet", label: "List", title: "Bullet list" },
  { action: "ordered", label: "1.", title: "Numbered list" },
  { action: "fence", label: "</> block", title: "Code block" },
];
const SHORTCUTS: ReadonlyArray<{ title: string; items: ReadonlyArray<Shortcut> }> = [
  { title: "Write", items: [
    { keys: [[MODIFIER, "N"]], description: "Create a note" },
    { keys: [[MODIFIER, "S"]], description: "Save the note now. Typing already saves on its own." },
    { keys: [[MODIFIER, "B"]], description: "Bold the selected Markdown in the source editor" },
    { keys: [[MODIFIER, "I"]], description: "Italicize the selected Markdown in the source editor" },
    { keys: [[MODIFIER, "K"]], description: "Turn the selection into a Markdown link" },
  ]},
  { title: "Read", items: [
    { keys: [[MODIFIER, "E"]], description: "Switch between Source and Preview" },
    { keys: [[MODIFIER, "L"]], description: "Show or hide line numbers in the source editor" },
  ]},
  { title: "Find a note", items: [
    { keys: [[MODIFIER, "F"]], description: "Focus library search (literal words; current filters apply)" },
    { keys: [["↓"]], description: "From the search box, step into the results" },
  ]},
  { title: "Move through results", items: [
    { keys: [["↑"], ["↓"]], description: "Previous or next result" },
    { keys: [["Home"], ["End"]], description: "First or last result" },
    { keys: [["Enter"], ["Space"]], description: "Open the focused note" },
    { keys: [["Escape"]], description: "Leave the results and return to search" },
  ]},
  { title: "In dialogs", items: [
    { keys: [["Enter"]], description: "Apply the dialog" },
    { keys: [["Escape"]], description: "Cancel and stay where you were" },
    { keys: [["Tab"], ["Shift", "Tab"]], description: "Reach every control, including Move / rename" },
  ]},
];
export class App {
  private editor: Editor | null = null;
  private busy = false;
  private pendingRefresh = false;
  private mode: "source" | "preview" = "preview";
  private lineNumbers = true;
  private readonly source = element("textarea", undefined, "source");
  private readonly sourceHistory: SourceHistory;
  private readonly gutter = element("div", undefined, "line-gutter");
  private readonly sourceWrap = element("div", undefined, "source-wrap");
  private readonly saveStatus = element("span", "", "save-status");
  private readonly wordCount = element("span", "", "word-count");
  private readonly saveError = element("div", "", "save-error");
  private readonly tools = element("div", undefined, "editor-tools");
  private readonly formatBar = element("div", undefined, "format-bar");
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
  private readonly updateNotice = element("button", "", "update-notice");
  private readonly error = element("div", "", "error");
  private readonly root = element("input");
  private readonly select = element("button", "Open library");
  private readonly appearanceSettings = element("button", "Appearance", "appearance-settings");
  private readonly focus = element("button", "Focus", "focus-action");
  private readonly workspace = element("main", undefined, "workspace");
  private appearance: AppearancePreference = "system";
  private appearanceMedia: MediaQueryList | null = null;
  private focusMode = false;
  private readonly onAppearanceChange = (): void => {
    if (this.appearance === "system") this.applyAppearance();
  };
  private readonly search = element("input");
  private readonly navigation = element("nav");
  private readonly list = element("div", "", "note-list");
  private readonly preview = element("article", "", "preview");
  private readonly previewScroll = element("div", "", "preview-scroll");
  private readonly metadata = element("div", "", "metadata");
  private readonly diagnostics = element("details", undefined, "diagnostics");
  private readonly count = element("span", "", "count");
  private readonly noteActionsTrigger = element("button", "…", "note-actions-trigger");
  private readonly noteActionsMenu = element("div", undefined, "popup-menu note-actions-menu");
  private readonly contextMenu = element("div", undefined, "popup-menu note-context-menu");
  private activeMenu: HTMLElement | null = null;
  private menuOrigin: HTMLElement | null = null;
  private contextPath: string | null = null;
  private contextClickGuard: HTMLElement | null = null;
  private readonly onMenuPointerDown = (event: MouseEvent): void => {
    const target = event.target as Node | null;
    if (!this.activeMenu || (target && (this.activeMenu.contains(target) || this.noteActionsTrigger.contains(target)))) return;
    this.closeMenu();
  };
  private readonly onMenuFocusIn = (event: FocusEvent): void => {
    const target=event.target as Node | null;
    if (!this.activeMenu || !target || this.activeMenu.contains(target) || target===this.menuOrigin) return;
    this.closeMenu();
  };
  private readonly onMenuViewportChange = (): void => {
    if (this.activeMenu) this.closeMenu();
  };
  private readonly onKeyDown = (event: KeyboardEvent): void => {
    if (event.defaultPrevented || event.isComposing || event.repeat || event.altKey) return;
    // Native modal focus/Enter/Escape behavior owns input while a choice is open.
    if (this.host.querySelector("dialog[open]")) return;
    if (!(event.ctrlKey || event.metaKey)) return;
    if (this.activeMenu) this.closeMenu();
    if (event.key.toLowerCase() === "f") {
      event.preventDefault();
      if (this.focusMode) this.setFocusMode(false);
      this.search.focus();this.search.select();return;
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
    if (event.key.toLowerCase() === "l") {
      event.preventDefault();
      this.toggleLineNumbers();
    }
    if (event.key.toLowerCase() === "n") {
      event.preventDefault();
      void this.mutate("create");
    }
    if (event.key.toLowerCase() === "k") {
      event.preventDefault();
      this.insertMarkdownLink();
      return;
    }
    if (this.host.ownerDocument.activeElement !== this.source) return;
    if (event.key.toLowerCase() === "b" || event.key.toLowerCase() === "i") {
      event.preventDefault();
      this.format(event.key.toLowerCase() === "b" ? "bold" : "italic");
    }
  };
  constructor(
    private readonly host: HTMLElement,
    private readonly api: Api,
  ) {
    host.innerHTML = "";
    const header = element("header");
    this.appearanceSettings.type = "button";
    this.appearanceSettings.dataset.testid = "appearance-settings";
    this.appearanceSettings.setAttribute("aria-haspopup", "dialog");
    this.appearanceSettings.addEventListener("click", () => { void this.configureAppearance(); });
    header.append(
      element("strong", "foglio", "wordmark"),
      this.appearanceSettings,
    );
    const form = element("form", undefined, "library-form");
    const label = element("label", "Open library");
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
    const workspace = this.workspace;
    const sidebar = element("aside", undefined, "sidebar");
    const libraryToggle = element("button", "Library", "library-toggle");
    libraryToggle.type = "button";
    libraryToggle.dataset.testid = "library-toggle";
    libraryToggle.addEventListener("click", () => {
      form.hidden = !form.hidden;
      if (!form.hidden) this.root.focus();
    });
    sidebar.append(libraryToggle, form, element("h2", "Browse"), this.navigation);
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
      const card=this.host.ownerDocument.activeElement as HTMLButtonElement;
      const index=cards.indexOf(card);
      if (index < 0) return;
      if (event.key === "ContextMenu" || (event.key === "F10" && event.shiftKey)) {
        event.preventDefault();
        const rect=card.getBoundingClientRect();
        this.openContextMenu(card, card.dataset.notePath!, rect.left + 12, rect.top + 12);
      }
      else if (event.key === "Escape") {event.preventDefault();this.search.focus();}
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
    this.source.wrap="off";
    this.source.spellcheck=false;
    this.source.addEventListener("keydown", event => {
      if (event.defaultPrevented || event.isComposing || event.altKey || this.source.readOnly) return;
      const mac = primaryModifier() === "⌘";
      if (mac ? !event.metaKey || event.ctrlKey : !event.ctrlKey || event.metaKey) return;
      const key = event.key.toLowerCase();
      const command = key === "z" ? (event.shiftKey ? "redo" : "undo")
        : !mac && key === "y" && !event.shiftKey ? "redo" : null;
      if (!command) return;
      event.preventDefault();
      this.sourceHistory.apply(command === "redo");
    });
    this.source.addEventListener("input",()=>{if (!this.busy) this.editor?.edit(this.source.value);this.updateGutter();this.updateWordCount();this.fitSource();});
    // Record after the input handler updates the editor's lossless raw body.
    this.sourceHistory = new SourceHistory(this.source, body => {
      this.editor?.restoreBody(body);this.updateGutter();this.fitSource();
    }, () => this.editor?.body ?? this.source.value);

    this.gutter.dataset.testid="line-gutter";
    this.gutter.setAttribute("role","presentation");
    this.gutter.setAttribute("aria-hidden","true");
    const modeToggle = element("div", undefined, "mode-toggle");
    modeToggle.setAttribute("role", "group");
    modeToggle.setAttribute("aria-label", "Editor mode");
    for (const mode of ["source", "preview"] as const) {
      const button=element("button",mode === "source" ? "Source" : "Preview");
      button.dataset.testid=mode+"-mode";
      button.addEventListener("click",()=>this.setMode(mode));
      modeToggle.append(button);
    }
    this.tools.append(modeToggle);
    for (const [action,label] of [["tag","Add tag"],["untag","Remove tag"]] as const) {
      const button=element("button",label);button.dataset.testid=action+"-note";
      button.addEventListener("click",()=>{void this.mutate(action,button);});this.tools.append(button);
    }
    const overflow=element("div",undefined,"document-overflow");
    this.noteActionsTrigger.type="button";
    this.noteActionsTrigger.dataset.testid="note-actions-trigger";
    this.noteActionsTrigger.setAttribute("aria-label","Note actions");
    this.noteActionsTrigger.setAttribute("aria-haspopup","menu");
    this.noteActionsTrigger.setAttribute("aria-expanded","false");
    this.noteActionsTrigger.setAttribute("aria-controls","note-actions-menu");
    this.noteActionsTrigger.addEventListener("click",()=>{
      if (this.activeMenu===this.noteActionsMenu) this.closeMenu(true);
      else {
        const rect=this.noteActionsTrigger.getBoundingClientRect();
        this.openMenu(this.noteActionsMenu,this.noteActionsTrigger,rect.left,rect.bottom+4);
      }
    });
    this.noteActionsTrigger.addEventListener("keydown",event=>{
      if (event.key!=="ArrowDown") return;
      event.preventDefault();
      const rect=this.noteActionsTrigger.getBoundingClientRect();
      this.openMenu(this.noteActionsMenu,this.noteActionsTrigger,rect.left,rect.bottom+4);
    });
    this.noteActionsMenu.id="note-actions-menu";
    this.noteActionsMenu.dataset.testid="note-actions-menu";
    this.noteActionsMenu.setAttribute("role","menu");
    this.noteActionsMenu.setAttribute("aria-label","Note actions");
    this.noteActionsMenu.hidden=true;
    for (const [action,label] of [["move","Move / rename"],["delete","Delete note"]] as const) {
      const button=element("button",label);button.type="button";button.setAttribute("role","menuitem");button.dataset.testid=action+"-note";
      button.addEventListener("click",()=>{
        this.closeMenu(true);
        if (action==="delete") {if (this.editor) void this.deleteNote(this.editor.path);}
        else void this.mutate(action);
      });
      this.noteActionsMenu.append(button);
    }
    this.bindMenu(this.noteActionsMenu);
    overflow.append(this.noteActionsTrigger,this.noteActionsMenu);
    this.contextMenu.dataset.testid="note-context-menu";
    this.contextMenu.setAttribute("role","menu");
    this.contextMenu.setAttribute("aria-label","Note actions");
    this.contextMenu.hidden=true;
    const contextDelete=element("button","Delete note");contextDelete.type="button";contextDelete.setAttribute("role","menuitem");contextDelete.dataset.testid="context-delete-note";
    contextDelete.addEventListener("click",()=>{
      const path=this.contextPath;
      this.closeMenu(true);
      if (path) void this.deleteNote(path);
    });
    this.contextMenu.append(contextDelete);this.bindMenu(this.contextMenu);
    this.focus.type="button";this.focus.dataset.testid="focus-mode";
    this.focus.addEventListener("click",()=>this.setFocusMode(!this.focusMode));
    this.tools.append(overflow,this.focus);
    this.saveStatus.dataset.testid="save-status";
    this.saveStatus.setAttribute("role","status");
    this.wordCount.dataset.testid="word-count";
    this.wordCount.setAttribute("role","status");
    this.wordCount.setAttribute("aria-label","Word count");
    this.saveError.dataset.testid="save-error";
    this.saveError.setAttribute("role","alert");
    this.retry.addEventListener("click",()=>{void this.editor?.retry();});
    for (const [action,label] of [["reload","Reload / discard local"],["copy","Save local as new note"]] as const) {
      const button=element("button",label);button.dataset.testid="conflict-"+action;
      button.addEventListener("click",()=>{void this.resolveConflict(action);});this.resolution.append(button);
    }
    this.formatBar.dataset.testid="format-bar";
    this.formatBar.setAttribute("role","toolbar");
    this.formatBar.setAttribute("aria-label","Markdown formatting");
    for (const control of FORMAT_CONTROLS) {
      const button=element("button",control.label);
      button.type="button";
      const shortcut=control.shortcut ? `${primaryModifier()}+${control.shortcut}` : null;
      button.title=shortcut ? `${control.title} (${shortcut})` : control.title;
      button.setAttribute("aria-label",control.title);
      button.dataset.testid="format-"+control.action;
      button.addEventListener("mousedown",event=>event.preventDefault());
      button.addEventListener("click",()=>this.format(control.action));
      this.formatBar.append(button);
    }
    this.previewScroll.append(this.preview);
    this.sourceWrap.append(this.gutter, this.source);
    const readerHeader=element("div",undefined,"reader-header");
    readerHeader.append(this.metadata,this.tools);
    reader.append(readerHeader, this.formatBar, this.saveError, this.retry, this.resolution, this.sourceWrap, this.previewScroll, this.wordCount);
    this.host.ownerDocument.addEventListener("keydown", this.onKeyDown);
    this.host.ownerDocument.addEventListener("mousedown", this.onMenuPointerDown);
    this.host.ownerDocument.addEventListener("focusin", this.onMenuFocusIn);
    this.host.ownerDocument.addEventListener("scroll", this.onMenuViewportChange, true);
    this.host.ownerDocument.defaultView?.addEventListener("resize", this.onMenuViewportChange);
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
      this.help();
    });
    footer.append(
      help,
      this.saveStatus,
      this.status,
      this.updateNotice,
    );
    host.append(
      header,
      this.error,
      workspace,
      this.diagnostics,
      footer,
      this.contextMenu,
    );
    this.empty(
      "Open your library",
      "Choose an existing notes folder above to begin.",
    );
  }
  private initializeAppearance(): Promise<void> {
    if (typeof window.matchMedia === "function") {
      this.appearanceMedia = window.matchMedia("(prefers-color-scheme: dark)");
      this.appearanceMedia.addEventListener?.("change", this.onAppearanceChange);
    }
    this.applyAppearance();
    return this.api.appearance().then(preference => {
      this.appearance = preference;
      this.applyAppearance();
    });
  }
  private applyAppearance(): void {
    const resolved = resolveAppearance(this.appearance, this.appearanceMedia);
    const document = this.host.ownerDocument;
    document.documentElement.dataset.appearance = resolved;
    this.appearanceSettings.textContent = `Appearance: ${appearanceLabel(this.appearance)}`;
    this.appearanceSettings.setAttribute("aria-label", `Appearance: ${appearanceLabel(this.appearance)} (${resolved})`);
  }
  private async configureAppearance(): Promise<void> {
    if (this.busy || this.selecting) return;
    const values = await this.dialog(
      "Appearance",
      "System follows your desktop setting. Light uses Sober; Dark uses the built-in Omarchy-inspired palette.",
      [{ label: "Appearance", value: this.appearance, options: ["system", "light", "dark"], testid: "appearance-preference" }],
    );
    if (!values) return;
    const preference = values[0] as AppearancePreference;
    try {
      await this.api.setAppearance(preference);
      this.appearance = preference;
      this.applyAppearance();
    } catch (error) {
      this.report(error);
    }
  }
  private setFocusMode(enabled: boolean): void {
    this.focusMode = enabled;
    this.workspace.classList.toggle("focus-mode", enabled);
    this.focus.textContent = enabled ? "Exit focus" : "Focus";
    this.focus.setAttribute("aria-pressed", String(enabled));
  }
  private bindMenu(menu: HTMLElement): void {
    menu.addEventListener("keydown",event=>{
      const items=[...menu.querySelectorAll<HTMLButtonElement>('[role="menuitem"]:not(:disabled)')];
      const index=items.indexOf(this.host.ownerDocument.activeElement as HTMLButtonElement);
      if (event.key==="Escape") {event.preventDefault();this.closeMenu(true);return;}
      if (event.key==="Tab") {
        event.preventDefault();
        const origin=this.menuOrigin;
        this.closeMenu();
        this.focusAdjacentTo(origin,event.shiftKey);
        return;
      }
      if (!["ArrowDown","ArrowUp","Home","End"].includes(event.key) || !items.length) return;
      event.preventDefault();
      const next=event.key==="Home" ? 0 : event.key==="End" ? items.length-1
        : (index+(event.key==="ArrowDown" ? 1 : -1)+items.length)%items.length;
      items[next]?.focus();
    });
  }
  private focusAdjacentTo(origin: HTMLElement | null, backward: boolean): void {
    if (!origin) return;
    const controls=[...this.host.querySelectorAll<HTMLElement>('button:not([disabled]), input:not([disabled]), textarea:not([disabled]), select:not([disabled]), [tabindex]:not([tabindex="-1"])')]
      .filter(control=>!control.closest("[hidden]") && control.getAttribute("aria-hidden")!=="true");
    const index=controls.indexOf(origin);
    if (index<0 || !controls.length) {if (origin.isConnected) origin.focus();return;}
    controls[(index+(backward ? -1 : 1)+controls.length)%controls.length]?.focus();
  }
  private openMenu(menu: HTMLElement, origin: HTMLElement, left: number, top: number): void {
    this.closeMenu();
    this.activeMenu=menu;this.menuOrigin=origin;menu.hidden=false;
    if (menu===this.noteActionsMenu) this.noteActionsTrigger.setAttribute("aria-expanded","true");
    const view=this.host.ownerDocument.defaultView;
    const bounds=menu.getBoundingClientRect();
    const width=view?.innerWidth ?? 0, height=view?.innerHeight ?? 0;
    const x=width ? Math.max(4,Math.min(left,width-bounds.width-4)) : left;
    const y=height ? Math.max(4,Math.min(top,height-bounds.height-4)) : top;
    menu.style.left=x+"px";menu.style.top=y+"px";
    menu.querySelector<HTMLButtonElement>('[role="menuitem"]:not(:disabled)')?.focus();
  }
  private closeMenu(restoreFocus = false): void {
    const origin=this.menuOrigin;
    this.noteActionsMenu.hidden=true;this.contextMenu.hidden=true;
    this.noteActionsTrigger.setAttribute("aria-expanded","false");
    this.activeMenu=null;this.menuOrigin=null;this.contextPath=null;this.contextClickGuard=null;
    if (restoreFocus && origin?.isConnected) origin.focus();
  }
  private openContextMenu(origin: HTMLElement, path: string, left: number, top: number): void {
    if (this.busy || this.selecting || !this.state?.root) return;
    this.openMenu(this.contextMenu,origin,left,top);this.contextPath=path;this.contextClickGuard=origin;
  }
  async start(): Promise<void> {
    try {
      await this.initializeAppearance();
    } catch (error) {
      // Theme settings must never block libraries or source editing.
      this.report(error);
    }
    this.checkForUpdate();
    await this.poll();
    if (!this.stopped)
      this.timer = setInterval(() => {
        void this.poll();
      }, 750);
  }
  // A release notice is purely informational: failures stay silent, and the
  // button simply opens the release page in the user's browser.
  private checkForUpdate(): void {
    this.updateNotice.type = "button";
    this.updateNotice.dataset.testid = "update-notice";
    this.updateNotice.hidden = true;
    void this.api
      .checkUpdate()
      .then((update: UpdateInfo | null) => {
        if (!update || this.stopped) return;
        if (update.signature === "tampered") {
          // A failed signature means the release cannot be trusted; surface a
          // warning without a link rather than helping the user reach it.
          this.updateNotice.textContent = `Update available: v${update.version} — signature verification failed`;
          this.updateNotice.dataset.state = "tampered";
          this.updateNotice.onclick = null;
        } else {
          this.updateNotice.textContent = `Update available: v${update.version}${update.signature === "verified" ? "" : " (unsigned release)"}`;
          this.updateNotice.setAttribute(
            "aria-label",
            `Update available: version ${update.version}. Open the release page.`,
          );
          this.updateNotice.onclick = () => void this.api.external(update.url);
        }
        this.updateNotice.hidden = false;
      })
      .catch(() => {});
  }
  stop(): void {
    this.stopped = true;
    this.appearanceMedia?.removeEventListener?.("change", this.onAppearanceChange);
    this.host.ownerDocument.removeEventListener("keydown", this.onKeyDown);
    this.host.ownerDocument.removeEventListener("mousedown", this.onMenuPointerDown);
    this.host.ownerDocument.removeEventListener("focusin", this.onMenuFocusIn);
    this.host.ownerDocument.removeEventListener("scroll", this.onMenuViewportChange, true);
    this.host.ownerDocument.defaultView?.removeEventListener("resize", this.onMenuViewportChange);
    this.closeMenu();
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
      const libraryForm = this.host.querySelector<HTMLFormElement>(".library-form");
      const libraryToggle = this.host.querySelector<HTMLButtonElement>("[data-testid=library-toggle]");
      if (libraryForm) libraryForm.hidden = !!state.root;
      if (libraryToggle) {
        libraryToggle.textContent = state.root
          ? `Library: ${state.root.split("/").filter(Boolean).pop() || state.root}`
          : "Open library";
        libraryToggle.title = state.root ?? "Open an existing local notes folder";
      }
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
      // browse_library scans the current filesystem. If the watcher advanced
      // between desktop_state and this call, its snapshot is newer than state;
      // render it now instead of leaving navigation stale until the next poll.
      if (browse.generation !== state.generation)
        this.state = { ...state, generation: browse.generation };
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
    const button = (key: string, text: string, active: boolean, action: () => void, count?: number) => {
      const b = element("button", undefined, active ? "filter active" : "filter");
      b.append(element("span", text, "filter-label"));
      if (count !== undefined) b.append(element("span", String(count), "filter-count"));
      b.type = "button";
      b.dataset.filterKey=key;
      b.setAttribute("aria-pressed", String(active));
      b.addEventListener("click", action);
      return b;
    };
    const notes = this.browse.notes;
    const folderCounts = new Map(this.browse.folders.map((folder) => [folder, 0]));
    for (const note of notes) {
      const components = note.path.split("/");
      components.pop();
      for (let index = 1; index <= components.length; index++) {
        const folder = components.slice(0, index).join("/");
        folderCounts.set(folder, (folderCounts.get(folder) ?? 0) + 1);
      }
    }
    nav.append(
      button("all", "All notes", !this.folder && !this.tag, () => {
        this.folder = null;
        this.tag = null;
        this.renderNavigation();
        void this.loadList();
      }, notes.length),
      element("h3", "Folders"),
    );
    for (const path of this.browse!.folders) {
      const b = button("folder:"+path, path || "/", this.folder === path, () => {
        this.folder = this.folder === path ? null : path;
        this.renderNavigation();
        void this.loadList();
      }, folderCounts.get(path) ?? 0);
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
    const focusedInContextMenu=this.activeMenu===this.contextMenu && !!focused && this.contextMenu.contains(focused);
    const menuPath=this.activeMenu===this.contextMenu && this.menuOrigin && this.list.contains(this.menuOrigin)
      ? this.menuOrigin.dataset.notePath : undefined;
    const focusedPath=this.list.contains(focused) ? focused?.dataset.notePath : menuPath;
    if (this.activeMenu===this.contextMenu) this.closeMenu();
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
        b.addEventListener("click", event => {
          if (this.contextClickGuard===b) {this.contextClickGuard=null;return;}
          if (event.button!==0) return;
          void this.open(row.path);
        });
        b.addEventListener("contextmenu",event=>{
          event.preventDefault();
          this.openContextMenu(b,row.path,event.clientX,event.clientY);
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
      const active=this.host.ownerDocument.activeElement;
      if (focusedPath && (active===this.host.ownerDocument.body || (focusedInContextMenu && this.contextMenu.contains(active)))) {
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
    const sourceScroll=this.sourceWrap.scrollTop, previewScroll=this.previewScroll.scrollTop;
    this.editor?.dispose();this.note=note;this.selected=note.path;
    this.editor=new Editor(note,this.api.save,()=>this.renderEditor(),()=>this.api.open(note.session,note.path));
    this.source.value=note.body;this.sourceHistory.reset();this.renderEditor();
    this.metadata.replaceChildren(
      element("span",note.path),element("span",note.tags.map(t=>"#"+t).join(" ")),
      element("small","Body editing preserves frontmatter. Tags are managed separately."),
    );
    if (preservePosition) this.source.setSelectionRange(start,end);
    this.sourceWrap.scrollTop=preservePosition ? sourceScroll : 0;
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
  private format(action: FormatAction): void {
    if (action === "link") {
      this.insertMarkdownLink();
      return;
    }
    if (this.busy || this.selecting || !this.editor || this.source.hidden || this.source.readOnly) return;
    const next=applyFormat(this.source.value,this.source.selectionStart,this.source.selectionEnd,action);
    this.sourceHistory.replace(next.text, next.start, next.end, () => {
      this.editor?.edit(this.source.value);
      this.updateGutter();
    });
    this.source.focus();
  }
  private renderEditor(): void {
    for (const b of this.host.querySelectorAll<HTMLButtonElement>(".note-card, [data-testid=new-note]")) b.disabled=this.busy || this.selecting;
    const editor=this.editor;
    this.tools.hidden=!editor;this.source.hidden=!editor || this.mode!=="source";
    this.sourceWrap.hidden=this.source.hidden;
    this.formatBar.hidden=this.source.hidden;
    this.updateGutter();this.updateWordCount();this.fitSource();
    this.preview.hidden=!!editor && this.mode!=="preview";
    this.previewScroll.hidden=this.preview.hidden;
    this.saveStatus.hidden=!editor;this.saveError.hidden=!editor?.message && !editor?.warning;
    this.wordCount.hidden=!editor;
    this.retry.hidden=editor?.status!=="save_error";
    this.source.readOnly=this.busy;
    this.retry.disabled=this.busy;
    this.resolution.hidden=editor?.status!=="conflict" && editor?.status!=="missing_on_disk";
    for (const b of this.resolution.querySelectorAll("button")) b.disabled=this.busy;
    for (const b of this.formatBar.querySelectorAll<HTMLButtonElement>("button")) b.disabled=this.busy;
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
  // Common markdown-editor binding (Ctrl/Cmd+K). Source mode only; preview
  // keeps the buffer untouched so the key cannot rewrite a hidden textarea.
  private insertMarkdownLink(): void {
    if (this.mode !== "source" || !this.editor || this.source.readOnly) return;
    const next = wrapMarkdownLink(this.source.value, this.source.selectionStart, this.source.selectionEnd);
    this.source.focus();
    if (next.text !== this.source.value) {
      this.sourceHistory.replace(next.text, next.selectionStart, next.selectionEnd, () => {
        this.editor?.edit(this.source.value);
        this.updateGutter();
        this.fitSource();
      });
    } else {
      this.source.setSelectionRange(next.selectionStart, next.selectionEnd);
    }
  }
  // Vim-style line-number visibility toggle (Ctrl/Cmd+L). Session state for
  // now; a future config layer will hydrate and persist this flag.
  private toggleLineNumbers(): void {
    this.lineNumbers = !this.lineNumbers;
    this.gutter.hidden = !this.lineNumbers;
  }
  // Live word count for the current note. English-style whitespace word
  // splitting: any run of non-whitespace is a word. Updated on keystroke and
  // on every render (note load, external sync, mode switch).
  private updateWordCount(): void {
    const text = this.editor ? this.editor.body : "";
    const words = text.trim() ? text.trim().split(/\s+/).length : 0;
    this.wordCount.textContent = words === 1 ? "1 word" : `${words} words`;
  }
  // Line-number gutter: one number per line of the source buffer. Refreshes on
  // every render (note load, mode switch, external sync) and on each keystroke.
  // The source wrapper owns scrolling; the gutter is a sibling of the opaque
  // textarea so overflow never lives on the text itself.
  private updateGutter(): void {
    if (this.source.hidden) return;
    const lines = this.source.value.split("\n").length;
    if (this.gutter.childElementCount !== lines) {
      this.gutter.textContent="";
      const frag = this.host.ownerDocument.createDocumentFragment();
      for (let i = 1; i <= lines; i++) {
        const n = element("div", String(i), "line-number");
        frag.append(n);
      }
      this.gutter.append(frag);
    } else {
      // Same count, but content may differ after external sync; rewrite text only.
      const nodes = this.gutter.children;
      for (let i = 0; i < nodes.length; i++) (nodes[i] as HTMLElement).textContent = String(i + 1);
    }
  }
  // Grow the textarea to its content so overflow lives on .source-wrap, not on
  // the text. field-sizing:content is preferred; this covers WebKitGTK without it.
  // clientHeight excludes the top border while scrollHeight does not, so the
  // fitted height must add the border back or the field stays 1px short.
  private fitSource(): void {
    if (this.source.hidden) return;
    this.source.style.width="0";this.source.style.height="0";
    const border=this.source.offsetHeight-this.source.clientHeight;
    this.source.style.width=Math.max(this.source.scrollWidth, this.sourceWrap.clientWidth - this.gutter.offsetWidth)+"px";
    this.source.style.height=Math.max(this.source.scrollHeight+border, this.sourceWrap.clientHeight)+"px";
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
  private help(): void {
    const origin=this.host.ownerDocument.activeElement as HTMLElement | null;
    const dialog=element("dialog",undefined,"shortcuts-dialog");dialog.dataset.testid="shortcuts-dialog";
    const form=element("form");form.method="dialog";
    const heading=element("h2","Keyboard shortcuts");heading.id="dialog-heading";dialog.setAttribute("aria-labelledby",heading.id);
    const modifier=primaryModifier();
    const description=element("p",modifier==="⌘" ? "Keys use ⌘ (Command). Every action has a button too; keys are only faster." : "Every action has a button too; keys are only faster.");
    description.id="dialog-detail";dialog.setAttribute("aria-describedby",description.id);
    const shortcuts=element("div",undefined,"shortcuts");
    for (const group of SHORTCUTS) {
      const section=element("section",undefined,"shortcut-group");
      const rows=element("ul");
      for (const shortcut of group.items) {
        const row=element("li");
        const cells=element("span",undefined,"shortcut-keys");
        shortcut.keys.forEach((combo,index)=>{
          if (index) cells.append(element("span","or","shortcut-separator"));
          const keys=element("span",undefined,"shortcut-combo");
          combo.forEach((key,position)=>{
            if (position) keys.append(element("span","+","shortcut-plus"));
            keys.append(element("kbd",key===MODIFIER ? modifier : key));
          });
          cells.append(keys);
        });
        row.append(cells,element("span",shortcut.description,"shortcut-description"));
        rows.append(row);
      }
      section.append(element("h3",group.title),rows);shortcuts.append(section);
    }
    const close=element("button","Close");close.type="button";close.dataset.testid="dialog-cancel";
    const finish=()=>{dialog.close();dialog.remove();if (origin?.isConnected) origin.focus();};
    close.addEventListener("click",finish);
    dialog.addEventListener("cancel",event=>{event.preventDefault();finish();});
    form.append(heading,description,shortcuts,close);dialog.append(form);
    this.host.append(dialog);dialog.showModal();close.focus();
  }
  private removeTagDialog(tags: readonly string[]): Promise<string[] | null> {
    return new Promise(resolve=>{
      const dialog=element("dialog",undefined,"remove-tag-dialog");dialog.dataset.testid="operation-dialog";
      const form=element("form");form.method="dialog";
      const heading=element("h2","Remove tag");heading.id="dialog-heading";dialog.setAttribute("aria-labelledby",heading.id);
      const description=element("p","Select a tag below to remove it from this note. Other frontmatter and the body stay unchanged.");
      description.id="dialog-detail";dialog.setAttribute("aria-describedby",description.id);
      const origin=this.host.ownerDocument.activeElement as HTMLElement | null;
      const tagsList=element("ul",undefined,"tag-removal-list");
      const finish=(value:string[]|null)=>{dialog.close();dialog.remove();if (origin?.isConnected) origin.focus();resolve(value);};
      for (const tag of tags) {
        const item=element("li");
        const remove=element("button",undefined,"remove-tag-pill");remove.type="button";remove.dataset.testid="remove-tag";
        remove.setAttribute("aria-label",`Remove tag ${tag}`);
        const name=element("span",tag,"remove-tag-name");
        const icon=element("span","×","remove-tag-x");icon.setAttribute("aria-hidden","true");
        remove.append(name,icon);remove.addEventListener("click",()=>finish([tag]));item.append(remove);tagsList.append(item);
      }
      const cancel=element("button","Cancel");cancel.type="button";cancel.dataset.testid="dialog-cancel";
      cancel.addEventListener("click",()=>finish(null));
      dialog.addEventListener("cancel",event=>{event.preventDefault();finish(null);});
      form.append(heading,description,tagsList,cancel);dialog.append(form);this.host.append(dialog);dialog.showModal();
      tagsList.querySelector<HTMLButtonElement>("button")?.focus();
    });
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
      // An informational dialog's only button just dismisses the message; a choice dialog's cancels an operation.
      const cancel=element("button",controls.length || destructive ? "Cancel" : "Close");cancel.type="button";cancel.dataset.testid="dialog-cancel";
      const finish=(value:string[]|null)=>{dialog.close();dialog.remove();if (origin?.isConnected) origin.focus();resolve(value);};
      cancel.addEventListener("click",()=>finish(null));form.append(cancel);
      if (controls.length || destructive) {const submit=element("button",typeof destructive === "string" ? destructive : destructive ? "Permanently delete" : "Apply");submit.type="submit";submit.dataset.testid="dialog-submit";form.append(submit);}
      form.addEventListener("submit",event=>{event.preventDefault();finish(controls.map(control=>control.value));});
      dialog.addEventListener("cancel",event=>{event.preventDefault();finish(null);});
      dialog.append(form);this.host.append(dialog);dialog.showModal();
      const first=controls[0];if (first) {first.focus();if (first instanceof HTMLInputElement) first.select();} else cancel.focus();
    });
  }
  private async mutate(action: "create" | "move" | "tag" | "untag", origin: HTMLElement | null = this.host.ownerDocument.activeElement as HTMLElement | null): Promise<void> {
    if (this.busy || this.selecting || !this.state?.root || (action!=="create" && !this.editor) || (action==="untag" && !this.note?.tags.length)) return;
    this.busy=true;this.renderEditor();this.error.hidden=true;
    try {
      if (await this.editor?.flush() === false) {
        await this.dialog("Unsaved changes", "Operation cancelled. Your source is retained; resolve the save error first.");return;
      }
      const editor=this.editor, session=this.state.session;
      const values=action==="untag"
        ? await this.removeTagDialog(this.note!.tags)
        : await this.dialog(action==="create"?"New note":action==="move"?"Move / rename note":"Add tag",
          action==="create" ? "The title determines the Markdown filename. Choose a folder inside your library, or leave it blank for the library root. Existing files will never be overwritten."
            : action==="move" ? "Use a complete library-relative .md path. Existing files will never be overwritten."
            : "Tags are case-sensitive. Other frontmatter and the body stay unchanged.",
          action==="create"
            ? [{label:"Note title",value:""},{label:"Folder (optional)",value:"",required:false,testid:"operation-folder",placeholder:"For example blog or blog/engineering"}]
            : [{label:action==="move" ? "Note path" : "Tag",value:action==="move"?editor!.path:""}]);
      if (values===null) return;
      const value=values[0] ?? "";
      const result=action==="create" ? await this.api.create(session,value,values[1] || null,"",[])
        : action==="move" ? await this.api.move(session,editor!.path,editor!.revision,value)
        : await this.api.tag(session,editor!.path,editor!.revision,value,action==="tag");
      if (result.session!==session || !result.file_committed) throw new Error("Invalid mutation acknowledgement");
      this.editor?.dispose();this.editor=null;this.note=null;this.selected=null;this.noteRequest++;
      this.renderEditor();
      await this.readNote(result.path);if (action==="create") this.setMode("source");
      if (result.warnings.length) this.report("File committed. "+result.warnings.join("\n"));
      if (this.state) this.state={...this.state,generation:-1};
    } catch(error) {this.report(error);} finally {
      this.busy=false;this.renderEditor();
      const active=this.host.ownerDocument.activeElement;
      if (!active || !active.isConnected || active===this.host.ownerDocument.body || active===origin) {
        if (origin?.isConnected && (!(origin instanceof HTMLButtonElement) || !origin.disabled)) origin.focus();
        else if (action==="untag") this.tools.querySelector<HTMLButtonElement>("[data-testid=tag-note]")?.focus();
      }
    }
    void this.poll();
  }
  private async deleteNote(path: string): Promise<void> {
    if (this.busy || this.selecting || !this.state?.root) return;
    const origin=this.host.ownerDocument.activeElement as HTMLElement | null;
    this.busy=true;this.renderEditor();this.error.hidden=true;
    try {
      if (await this.editor?.flush() === false) {
        await this.dialog("Unsaved changes", "Operation cancelled. Your source is retained; resolve the save error first.");return;
      }
      const session=this.state.session;
      const target=this.editor?.path===path ? this.editor : await this.api.open(session,path);
      if (!target || target.session!==session || target.path!==path) throw new Error("Invalid note snapshot");
      const confirmed=await this.dialog("Permanently delete note?",`Delete ${path} from disk? There is no undo.`,undefined,true);
      if (confirmed===null) return;
      const result=await this.api.delete(session,path,target.revision);
      if (result.session!==session || result.path!==path || result.revision!==null || !result.file_committed) throw new Error("Invalid deletion acknowledgement");
      if (this.editor?.path===path) {
        this.editor.dispose();this.editor=null;this.note=null;this.selected=null;this.noteRequest++;
        this.renderEditor();this.empty("Note deleted", "The selected file was permanently removed.");
      }
      if (result.warnings.length) this.report("File committed. "+result.warnings.join("\n"));
      if (this.state) this.state={...this.state,generation:-1};
    } catch(error) {this.report(error);} finally {
      this.busy=false;this.renderEditor();
      const active=this.host.ownerDocument.activeElement;
      if (!active || !active.isConnected || active===this.host.ownerDocument.body || active===origin) {
        if (origin?.isConnected && (!(origin instanceof HTMLButtonElement) || !origin.disabled)) origin.focus();
        else (this.list.querySelector<HTMLButtonElement>(".note-card:not(:disabled)") ?? this.search).focus();
      }
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
