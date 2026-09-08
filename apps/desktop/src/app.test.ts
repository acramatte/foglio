// @vitest-environment jsdom
import { describe, it, expect, vi, afterEach } from "vitest";
import { App } from "./app";
import type { Api, Note, DesktopState, Browse } from "./api";
const state: DesktopState = {session:1,root:"/notes",generation:1,watcher_active:true,error:null};
const browse: Browse = {session:1,generation:1,root:"/notes",notes:[{path:"a.md",title:"One",tags:["work"]},{path:"b.md",title:"Two",tags:[]}],folders:["empty"],diagnostics:[],incomplete:false};
const note = (path:string, body=path):Note => ({session:1,path,title:path,tags:[],body,revision:"hash"});
function deferred<T>() { let resolve!:(value:T)=>void; const promise = new Promise<T>(r => {resolve=r}); return {promise,resolve}; }
let app:App;
afterEach(() => {app?.stop(); document.body.replaceChildren();});
function setup(overrides:Partial<Api> = {}) {
 const api:Api = {save:vi.fn().mockResolvedValue({session:1,path:"a.md",revision:"new",file_committed:true,warnings:[]}),create:vi.fn(),move:vi.fn(),delete:vi.fn(),tag:vi.fn(),close:vi.fn().mockResolvedValue(undefined),state:vi.fn().mockResolvedValue(state),select:vi.fn().mockResolvedValue(state),browse:vi.fn().mockResolvedValue(browse),search:vi.fn().mockResolvedValue({session:1,hits:[],incomplete:false}),open:vi.fn(async(_s,path)=>note(path)),resolve:vi.fn().mockResolvedValue({session:1,path:"b.md"}),external:vi.fn().mockResolvedValue(undefined),...overrides};
 const host=document.createElement("div");document.body.append(host);app=new App(host,api);return {host,api};
}
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
 it("retries a dirty deferred watcher refresh after local reversion",async()=>{
  const {host,api}=setup();await app.start();await app.open("a.md");const source=host.querySelector<HTMLTextAreaElement>("textarea")!;
  source.value="local";source.dispatchEvent(new Event("input"));vi.mocked(api.state).mockResolvedValue({...state,generation:2});vi.mocked(api.browse).mockResolvedValue({...browse,generation:2});vi.mocked(api.open).mockResolvedValue({...note("a.md","external"),revision:"external"});await app.poll();
  source.value="a.md";source.dispatchEvent(new Event("input"));await app.poll();expect(source.value).toBe("external");expect(host.querySelector("article")?.textContent).toContain("external");
 });
 it("flushes before note navigation and protects the buffer during watcher refresh",async()=>{
  const saved=deferred<Awaited<ReturnType<Api['save']>>>();const {host,api}=setup({save:()=>saved.promise});await app.start();await app.open("a.md");
  const source=host.querySelector<HTMLTextAreaElement>("textarea")!;source.value="new local";source.dispatchEvent(new Event("input"));
  vi.mocked(api.state).mockResolvedValue({...state,generation:2});vi.mocked(api.browse).mockResolvedValue({...browse,generation:2});await app.poll();
  expect(source.value).toBe("new local");expect(api.open).toHaveBeenCalledTimes(1);
  const navigation=app.open("b.md");expect(source.readOnly).toBe(true);expect(api.open).not.toHaveBeenCalledWith(1,"b.md");
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
  const closing=app.requestClose();expect(api.close).not.toHaveBeenCalled();saved.resolve({session:1,path:"a.md",revision:"r1",file_committed:true,warnings:[]});await closing;expect(api.close).toHaveBeenCalledOnce();
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
