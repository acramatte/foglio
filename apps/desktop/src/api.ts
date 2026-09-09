import { invoke } from "@tauri-apps/api/core";
export interface DesktopState {
  session: number;
  root: string | null;
  generation: number;
  watcher_active: boolean;
  error: string | null;
}
export interface Summary {
  path: string;
  title: string;
  tags: string[];
}
export interface Browse {
  session: number;
  generation: number;
  root: string;
  notes: Summary[];
  folders: string[];
  diagnostics: { path: string; code: string; message: string }[];
  incomplete: boolean;
}
export interface Search {
  session: number;
  hits: {
    path: string;
    title: string;
    snippet: string;
    rank: number;
  }[];
  incomplete: boolean;
}
export interface Note {
  session: number;
  path: string;
  title: string;
  tags: string[];
  body: string;
  source: string;
  revision: string;
}
export interface Mutation {
  session: number;
  path: string;
  revision: string | null;
  file_committed: boolean;
  warnings: string[];
}
export interface Api {
  copy(session: number, path: string, observedRevision: string | null, destination: string, baseSource: string, body: string): Promise<Mutation>;
  save(session: number, path: string, revision: string, body: string): Promise<Mutation>;
  create(session: number, title: string, folder: string | null, body: string, tags: string[]): Promise<Mutation>;
  move(session: number, path: string, revision: string, destination: string): Promise<Mutation>;
  delete(session: number, path: string, revision: string): Promise<Mutation>;
  tag(session: number, path: string, revision: string, tag: string, add: boolean): Promise<Mutation>;
  close(): Promise<void>;
  state(): Promise<DesktopState>;
  select(path: string): Promise<DesktopState>;
  browse(session: number): Promise<Browse>;
  search(
    session: number,
    query: string,
    tag: string | null,
    folder: string | null,
  ): Promise<Search>;
  open(session: number, path: string): Promise<Note>;
  resolve(
    session: number,
    fromPath: string,
    target: string,
  ): Promise<{ session: number; path: string }>;
  external(url: string): Promise<void>;
}
export const api: Api = {
  copy: (session, path, observedRevision, destination, baseSource, body) => invoke("save_note_copy", {session, path, observedRevision, destination, baseSource, body}),
  save: (session, path, revision, body) => invoke("save_note", {session, path, revision, body}),
  create: (session, title, folder, body, tags) => invoke("create_note", {session, title, folder, body, tags}),
  move: (session, path, revision, destination) => invoke("move_note", {session, path, revision, destination}),
  delete: (session, path, revision) => invoke("delete_note", {session, path, revision}),
  tag: (session, path, revision, tag, add) => invoke("change_tag", {session, path, revision, tag, add}),
  close: () => invoke("finish_close"),
  state: () => invoke("desktop_state"),
  select: (path) => invoke("select_library", { path }),
  browse: (session) => invoke("browse_library", { session }),
  search: (session, query, tag, folder) =>
    invoke("search_notes", { session, query, tag, folder }),
  open: (session, path) => invoke("open_note", { session, path }),
  resolve: (session, fromPath, target) =>
    invoke("resolve_note_link", { session, fromPath, target }),
  external: (url) => invoke("open_external_link", { url }),
};
export function errorCode(error: unknown): string {
  if (typeof error === "string") {try {return errorCode(JSON.parse(error));} catch {return "";}}
  return error && typeof error === "object" && "code" in error ? String(error.code) : "";
}
export function errorText(error: unknown): string {
  if (typeof error === "string") {
    try {
      return errorText(JSON.parse(error));
    } catch {
      return error;
    }
  }
  if (error && typeof error === "object" && "message" in error)
    return `${"code" in error ? String(error.code) + ": " : ""}${String(error.message)}`;
  return "Unable to complete the request. Check the library and try again.";
}
