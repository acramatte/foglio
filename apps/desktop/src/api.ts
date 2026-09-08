import { invoke } from "@tauri-apps/api/core";
export interface DesktopState {
  session: number;
  root: string | null;
  generation: number;
  watcher_active: boolean;
  error: string | null;
}
export interface Summary {
  id: string | null;
  path: string;
  title: string;
  tags: string[];
  ambiguous: boolean;
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
    id: string;
    path: string;
    title: string;
    snippet: string;
    rank: number;
  }[];
  incomplete: boolean;
}
export interface Note {
  session: number;
  id: string;
  path: string;
  title: string;
  tags: string[];
  body: string;
  revision: string;
}
export interface Api {
  state(): Promise<DesktopState>;
  select(path: string): Promise<DesktopState>;
  browse(session: number): Promise<Browse>;
  search(
    session: number,
    query: string,
    tag: string | null,
    folder: string | null,
  ): Promise<Search>;
  open(session: number, id: string): Promise<Note>;
  resolve(
    session: number,
    fromPath: string,
    target: string,
  ): Promise<{ session: number; id: string }>;
  external(url: string): Promise<void>;
}
export const api: Api = {
  state: () => invoke("desktop_state"),
  select: (path) => invoke("select_library", { path }),
  browse: (session) => invoke("browse_library", { session }),
  search: (session, query, tag, folder) =>
    invoke("search_notes", { session, query, tag, folder }),
  open: (session, id) => invoke("open_note", { session, id }),
  resolve: (session, fromPath, target) =>
    invoke("resolve_note_link", { session, fromPath, target }),
  external: (url) => invoke("open_external_link", { url }),
};
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
