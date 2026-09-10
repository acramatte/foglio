// @vitest-environment jsdom
import { describe, it, expect, vi, afterEach } from "vitest";
import { App } from "./app";
import type { Api, Note, DesktopState, Browse } from "./api";
const state: DesktopState = {session:1,root:"/notes",generation:1,watcher_active:true,error:null};
const browse: Browse = {session:1,generation:1,root:"/notes",notes:[{path:"a.md",title:"One",tags:["work"]},{path:"b.md",title:"Two",tags:[]}],folders:["empty"],diagnostics:[],incomplete:false};
const note = (path:string, body=path):Note => ({session:1,path,title:path,tags:[],body,source:body,revision:"hash"});
function deferred<T>() { let resolve!:(value:T)=>void; const promise = new Promise<T>(r => {resolve=r}); return {promise,resolve}; }
let app:App;
afterEach(() => {app?.stop(); document.body.replaceChildren();});
function setup(overrides:Partial<Api> = {}) {
 const api:Api = {copy:vi.fn(),save:vi.fn().mockResolvedValue({session:1,path:"a.md",revision:"new",file_committed:true,warnings:[]}),create:vi.fn(),move:vi.fn(),delete:vi.fn(),tag:vi.fn(),close:vi.fn().mockResolvedValue(undefined),state:vi.fn().mockResolvedValue(state),select:vi.fn().mockResolvedValue(state),browse:vi.fn().mockResolvedValue(browse),search:vi.fn().mockResolvedValue({session:1,hits:[],incomplete:false}),open:vi.fn(async(_s,path)=>note(path)),resolve:vi.fn().mockResolvedValue({session:1,path:"b.md"}),external:vi.fn().mockResolvedValue(undefined),...overrides};
 const host=document.createElement("div");document.body.append(host);app=new App(host,api);return {host,api};
}
describe("Phase 6 conflict choices",()=>{
 const click=(host:HTMLElement,id:string)=>host.querySelector<HTMLButtonElement>(`[data-testid=${id}]`)!.click();
 const status=(host:HTMLElement)=>host.querySelector<HTMLElement>("[data-testid=save-status]")!.dataset.state;
 async function conflicted(missing=false) {
  HTMLDialogElement.prototype.showModal=function(){this.open=true;};HTMLDialogElement.prototype.close=function(){this.open=false;};
  let disk:Note|null={...note("a.md","disk"),revision:"disk"};
  const {host,api}=setup();await app.start();await app.open("a.md");
  const source=host.querySelector<HTMLTextAreaElement>("textarea")!;source.value="local";source.dispatchEvent(new Event("input"));
  if (missing) disk=null;
  vi.mocked(api.open).mockImplementation(async(_s,path)=>{if(path!=="a.md") return {...note(path,"local"),revision:"copy"};if(!disk) throw {code:"not_found",message:"Missing"};return disk;});
  vi.mocked(api.state).mockResolvedValue({...state,generation:2});vi.mocked(api.browse).mockResolvedValue({...browse,generation:2});
  await app.poll();expect(status(host)).toBe(missing?"missing_on_disk":"conflict");
  return {host,api,source,setDisk:(value:Note|null)=>{disk=value;}};
 }
 async function dialog(host:HTMLElement,action:string) {
  click(host,"conflict-"+action);await vi.waitFor(()=>expect(host.querySelector("dialog")).not.toBeNull());
 }
 function submit(host:HTMLElement,path="copy.md") {
  const value=host.querySelector<HTMLInputElement>("[data-testid=operation-value]");if(value)value.value=path;
  host.querySelector("dialog form")!.dispatchEvent(new Event("submit",{cancelable:true}));
 }
 it("cancels explicit discard, then reloads only the confirmed current disk snapshot",async()=>{
  const {host,api,source}=await conflicted();await dialog(host,"reload");
  expect(source.readOnly).toBe(true);click(host,"dialog-cancel");await vi.waitFor(()=>expect(source.readOnly).toBe(false));
  expect(source.value).toBe("local");expect(status(host)).toBe("conflict");
  await dialog(host,"reload");expect(host.querySelector("[data-testid=dialog-submit]")?.textContent).toBe("Discard local source");submit(host);
  await vi.waitFor(()=>expect(source.value).toBe("disk"));expect(status(host)).toBe("clean");expect(api.save).not.toHaveBeenCalled();
 });
 it.each(["reload","copy"])("retains the buffer when the source changes again during %s",async(action)=>{
  const {host,api,source,setDisk}=await conflicted();await dialog(host,action);
  setDisk({...note("a.md","new disk"),revision:"new disk"});submit(host);
  await vi.waitFor(()=>expect(source.readOnly).toBe(false));expect(source.value).toBe("local");expect(status(host)).toBe("conflict");
  expect(api.copy).not.toHaveBeenCalled();expect(host.textContent).toContain("changed again");
 });
 it("rechecks confirmed absence before discarding and never follows a reappeared path",async()=>{
  const {host,source,setDisk}=await conflicted(true);await dialog(host,"reload");
  setDisk({...note("a.md","reappeared"),revision:"reappeared"});submit(host);
  await vi.waitFor(()=>expect(source.readOnly).toBe(false));expect(source.value).toBe("local");expect(status(host)).toBe("conflict");
 });
 it("explicitly discards a missing buffer without any mutation",async()=>{
  const {host,api}=await conflicted(true);await dialog(host,"reload");submit(host);
  await vi.waitFor(()=>expect(host.querySelector<HTMLElement>("[data-testid=save-status]")!.hidden).toBe(true));
  expect(app.hasUnsavedChanges).toBe(false);expect(api.save).not.toHaveBeenCalled();expect(api.copy).not.toHaveBeenCalled();
 });
 it.each([false,true])("copies retained base source and body with an explicit present/absent guard (missing=%s)",async(missing)=>{
  const {host,api,source}=await conflicted(missing);
  vi.mocked(api.copy).mockResolvedValue({session:1,path:"copy.md",revision:"copy",file_committed:true,warnings:[]});
  await dialog(host,"copy");submit(host);await vi.waitFor(()=>expect(source.readOnly).toBe(false));
  expect(api.copy).toHaveBeenCalledWith(1,"a.md",missing?null:"disk","copy.md","a.md","local");
  expect(host.querySelector(".metadata")?.textContent).toContain("copy.md");expect(source.value).toBe("local");expect(status(host)).toBe("clean");
 });
 it("rejects copying a missing note back to its original path",async()=>{
  const {host,api,source}=await conflicted(true);await dialog(host,"copy");submit(host,"a.md");
  await vi.waitFor(()=>expect(source.readOnly).toBe(false));expect(api.copy).not.toHaveBeenCalled();expect(source.value).toBe("local");expect(status(host)).toBe("missing_on_disk");
 });
 it.each(["destination_exists","conflict"])("retains buffer and choices after backend %s",async(code)=>{
  const {host,api,source}=await conflicted();vi.mocked(api.copy).mockRejectedValue({code,message:"guard failed"});
  await dialog(host,"copy");submit(host);await vi.waitFor(()=>expect(source.readOnly).toBe(false));
  expect(source.value).toBe("local");expect(status(host)).toBe("conflict");expect(host.textContent).toContain("guard failed");
 });
 it("retains the buffer and committed-path evidence when a copy cannot be reopened",async()=>{
  const {host,api,source}=await conflicted();
  vi.mocked(api.copy).mockImplementation(async()=>{vi.mocked(api.open).mockRejectedValue({code:"not_found",message:"Copy removed"});return {session:1,path:"copy.md",revision:"copy",file_committed:true,warnings:[]};});
  await dialog(host,"copy");submit(host);await vi.waitFor(()=>expect(source.readOnly).toBe(false));
  expect(source.value).toBe("local");expect(host.textContent).toContain("Copy committed at copy.md; it could not be reopened");expect(api.copy).toHaveBeenCalledOnce();
 });
 it("holds the operation lock until a committed copy's follow-up read completes",async()=>{
  const {host,api,source}=await conflicted();const pending=deferred<Note>();
  vi.mocked(api.copy).mockImplementation(async()=>{vi.mocked(api.open).mockReturnValue(pending.promise);return {session:1,path:"copy.md",revision:"copy",file_committed:true,warnings:[]};});
  await dialog(host,"copy");submit(host);await vi.waitFor(()=>expect(api.open).toHaveBeenCalledWith(1,"copy.md"));
  await app.open("b.md");await app.requestClose();expect(source.readOnly).toBe(true);expect(api.close).not.toHaveBeenCalled();
  pending.resolve({...note("copy.md","local"),revision:"copy"});await vi.waitFor(()=>expect(source.readOnly).toBe(false));
 });
});
describe("Phase 7 keyboard and accessibility", () => {
 const key=(key:string, extra:KeyboardEventInit={})=>document.activeElement!.dispatchEvent(new KeyboardEvent("keydown",{key,ctrlKey:true,bubbles:true,cancelable:true,...extra}));
 const get=<T extends HTMLElement>(host:HTMLElement,id:string)=>host.querySelector<T>(`[data-testid=${id}]`)!;
 const dialogs=()=>{HTMLDialogElement.prototype.showModal=function(){this.open=true;};HTMLDialogElement.prototype.close=function(){this.open=false;};};
 it("focuses existing search, enters results, and retains result focus across refresh",async()=>{
  const {host}=setup();await app.start();await app.open("a.md");get<HTMLButtonElement>(host,"source-mode").click();
  key("f");expect(document.activeElement).toBe(get(host,"search-input"));
  key("ArrowDown",{ctrlKey:false});expect(document.activeElement?.getAttribute("data-note-path")).toBe("a.md");
  await app.loadList();expect(document.activeElement?.getAttribute("data-note-path")).toBe("a.md");
  key("ArrowDown",{ctrlKey:false});expect(document.activeElement?.getAttribute("data-note-path")).toBe("b.md");
  key("Escape",{ctrlKey:false});expect(document.activeElement).toBe(get(host,"search-input"));
 });
 it("provides labelled help, suppresses modal shortcuts, and returns focus on Escape",async()=>{
  dialogs();const {host,api}=setup();await app.start();await app.open("a.md");
  const help=get<HTMLButtonElement>(host,"keyboard-help");expect(help).not.toBeNull();help.focus();help.click();
  const dialog=host.querySelector("dialog")!;expect(dialog.getAttribute("aria-describedby")).toBe("dialog-detail");expect(dialog.textContent).toContain("Ctrl+F");
  key("n");key("e");expect(host.querySelectorAll("dialog")).toHaveLength(1);expect(get<HTMLElement>(host,"preview").hidden).toBe(false);expect(api.create).not.toHaveBeenCalled();
  dialog.dispatchEvent(new Event("cancel",{cancelable:true}));expect(document.activeElement).toBe(help);
 });
 it("focuses preview on mode switch and restores move trigger after cancellation",async()=>{
  dialogs();const {host}=setup();await app.start();await app.open("a.md");get<HTMLButtonElement>(host,"source-mode").click();key("e");
  expect(document.activeElement).toBe(host.querySelector(".preview-scroll"));
  const move=get<HTMLButtonElement>(host,"move-note");move.focus();move.click();await vi.waitFor(()=>expect(host.querySelector("dialog")).not.toBeNull());
  expect(document.activeElement).toBe(get(host,"operation-value"));host.querySelector("dialog")!.dispatchEvent(new Event("cancel",{cancelable:true}));
  await vi.waitFor(()=>expect(move.disabled).toBe(false));expect(document.activeElement).toBe(move);
 });
 it("keeps filter focus and offers an actionable empty-results reset",async()=>{
  const {host}=setup();await app.start();const filter=[...host.querySelectorAll<HTMLButtonElement>("nav button")].find(b=>b.textContent==="empty")!;filter.focus();filter.click();
  expect(document.activeElement?.textContent).toBe("empty");
  await vi.waitFor(()=>expect(get(host,"clear-filters")).not.toBeNull());get<HTMLButtonElement>(host,"clear-filters").click();
  await vi.waitFor(()=>expect(host.querySelectorAll(".note-card")).toHaveLength(2));expect(document.activeElement).toBe(get(host,"search-input"));
  expect(host.querySelector(".count")?.getAttribute("role")).toBe("status");expect(host.querySelector("nav")?.getAttribute("aria-label")).toBe("Library filters");
 });
 it("disables navigation through a mutation acknowledgement, including refreshed cards",async()=>{
  dialogs();const pending=deferred<Awaited<ReturnType<Api["delete"]>>>();
  const {host,api}=setup({delete:vi.fn(()=>pending.promise)});await app.start();await app.open("a.md");
  get<HTMLButtonElement>(host,"delete-note").click();await vi.waitFor(()=>expect(host.querySelector("dialog")).not.toBeNull());
  host.querySelector("dialog form")!.dispatchEvent(new Event("submit",{cancelable:true}));
  await vi.waitFor(()=>expect(api.delete).toHaveBeenCalled());
  expect(host.querySelector<HTMLButtonElement>(".note-card")!.disabled).toBe(true);
  expect(get<HTMLButtonElement>(host,"new-note").disabled).toBe(true);
  await app.loadList();expect(host.querySelector<HTMLButtonElement>(".note-card")!.disabled).toBe(true);
  pending.resolve({session:1,path:"a.md",revision:null,file_committed:true,warnings:[]});
  await vi.waitFor(()=>expect(get<HTMLButtonElement>(host,"new-note").disabled).toBe(false));
  expect(host.querySelector<HTMLButtonElement>(".note-card")!.disabled).toBe(false);
 });
 it("offers explicit result retry without changing literal search",async()=>{
  const {host,api}=setup({search:vi.fn().mockRejectedValue({code:"busy",message:"Library busy"})});await app.start();const search=get<HTMLInputElement>(host,"search-input");search.value="literal words";await app.loadList();
  const retry=get<HTMLButtonElement>(host,"retry-results");expect(retry).not.toBeNull();vi.mocked(api.search).mockResolvedValue({session:1,hits:[],incomplete:false});retry.click();
  await vi.waitFor(()=>expect(api.search).toHaveBeenCalledTimes(2));expect(api.search).toHaveBeenLastCalledWith(1,"literal words",null,null);
 });
});
describe("desktop UI state", () => {
 it("toggles source and preview from either mode with the keyboard shortcut",async()=>{
  const {host}=setup();await app.start();await app.open("a.md");
  host.querySelector<HTMLButtonElement>("[data-testid=source-mode]")!.click();
  const source=host.querySelector<HTMLTextAreaElement>("[data-testid=source]")!;
  source.dispatchEvent(new KeyboardEvent("keydown",{key:"e",ctrlKey:true,bubbles:true}));
  expect(host.querySelector<HTMLElement>("[data-testid=preview]")!.hidden).toBe(false);
  document.dispatchEvent(new KeyboardEvent("keydown",{key:"e",ctrlKey:true,bubbles:true}));
  expect(source.hidden).toBe(false);
 });
 it("keeps note content inside a separate scroll wrapper when reloading the list",async()=>{
  const {host}=setup();await app.start();
  const list=host.querySelector<HTMLElement>("[data-testid=note-list]")!;
  const scroll=host.querySelector<HTMLElement>(".note-list-scroll")!;
  expect(scroll).not.toBeNull();expect(list.parentElement).toBe(scroll);
  expect(scroll.contains(host.querySelector("[data-testid=search-input]"))).toBe(false);
  expect(scroll.contains(host.querySelector("[data-testid=new-note]"))).toBe(false);
  await app.loadList();
  expect(list.parentElement).toBe(scroll);
  expect(list.querySelectorAll(".note-card")).toHaveLength(2);
 });
 it("owns preview scrolling in a wrapper that follows mode and note changes",async()=>{
  const {host}=setup();await app.start();
  const preview=host.querySelector<HTMLElement>("[data-testid=preview]")!;
  const scroll=host.querySelector<HTMLElement>(".preview-scroll")!;
  expect(scroll).not.toBeNull();expect(preview.parentElement).toBe(scroll);
  expect(scroll.hidden).toBe(false);
  await app.open("a.md");scroll.scrollTop=120;
  host.querySelector<HTMLButtonElement>("[data-testid=source-mode]")!.click();
  expect(scroll.hidden).toBe(true);
  host.querySelector<HTMLButtonElement>("[data-testid=preview-mode]")!.click();
  expect(scroll.hidden).toBe(false);expect(scroll.scrollTop).toBe(120);
  await app.open("b.md");expect(scroll.scrollTop).toBe(0);
 });
 it("opens the new-note dialog with Ctrl+N",async()=>{
  HTMLDialogElement.prototype.showModal=function(){this.open=true;};HTMLDialogElement.prototype.close=function(){this.open=false;};
  const {host,api}=setup({create:vi.fn().mockResolvedValue({session:1,path:"System-designs.md",revision:"r1",file_committed:true,warnings:[]})});
  await app.start();document.dispatchEvent(new KeyboardEvent("keydown",{key:"n",ctrlKey:true}));
  await vi.waitFor(()=>expect(host.querySelector("dialog")).not.toBeNull());
  const input=host.querySelector<HTMLInputElement>("[data-testid=operation-value]")!;
  input.value="System designs";
  host.querySelector("dialog form")!.dispatchEvent(new Event("submit",{cancelable:true}));
  await vi.waitFor(()=>expect(api.create).toHaveBeenCalledWith(1,"System designs",null,"",[]));
 });
 it("derives a portable filename when the optional folder is blank",async()=>{
  HTMLDialogElement.prototype.showModal=function(){this.open=true;};HTMLDialogElement.prototype.close=function(){this.open=false;};
  const {host,api}=setup({create:vi.fn().mockResolvedValue({session:1,path:"System-designs.md",revision:"r1",file_committed:true,warnings:[]})});
  await app.start();host.querySelector<HTMLButtonElement>("[data-testid=new-note]")!.click();
  await vi.waitFor(()=>expect(host.querySelector("dialog")).not.toBeNull());
  const input=host.querySelector<HTMLInputElement>("[data-testid=operation-value]")!;
  const folder=host.querySelector<HTMLInputElement>("[data-testid=operation-folder]")!;
  expect(input.labels?.[0]?.textContent).toBe("Note title");expect(folder.labels?.[0]?.textContent).toBe("Folder (optional)");expect(folder.required).toBe(false);input.value="System designs";
  host.querySelector("dialog form")!.dispatchEvent(new Event("submit",{cancelable:true}));
  await vi.waitFor(()=>expect(api.create).toHaveBeenCalledWith(1,"System designs",null,"",[]));
 });
 it("derives the filename inside an optional nested folder",async()=>{
  HTMLDialogElement.prototype.showModal=function(){this.open=true;};HTMLDialogElement.prototype.close=function(){this.open=false;};
  const {host,api}=setup({create:vi.fn().mockResolvedValue({session:1,path:"blog/engineering/Sprite.md",revision:"r1",file_committed:true,warnings:[]})});
  await app.start();host.querySelector<HTMLButtonElement>("[data-testid=new-note]")!.click();
  await vi.waitFor(()=>expect(host.querySelector("dialog")).not.toBeNull());
  host.querySelector<HTMLInputElement>("[data-testid=operation-value]")!.value="Sprite";
  host.querySelector<HTMLInputElement>("[data-testid=operation-folder]")!.value="blog/engineering";
  host.querySelector("dialog form")!.dispatchEvent(new Event("submit",{cancelable:true}));
  await vi.waitFor(()=>expect(api.create).toHaveBeenCalledWith(1,"Sprite","blog/engineering","",[]));
  expect(api.open).toHaveBeenCalledWith(1,"blog/engineering/Sprite.md");
 });
 it("offers existing tags for removal and disables removal when none exist",async()=>{
  HTMLDialogElement.prototype.showModal=function(){this.open=true;};HTMLDialogElement.prototype.close=function(){this.open=false;};
  const tagged=note("a.md");tagged.tags=["work","urgent"];
  const {host,api}=setup({open:vi.fn(async(_s,path)=>path==="a.md"?tagged:note(path)),tag:vi.fn().mockResolvedValue({session:1,path:"a.md",revision:"r1",file_committed:true,warnings:[]})});
  await app.start();await app.open("a.md");host.querySelector<HTMLButtonElement>("[data-testid=untag-note]")!.click();
  await vi.waitFor(()=>expect(host.querySelector("dialog")).not.toBeNull());
  const select=host.querySelector<HTMLSelectElement>("select[data-testid=operation-value]")!;
  expect([...select.options].map(option=>option.value)).toEqual(["work","urgent"]);select.value="urgent";
  host.querySelector("dialog form")!.dispatchEvent(new Event("submit",{cancelable:true}));
  await vi.waitFor(()=>expect(api.tag).toHaveBeenCalledWith(1,"a.md","hash","urgent",false));
  await app.open("b.md");expect(host.querySelector<HTMLButtonElement>("[data-testid=untag-note]")!.disabled).toBe(true);
 });
 it("retains the mutation lock through its follow-up open",async()=>{
  HTMLDialogElement.prototype.showModal=function(){this.open=true;};HTMLDialogElement.prototype.close=function(){this.open=false;};
  const opened=deferred<Note>();const {host,api}=setup({create:vi.fn().mockResolvedValue({session:1,path:"one.md",revision:"r1",file_committed:true,warnings:[]}),open:vi.fn((_s,path)=>path==="one.md"?opened.promise:Promise.resolve(note(path)))});
  await app.start();await app.open("a.md");host.querySelector<HTMLButtonElement>("[data-testid=new-note]")!.click();
  await vi.waitFor(()=>expect(host.querySelector("dialog")).not.toBeNull());host.querySelector<HTMLInputElement>("[data-testid=operation-value]")!.value="one";host.querySelector("dialog form")!.dispatchEvent(new Event("submit",{cancelable:true}));
  await vi.waitFor(()=>expect(api.open).toHaveBeenCalledWith(1,"one.md"));host.querySelector<HTMLButtonElement>("[data-testid=new-note]")!.click();await Promise.resolve();
  expect(host.querySelector("dialog")).toBeNull();expect(host.querySelector<HTMLTextAreaElement>("textarea")!.readOnly).toBe(true);
  opened.resolve(note("one.md"));await vi.waitFor(()=>expect(host.querySelector<HTMLTextAreaElement>("textarea")!.readOnly).toBe(false));expect(api.create).toHaveBeenCalledOnce();
 });
 it("requires explicit resolution after an observed conflict even if local edits revert",async()=>{
  const {host,api}=setup();await app.start();await app.open("a.md");const source=host.querySelector<HTMLTextAreaElement>("textarea")!;
  source.value="local";source.dispatchEvent(new Event("input"));vi.mocked(api.state).mockResolvedValue({...state,generation:2});vi.mocked(api.browse).mockResolvedValue({...browse,generation:2});vi.mocked(api.open).mockResolvedValue({...note("a.md","external"),revision:"external"});await app.poll();
  source.value="a.md";source.dispatchEvent(new Event("input"));await app.poll();expect(source.value).toBe("a.md");expect(host.querySelector("[data-testid=save-status]")?.getAttribute("data-state")).toBe("conflict");
 });
 it("flushes before note navigation and protects the buffer during watcher refresh",async()=>{
  const saved=deferred<Awaited<ReturnType<Api['save']>>>();const {host,api}=setup({save:()=>saved.promise});await app.start();await app.open("a.md");
  const source=host.querySelector<HTMLTextAreaElement>("textarea")!;source.value="new local";source.dispatchEvent(new Event("input"));
  vi.mocked(api.state).mockResolvedValue({...state,generation:2});vi.mocked(api.browse).mockResolvedValue({...browse,generation:2});await app.poll();
  expect(source.value).toBe("new local");expect(api.open).toHaveBeenCalledTimes(2);
  const navigation=app.open("b.md");expect(source.readOnly).toBe(true);expect(api.open).not.toHaveBeenCalledWith(1,"b.md");
  vi.mocked(api.open).mockImplementation(async(_s,path)=>({...note(path,path==="a.md"?"new local":path),revision:path==="a.md"?"r1":"hash"}));
  saved.resolve({session:1,path:"a.md",revision:"r1",file_committed:true,warnings:[]});await navigation;
  expect(api.open).toHaveBeenLastCalledWith(1,"b.md");
 });
 it("rejects a watcher read that predates a completed save",async()=>{
  const disk=deferred<Note>();const {host,api}=setup();await app.start();await app.open("a.md");
  vi.mocked(api.state).mockResolvedValue({...state,generation:2});vi.mocked(api.browse).mockResolvedValue({...browse,generation:2});vi.mocked(api.open).mockReturnValue(disk.promise);
  const refresh=app.poll();await Promise.resolve();await Promise.resolve();
  const source=host.querySelector<HTMLTextAreaElement>("textarea")!;source.value="new local";source.dispatchEvent(new Event("input"));
  host.dispatchEvent(new KeyboardEvent("keydown",{key:"s",ctrlKey:true}));await Promise.resolve();await Promise.resolve();await Promise.resolve();
  disk.resolve(note("a.md","stale disk"));await refresh;
  expect(source.value).toBe("new local");expect(host.querySelector("article")?.textContent).toContain("new local");
 });
 it("waits for a save before completing native close",async()=>{
  const saved=deferred<Awaited<ReturnType<Api['save']>>>();const {host,api}=setup({save:()=>saved.promise});await app.start();await app.open("a.md");
  const source=host.querySelector<HTMLTextAreaElement>("textarea")!;source.value="last edit";source.dispatchEvent(new Event("input"));
  const closing=app.requestClose();expect(api.close).not.toHaveBeenCalled();vi.mocked(api.open).mockResolvedValue({...note("a.md","last edit"),revision:"r1"});saved.resolve({session:1,path:"a.md",revision:"r1",file_committed:true,warnings:[]});await closing;expect(api.close).toHaveBeenCalledOnce();
 });
 it("cancels failed navigation and close without discarding source",async()=>{
  HTMLDialogElement.prototype.showModal=function(){this.open=true;};HTMLDialogElement.prototype.close=function(){this.open=false;};
  const {host,api}=setup({save:vi.fn().mockRejectedValue({code:"io",message:"denied"})});await app.start();await app.open("a.md");
  const source=host.querySelector<HTMLTextAreaElement>("textarea")!;source.value="retained";source.dispatchEvent(new Event("input"));
  const navigation=app.open("b.md");await vi.waitFor(()=>expect(host.querySelector("dialog")).not.toBeNull());host.querySelector<HTMLButtonElement>("[data-testid=dialog-cancel]")!.click();await navigation;
  expect(api.open).not.toHaveBeenCalledWith(1,"b.md");expect(source.value).toBe("retained");
  const closing=app.requestClose();await vi.waitFor(()=>expect(host.querySelector("dialog")).not.toBeNull());host.querySelector<HTMLButtonElement>("[data-testid=dialog-cancel]")!.click();await closing;expect(api.close).not.toHaveBeenCalled();expect(source.value).toBe("retained");
 });
 it("keeps navigation safe while a generation refresh is pending", async() => {
  const pending = deferred<Browse>(); const {host,api}=setup(); await app.start();
  vi.mocked(api.state).mockResolvedValue({...state,generation:2});
  vi.mocked(api.browse).mockReturnValue(pending.promise);
  const refreshing=app.poll(); await Promise.resolve(); await Promise.resolve();
  const errors: unknown[]=[]; const listener=(e:ErrorEvent)=>{errors.push(e.error);e.preventDefault();};window.addEventListener("error",listener);
  host.querySelector<HTMLButtonElement>("nav button")!.click();
  window.removeEventListener("error",listener);
  expect(errors).toEqual([]); expect(host.querySelector("nav button")).not.toBeNull();
  pending.resolve({...browse,generation:2});await refreshing;
  expect(host.querySelector("nav")?.textContent).toContain("empty");
 });
 it("ignores a slow previous note response", async() => {
  const slow=deferred<Note>(); const {host}=setup({open:vi.fn((_s,path)=> path==="a.md" ? slow.promise : Promise.resolve(note(path)))});
  await app.start();const pending=app.open("a.md"); await app.open("b.md");slow.resolve(note("a.md"));await pending;
  expect(host.querySelector("article")?.textContent?.trim()).toBe("b.md");
 });
 it("ignores a previous library response and renders diagnostics as text", async() => {
  const slow=deferred<Note>();const {host}=setup({open:()=>slow.promise,select:async()=>({...state,session:2,root:"/other"}),browse:async(session)=>({...browse,session,root:session===2?"/other":"/notes",diagnostics:[{path:"<script>",code:"bad",message:"<img onerror=alert(1)>"}]})});
  await app.start();const pending=app.open("a.md");await app.choose("/other");slow.resolve(note("a.md","STALE BODY"));await pending;
  expect(host.querySelector("article")?.textContent).not.toContain("STALE BODY");
  expect(host.querySelector("[data-testid=diagnostics] script, [data-testid=diagnostics] img")).toBeNull();
 });
 it("drops stale search responses", async() => {
  const slow=deferred<Awaited<ReturnType<Api['search']>>>();const {host}=setup({search:vi.fn((_s,q)=>q==="slow"?slow.promise:Promise.resolve({session:1,hits:[{title:"Fresh",path:"b.md",snippet:"",rank:1}],incomplete:false}))});
  await app.start();const input=host.querySelector<HTMLInputElement>("[data-testid=search-input]")!;
  input.value="slow";const pending=app.loadList();input.value="fast";await app.loadList();slow.resolve({session:1,hits:[{title:"Stale",path:"a.md",snippet:"",rank:1}],incomplete:false});await pending;
  expect(host.querySelector("[data-testid=note-list]")?.textContent).toContain("Fresh");
  expect(host.querySelector("[data-testid=note-list]")?.textContent).not.toContain("Stale");
 });
 it("keeps the selected path on edits and does not retarget an external rename", async() => {
  const {host,api}=setup(); await app.start();await app.open("a.md");
  vi.mocked(api.state).mockResolvedValue({...state,generation:2});
  vi.mocked(api.browse).mockResolvedValue({...browse,generation:2});
  vi.mocked(api.open).mockResolvedValue({...note("a.md","Edited body"),revision:"new hash"});await app.poll();
  expect(host.querySelector("article")?.textContent?.trim()).toBe("Edited body");
  expect(host.querySelector('[data-note-path="a.md"]')?.getAttribute("aria-pressed")).toBe("true");
  vi.mocked(api.state).mockResolvedValue({...state,generation:3});
  vi.mocked(api.browse).mockResolvedValue({...browse,generation:3,notes:[{path:"moved.md",title:"Moved",tags:[]}]});
  vi.mocked(api.open).mockRejectedValue("note not found: a.md");await app.poll();
  expect(api.open).toHaveBeenLastCalledWith(1,"a.md");
  expect(host.querySelector("article")?.textContent).toContain("Note unavailable");
  expect(host.querySelector("article")?.textContent).not.toContain("Edited body");
  const moved=host.querySelector<HTMLButtonElement>('[data-note-path="moved.md"]')!;
  expect(moved.disabled).toBe(false);expect(moved.getAttribute("aria-pressed")).toBe("false");
  vi.mocked(api.open).mockResolvedValue(note("moved.md","Edited body"));moved.click();
  await Promise.resolve();expect(api.open).toHaveBeenLastCalledWith(1,"moved.md");
 });
 it("makes all listed Markdown available and keeps healthy monitoring quiet", async() => {
  const {host,api}=setup();await app.start();
  const cards=host.querySelectorAll<HTMLButtonElement>(".note-card");
  expect([...cards].every(card=>!card.disabled)).toBe(true);
  cards[0]!.click();await Promise.resolve();expect(api.open).toHaveBeenCalledWith(1,"a.md");
  expect(host.querySelector("header .status")).toBeNull();
  expect(host.querySelector("footer .status")?.textContent).toBe("Monitoring external changes");
  expect(host.querySelector("footer .status")?.getAttribute("title")).toContain("external editors");
  expect(host.textContent).not.toMatch(/adopt|valid IDs|Saving/);
  vi.mocked(api.state).mockResolvedValue({...state,watcher_active:false,error:"Watcher failed"});await app.poll();
  expect(host.querySelector<HTMLElement>('[role="alert"]')!.hidden).toBe(false);
  expect(host.querySelector('[role="alert"]')?.textContent).toBe("Watcher failed");
 });
 it("ignores resolved links after the selected path changes", async() => {
  const slow=deferred<{session:number;path:string}>();
  const {host,api}=setup({resolve:()=>slow.promise,open:vi.fn(async(_s,path)=>note(path,"[Note](other.md)"))});
  await app.start();await app.open("a.md");host.querySelector<HTMLAnchorElement>("article a")!.click();
  await app.open("b.md");slow.resolve({session:1,path:"other.md"});await Promise.resolve();await Promise.resolve();
  expect(api.open).not.toHaveBeenCalledWith(1,"other.md");
 });
 it("routes user-activated links only through controlled commands", async() => {
  const {host,api}=setup({open:async(_s,path)=>note(path,"[Web](https://example.org) [Note](other.md)")});await app.start();await app.open("a.md");
  const anchors=host.querySelectorAll<HTMLAnchorElement>("article a");anchors[0]!.click();expect(api.external).toHaveBeenCalledWith("https://example.org");anchors[1]!.click();expect(api.resolve).toHaveBeenCalledWith(1,"a.md","other.md");
 });
});
