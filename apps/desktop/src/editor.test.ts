import { afterEach, describe, expect, it, vi } from "vitest";
import { Editor, preserveNewlines } from "./editor";
import type { Mutation, Note } from "./api";
const note: Note = {session:1,path:"a.md",title:"A",tags:[],body:"old",source:"old",revision:"r0"};
const ack = (revision="r1"): Mutation => ({session:1,path:"a.md",revision,file_committed:true,warnings:[]});
function deferred<T>() { let resolve!:(value:T)=>void; const promise=new Promise<T>(r=>{resolve=r;});return {promise,resolve}; }
afterEach(()=>vi.useRealTimers());
describe("guarded autosave",()=>{
 it("retries only readback after a committed save whose verification failed",async()=>{
  const save=vi.fn().mockResolvedValue(ack());const read=vi.fn().mockRejectedValueOnce({code:"io",message:"Read denied"}).mockResolvedValue({...note,body:"local",revision:"r1"});
  const editor=new Editor(note,save,()=>{},read);editor.edit("local");expect(await editor.flush()).toBe(false);
  expect(editor.status).toBe("save_error");expect(await editor.retry()).toBe(true);expect(save).toHaveBeenCalledOnce();expect(read).toHaveBeenCalledTimes(2);editor.dispose();
 });
 it.each(["changed","missing"])("verifies the committed path before clearing the buffer or permitting close (%s)",async(kind)=>{
  const disk=deferred<Note>();const read=vi.fn(()=>kind==="missing"?Promise.reject({code:"not_found",message:"Missing"}):disk.promise);
  const editor=new Editor(note,vi.fn().mockResolvedValue(ack()),()=>{},read);
  editor.edit("only retained local version");const flush=editor.flush();
  await Promise.resolve();await Promise.resolve();expect(read).toHaveBeenCalledOnce();
  disk.resolve({...note,body:"external",revision:"external"});expect(await flush).toBe(false);
  expect(editor.body).toBe("only retained local version");expect(editor.status).toBe(kind==="missing"?"missing_on_disk":"conflict");editor.dispose();
 });
 it("pauses debounce throughout a slow observation and blocks a divergent queued save",async()=>{
  vi.useFakeTimers();const disk=deferred<Note>();const save=vi.fn();const editor=new Editor(note,save,()=>{});
  editor.edit("local");const check=editor.inspect(()=>disk.promise);const flush=editor.flush();
  await vi.advanceTimersByTimeAsync(1500);expect(save).not.toHaveBeenCalled();
  disk.resolve({...note,revision:"external",body:"external"});await check;expect(await flush).toBe(false);
  expect(editor.body).toBe("local");expect(editor.status).toBe("conflict");editor.dispose();
 });
 it.each([false,true])("reads after an in-flight acknowledgement and guards newer typing (external=%s)",async(external)=>{
  vi.useFakeTimers();const committed=deferred<Mutation>();const disk=deferred<Note>();
  const save=vi.fn().mockReturnValueOnce(committed.promise).mockResolvedValue(ack("r2"));const read=vi.fn(()=>disk.promise);
  const editor=new Editor(note,save,()=>{});editor.edit("first");await vi.advanceTimersByTimeAsync(400);
  editor.edit("second");const check=editor.inspect(read);expect(read).not.toHaveBeenCalled();committed.resolve(ack());
  await vi.advanceTimersByTimeAsync(1000);expect(read).toHaveBeenCalledOnce();expect(save).toHaveBeenCalledOnce();
  disk.resolve({...note,revision:external?"external":"r1",body:external?"external":"first"});await check;
  expect(editor.status).toBe(external?"conflict":"dirty");expect(editor.body).toBe("second");
  await vi.advanceTimersByTimeAsync(400);expect(save).toHaveBeenCalledTimes(external?1:2);editor.dispose();
 });
 it("pauses a dirty buffer on divergent disk observations, even after local reversion",async()=>{
  vi.useFakeTimers();const save=vi.fn();const editor=new Editor(note,save,()=>{});
  editor.edit("local");editor.observe({...note,revision:"external",body:"external"});
  expect(editor.status).toBe("conflict");editor.edit("old");
  await vi.advanceTimersByTimeAsync(1000);expect(save).not.toHaveBeenCalled();expect(editor.status).toBe("conflict");editor.dispose();
 });
 it("ignores unchanged observations but never recreates an observed missing path",async()=>{
  vi.useFakeTimers();const save=vi.fn();const editor=new Editor(note,save,()=>{});
  editor.edit("local");editor.observe(note);expect(editor.status).toBe("dirty");
  editor.observe(null);expect(editor.status).toBe("missing_on_disk");
  editor.observe(note);expect(editor.status).toBe("conflict");
  await vi.advanceTimersByTimeAsync(1000);expect(save).not.toHaveBeenCalled();editor.dispose();
 });
 it("preserves mixed newlines across separated unsaved edits and exact reversion",async()=>{
  const body="A\r\nB\nC\r\nD";const save=vi.fn().mockResolvedValue(ack());const editor=new Editor({...note,body},save,()=>{});
  editor.edit("XA\nB\nC\nD");editor.edit("XA\nB\nC\nDY");expect(editor.body).toBe("XA\r\nB\nC\r\nDY");
  editor.edit("A\nB\nC\nD");expect(editor.body).toBe(body);expect(await editor.flush()).toBe(true);expect(save).not.toHaveBeenCalled();
  editor.edit("");editor.edit("restored\nline");expect(editor.body).toBe("restored\r\nline");editor.dispose();
 });
 it("debounces, snapshots, serializes and never cleans a newer generation",async()=>{
  vi.useFakeTimers();const first=deferred<Mutation>();const save=vi.fn().mockReturnValueOnce(first.promise).mockResolvedValue(ack("r2"));
  const editor=new Editor(note,save,()=>{});editor.edit("one");await vi.advanceTimersByTimeAsync(400);editor.edit("two");await vi.advanceTimersByTimeAsync(600);
  expect(save).toHaveBeenCalledTimes(1);expect(editor.status).toBe("saving");
  first.resolve(ack());await vi.advanceTimersByTimeAsync(0);expect(editor.status).toBe("dirty");
  await vi.advanceTimersByTimeAsync(400);expect(save).toHaveBeenNthCalledWith(2,1,"a.md","r1","two");expect(editor.status).toBe("clean");editor.dispose();
 });
 it("flushes all in-flight generations before allowing navigation",async()=>{
  const first=deferred<Mutation>();const save=vi.fn().mockReturnValueOnce(first.promise).mockResolvedValue(ack("r2"));const editor=new Editor(note,save,()=>{});
  editor.edit("one");const flush=editor.flush();editor.edit("two");first.resolve(ack());expect(await flush).toBe(true);expect(editor.body).toBe("two");expect(editor.revision).toBe("r2");expect(save).toHaveBeenCalledTimes(2);editor.dispose();
 });
 it.each([['conflict','conflict'],['not_found','missing_on_disk'],['io','save_error']])("retains buffer and pauses on %s",async(code,status)=>{
  vi.useFakeTimers();const save=vi.fn().mockRejectedValue(JSON.stringify({code,message:"Cannot save"}));const editor=new Editor(note,save,()=>{});
  editor.edit("precious");expect(await editor.flush()).toBe(false);expect(editor.status).toBe(status);expect(editor.body).toBe("precious");editor.edit("still precious");await vi.advanceTimersByTimeAsync(5000);expect(save).toHaveBeenCalledTimes(1);expect(await editor.flush()).toBe(false);editor.dispose();
 });
 it("explicit retry uses the original precondition and committed warnings are success",async()=>{
  const save=vi.fn().mockRejectedValueOnce({code:"io",message:"denied"}).mockResolvedValue({...ack(),warnings:["Index degraded"]});const editor=new Editor(note,save,()=>{});editor.edit("new");await editor.flush();expect(await editor.retry()).toBe(true);expect(save).toHaveBeenLastCalledWith(1,"a.md","r0","new");expect(editor.status).toBe("clean");expect(editor.warning).toContain("Index degraded");editor.dispose();
 });
 it("does not write untouched source or reverted edits",async()=>{
  const save=vi.fn();const editor=new Editor({...note,body:"a\r\nb\r\n"},save,()=>{});editor.edit("a\nb\n");expect(await editor.flush()).toBe(true);expect(save).not.toHaveBeenCalled();editor.edit("different");editor.edit("a\nb\n");expect(await editor.flush()).toBe(true);expect(save).not.toHaveBeenCalled();editor.dispose();
 });
 it("preserves unsupported syntax, mixed untouched newlines and inserted CRLF",()=>{
  const body="[[wiki]]\r\n![image](a.png)\n:::raw\r\nend";expect(preserveNewlines(body,"[[wiki]]\n![image](a.png)\n:::raw\nend")).toBe(body);
  expect(preserveNewlines(body,"[[wiki]]\n![image](a.png)\n:::raw\nnew\nend")).toBe("[[wiki]]\r\n![image](a.png)\n:::raw\r\nnew\r\nend");
 });
});
