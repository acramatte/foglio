// @vitest-environment jsdom
import { describe, it, expect, vi, afterEach } from "vitest";
import { App } from "./app";
import type { Api, Note, DesktopState, Browse } from "./api";
const state: DesktopState = {session:1,root:"/notes",generation:1,watcher_active:true,error:null};
const browse: Browse = {session:1,generation:1,root:"/notes",notes:[{id:"one",path:"a.md",title:"One",tags:["work"],ambiguous:false},{id:"two",path:"b.md",title:"Two",tags:[],ambiguous:false}],folders:["empty"],diagnostics:[],incomplete:false};
const note = (id:string, body=id):Note => ({session:1,id,path:id+".md",title:id,tags:[],body,revision:"hash"});
function deferred<T>() { let resolve!:(value:T)=>void; const promise = new Promise<T>(r => {resolve=r}); return {promise,resolve}; }
let app:App;
afterEach(() => {app?.stop(); document.body.replaceChildren();});
function setup(overrides:Partial<Api> = {}) {
 const api:Api = {state:vi.fn().mockResolvedValue(state),select:vi.fn().mockResolvedValue(state),browse:vi.fn().mockResolvedValue(browse),search:vi.fn().mockResolvedValue({session:1,hits:[],incomplete:false}),open:vi.fn(async(_s,id)=>note(id)),resolve:vi.fn().mockResolvedValue({session:1,id:"two"}),external:vi.fn().mockResolvedValue(undefined),...overrides};
 const host=document.createElement("div");document.body.append(host);app=new App(host,api);return {host,api};
}
describe("read-only UI state", () => {
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
  const slow=deferred<Note>(); const {host}=setup({open:vi.fn((_s,id)=> id==="one" ? slow.promise : Promise.resolve(note(id)))});
  await app.start();const pending=app.open("one"); await app.open("two");slow.resolve(note("one"));await pending;
  expect(host.querySelector("article")?.textContent?.trim()).toBe("two");
 });
 it("ignores a previous library response and renders diagnostics as text", async() => {
  const slow=deferred<Note>();const {host}=setup({open:()=>slow.promise,select:async()=>({...state,session:2,root:"/other"}),browse:async(session)=>({...browse,session,root:session===2?"/other":"/notes",diagnostics:[{path:"<script>",code:"bad",message:"<img onerror=alert(1)>"}]})});
  await app.start();const pending=app.open("one");await app.choose("/other");slow.resolve(note("one","STALE BODY"));await pending;
  expect(host.querySelector("article")?.textContent).not.toContain("STALE BODY");
  expect(host.querySelector("[data-testid=diagnostics] script, [data-testid=diagnostics] img")).toBeNull();
 });
 it("drops stale search responses", async() => {
  const slow=deferred<Awaited<ReturnType<Api['search']>>>();const {host}=setup({search:vi.fn((_s,q)=>q==="slow"?slow.promise:Promise.resolve({session:1,hits:[{id:"two",title:"Fresh",path:"b.md",snippet:"",rank:1}],incomplete:false}))});
  await app.start();const input=host.querySelector<HTMLInputElement>("[data-testid=search-input]")!;
  input.value="slow";const pending=app.loadList();input.value="fast";await app.loadList();slow.resolve({session:1,hits:[{id:"one",title:"Stale",path:"a.md",snippet:"",rank:1}],incomplete:false});await pending;
  expect(host.querySelector("[data-testid=note-list]")?.textContent).toContain("Fresh");
  expect(host.querySelector("[data-testid=note-list]")?.textContent).not.toContain("Stale");
 });
 it("updates moved notes by ID and replaces a deleted preview", async() => {
  const {host,api}=setup(); await app.start();await app.open("one");
  vi.mocked(api.state).mockResolvedValue({...state,generation:2});vi.mocked(api.browse).mockResolvedValue({...browse,generation:2});vi.mocked(api.open).mockResolvedValue({...note("one","Moved body"),path:"moved.md"});await app.poll();
  expect(host.querySelector("article")?.textContent?.trim()).toBe("Moved body");expect(host.textContent).toContain("moved.md");
  vi.mocked(api.state).mockResolvedValue({...state,generation:3});vi.mocked(api.browse).mockResolvedValue({...browse,generation:3});vi.mocked(api.open).mockRejectedValue("note not found");await app.poll();
  expect(host.querySelector("article")?.textContent).toContain("Note unavailable");expect(host.querySelector("article")?.textContent).not.toContain("Moved body");
 });
 it("routes user-activated links only through controlled commands", async() => {
  const {host,api}=setup({open:async(_s,id)=>note(id,"[Web](https://example.org) [Note](other.md)")});await app.start();await app.open("one");
  const anchors=host.querySelectorAll<HTMLAnchorElement>("article a");anchors[0]!.click();expect(api.external).toHaveBeenCalledWith("https://example.org");anchors[1]!.click();expect(api.resolve).toHaveBeenCalledWith(1,"one.md","other.md");
 });
});
